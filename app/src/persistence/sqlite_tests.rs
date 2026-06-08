use std::path::PathBuf;
use std::sync::Arc;

use diesel::connection::SimpleConnection;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;

use super::{
    app_database_file_path, database_file_path_for_scope, deduplicate_events, read_sqlite_data,
    save_app_state, setup_database,
};
use crate::app_state::{
    AppState, LeafContents, LeafSnapshot, PaneNodeSnapshot, TabSnapshot, TerminalPaneSnapshot,
    WindowSnapshot,
};
use crate::persistence::{BlockCompleted, ModelEvent, PersistenceScope};
use crate::tab::SelectedTabColor;
use crate::terminal::model::block::SerializedBlock;
use crate::terminal::ShellLaunchData;

#[test]
fn app_scope_database_path_matches_app_database_path() {
    assert_eq!(
        database_file_path_for_scope(&PersistenceScope::App),
        app_database_file_path()
    );
}

#[test]
fn remote_server_daemon_scope_database_path_uses_identity_data_dir() {
    let path = database_file_path_for_scope(&PersistenceScope::RemoteServerDaemon {
        identity_key: "user@example.com/ssh host".to_string(),
    });
    let expected_data_dir =
        remote_server::setup::remote_server_daemon_data_dir("user@example.com/ssh host");

    assert!(path.is_absolute());
    assert_eq!(
        path,
        PathBuf::from(shellexpand::tilde(&expected_data_dir).into_owned()).join("warp.sqlite")
    );
}

#[test]
fn remote_server_daemon_scope_database_path_handles_empty_identity_key() {
    let path = database_file_path_for_scope(&PersistenceScope::RemoteServerDaemon {
        identity_key: String::new(),
    });
    let expected_data_dir = remote_server::setup::remote_server_daemon_data_dir("");

    assert_eq!(
        path,
        PathBuf::from(shellexpand::tilde(&expected_data_dir).into_owned()).join("warp.sqlite")
    );
}

#[cfg(unix)]
#[test]
fn remote_server_daemon_database_permissions_are_owner_only() {
    use std::fs::Permissions;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let daemon_dir = tempdir.path().join("daemon");
    let database_path = daemon_dir.join("warp.sqlite");

    std::fs::create_dir_all(&daemon_dir).expect("daemon dir should be created");
    std::fs::set_permissions(&daemon_dir, Permissions::from_mode(0o755))
        .expect("daemon dir permissions should be set");
    std::fs::write(&database_path, b"").expect("database file should be created");
    std::fs::set_permissions(&database_path, Permissions::from_mode(0o644))
        .expect("database file permissions should be set");

    super::ensure_owner_only_dir(&daemon_dir).expect("daemon dir should be owner-only");
    super::ensure_owner_only_file(&database_path).expect("database file should be owner-only");

    assert_eq!(daemon_dir.metadata().unwrap().mode() & 0o777, 0o700);
    assert_eq!(database_path.metadata().unwrap().mode() & 0o777, 0o600);
}

#[test]
fn sqlite_read_restores_app_state() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let database_path = tempdir.path().join("warp.sqlite");
    let mut conn = setup_database(&database_path).expect("database should initialize");

    let app_state = AppState {
        windows: vec![test_terminal_window_snapshot(false)],
        active_window_index: Some(0),
    };
    save_app_state(&mut conn, &app_state).expect("app state should save");

    let restored = read_sqlite_data(&mut conn).expect("persisted data should load");
    assert_eq!(restored.app_state.windows.len(), 1);
}

#[test]
fn test_deduplicate_snapshots() {
    let completed_block_1 = BlockCompleted {
        pane_id: vec![1, 2, 3],
        block: Arc::new(SerializedBlock::default()),
        is_local: true,
    };
    let completed_block_2 = BlockCompleted {
        pane_id: vec![4, 5, 6],
        block: Arc::new(SerializedBlock::default()),
        is_local: true,
    };
    let snapshot_1 = AppState {
        active_window_index: Some(1),
        windows: Default::default(),
    };
    let snapshot_2 = AppState {
        active_window_index: Some(2),
        windows: Default::default(),
    };
    let snapshot_3 = AppState {
        active_window_index: Some(3),
        windows: Default::default(),
    };

    let original_events = vec![
        ModelEvent::DeleteBlocks(vec![7, 8, 9]),
        ModelEvent::Snapshot(snapshot_1.clone()),
        ModelEvent::SaveBlock(completed_block_1.clone()),
        ModelEvent::Snapshot(snapshot_2.clone()),
        ModelEvent::SaveBlock(completed_block_2.clone()),
        ModelEvent::Snapshot(snapshot_3.clone()),
        ModelEvent::DeleteBlocks(vec![10, 11, 12]),
    ];

    let filtered_events = deduplicate_events(original_events);
    assert_eq!(filtered_events.len(), 5);

    assert!(matches!(&filtered_events[0], &ModelEvent::DeleteBlocks(_)));
    // The first snapshot should have been filtered out.
    assert!(matches!(&filtered_events[1], &ModelEvent::SaveBlock(_)));
    // The second snapshot should have been filtered out.
    assert!(matches!(&filtered_events[2], &ModelEvent::SaveBlock(_)));
    // The third snapshot should be preserved.
    match &filtered_events[3] {
        ModelEvent::Snapshot(snapshot) => assert_eq!(snapshot, &snapshot_3),
        other => panic!("Expected ModelEvent::Snapshot, got {other:?}"),
    }
    assert!(matches!(&filtered_events[4], &ModelEvent::DeleteBlocks(_)));
}

#[test]
fn test_deduplicate_no_snapshots() {
    let original_events = vec![ModelEvent::SaveBlock(BlockCompleted {
        pane_id: vec![1, 2, 3],
        block: Default::default(),
        is_local: true,
    })];
    let filtered_events = deduplicate_events(original_events);
    assert_eq!(filtered_events.len(), 1);
    assert!(matches!(&filtered_events[0], &ModelEvent::SaveBlock(_)));
}

fn test_terminal_window_snapshot(vertical_tabs_panel_open: bool) -> WindowSnapshot {
    WindowSnapshot {
        tabs: vec![TabSnapshot {
            custom_title: None,
            root: PaneNodeSnapshot::Leaf(LeafSnapshot {
                is_focused: true,
                custom_vertical_tabs_title: None,
                contents: LeafContents::Terminal(TerminalPaneSnapshot {
                    uuid: vec![u8::from(vertical_tabs_panel_open) + 1],
                    cwd: Some("/tmp".to_string()),
                    shell_launch_data: Some(ShellLaunchData::Executable {
                        executable_path: PathBuf::from("/bin/zsh"),
                        shell_type: crate::terminal::shell::ShellType::Zsh,
                    }),
                    is_active: true,
                    is_read_only: false,
                    active_profile_id: None,
                }),
            }),
            default_directory_color: None,
            selected_color: SelectedTabColor::default(),
            left_panel: None,
            right_panel: None,
        }],
        active_tab_index: 0,
        bounds: None,
        fullscreen_state: Default::default(),
        quake_mode: false,
        universal_search_width: None,
        warp_ai_width: None,
        voltron_width: None,
        warp_drive_index_width: None,
        left_panel_open: false,
        vertical_tabs_panel_open,
        left_panel_width: None,
        right_panel_width: None,
    }
}

#[test]
fn test_sqlite_round_trips_vertical_tabs_panel_open() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let database_path = tempdir.path().join("warp.sqlite");
    let mut conn = setup_database(&database_path).expect("database should initialize");

    let app_state = AppState {
        windows: vec![
            test_terminal_window_snapshot(false),
            test_terminal_window_snapshot(true),
        ],
        active_window_index: Some(1),
    };

    save_app_state(&mut conn, &app_state).expect("app state should save");

    let restored = read_sqlite_data(&mut conn)
        .expect("app state should load")
        .app_state;

    assert_eq!(restored.active_window_index, Some(1));
    assert_eq!(
        restored
            .windows
            .iter()
            .map(|window| window.vertical_tabs_panel_open)
            .collect::<Vec<_>>(),
        vec![false, true]
    );
}

#[test]
fn test_sqlite_round_trips_custom_vertical_tabs_title() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let database_path = tempdir.path().join("warp.sqlite");
    let mut conn = setup_database(&database_path).expect("database should initialize");

    let app_state = AppState {
        windows: vec![WindowSnapshot {
            tabs: vec![TabSnapshot {
                custom_title: None,
                root: PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: true,
                    custom_vertical_tabs_title: Some("Production API".to_string()),
                    contents: LeafContents::Terminal(TerminalPaneSnapshot {
                        uuid: vec![42],
                        cwd: Some("/tmp".to_string()),
                        shell_launch_data: Some(ShellLaunchData::Executable {
                            executable_path: PathBuf::from("/bin/zsh"),
                            shell_type: crate::terminal::shell::ShellType::Zsh,
                        }),
                        is_active: true,
                        is_read_only: false,
                        active_profile_id: None,
                    }),
                }),
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                left_panel: None,
                right_panel: None,
            }],
            active_tab_index: 0,
            bounds: None,
            fullscreen_state: Default::default(),
            quake_mode: false,
            universal_search_width: None,
            warp_ai_width: None,
            voltron_width: None,
            warp_drive_index_width: None,
            left_panel_open: false,
            vertical_tabs_panel_open: false,
            left_panel_width: None,
            right_panel_width: None,
        }],
        active_window_index: Some(0),
    };

    save_app_state(&mut conn, &app_state).expect("app state should save");

    let restored = read_sqlite_data(&mut conn)
        .expect("app state should load")
        .app_state;

    let PaneNodeSnapshot::Leaf(LeafSnapshot {
        custom_vertical_tabs_title,
        ..
    }) = &restored.windows[0].tabs[0].root
    else {
        panic!("Expected terminal pane leaf");
    };
    assert_eq!(
        custom_vertical_tabs_title.as_deref(),
        Some("Production API")
    );
}

// Regression: GH#10083. The macOS green-tile button could leave a 1px-wide
// window bound in `AppContext::window_bounds`, which previously round-tripped
// through SQLite and restored as an unusable 1px sliver. Bounds below the
// platform minimum window size must be dropped on save.
#[test]
fn test_sqlite_drops_too_small_bounds_on_save() {
    use diesel::prelude::*;

    use crate::persistence::schema::windows;

    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let database_path = tempdir.path().join("warp.sqlite");
    let mut conn = setup_database(&database_path).expect("database should initialize");

    let mut snapshot = test_terminal_window_snapshot(false);
    snapshot.bounds = Some(RectF::new(
        Vector2F::new(0.0, -1410.0),
        Vector2F::new(1.0, 1410.0),
    ));

    let app_state = AppState {
        windows: vec![snapshot],
        active_window_index: Some(0),
    };

    save_app_state(&mut conn, &app_state).expect("app state should save");

    // Query the row directly so the assertion isolates the save guard and is
    // not masked by the read-side guard in `read_sqlite_data`.
    let row: (Option<f32>, Option<f32>, Option<f32>, Option<f32>) = windows::dsl::windows
        .select((
            windows::columns::window_width,
            windows::columns::window_height,
            windows::columns::origin_x,
            windows::columns::origin_y,
        ))
        .first(&mut conn)
        .expect("a windows row should have been inserted");

    assert_eq!(
        row,
        (None, None, None, None),
        "save-path guard must persist NULL bound columns for sub-minimum geometry"
    );
}

// Regression: GH#10083. Users whose warp.sqlite already contains a 1px row
// (because they hit the bug on an earlier build) must still recover to default
// geometry on next launch rather than restoring the sliver.
#[test]
fn test_sqlite_drops_too_small_bounds_on_read() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let database_path = tempdir.path().join("warp.sqlite");
    let mut conn = setup_database(&database_path).expect("database should initialize");

    // Save with no bounds so a row exists, then corrupt it directly to bypass
    // the save-path guard and simulate a pre-existing bad row.
    let app_state = AppState {
        windows: vec![test_terminal_window_snapshot(false)],
        active_window_index: Some(0),
    };
    save_app_state(&mut conn, &app_state).expect("app state should save");

    conn.batch_execute(
        "UPDATE windows \
         SET window_width = 1.0, window_height = 1410.0, \
             origin_x = 0.0, origin_y = -1410.0",
    )
    .expect("corrupting update should succeed");

    let restored = read_sqlite_data(&mut conn)
        .expect("app state should load")
        .app_state;

    assert_eq!(restored.windows.len(), 1);
    assert!(
        restored.windows[0].bounds.is_none(),
        "tiny persisted bounds must be discarded on read so users recover from a corrupt DB"
    );
}
