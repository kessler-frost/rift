use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Once;
use std::{fs, thread};

use anyhow::{anyhow, bail, Context, Result};
use diesel::connection::{DefaultLoadingMode, SimpleConnection};
use diesel::result::Error;
use diesel::sqlite::SqliteConnection;
use diesel::{
    BelongingToDsl, BoolExpressionMethods, Connection, ExpressionMethods, GroupedBy,
    OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper,
};
use diesel_migrations::MigrationHarness;
use itertools::Itertools;
use libsqlite3_sys as sqlite3;
use lsp::supported_servers::LSPServerType;
use num_traits::FromPrimitive;
use rift_core::telemetry::EnablementState;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use riftui::platform::FullscreenState;
use riftui::windowing::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH};
use riftui::AppContext;

use super::block_list::{delete_blocks, save_block};
use super::model::{
    self, CurrentUserInformation, MCPEnvironmentVariables,
    NewApp, NewCommand, NewServerExperiment, NewTab, NewTeam, NewWindow, NewWorkspace,
    NewWorkspaceTeam, Project, Tab, Window,
    WorkspaceMetadata as WorkspaceMetadataModel, SETTINGS_PANE_KIND, TERMINAL_PANE_KIND,
    WELCOME_PANE_KIND,
};
use super::{
    schema, BlockCompleted, FinishedCommandMetadata, ModelEvent, PersistedData, PersistenceScope,
    StartedCommandMetadata, WriterHandles,
};
use crate::app_state::{
    AppState, BranchSnapshot, LeafContents, LeafSnapshot, LeftPanelSnapshot, PaneFlex,
    PaneNodeSnapshot, RightPanelSnapshot, SettingsPaneSnapshot, SplitDirection, TabSnapshot,
    TerminalPaneSnapshot, WindowSnapshot,
};
use crate::auth::auth_manager::PersistedCurrentUserInformation;
use crate::auth::UserUid;
use crate::persistence::block_list::get_all_restored_blocks;
use crate::persistence::model::{
    NewTeamSettings, UserProfile, GET_STARTED_PANE_KIND,
};
use crate::server::experiments::ServerExperiment;
use crate::server::ids::ServerId;
use crate::settings_view::SettingsSection;
use crate::suggestions::ignored_suggestions_model::SuggestionType;
use crate::tab::SelectedTabColor;
use crate::terminal::history::PersistedCommand;
use crate::terminal::ShellLaunchData;
use crate::themes::theme::AnsiColorIdentifier;
use crate::workspaces::team::Team as TeamMetadata;
use crate::workspaces::user_profiles::{user_profile_from_persistence, UserProfileWithUID};
use crate::workspaces::workspace::{Workspace as WorkspaceMetadata, WorkspaceUid};
use crate::{report_error, safe_info, send_telemetry_from_app_ctx};

// Choose a power of 2 that seems to be a reasonable upper bound for how many
// events to queue.
const CHANNEL_SIZE: usize = 1024;
const COMMANDS_COUNT_LIMIT: i64 = 10000;

const RIFT_SQLITE_FILE_NAME: &str = "warp.sqlite";

/// Runs any migrations and creates the Sqlite database if it doesn't exist.
/// Reads from the sqlite database to get the app state for session restoration.
/// Starts a writer thread that listens for ModelEvents and processes them.
pub fn initialize(
    ctx: &mut AppContext,
    scope: PersistenceScope,
) -> (Option<Box<PersistedData>>, Option<WriterHandles>) {
    unsafe {
        // Set up logging before any SQLite calls.
        init_logging();
    }
    let database_path = database_file_path_for_scope(&scope);
    match init_db(&scope) {
        Ok(mut conn) => {
            let persisted_data = read_persisted_data(&mut conn, ctx);

            let writer_handles = match start_writer(conn, database_path.clone()) {
                Ok(writer_handles) => Some(writer_handles),
                Err(err) => {
                    send_telemetry_from_app_ctx!(
                        TelemetryEvent::DatabaseWriteError(err.to_string()),
                        ctx
                    );
                    report_db_error("starting writer", err, &database_path);
                    None
                }
            };
            (persisted_data, writer_handles)
        }
        Err(err) => {
            send_telemetry_from_app_ctx!(
                TelemetryEvent::DatabaseStartUpError(err.to_string()),
                ctx
            );
            report_db_error("initialization", err, &database_path);
            (None, None)
        }
    }
}

fn read_persisted_data(
    conn: &mut SqliteConnection,
    _ctx: &mut AppContext,
) -> Option<Box<PersistedData>> {
    match read_sqlite_data(conn) {
        Ok(app_state) => Some(Box::new(app_state)),
        Err(err) => {
            send_telemetry_from_app_ctx!(TelemetryEvent::DatabaseReadError(err.to_string()), ctx);
            report_error!(anyhow::Error::new(err).context("Failed to read persisted data"));
            None
        }
    }
}

/// Returns a read-only connection to the sqlite database.
/// We want only one write connection to exist and use event processing to write any data needed.
pub fn establish_ro_connection(database_url: &str) -> Result<SqliteConnection> {
    establish_connection(database_url, true)
}

fn establish_connection(database_url: &str, read_only: bool) -> Result<SqliteConnection> {
    let full_database_url = if read_only {
        &format!("file:{database_url}?mode=ro")
    } else {
        database_url
    };
    let mut conn = SqliteConnection::establish(full_database_url)?;
    conn.batch_execute(
        r#"
        PRAGMA foreign_keys = ON;           -- enforce foreign key constraints
        PRAGMA busy_timeout = 1000;         -- sleep for up to 1s if the database is busy
    "#,
    )?;

    // Enable WAL mode, checkpointing whenever the log is at least 500 pages long (in theory,
    // around 2MB). In addition, SQLite will automatically checkpoint when the app closes its
    // database connection.
    // The auto-checkpoint interval is lowered from the default of 1000 because all writes
    // already run in a background thread and can afford to checkpoint slightly more often.
    // At the default value, the WAL can grow larger than a typical database (for our usage).
    conn.batch_execute(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA wal_autocheckpoint=500;
    "#,
    )
    .context("Failed to enable WAL")?;

    Ok(conn)
}

/// Set up SQLite [error logging](https://www.sqlite.org/errlog.html)
///
/// ## Safety
/// Setting up SQLite logging is not thread-safe. No other SQLite calls may be made while this
/// function is running.
unsafe fn init_logging() {
    use std::ffi::{c_char, c_int, c_void, CStr};
    use std::{panic, ptr};

    extern "C-unwind" fn log_callback(_data: *mut c_void, err_code: c_int, msg: *const c_char) {
        // `err_code` is an extended error code (https://www.sqlite.org/rescode.html#primary_result_codes_versus_extended_result_codes).
        // In general, the least-significant byte of an extended error code is the primary error
        // code it belongs to. Each primary error code can also be used where an extended error
        // code is expected (for example, `SQLITE_SCHEMA` has no extended error codes).
        let primary_error_code = err_code & 0xFF;
        let level = match (primary_error_code, err_code) {
            // This usually means that a schema change invalidated a prepared statement.
            (sqlite3::SQLITE_SCHEMA, _) => log::Level::Debug,
            // These are used with sqlite3_log, in extensions.
            (sqlite3::SQLITE_NOTICE | sqlite3::SQLITE_WARNING, _) => log::Level::Warn,
            // According to the docs, this error means that the database file was moved (or deleted),
            // so SQLite can't safely modify it and the rollback journal:
            //     https://www.sqlite.org/rescode.html#readonly_dbmoved
            // This is mostly outside of Warp's control (e.g. the user or some system program is
            // moving around files in the user data directory), so downgrade to a warning.
            (_, sqlite3::SQLITE_READONLY_DBMOVED) => log::Level::Warn,
            _ => log::Level::Error,
        };

        // Safety: the message pointer came from the SQLite library, which promises that it's a
        // valid C string pointer.
        let msg = unsafe { CStr::from_ptr(msg) };
        let err_message = String::from_utf8_lossy(msg.to_bytes());
        // Sentry shouldn't panic, but to be safe, make sure we don't unwind across the FFI
        // boundary.
        let _ = panic::catch_unwind(|| {
            // We report SQLite errors to Sentry in a more-structured format so that they have
            // better grouping (all are under the same Sentry issue, with details for the specific
            // error kind). Warning and debug SQLite messages are logged - with the default
            // sentry_log configuration, warnings are added as breadcrumbs to other events and
            // debug messages are ignored.
            // In local builds without crash reporting, all SQLite messages get logged locally.

            #[cfg(feature = "crash_reporting")]
            if level == log::Level::Error {
                sentry::with_scope(
                    |scope| {
                        let mut context = std::collections::BTreeMap::new();
                        context.insert("message".to_string(), err_message.into());
                        context.insert("code".to_string(), err_code.into());
                        context.insert(
                            "code_description".to_string(),
                            sqlite3::code_to_str(err_code).into(),
                        );
                        scope.set_context("sqlite", sentry::protocol::Context::Other(context));
                    },
                    || {
                        sentry::capture_message(
                            "Sqlite Error",
                            sentry_log::convert_log_level(level),
                        )
                    },
                );
                return;
            }

            log::log!(
                level,
                "SQLite error {} ({}): {}",
                err_code,
                sqlite3::code_to_str(err_code),
                err_message
            );
        });
    }

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let null: *const c_void = ptr::null();
        // Diesel doesn't expose SQLite's logging/tracing APIs, but the FFI bindings do.
        let status = sqlite3::sqlite3_config(
            sqlite3::SQLITE_CONFIG_LOG,
            log_callback as extern "C-unwind" fn(_, _, _),
            null,
        );

        if status != sqlite3::SQLITE_OK {
            log::error!(
                "Error setting up SQLite logging: {}",
                sqlite3::code_to_str(status)
            );
        }
    });
}

/// Determines the db path, establishes a connection and runs any migrations.
pub(super) fn init_db(scope: &PersistenceScope) -> Result<SqliteConnection> {
    // First, make sure the parent directory of the file exists, otherwise
    // we'll get an error if the file doesn't already exist.
    let db_path = database_file_path_for_scope(scope);
    // If we fail to create the necessary directories, log a warning and
    // continue; we'll return a sqlite error if it actually fails to initialize
    // a database connection.
    let db_parent = db_path
        .parent()
        .expect("database file path should be absolute");
    if let Err(err) = std::fs::create_dir_all(db_parent) {
        log::warn!(
            "Encountered an error while creating parent directories for sqlite database: {err:#}"
        );
    }
    if matches!(scope, PersistenceScope::App) {
        migrate_old_sqlite_into_secure_container_if_needed(&db_path);
    }

    setup_database(&db_path)
}

fn migrate_old_sqlite_into_secure_container_if_needed(db_path: &Path) {
    let old_db_path = rift_core::paths::state_dir().join(RIFT_SQLITE_FILE_NAME);
    if old_db_path == db_path || !old_db_path.exists() || db_path.exists() {
        return;
    }

    match std::fs::rename(&old_db_path, db_path) {
        Ok(_) => {
            safe_info!(
                safe: ("Migrated SQLite database into application container"),
                full: ("Migrated SQLite database from `{}` to `{}`", old_db_path.display(), db_path.display())
            );

            // Also migrate the associated WAL and SHM files.
            let old_wal = old_db_path.with_extension("sqlite-wal");
            let old_shm = old_db_path.with_extension("sqlite-shm");
            let new_wal = db_path.with_extension("sqlite-wal");
            let new_shm = db_path.with_extension("sqlite-shm");

            if let Err(err) = std::fs::rename(&old_wal, &new_wal) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    report_error!(anyhow::Error::new(err)
                        .context("Failed to migrate SQLite WAL into application container"));
                }
            } else {
                log::info!("Migrated SQLite WAL into application container");
            }

            if let Err(err) = std::fs::rename(&old_shm, &new_shm) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    report_error!(anyhow::Error::new(err)
                        .context("Failed to migrate SQLite SHM into application container"));
                }
            } else {
                log::info!("Migrated SQLite shared memory file into application container");
            }
        }
        Err(err) => {
            report_error!(anyhow::Error::new(err)
                .context("Failed to migrate SQLite database into application container"));
        }
    }
}

/// Creates or connects to the database at `database_path` and runs any migrations.
fn setup_database(database_path: &Path) -> Result<SqliteConnection> {
    let db_url = database_path
        .to_str()
        .ok_or_else(|| anyhow!("Failed to convert db path to a string"))?;
    let mut conn = establish_connection(db_url, false)?;

    safe_info!(
        safe: ("Connecting to SQLite database"),
        full: ("Connecting to SQLite database at {db_url}")
    );
    conn.run_pending_migrations(persistence::MIGRATIONS)
        .map_err(|e| anyhow!(e))
        .context("Failed to perform migrations")?;
    Ok(conn)
}

/// The path at which the sqlite database is located for the given scope.
///
/// Integration tests that initialize the database with known data should use
/// this function to determine where to create the database file.
pub fn database_file_path_for_scope(scope: &PersistenceScope) -> PathBuf {
    match scope {
        PersistenceScope::App => app_database_file_path(),
    }
}

fn app_database_file_path() -> PathBuf {
    rift_core::paths::secure_state_dir()
        .unwrap_or_else(rift_core::paths::state_dir)
        .join(RIFT_SQLITE_FILE_NAME)
}

fn start_writer(conn: SqliteConnection, database_path: PathBuf) -> Result<WriterHandles> {
    let (tx, rx) = std::sync::mpsc::sync_channel(CHANNEL_SIZE);
    let mut current_conn = conn;
    let handle = thread::Builder::new()
        .name("SQLite Writer".into())
        .spawn(move || {
            let mut paused = false;
            loop {
                let events = match rx.recv() {
                    Ok(event) => {
                        // Wait for there to be at least one event, but collect any other pending
                        // events too. This way, we can start dropping redundant events if the
                        // writer thread is falling behind.
                        let mut events = vec![event];
                        events.extend(rx.try_iter());
                        deduplicate_events(events)
                    }
                    Err(_) => {
                        log::warn!(
                            "SQLite event sender has closed; terminating SQLite writer thread."
                        );
                        break;
                    }
                };

                for event in events {
                    match event {
                        ModelEvent::PauseAndRemoveDatabase => {
                            paused = true;
                            log::info!("SQLite Writer is paused");

                            if let Err(err) = std::fs::remove_file(&database_path) {
                                report_error!(anyhow::Error::new(err)
                                    .context("Error removing SQLite database"));
                            } else {
                                log::info!("Removed SQLite database");
                            }
                        }
                        ModelEvent::Terminate => {
                            log::info!("Shutting down SQLite writer thread");
                            return;
                        }
                        event => {
                            if paused {
                                log::info!("Ignoring event as SQLite Writer is on pause");
                                continue;
                            }
                            if let Err(err) = handle_model_event(event, &mut current_conn) {
                                report_db_error("Model", err, &database_path);
                            }
                        }
                    }
                }
            }
        })?;
    Ok(WriterHandles { handle, sender: tx })
}

/// Handles a single [`ModelEvent`] by dispatching to an event-specific function.
/// Events which affect the SQLite writer event loop _must_ instead be handled by the event loop itself:
/// * [`ModelEvent::PauseAndRemoveDatabase`]
/// * [`ModelEvent::Terminate`]
fn handle_model_event(event: ModelEvent, connection: &mut SqliteConnection) -> anyhow::Result<()> {
    match event {
        ModelEvent::PauseAndRemoveDatabase | ModelEvent::Terminate => {
            panic!("Unhandled control-flow event {event:?}");
        }
        ModelEvent::SaveBlock(BlockCompleted {
            pane_id,
            block,
            is_local,
        }) => save_block(connection, pane_id, &block, is_local).context("error saving block"),
        ModelEvent::DeleteBlocks(pane_id) => {
            // Delete the blocks even if the setting is off so users can still remove
            // panes and have their data deleted locally.
            delete_blocks(connection, pane_id).context("error deleting blocks")
        }
        ModelEvent::Snapshot(app_state) => {
            save_app_state(connection, &app_state).context("error saving app state")
        }
        ModelEvent::UpsertProject { project } => {
            save_project(connection, project).context("error upserting project")
        }
        ModelEvent::DeleteProject { path } => {
            delete_project(connection, &path).context("error deleting project")
        }
        ModelEvent::UpsertWorkspace { workspace } => {
            save_workspace(connection, *workspace).context("error upserting workspace")
        }
        ModelEvent::UpsertWorkspaces { workspaces } => {
            save_workspaces(connection, workspaces).context("error upserting workspaces")
        }
        ModelEvent::SetCurrentWorkspace { workspace_uid } => {
            set_current_workspace(connection, workspace_uid)
                .context("error setting current workspace")
        }
        ModelEvent::InsertCommand { metadata } => {
            insert_command(connection, metadata).context("error inserting command")
        }
        ModelEvent::UpdateFinishedCommand { metadata } => {
            update_finished_command(connection, metadata).context("error updating finished command")
        }
        ModelEvent::UpsertUserProfiles { profiles } => {
            upsert_user_profiles(connection, profiles).context("error updating user profiles")
        }
        ModelEvent::ClearUserProfiles => {
            clear_user_profiles(connection).context("error clearing user profiles")
        }
        ModelEvent::SaveExperiments { experiments } => {
            save_experiments(connection, experiments).context("error saving experiments")
        }
        ModelEvent::UpsertCurrentUserInformation { user_information } => {
            upsert_current_user_information(connection, user_information)
                .context("error upserting user information")
        }
        ModelEvent::UpsertMCPServerEnvironmentVariables {
            mcp_server_uuid,
            environment_variables,
        } => upsert_mcp_server_environment_variables(
            connection,
            mcp_server_uuid,
            environment_variables,
        )
        .context("error upserting mcp server mcp_environment variables"),
        ModelEvent::AddIgnoredSuggestion {
            suggestion,
            suggestion_type,
        } => add_ignored_suggestion(connection, suggestion, suggestion_type)
            .context("error adding ignored suggestion"),
        ModelEvent::RemoveIgnoredSuggestion {
            suggestion,
            suggestion_type,
        } => remove_ignored_suggestion(connection, suggestion, suggestion_type)
            .context("error removing ignored suggestion"),
        ModelEvent::UpsertWorkspaceLanguageServer {
            workspace_path,
            lsp_type,
            enabled,
        } => upsert_workspace_language_server(connection, &workspace_path, lsp_type, enabled)
            .context("error upserting workspace language server"),
    }
}

/// Report a database error and additional context for debugging.
fn report_db_error(err_kind: &str, err: anyhow::Error, database_path: &Path) {
    // Sentry reports indicate that the database is sometimes missing/inaccessible, so check its
    // permissions and whether or not it exists.
    fn log_access(prefix: &str, path: &Path) {
        match fs::metadata(path) {
            Ok(metadata) => {
                cfg_if::cfg_if! {
                    if #[cfg(windows)] {
                        use async_fs::windows::MetadataExt;
                        // Windows does not have the same notion of permissions as Unix-based file systems.
                        // See more about what File Attributes contain [here](https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants).
                        let attributes = metadata.file_attributes();
                        safe_info!(
                            safe: ("{prefix} attributes: {attributes}"),
                            full: ("{prefix} {} attributes: {attributes}", path.display())
                        );
                    } else {
                        use async_fs::unix::PermissionsExt;
                        let mode = metadata.permissions().mode();
                        safe_info!(
                            safe: ("{prefix} permissions: {mode:o}"),
                            full: ("{prefix} {} permissions: {mode:o}", path.display())
                        );
                    }
                }
            }
            Err(err) => {
                safe_info!(
                    safe: ("{prefix} is inaccessible: {err}"),
                    full: ("{prefix} {} is inaccessible: {err}", path.display())
                );
            }
        }
    }

    if let Some(parent) = database_path.parent() {
        log_access("Database directory", parent);
    }
    log_access("Database", database_path);

    report_error!(err.context(format!("SQLite {err_kind} error")));
}

/// Filter a collection of model events to remove skippable events:
/// * [`ModelEvent::Snapshot`] includes the entire app state, so we only need the latest one.
fn deduplicate_events(events: Vec<ModelEvent>) -> Vec<ModelEvent> {
    let last_snapshot = events
        .iter()
        .enumerate()
        .rfind(|(_, event)| matches!(event, ModelEvent::Snapshot(_)));
    match last_snapshot {
        Some((last_snapshot_index, _)) => events
            .into_iter()
            .enumerate()
            .filter_map(|(index, event)| match event {
                ModelEvent::Snapshot(_) if index < last_snapshot_index => None,
                event => Some(event),
            })
            .collect(),
        None => events,
    }
}

// Used in the save_app_state function to help make the code more readable.
struct SaveAppStateNodeTraversal<'a> {
    node: &'a PaneNodeSnapshot,
    flex: Option<f32>,
    parent_pane_node_id: Option<i32>,
}

// Saves the app state snapshot in the sqlite database. Removes any old app state.
// Does so in a transaction so we're never in a partial state.
fn save_app_state(conn: &mut SqliteConnection, app_state: &AppState) -> Result<()> {
    conn.transaction::<(), Error, _>(|conn| {
        // Remove old app state
        diesel::delete(schema::app::dsl::app).execute(conn)?;
        diesel::delete(schema::terminal_panes::dsl::terminal_panes).execute(conn)?;
        diesel::delete(schema::notebook_panes::dsl::notebook_panes).execute(conn)?;
        diesel::delete(schema::code_panes::dsl::code_panes).execute(conn)?;
        diesel::delete(schema::env_var_collection_panes::dsl::env_var_collection_panes)
            .execute(conn)?;
        diesel::delete(schema::workflow_panes::dsl::workflow_panes).execute(conn)?;
        diesel::delete(schema::settings_panes::dsl::settings_panes).execute(conn)?;
        diesel::delete(schema::ai_memory_panes::dsl::ai_memory_panes).execute(conn)?;
        diesel::delete(schema::ai_document_panes::dsl::ai_document_panes).execute(conn)?;
        diesel::delete(schema::mcp_server_panes::dsl::mcp_server_panes).execute(conn)?;
        diesel::delete(schema::code_review_panes::dsl::code_review_panes).execute(conn)?;
        diesel::delete(schema::ambient_agent_panes::dsl::ambient_agent_panes).execute(conn)?;
        diesel::delete(schema::welcome_panes::dsl::welcome_panes).execute(conn)?;
        diesel::delete(schema::pane_leaves::dsl::pane_leaves).execute(conn)?;
        diesel::delete(schema::pane_branches::dsl::pane_branches).execute(conn)?;
        diesel::delete(schema::pane_nodes::dsl::pane_nodes).execute(conn)?;
        diesel::delete(schema::tabs::dsl::tabs).execute(conn)?;
        diesel::delete(schema::windows::dsl::windows).execute(conn)?;
        diesel::delete(schema::active_mcp_servers::dsl::active_mcp_servers).execute(conn)?;
        diesel::delete(schema::panels::dsl::panels).execute(conn)?;

        let mut active_window_id = None;

        for (idx, window) in app_state.windows.iter().enumerate() {
            // Just save zero as the tab index, if we overflow when converting
            // unsigned to signed.
            let active_tab_index: i32 = window.active_tab_index.try_into().unwrap_or(0);

            // In the database each individual field is nullable but in practice these
            // fields are either all null or all non-null as they together represent
            // the stored window bound. Bounds smaller than the platform minimum
            // window size are treated as missing so that we fall back to default
            // geometry on restore instead of replaying a corrupt size (see GH#10083).
            let (window_width, window_height, origin_x, origin_y) = match window.bounds {
                Some(rect)
                    if rect.size().x() >= MIN_WINDOW_WIDTH
                        && rect.size().y() >= MIN_WINDOW_HEIGHT =>
                {
                    (
                        Some(rect.size().x()),
                        Some(rect.size().y()),
                        Some(rect.origin().x()),
                        Some(rect.origin().y()),
                    )
                }
                _ => (None, None, None, None),
            };

            let new_window = NewWindow {
                active_tab_index,
                window_width,
                window_height,
                origin_x,
                origin_y,
                quake_mode: window.quake_mode,
                universal_search_width: window.universal_search_width,
                warp_ai_width: window.warp_ai_width,
                voltron_width: window.voltron_width,
                warp_drive_index_width: window.warp_drive_index_width,
                left_panel_open: Some(window.left_panel_open),
                vertical_tabs_panel_open: Some(window.vertical_tabs_panel_open),
                fullscreen_state: window.fullscreen_state as i32,
                agent_management_filters: None,
            };
            diesel::insert_into(schema::windows::dsl::windows)
                .values(new_window)
                .execute(conn)?;

            // We cannot directly return the id from the insert so perform
            // a second query for the id https://github.com/diesel-rs/diesel/issues/771.
            let window_id: i32 = schema::windows::dsl::windows
                .select(schema::windows::columns::id)
                .order(schema::windows::columns::id.desc())
                .first(conn)?;

            if app_state
                .active_window_index
                .map(|id| id == idx)
                .unwrap_or(false)
            {
                active_window_id = Some(window_id)
            }

            let tabs: Vec<NewTab> = window
                .tabs
                .iter()
                .map(|tab| NewTab {
                    window_id,
                    custom_title: tab.custom_title.clone(),
                    // We only persist and restore the selected color here
                    // (the default color based on the pwd is separately persisted and then applied on-restore)
                    color: match tab.selected_color {
                        // Keep the column NULL for the common no-override case
                        SelectedTabColor::Unset => None,
                        _ => serde_yaml::to_string(&tab.selected_color).ok(),
                    },
                })
                .collect();

            diesel::insert_into(schema::tabs::dsl::tabs)
                .values(tabs)
                .execute(conn)?;

            // Same ID issue as above.
            let tab_ids: Vec<i32> = schema::tabs::dsl::tabs
                .filter(schema::tabs::columns::window_id.eq(window_id))
                .select(schema::tabs::columns::id)
                .order(schema::tabs::columns::id.desc())
                .load(conn)?;

            // Since we retrieved the tab ids in descending order, we need to reverse them when we
            // iterate to restore the correct order.
            for (tab_id, tab) in tab_ids.iter().rev().zip(window.tabs.iter()) {
                let mut pane_nodes = VecDeque::new();
                pane_nodes.push_back(SaveAppStateNodeTraversal {
                    node: &tab.root,
                    flex: None,
                    parent_pane_node_id: None,
                });

                if tab.left_panel.is_some() || tab.right_panel.is_some() {
                    let new_panel = model::NewPanel {
                        tab_id: *tab_id,
                        left_panel: tab
                            .left_panel
                            .as_ref()
                            .and_then(|p| serde_json::to_string(p).ok()),
                        right_panel: tab
                            .right_panel
                            .as_ref()
                            .and_then(|p| serde_json::to_string(p).ok()),
                    };
                    diesel::insert_into(schema::panels::dsl::panels)
                        .values(new_panel)
                        .execute(conn)?;
                }

                while !pane_nodes.is_empty() {
                    let SaveAppStateNodeTraversal {
                        node: pane_node,
                        flex,
                        parent_pane_node_id,
                    } = pane_nodes.pop_front().expect("Should have node");

                    // Skip leaves whose content types don't get a
                    // corresponding `pane_leaves` row on save. Otherwise the
                    // `pane_nodes` insert below would create an orphan row
                    // (is_leaf=true, but no matching row in `pane_leaves`),
                    // and `read_node` would fail to resolve the leaf on
                    // restore, causing the entire surrounding tab to be
                    // dropped. See `LeafContents::is_persisted`.
                    if let PaneNodeSnapshot::Leaf(leaf) = pane_node {
                        if !leaf.contents.is_persisted() {
                            continue;
                        }
                    }

                    let is_leaf = matches!(pane_node, PaneNodeSnapshot::Leaf(_));
                    let new_pane_node = model::NewPaneNode {
                        tab_id: *tab_id,
                        parent_pane_node_id,
                        flex,
                        is_leaf,
                    };

                    diesel::insert_into(schema::pane_nodes::dsl::pane_nodes)
                        .values(new_pane_node)
                        .execute(conn)?;

                    // Same ID issue as above.
                    let pane_node_id = schema::pane_nodes::dsl::pane_nodes
                        .select(schema::pane_nodes::columns::id)
                        .order(schema::pane_nodes::columns::id.desc())
                        .first(conn)?;
                    match pane_node {
                        PaneNodeSnapshot::Branch(pane_group) => {
                            let new_pane_branch = model::NewPaneBranch {
                                pane_node_id,
                                horizontal: pane_group.direction == SplitDirection::Horizontal,
                            };
                            diesel::insert_into(schema::pane_branches::dsl::pane_branches)
                                .values(new_pane_branch)
                                .execute(conn)?;

                            for (flex, child_pane_node) in &pane_group.children {
                                pane_nodes.push_back(SaveAppStateNodeTraversal {
                                    node: child_pane_node,
                                    flex: Some(flex.0),
                                    parent_pane_node_id: Some(pane_node_id),
                                });
                            }
                        }
                        PaneNodeSnapshot::Leaf(pane) => {
                            save_pane_state(conn, pane_node_id, pane)?;
                        }
                    }
                }
            }
        }

        let new_app = NewApp { active_window_id };

        diesel::insert_into(schema::app::dsl::app)
            .values(new_app)
            .execute(conn)?;

        // MCP servers were an AI feature and are no longer persisted.

        Ok(())
    })?;
    Ok(())
}

/// Saves the state of an individual pane, after the corresponding `pane_nodes` entry
/// has been written.
fn save_pane_state(
    conn: &mut SqliteConnection,
    id: i32,
    snapshot: &LeafSnapshot,
) -> Result<(), Error> {
    // The pane_leaves row must be inserted first to satisfy foreign key constraints on the
    // kind-specific tables.
    let kind = match &snapshot.contents {
        LeafContents::Terminal(_) => TERMINAL_PANE_KIND,
        LeafContents::Settings(_) => SETTINGS_PANE_KIND,
        LeafContents::GetStarted => GET_STARTED_PANE_KIND,
        LeafContents::Welcome { .. } => WELCOME_PANE_KIND,
    };

    let leaf = model::NewPane {
        pane_node_id: id,
        kind: kind.into(),
        is_focused: snapshot.is_focused,
        custom_vertical_tabs_title: snapshot.custom_vertical_tabs_title.clone(),
    };

    diesel::insert_into(schema::pane_leaves::dsl::pane_leaves)
        .values(leaf)
        .execute(conn)?;

    match &snapshot.contents {
        LeafContents::Terminal(terminal_snapshot) => {
            let terminal = model::NewTerminalPane {
                id,
                uuid: terminal_snapshot.uuid.clone(),
                cwd: terminal_snapshot.cwd.clone(),
                is_active: terminal_snapshot.is_active,
                shell_launch_data: terminal_snapshot
                    .shell_launch_data
                    .as_ref()
                    .and_then(|shell| serde_json::to_string(shell).ok()),
                input_config: None,
                llm_model_override: None,
                active_profile_id: terminal_snapshot
                    .active_profile_id
                    .as_ref()
                    .and_then(|sync_id| serde_json::to_string(sync_id).ok()),
                conversation_ids: None,
                active_conversation_id: None,
            };

            diesel::insert_into(schema::terminal_panes::dsl::terminal_panes)
                .values(terminal)
                .execute(conn)?;
        }
        LeafContents::Settings(settings_pane_snapshot) => {
            let current_page = match settings_pane_snapshot {
                SettingsPaneSnapshot::Local { current_page, .. } => current_page,
            };

            let settings_pane = model::NewSettingsPane {
                id,
                current_page: current_page.to_string(),
            };

            diesel::insert_into(schema::settings_panes::dsl::settings_panes)
                .values(settings_pane)
                .execute(conn)?;
        }
        LeafContents::GetStarted => {
            // Stateless
        }
        LeafContents::Welcome { startup_directory } => {
            let welcome_pane = model::NewWelcomePane {
                id,
                startup_directory: startup_directory
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            };
            diesel::insert_into(schema::welcome_panes::dsl::welcome_panes)
                .values(welcome_pane)
                .execute(conn)?;
        }
    }

    Ok(())
}

fn get_all_workspace_language_servers_by_workspace(
    conn: &mut SqliteConnection,
) -> Result<HashMap<PathBuf, HashMap<LSPServerType, EnablementState>>, diesel::result::Error> {
    use schema::workspace_language_server::dsl::*;
    use schema::workspace_metadata;

    let results = workspace_language_server
        .inner_join(workspace_metadata::table)
        .select((workspace_metadata::repo_path, language_server_name, enabled))
        .load::<(String, String, String)>(conn)?;

    let mut grouped: HashMap<PathBuf, HashMap<LSPServerType, EnablementState>> = HashMap::new();
    for (path_str, server_name, enablement_str) in results {
        let path = PathBuf::from(path_str);
        let Some(server_type) = serde_json::from_str(&server_name).ok() else {
            continue;
        };

        // `EnablementState` (telemetry) is not serde-able; the restored value is no
        // longer consumed, so default it rather than deserializing.
        let _ = enablement_str;
        grouped
            .entry(path)
            .or_default()
            .insert(server_type, EnablementState::Always);
    }

    Ok(grouped)
}

fn upsert_workspace_language_server(
    conn: &mut SqliteConnection,
    workspace_path: &Path,
    server_type: LSPServerType,
    enablement: EnablementState,
) -> Result<()> {
    use schema::workspace_language_server::dsl::*;
    use schema::workspace_metadata::dsl::*;
    let path_string = workspace_path.to_string_lossy().to_string();

    // Try to find existing workspace
    let metadata = workspace_metadata
        .filter(repo_path.eq(&path_string))
        .first::<WorkspaceMetadataModel>(conn)
        .optional()?
        .ok_or(anyhow::anyhow!("Can't find workspace for path"))?;

    let ws_id = metadata.id;
    let server_name = serde_json::to_string(&server_type)?;

    // Now upsert the language server setting
    // Check if record already exists
    let existing = workspace_language_server
        .filter(workspace_id.eq(ws_id))
        .filter(language_server_name.eq(server_name.clone()))
        .first::<model::WorkspaceLanguageServer>(conn)
        .optional()?;

    // `EnablementState` (telemetry) is not serde-able; persist a simple marker.
    let enablement_str = if enablement.is_enabled() {
        "enabled"
    } else {
        "disabled"
    }
    .to_string();

    if let Some(existing_record) = existing {
        // Update existing record
        diesel::update(workspace_language_server.find(existing_record.id))
            .set(enabled.eq(enablement_str))
            .execute(conn)?;
    } else {
        // Insert new record
        let new_language_server = model::NewWorkspaceLanguageServer {
            workspace_id: ws_id,
            language_server_name: server_name,
            enabled: enablement_str.to_string(),
        };

        diesel::insert_into(workspace_language_server)
            .values(&new_language_server)
            .execute(conn)?;
    }

    Ok(())
}

fn save_project(conn: &mut SqliteConnection, project: Project) -> Result<()> {
    use schema::projects::dsl::*;

    diesel::insert_into(projects)
        .values(project.clone())
        .on_conflict(path)
        .do_update()
        .set(&project)
        .execute(conn)?;

    Ok(())
}

fn get_all_projects(conn: &mut SqliteConnection) -> Result<Vec<Project>, diesel::result::Error> {
    use schema::projects::dsl::*;

    Ok(projects
        .load_iter::<Project, DefaultLoadingMode>(conn)?
        .filter_map(|item| item.ok())
        .collect_vec())
}

fn delete_project(conn: &mut SqliteConnection, project_path: &str) -> Result<()> {
    use schema::projects::dsl::*;

    diesel::delete(projects.filter(path.eq(project_path))).execute(conn)?;

    Ok(())
}

fn get_all_ignored_suggestions(
    conn: &mut SqliteConnection,
) -> Result<Vec<(String, SuggestionType)>, diesel::result::Error> {
    use schema::ignored_suggestions::dsl::*;

    Ok(ignored_suggestions
        .select((suggestion, suggestion_type))
        .load::<(String, String)>(conn)?
        .into_iter()
        .filter_map(|(suggestion_text, suggestion_type_str)| {
            SuggestionType::from_str(&suggestion_type_str)
                .map(|parsed_suggestion_type| (suggestion_text, parsed_suggestion_type))
        })
        .collect())
}

fn add_ignored_suggestion(
    conn: &mut SqliteConnection,
    suggestion_text: String,
    suggestion_type_param: SuggestionType,
) -> Result<()> {
    use schema::ignored_suggestions::dsl::*;

    let new_suggestion = model::NewIgnoredSuggestion {
        suggestion: suggestion_text,
        suggestion_type: suggestion_type_param.as_str().to_string(),
    };

    diesel::insert_into(ignored_suggestions)
        .values(&new_suggestion)
        .on_conflict((suggestion, suggestion_type))
        .do_nothing()
        .execute(conn)?;

    Ok(())
}

fn remove_ignored_suggestion(
    conn: &mut SqliteConnection,
    suggestion_text: String,
    suggestion_type_param: SuggestionType,
) -> Result<()> {
    use schema::ignored_suggestions::dsl::*;

    diesel::delete(
        ignored_suggestions.filter(
            suggestion
                .eq(suggestion_text)
                .and(suggestion_type.eq(suggestion_type_param.as_str())),
        ),
    )
    .execute(conn)?;

    Ok(())
}

fn save_workspace(conn: &mut SqliteConnection, workspace: WorkspaceMetadata) -> Result<()> {
    // Set all existing workspaces as not selected
    diesel::update(workspaces)
        .set(is_selected.eq(false))
        .execute(conn)?;

    // Save new workspace and set it as current workspace
    use schema::workspaces::dsl::*;
    let new_workspace = NewWorkspace {
        name: workspace.name,
        server_uid: workspace.uid.into(),
        is_selected: true,
    };

    diesel::insert_into(workspaces)
        .values(&new_workspace)
        .on_conflict(schema::workspaces::dsl::server_uid)
        .do_update()
        // If there's already a workspace with this server_uid, then lets just update the other values
        .set(&new_workspace)
        .execute(conn)?;

    // Save teams for workspace
    for team in workspace.teams {
        use schema::teams::dsl::*;
        use schema::workspace_teams::dsl::*;
        let new_team = NewTeam {
            name: team.name,
            server_uid: team.uid.into(),
            billing_metadata_json: serde_json::to_string(&team.billing_metadata).ok(),
        };
        diesel::insert_into(teams)
            .values(&new_team)
            .on_conflict(server_uid)
            .do_update()
            // If there's already a team with this server_uid, then lets just update the other values
            .set(&new_team)
            .execute(conn)?;

        let team_db_id: i32 = schema::teams::dsl::teams
            .filter(schema::teams::dsl::server_uid.eq::<String>(team.uid.into()))
            .select(schema::teams::dsl::id)
            .first(conn)?;

        diesel::delete(
            schema::team_members::dsl::team_members
                .filter(schema::team_members::dsl::team_id.eq(team_db_id)),
        )
        .execute(conn)?;

        for member in &team.members {
            let new_member = model::NewTeamMember {
                team_id: team_db_id,
                user_uid: member.uid.as_string(),
                email: member.email.clone(),
                role: serde_json::to_string(&member.role).unwrap_or_default(),
            };
            diesel::insert_into(schema::team_members::dsl::team_members)
                .values(&new_member)
                .execute(conn)?;
        }

        let new_workspace_team = NewWorkspaceTeam {
            workspace_server_uid: workspace.uid.into(),
            team_server_uid: team.uid.into(),
        };
        diesel::insert_into(workspace_teams)
            .values(&new_workspace_team)
            .on_conflict((workspace_server_uid, team_server_uid))
            .do_update()
            .set(&new_workspace_team)
            .execute(conn)?;
    }

    Ok(())
}

fn save_workspaces(
    conn: &mut SqliteConnection,
    workspaces_to_insert: Vec<WorkspaceMetadata>,
) -> Result<()> {
    use schema::team_settings::dsl::*;
    use schema::teams::dsl::*;
    use schema::workspace_teams::dsl::*;
    use schema::workspaces::dsl::*;

    // Get currently selected workspace uid if there is one
    let current_workspace_uid: Option<WorkspaceUid> = workspaces
        .filter(is_selected.eq(true))
        .select(schema::workspaces::dsl::server_uid)
        .first::<String>(conn)
        .optional()?
        .map(|uid| uid.into());

    // Remove all team_members/team_settings/workspaces/teams/workspace_teams stored locally.
    diesel::delete(schema::team_members::dsl::team_members).execute(conn)?;
    diesel::delete(team_settings).execute(conn)?;
    diesel::delete(workspace_teams).execute(conn)?;
    diesel::delete(teams).execute(conn)?;
    diesel::delete(workspaces).execute(conn)?;

    // Insert workspaces returned by server (doing nothing on conflict), set is_selected
    // to true for the current_workspace_uid if it is in the list of workspaces.
    let new_workspace_values: Vec<NewWorkspace> = workspaces_to_insert
        .clone()
        .into_iter()
        .map(|workspace| NewWorkspace {
            server_uid: workspace.uid.into(),
            name: workspace.name,
            is_selected: current_workspace_uid
                .map(|current_uid| workspace.uid == current_uid)
                .unwrap_or(false),
        })
        .collect();
    diesel::insert_or_ignore_into(workspaces)
        .values(&new_workspace_values)
        .execute(conn)?;

    // Insert teams returned by server (doing nothing on conflict)
    let new_team_values: Vec<NewTeam> = workspaces_to_insert
        .clone()
        .into_iter()
        .flat_map(|workspace| {
            workspace
                .teams
                .into_iter()
                .map(|team| NewTeam {
                    server_uid: team.uid.into(),
                    name: team.name.clone(),
                    billing_metadata_json: serde_json::to_string(&team.billing_metadata).ok(),
                })
                .collect::<Vec<NewTeam>>()
        })
        .collect();
    diesel::insert_or_ignore_into(teams)
        .values(&new_team_values)
        .execute(conn)?;

    // We cannot directly return the id from the insert so perform
    // a second query for the id https://github.com/diesel-rs/diesel/issues/771.
    let teams_with_id: Vec<(i32, String)> = schema::teams::dsl::teams
        .select((schema::teams::dsl::id, schema::teams::dsl::server_uid))
        .load(conn)?;
    let teams_by_server_uid: HashMap<&String, i32> = HashMap::from_iter(
        teams_with_id
            .iter()
            .map(|(table_id, table_server_uid)| (table_server_uid, *table_id)),
    );

    // Insert workspace_teams returned by server (doing nothing on conflict)
    let workspace_teams_values: Vec<NewWorkspaceTeam> = workspaces_to_insert
        .clone()
        .into_iter()
        .flat_map(|workspace| {
            workspace
                .teams
                .into_iter()
                .map(|team| NewWorkspaceTeam {
                    workspace_server_uid: workspace.uid.into(),
                    team_server_uid: team.uid.into(),
                })
                .collect::<Vec<NewWorkspaceTeam>>()
        })
        .collect();
    diesel::insert_or_ignore_into(workspace_teams)
        .values(&workspace_teams_values)
        .execute(conn)?;

    // Cache workspace settings returned by the server (overwriting any existing settings)
    let team_settings_values: Vec<NewTeamSettings> = workspaces_to_insert
        .clone()
        .into_iter()
        .flat_map(|workspace| {
            workspace.teams.into_iter().filter_map(|team| {
                let serialized_settings_json =
                    serde_json::to_string(&team.organization_settings).ok()?;
                let team_id_match = teams_by_server_uid.get(&team.uid.uid())?;
                Some(NewTeamSettings {
                    team_id: *team_id_match,
                    settings_json: serialized_settings_json,
                })
            })
        })
        .collect();
    diesel::insert_into(schema::team_settings::dsl::team_settings)
        .values(&team_settings_values)
        .execute(conn)?;

    // Cache team members
    let team_member_values: Vec<model::NewTeamMember> = workspaces_to_insert
        .clone()
        .into_iter()
        .flat_map(|workspace| {
            workspace.teams.into_iter().flat_map(|team| {
                let team_id_match = teams_by_server_uid.get(&team.uid.uid()).copied();
                team.members.into_iter().filter_map(move |member| {
                    Some(model::NewTeamMember {
                        team_id: team_id_match?,
                        user_uid: member.uid.as_string(),
                        email: member.email,
                        role: serde_json::to_string(&member.role).unwrap_or_default(),
                    })
                })
            })
        })
        .collect();
    if !team_member_values.is_empty() {
        diesel::insert_into(schema::team_members::dsl::team_members)
            .values(&team_member_values)
            .execute(conn)?;
    }

    if let Some(current_workspace_uid) = current_workspace_uid {
        if !workspaces_to_insert
            .iter()
            .any(|workspace| workspace.uid == current_workspace_uid)
        {
            // If the currently selected workspace is not in the list of workspaces, set
            // the first workspace as the current workspace.
            if let Some(first_workspace) = workspaces_to_insert.first() {
                diesel::update(workspaces.filter(
                    schema::workspaces::dsl::server_uid.eq::<String>(first_workspace.uid.into()),
                ))
                .set(is_selected.eq(true))
                .execute(conn)?;
            }
        }
    }

    Ok(())
}

fn set_current_workspace(conn: &mut SqliteConnection, workspace_uid: WorkspaceUid) -> Result<()> {
    use schema::workspaces::dsl::*;

    // Set all existing workspaces as not selected
    diesel::update(workspaces)
        .set(is_selected.eq(false))
        .execute(conn)?;

    diesel::update(
        workspaces.filter(schema::workspaces::dsl::server_uid.eq::<String>(workspace_uid.into())),
    )
    .set(is_selected.eq(true))
    .execute(conn)?;

    Ok(())
}

fn read_root_node(conn: &mut SqliteConnection, tab_id_val: i32) -> Result<PaneNodeSnapshot> {
    use schema::pane_nodes::dsl::*;

    let pane_node: model::PaneNode = schema::pane_nodes::dsl::pane_nodes
        .filter(tab_id.eq(tab_id_val))
        .filter(parent_pane_node_id.is_null())
        .first(conn)?;
    read_node(conn, pane_node)
}

/// Reads a saved node back into a snapshot.
fn read_node(conn: &mut SqliteConnection, node: model::PaneNode) -> Result<PaneNodeSnapshot> {
    match node.is_leaf {
        true => {
            let pane = schema::pane_leaves::dsl::pane_leaves
                .filter(schema::pane_leaves::columns::pane_node_id.eq(node.id))
                .first::<model::PaneLeaf>(conn)?;

            let contents = match pane.kind.as_ref() {
                TERMINAL_PANE_KIND => {
                    let terminal_pane = schema::terminal_panes::dsl::terminal_panes
                        .find(node.id)
                        .select(model::TerminalPane::as_select())
                        .first(conn)?;

                    let shell_launch_data: Option<ShellLaunchData> = terminal_pane
                        .shell_launch_data
                        .and_then(|shell_str| serde_json::from_str(&shell_str).ok());
                    let active_profile_id = terminal_pane
                        .active_profile_id
                        .and_then(|profile_str| serde_json::from_str(&profile_str).ok());
                    // Don't provide a fallback here - let the higher-level code with AppContext handle it

                    LeafContents::Terminal(TerminalPaneSnapshot {
                        uuid: terminal_pane.uuid,
                        cwd: terminal_pane.cwd,
                        is_active: terminal_pane.is_active,
                        is_read_only: false,
                        shell_launch_data,
                        active_profile_id,
                    })
                }
                SETTINGS_PANE_KIND => {
                    let settings_pane = schema::settings_panes::dsl::settings_panes
                        .find(node.id)
                        .select(model::SettingsPane::as_select())
                        .first(conn)?;

                    let current_page = SettingsSection::from_str(&settings_pane.current_page)
                        .ok()
                        .unwrap_or_default();
                    LeafContents::Settings(SettingsPaneSnapshot::Local {
                        current_page,
                        search_query: None,
                    })
                }
                GET_STARTED_PANE_KIND => LeafContents::GetStarted,
                WELCOME_PANE_KIND => {
                    let welcome_pane = schema::welcome_panes::dsl::welcome_panes
                        .find(node.id)
                        .select(model::WelcomePane::as_select())
                        .first(conn)?;
                    LeafContents::Welcome {
                        startup_directory: welcome_pane.startup_directory.map(PathBuf::from),
                    }
                }
                other => bail!("Unrecognized pane kind: {other}"),
            };

            Ok(PaneNodeSnapshot::Leaf(LeafSnapshot {
                is_focused: pane.is_focused,
                custom_vertical_tabs_title: pane.custom_vertical_tabs_title,
                contents,
            }))
        }
        false => {
            let pane_branch = schema::pane_branches::dsl::pane_branches
                .filter(schema::pane_branches::columns::pane_node_id.eq(node.id))
                .first::<model::PaneBranch>(conn)?;

            let child_nodes = schema::pane_nodes::dsl::pane_nodes
                .filter(schema::pane_nodes::columns::parent_pane_node_id.eq(node.id))
                .order(schema::pane_nodes::columns::id.asc())
                .load::<model::PaneNode>(conn)?;

            let mut children = Vec::new();
            for child_node in child_nodes {
                children.push((
                    PaneFlex(child_node.flex.unwrap_or(1.)),
                    read_node(conn, child_node)?,
                ));
            }

            let direction = match pane_branch.horizontal {
                true => SplitDirection::Horizontal,
                false => SplitDirection::Vertical,
            };
            Ok(PaneNodeSnapshot::Branch(BranchSnapshot {
                direction,
                children,
            }))
        }
    }
}

/// This is not in a transaction. The interface for a transaction is a bit awkward,
/// and makes it invalid to write the logic recursively. It's ok it's not in a
/// transaction because we should be the only connection using the database.
///
/// One notable exception is the case where there may be two warp apps running
/// in the same bundle. In this case, we may read some garbage, but all that will
/// happen is the user won't have session restoration.
///
/// In the future, the awkwardness of the transaction interface is resolved in diesel 2.0.0.
fn read_sqlite_data(conn: &mut SqliteConnection) -> Result<PersistedData, Error> {
    use schema::windows::dsl::*;

    let active_window_id = schema::app::dsl::app
        .select(schema::app::dsl::active_window_id)
        .first::<Option<i32>>(conn)
        .optional()?
        .flatten();
    let db_windows = windows.load::<Window>(conn)?;

    let mut active_window_index: Option<usize> = None;

    let db_tabs = Tab::belonging_to(&db_windows)
        .order_by(schema::tabs::columns::id.asc())
        .load::<Tab>(conn)?
        .grouped_by(&db_windows);

    let db_panels = schema::panels::dsl::panels
        .load::<model::Panel>(conn)?
        .into_iter()
        .map(|p| (p.tab_id, p))
        .collect::<HashMap<_, _>>();

    let saved_windows: Vec<_> = db_windows
        .into_iter()
        .enumerate()
        .zip(db_tabs)
        .map(|((idx, window), tabs_for_window)| {
            let saved_tabs: Vec<_> = tabs_for_window
                .into_iter()
                .filter_map(|tab| {
                    let root = read_root_node(conn, tab.id).ok()?;
                    let panel = db_panels.get(&tab.id);

                    let left_panel = panel
                        .and_then(|p| p.left_panel.as_ref())
                        .and_then(|s| serde_json::from_str::<LeftPanelSnapshot>(s).ok());

                    let right_panel = panel
                        .and_then(|p| p.right_panel.as_ref())
                        .and_then(|s| serde_json::from_str::<RightPanelSnapshot>(s).ok());

                    Some(TabSnapshot {
                        root,
                        custom_title: tab.custom_title,
                        default_directory_color: None,
                        selected_color: tab
                            .color
                            .as_deref()
                            .and_then(|s| {
                                serde_yaml::from_str::<SelectedTabColor>(s)
                                    .ok()
                                    .or_else(|| {
                                        // Fall back to the old format which stored a bare AnsiColorIdentifier
                                        serde_yaml::from_str::<AnsiColorIdentifier>(s)
                                            .ok()
                                            .map(SelectedTabColor::Color)
                                    })
                            })
                            .unwrap_or_default(),
                        left_panel,
                        right_panel,
                    })
                })
                .collect();

            if active_window_id
                .map(|window_id| window.id == window_id)
                .unwrap_or(false)
            {
                active_window_index = Some(idx);
            }

            // Default active tab index to 0 if we overflow when converting.
            let tab_index: usize = window.active_tab_index.try_into().unwrap_or(0);

            let fullscreen_state_val =
                FullscreenState::from_i32(window.fullscreen_state).unwrap_or_default();

            // The origin and size of the bound should be all null or all non-null.
            // Reject bounds smaller than the platform minimum window size so users
            // with an already-corrupted warp.sqlite (see GH#10083) restore to
            // default geometry instead of a sliver.
            let bounds = match (
                window.window_width,
                window.window_height,
                window.origin_x,
                window.origin_y,
            ) {
                (Some(mut width), Some(mut height), Some(x), Some(y))
                    if width >= MIN_WINDOW_WIDTH && height >= MIN_WINDOW_HEIGHT =>
                {
                    // When fullscreen or maximized, the `inner_size` we snapshotted will be the
                    // size of the full screen. This will cause problems with winit. When you set
                    // maximized/fullscreen, setting the inner_size will by the size the window
                    // takes _after_ the user toggles _out_ of fullscreen/maximized. Therefore, we
                    // don't want to set the size to take the full screen because the window will
                    // appear to remain in maximized/fullscreen. We multiply each dimension by 0.8
                    // to prevent taking the full screen while choosing a reasonable size.
                    if !cfg!(target_os = "macos") && fullscreen_state_val != FullscreenState::Normal
                    {
                        width *= 0.8;
                        height *= 0.8;
                    }
                    Some(RectF::new(
                        Vector2F::new(x, y),
                        Vector2F::new(width, height),
                    ))
                }
                _ => None,
            };

            let left_panel_width: Option<f32> = saved_tabs.get(tab_index).and_then(|tab| match tab
                .left_panel
                .as_ref()
            {
                Some(LeftPanelSnapshot { width, .. }) => Some(*width as f32),
                _ => None,
            });

            let right_panel_width: Option<f32> =
                saved_tabs
                    .get(tab_index)
                    .and_then(|tab| match tab.right_panel.as_ref() {
                        Some(RightPanelSnapshot { width, .. }) => Some(*width as f32),
                        _ => None,
                    });

            let window_left_panel_open = window.left_panel_open.unwrap_or_else(|| {
                saved_tabs
                    .get(tab_index)
                    .and_then(|tab| tab.left_panel.as_ref())
                    .is_some()
            });

            WindowSnapshot {
                tabs: saved_tabs,
                active_tab_index: tab_index,
                quake_mode: window.quake_mode,
                bounds,
                universal_search_width: window.universal_search_width,
                warp_ai_width: window.warp_ai_width,
                voltron_width: window.voltron_width,
                warp_drive_index_width: window.warp_drive_index_width,
                left_panel_open: window_left_panel_open,
                vertical_tabs_panel_open: window.vertical_tabs_panel_open.unwrap_or(false),
                fullscreen_state: fullscreen_state_val,
                left_panel_width,
                right_panel_width,
            }
        })
        .collect();

    let db_teams: Vec<model::Team> = schema::teams::dsl::teams.load(conn)?;

    let team_member_rows: Vec<model::TeamMemberRow> =
        schema::team_members::dsl::team_members.load(conn)?;
    let members_by_team_id: HashMap<i32, Vec<crate::workspaces::team::TeamMember>> =
        team_member_rows
            .into_iter()
            .fold(HashMap::new(), |mut acc, row| {
                let member = crate::workspaces::team::TeamMember {
                    uid: UserUid::new(&row.user_uid),
                    email: row.email,
                    role: serde_json::from_str(&row.role)
                        .unwrap_or(crate::workspaces::team::MembershipRole::User),
                };
                acc.entry(row.team_id).or_default().push(member);
                acc
            });

    let team_settings_rows: Vec<model::TeamSetting> =
        schema::team_settings::dsl::team_settings.load(conn)?;
    let settings_by_team_id: HashMap<i32, String> = team_settings_rows
        .into_iter()
        .map(|ts| (ts.team_id, ts.settings_json))
        .collect();

    let teams: Vec<TeamMetadata> = db_teams
        .into_iter()
        .map(|team| {
            let team_settings = settings_by_team_id
                .get(&team.id)
                .and_then(|json| serde_json::from_str(json).ok());

            let billing_metadata = team
                .billing_metadata_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok());

            let members = members_by_team_id.get(&team.id).cloned();

            TeamMetadata::from_local_cache(
                ServerId::from_string_lossy(team.server_uid),
                team.name,
                team_settings,
                billing_metadata,
                members,
            )
        })
        .collect();

    let workspace_teams: Vec<model::WorkspaceTeam> = schema::workspace_teams::dsl::workspace_teams
        .load_iter::<model::WorkspaceTeam, DefaultLoadingMode>(conn)?
        .filter_map(|workspace_team| workspace_team.ok())
        .collect();

    let workspaces: Vec<WorkspaceMetadata> = schema::workspaces::dsl::workspaces
        .load_iter::<model::Workspace, DefaultLoadingMode>(conn)?
        .filter_map(|workspace| {
            workspace.ok().map(|workspace| {
                let teams_for_workspace = workspace_teams
                    .iter()
                    .filter_map(|workspace_team| {
                        if workspace_team.workspace_server_uid == workspace.server_uid {
                            teams.iter().find(|team| {
                                team.uid
                                    == ServerId::from_string_lossy(&workspace_team.team_server_uid)
                            })
                        } else {
                            None
                        }
                    })
                    .cloned()
                    .collect();
                WorkspaceMetadata::from_local_cache(
                    workspace.server_uid.into(),
                    workspace.name,
                    Some(teams_for_workspace),
                )
            })
        })
        .collect();

    let current_workspace_uid: Option<WorkspaceUid> = schema::workspaces::dsl::workspaces
        .filter(schema::workspaces::dsl::is_selected.eq(true))
        .select(schema::workspaces::dsl::server_uid)
        .first::<String>(conn)
        .optional()?
        .map(|uid| uid.into());

    let commands = schema::commands::dsl::commands
        // Ensure the commands come into memory sorted chronologically.
        .order(schema::commands::columns::id.desc())
        .load_iter::<model::Command, DefaultLoadingMode>(conn)?
        .filter_map(|command| command.ok())
        .map(PersistedCommand::from)
        .collect();

    let user_profiles = schema::user_profiles::dsl::user_profiles
        .load_iter::<model::UserProfile, DefaultLoadingMode>(conn)?
        .filter_map(|user_profile| user_profile.ok())
        .map(user_profile_from_persistence)
        .collect();

    let server_experiments = schema::server_experiments::dsl::server_experiments
        .load_iter::<model::ServerExperiment, DefaultLoadingMode>(conn)?
        .filter_map(|server_experiment| server_experiment.ok())
        .filter_map(|server_experiment| {
            ServerExperiment::from_string(server_experiment.experiment).ok()
        })
        .collect();

    let _restored_blocks = get_all_restored_blocks(conn)?;

    let app_state = AppState {
        windows: saved_windows,
        active_window_index,
    };

    let workspace_language_servers = get_all_workspace_language_servers_by_workspace(conn)?;
    let projects = get_all_projects(conn)?;
    let ignored_suggestions = get_all_ignored_suggestions(conn)?;

    Ok(PersistedData {
        app_state,
        workspaces,
        current_workspace_uid,
        command_history: commands,
        user_profiles,
        experiments: server_experiments,
        workspace_language_servers,
        projects,
        ignored_suggestions,
    })
}

impl From<StartedCommandMetadata> for model::NewCommand {
    fn from(metadata: StartedCommandMetadata) -> Self {
        Self {
            command: metadata.command,
            exit_code: None,
            start_ts: metadata.start_ts.map(|ts| ts.naive_utc()),
            completed_ts: None,
            pwd: metadata.pwd,
            shell: metadata.shell,
            username: metadata.username,
            hostname: metadata.hostname,
            session_id: metadata.session_id.and_then(|id| {
                // The `SessionID` is a wrapper around a `u64`. However diesel only allows
                // writing signed values for sqlite, which means we must convert it into an `i64`.
                // This is a shortcoming of how we represent the `SessionID`: we aren't guaranteed
                // (from a type safety perspective) that we can write it into SQLite. This is
                // another reason why the `SessionID` should be created within Rust and then passed
                // to our bootstrap scripts instead of the other way around: it would allow us to
                // create a random ID that could either be a `u16` or a `u32`.
                let id: u64 = id.into();
                id.try_into().ok()
            }),
            git_branch: metadata.git_branch,
            cloud_workflow_id: None,
            workflow_command: None,
            is_agent_executed: None,
        }
    }
}

fn insert_command(
    conn: &mut SqliteConnection,
    command_metadata: StartedCommandMetadata,
) -> Result<(), Error> {
    use schema::commands::dsl::*;

    conn.transaction::<(), Error, _>(|conn| {
        let command_count: i64 = commands.count().first(conn)?;
        if command_count == COMMANDS_COUNT_LIMIT {
            let oldest_command_id: i32 =
                commands.select(id).order(id.asc()).limit(1).first(conn)?;
            diesel::delete(commands.filter(id.eq(oldest_command_id))).execute(conn)?;
        }

        let new_command: NewCommand = command_metadata.into();
        diesel::insert_into(schema::commands::dsl::commands)
            .values(new_command)
            .execute(conn)?;
        Ok(())
    })
}

fn update_finished_command(
    conn: &mut SqliteConnection,
    completed_command: FinishedCommandMetadata,
) -> Result<(), Error> {
    use schema::commands::dsl::*;

    let completed_command_session_id: Option<i64> =
        completed_command.session_id.as_u64().try_into().ok();

    conn.transaction::<(), Error, _>(|conn| {
        diesel::update(commands)
            .filter(start_ts.eq(Some(completed_command.start_ts.naive_utc())))
            .filter(session_id.eq(completed_command_session_id))
            .set((
                exit_code.eq(completed_command.exit_code.value()),
                completed_ts.eq(completed_command.completed_ts.naive_utc()),
            ))
            .execute(conn)?;
        Ok(())
    })
}

fn upsert_user_profiles(
    conn: &mut SqliteConnection,
    profiles: Vec<UserProfileWithUID>,
) -> Result<(), Error> {
    use schema::user_profiles::dsl::*;

    conn.transaction::<(), Error, _>(|conn| {
        for profile in profiles {
            // Delete any stale profile with that uid
            diesel::delete(
                schema::user_profiles::dsl::user_profiles
                    .filter(firebase_uid.eq(profile.firebase_uid.to_string())),
            )
            .execute(conn)?;

            // Insert a new user profile row
            let new_user_profile = UserProfile {
                firebase_uid: profile.firebase_uid.to_string(),
                photo_url: profile.photo_url,
                display_name: profile.display_name,
                email: profile.email,
            };
            diesel::insert_into(schema::user_profiles::dsl::user_profiles)
                .values(new_user_profile)
                .execute(conn)?;
        }
        Ok(())
    })
}

fn save_experiments(
    conn: &mut SqliteConnection,
    experiments: Vec<ServerExperiment>,
) -> Result<(), Error> {
    conn.transaction::<(), Error, _>(|conn| {
        diesel::delete(schema::server_experiments::dsl::server_experiments).execute(conn)?;

        let new_experiments = experiments
            .into_iter()
            .map(|experiment| NewServerExperiment {
                experiment: experiment.to_string(),
            })
            .collect_vec();

        diesel::insert_into(schema::server_experiments::dsl::server_experiments)
            .values(new_experiments)
            .execute(conn)?;
        Ok(())
    })
}

fn clear_user_profiles(conn: &mut SqliteConnection) -> Result<(), Error> {
    conn.transaction::<(), Error, _>(|conn| {
        diesel::delete(schema::user_profiles::dsl::user_profiles).execute(conn)?;

        Ok(())
    })
}

fn upsert_current_user_information(
    conn: &mut SqliteConnection,
    user_information: PersistedCurrentUserInformation,
) -> Result<(), Error> {
    conn.transaction::<(), Error, _>(|conn| {
        diesel::delete(schema::current_user_information::dsl::current_user_information)
            .execute(conn)?;

        diesel::insert_into(schema::current_user_information::dsl::current_user_information)
            .values(CurrentUserInformation {
                email: user_information.email,
            })
            .execute(conn)?;
        Ok(())
    })
}

fn upsert_mcp_server_environment_variables(
    conn: &mut SqliteConnection,
    mcp_server_uuid: Vec<u8>,
    environment_variables: String,
) -> Result<(), Error> {
    conn.transaction::<(), Error, _>(|conn| {
        let env_vars = MCPEnvironmentVariables {
            mcp_server_uuid,
            environment_variables,
        };
        diesel::insert_into(schema::mcp_environment_variables::dsl::mcp_environment_variables)
            .values(&env_vars)
            .on_conflict(schema::mcp_environment_variables::dsl::mcp_server_uuid)
            .do_update()
            .set(&env_vars)
            .execute(conn)?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "sqlite_tests.rs"]
mod tests;
