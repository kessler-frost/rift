// Suppress warnings about rustdoc style.
#![allow(clippy::doc_lazy_continuation)]

mod alloc;
mod antivirus;
#[cfg(target_os = "macos")]
mod app_menus;
mod app_services;
mod app_state;
mod auth;
mod banner;
mod chip_configurator;
mod coding_entrypoints;
mod coding_panel_enablement_state;
mod command_palette;
mod completer;
#[allow(dead_code)]
mod context_chips;
#[cfg(enable_crash_recovery)]
mod crash_recovery;
mod debug_dump;
mod default_terminal;
mod download_method;
#[cfg(windows)]
mod dynamic_libraries;
mod external_secrets;
mod global_resource_handles;
mod gpu_state;
mod input_classifier;
mod interval_timer;
mod linear;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod login_item;
mod menu;
mod modal;
mod network;
mod notification;
mod palette;
mod persistence;
mod platform;
#[cfg(feature = "plugin_host")]
mod plugin;
mod prefix;
#[cfg(target_os = "macos")]
mod pricing;
mod profiling;
mod projects;
mod prompt;
mod quit_warning;
#[allow(dead_code)]
mod resource_limits;
mod safe_triangle;
mod search_bar;
mod server;
mod session_management;
mod shell_indicator;
mod suggestions;
mod system;
mod tab;
#[cfg(test)]
mod test_util;
mod throttle;
mod tips;
mod tracing;
mod ui_components;
mod undo_close;
mod uri;
mod user_config;
pub mod util;
mod view_components;
mod vim_registers;
mod rift_managed_paths_watcher;
mod window_settings;
mod workspaces;

// PLEASE DO NOT ADD MORE PUBLIC MODULES!
//
// Any modules which we make public outside of the `rift` crate lose dead code
// checking support, as the compiler cannot make any assumptions about whether
// or not the function/type is used by another crate that pulls in this one as
// a dependency.
//
// If you feel the need to export a module so that a type or function within it
// can be used by an integration test, you should define a new assertion function
// in the rift::integration_testing::assertions module (or a sub-module).  These
// functions will allow us to keep types internal to this crate and expose a
// simpler API for integration tests to consume.
pub mod appearance;
pub mod channel;
pub mod editor;
pub mod features;
pub mod input_suggestions;
#[cfg(feature = "integration_tests")]
pub mod integration_testing;
pub mod keyboard;
pub mod launch_configs;
pub mod pane_group;
pub mod resource_center;
pub mod root_view;
pub mod search;
pub mod settings;
pub mod settings_view;
pub mod tab_configs;
pub mod terminal;
pub mod themes;
use auth::auth_manager::AuthManager;
use auth::auth_state::{AuthState, AuthStateProvider};
use quit_warning::UnsavedStateSummary;
#[cfg(feature = "local_fs")]
use repo_metadata::{
    repositories::DetectedRepositories, watcher::DirectoryWatcher, RepoMetadataModel,
};
use rift_cli::agent::AgentCommand;
use rift_cli::{CliCommand, GlobalOptions};
use server::telemetry::context_provider::AppTelemetryContextProvider;
#[cfg(feature = "local_fs")]
use settings::import::model::ImportedConfigModel;
use settings_view::pane_manager::SettingsPaneManager;
use terminal::general_settings::GeneralSettings;
use terminal::keys_settings::KeysSettings;
#[cfg(all(not(target_family = "wasm"), feature = "local_tty"))]
use terminal::local_shell::LocalShellState;
pub use util::bindings::cmd_or_ctrl_shift;
#[cfg(feature = "local_fs")]
use watcher::HomeDirectoryWatcher;

pub mod workspace;

use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

use ::settings::{Setting, ToggleableSetting};
#[cfg(feature = "local_tty")]
use anyhow::Context;
use anyhow::{anyhow, Result};
use appearance::{Appearance, AppearanceManager};
use channel::ChannelState;
use interval_timer::IntervalTimer;
use itertools::Itertools;
#[cfg(feature = "integration_tests")]
pub use persistence::testing as sqlite_testing;
#[cfg(feature = "plugin_host")]
pub use plugin::{run_plugin_host, PLUGIN_HOST_FLAG};
pub use rift_core::errors::{report_error, report_if_error};
use rift_core::execution_mode::{AppExecutionMode, ExecutionMode};
use settings::{ExtraMetaKeys, PrivacySettings};
use terminal::input;
use terminal::session_settings::SessionSettings;
use url::Url;
// Re-export the debounce function to simplify imports.
pub use rift_core::r#async::debounce;
// Re-export the send_telemetry_from_ctx macro at the crate root level
pub use rift_core::send_telemetry_from_app_ctx;
pub use rift_core::send_telemetry_from_ctx;
// Re-export the safe logging macros at the crate root level for backwards compatibility
pub use rift_core::{safe_debug, safe_error, safe_info, safe_warn};
#[cfg(feature = "local_fs")]
use rift_files::FileModel;
use rift_logging::LogDestination;
use riftui::integration::TestDriver;
use riftui::platform::app::ApproveTerminateResult;
use riftui::platform::TerminationMode;
use riftui::windowing::state::ApplicationStage;
use riftui::{App, AppContext, Event, SingletonEntity, WindowId};
use window_settings::WindowSettings;
use workspace::sync_inputs::SyncedInputState;

use self::features::FeatureFlag;
use crate::antivirus::AntivirusInfo;
use crate::app_state::AppState;
use crate::context_chips::prompt::Prompt;
use crate::default_terminal::DefaultTerminal;
pub use crate::global_resource_handles::{GlobalResourceHandles, GlobalResourceHandlesProvider};
use crate::gpu_state::GPUState;
use crate::network::NetworkStatus;
use crate::notification::NotificationContext;
use crate::palette::PaletteMode;
use crate::persistence::PersistenceWriter;
use crate::projects::ProjectManagementModel;
use crate::root_view::{
    quake_mode_window_id, quake_mode_window_is_open, OpenFromRestoredArg, OpenPath,
};
pub use crate::server::telemetry::{
    AgentModeEntrypoint, AgentModeEntrypointSelectionType, TelemetryEvent,
};
use crate::server::telemetry::{AppStartupInfo, PaletteSource, TelemetryCollector};
use crate::session_management::{RunningSessionSummary, SessionNavigationData};
use crate::settings::manager::SettingsManager;
use crate::settings::{AccessibilitySettings, ScrollSettings, SelectionSettings};
use crate::settings_view::keybindings::KeybindingChangedNotifier;
use crate::settings_view::DisplayCount;
use crate::suggestions::ignored_suggestions_model::IgnoredSuggestionsModel;
use crate::system::SystemStats;
use crate::terminal::keys::TerminalKeybindings;
use crate::terminal::resizable_data::ResizableData;
use crate::terminal::view::inline_banner::ByoLlmAuthBannerSessionState;
use crate::terminal::{AudibleBell, CustomSecretRegexUpdater, History};
use crate::undo_close::UndoCloseStack;
use crate::user_config::RiftConfig;
use crate::util::bindings::is_binding_cross_platform;
use crate::vim_registers::VimRegisters;
use crate::rift_managed_paths_watcher::{ensure_rift_watch_roots_exist, RiftManagedPathsWatcher};
use crate::workspace::{
    ActiveSession, PaneViewLocator, ToastStack, Workspace, WorkspaceAction,
};
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::update_manager::TeamUpdateManager;
use crate::workspaces::user_profiles::UserProfiles;
use crate::workspaces::user_workspaces::UserWorkspaces;

/// Our embedded application assets.
pub static ASSETS: rift_assets::Assets = rift_assets::Assets;

/// Launch mode for how to start up Rift.
#[allow(clippy::large_enum_variant)]
pub enum LaunchMode {
    /// Run the regular GUI application.
    App {
        args: rift_cli::AppArgs,
        /// API key for server authentication, if provided via `--api-key` or `RIFT_API_KEY`.
        /// Only used on dogfood channels.
        api_key: Option<String>,
    },

    /// Run the Rift command-line SDK.
    CommandLine {
        command: rift_cli::CliCommand,
        global_options: GlobalOptions,
        debug: bool,
        /// Whether this CLI invocation is running in a sandboxed environment.
        is_sandboxed: bool,
        /// Override for computer use permission from CLI flags. If None, uses default behavior.
        computer_use_override: Option<bool>,
    },
    /// Run a test - this may be an integration test or an eval.
    Test {
        driver: Box<Option<TestDriver>>,
        is_integration_test: bool,
    },

}

impl LaunchMode {
    fn args(&self) -> Cow<'_, rift_cli::AppArgs> {
        match self {
            LaunchMode::App { args, .. } => Cow::Borrowed(args),
            LaunchMode::CommandLine { .. } | LaunchMode::Test { .. } => {
                Cow::Owned(rift_cli::AppArgs::default())
            }
        }
    }

    /// Returns `true` if this process is running an integration test.
    fn is_integration_test(&self) -> bool {
        match self {
            LaunchMode::Test {
                is_integration_test,
                ..
            } => *is_integration_test,
            LaunchMode::App { .. } | LaunchMode::CommandLine { .. } => false,
        }
    }

    fn take_test_driver(&mut self) -> Option<TestDriver> {
        match self {
            LaunchMode::Test { driver, .. } => driver.take(),
            LaunchMode::App { .. } | LaunchMode::CommandLine { .. } => None,
        }
    }

    /// Add an URL to open. Only supported for [`LaunchMode::App`]
    #[allow(dead_code)]
    fn add_url(&mut self, url: Url) {
        if let LaunchMode::App { ref mut args, .. } = self {
            args.urls.push(url);
        }
    }

    fn execution_mode(&self) -> ExecutionMode {
        match self {
            LaunchMode::App { .. } => ExecutionMode::App,
            LaunchMode::CommandLine { .. } => ExecutionMode::Sdk,
            LaunchMode::Test { .. } => ExecutionMode::App,
        }
    }

    fn is_sandboxed(&self) -> bool {
        match self {
            LaunchMode::CommandLine { is_sandboxed, .. } => *is_sandboxed,
            LaunchMode::App { .. } | LaunchMode::Test { .. } => false,
        }
    }

    /// Returns `true` if Rift should run headlessly, without a visible UI.
    fn is_headless(&self) -> bool {
        match self {
            LaunchMode::CommandLine { command, .. } => match command {
                CliCommand::Agent(AgentCommand::Run(args)) => !args.gui,
                _ => true,
            },
            LaunchMode::App { .. } | LaunchMode::Test { .. } => false,
        }
    }


    /// Whether or not to start a crash recovery process (on platforms that support it).
    #[cfg(enable_crash_recovery)]
    pub(crate) fn crash_recovery_enabled(&self) -> bool {
        match self {
            LaunchMode::App { .. } => true,
            LaunchMode::CommandLine { .. } | LaunchMode::Test { .. } => false,
        }
    }

    /// Whether Sentry / crash reporting should be initialized.
    #[cfg_attr(not(feature = "crash_reporting"), allow(dead_code))]
    pub(crate) fn needs_crash_reporting(&self) -> bool {
        match self {
            LaunchMode::App { .. } | LaunchMode::CommandLine { .. } | LaunchMode::Test { .. } => {
                true
            }
        }
    }

    /// Whether profiling and tracing should be initialized.
    pub(crate) fn needs_profiling(&self) -> bool {
        match self {
            LaunchMode::App { .. } | LaunchMode::CommandLine { .. } | LaunchMode::Test { .. } => {
                true
            }
        }
    }

    /// Log destination for this mode.
    fn log_destination(&self) -> Option<LogDestination> {
        match self {
            LaunchMode::CommandLine { debug, .. } => {
                if *debug {
                    Some(LogDestination::Stderr)
                } else {
                    Some(LogDestination::File)
                }
            }
            LaunchMode::App { .. } | LaunchMode::Test { .. } => None,
        }
    }

}

/// If the given event is a key down event containing alt modifiers, and those
/// alt modifiers should be treated as meta keys, then remove the alts and
/// prefix the keys with an escape. See WAR-472.
fn apply_extra_meta_keys(event: &mut Event, extra_metas: ExtraMetaKeys) {
    if let Event::KeyDown {
        keystroke, details, ..
    } = event
    {
        let left_as_meta = extra_metas.left_alt && details.left_alt;
        let right_as_meta = extra_metas.right_alt && details.right_alt;
        if left_as_meta || right_as_meta {
            let side = match (left_as_meta, right_as_meta) {
                (true, true) => "left+right alt",
                (true, false) => "left alt",
                (false, true) => "right alt",
                (false, false) => unreachable!(),
            };
            log::info!("Treating {side} as meta");
            keystroke.alt = false;
            keystroke.meta = true;
        }
    }
}

fn apply_scroll_multiplier(event: &mut Event, app: &AppContext) {
    if let Event::ScrollWheel { delta, precise, .. } = event {
        if !*precise {
            let scroll_multiplier = *ScrollSettings::as_ref(app).mouse_scroll_multiplier.value();
            *delta *= scroll_multiplier;
        }
    }
}

/// Runs the app. If a subcommand was requested, it'll be run instead of the main application.
pub fn run() -> Result<()> {
    // Perform any necessary platform-specific initialization.
    platform::init();

    // Ensure feature flags are initialized before parsing command-line arguments.
    features::init_feature_flags();

    // Parse command-line arguments.
    let args = rift_cli::Args::from_env();

    if let Some(command) = args.command() {
        #[cfg(windows)]
        if command.prints_to_stdout() {
            // We attach a console to ensure that all standard output gets printed correctly.
            rift_util::windows::attach_to_parent_console();
        }
        match command {
            #[cfg(all(feature = "local_tty", unix))]
            rift_cli::Command::Worker(rift_cli::WorkerCommand::TerminalServer(args)) => {
                // If we were asked to run as a terminal server (as opposed to the main
                // GUI application), do so immediately.  Ideally, the terminal server would
                // be a separate binary, but it's much easier to distribute a single binary,
                // so starting the terminal server event loop immediately is the closest
                // approximation we can get to running a separate binary.
                crate::terminal::local_tty::server::run_terminal_server(args);
                return Ok(());
            }
            #[cfg(feature = "plugin_host")]
            rift_cli::Command::Worker(rift_cli::WorkerCommand::PluginHost { .. }) => {
                return crate::run_plugin_host();
            }
            #[cfg(feature = "local_tty")]
            rift_cli::Command::Worker(rift_cli::WorkerCommand::MinidumpServer { socket_name }) => {
                cfg_if::cfg_if! {
                    if #[cfg(all(linux_or_windows, feature = "crash_reporting"))] {
                        return crate::crash_reporting::run_minidump_server(socket_name);
                    } else {
                        let _ = socket_name;
                        panic!("The minidump server is not supported on this platform");
                    }
                }
            }
            #[cfg(not(target_family = "wasm"))]
            #[cfg(not(target_family = "wasm"))]
            rift_cli::Command::Worker(rift_cli::WorkerCommand::RipgrepSearch {
                parent,
                ignore_case,
                multiline,
                pattern,
                paths,
            }) => {
                rift_ripgrep::search::run_search_subprocess(
                    std::slice::from_ref(pattern),
                    paths.clone(),
                    *ignore_case,
                    *multiline,
                    parent.pid,
                )
                .map_err(|err| anyhow!(err.to_string()))?;
                return Ok(());
            }
            #[cfg(not(any(
                feature = "local_tty",
                feature = "plugin_host",
                not(target_family = "wasm")
            )))]
            rift_cli::Command::Worker(worker) => {
                // Need this case to handle platforms where there are no enum variants in
                // rift_cli::WorkerCommand, as we still need to check Command::Worker.

                // On wasm, specifically, we should fail spectacularly if we get here.
                #[cfg(target_family = "wasm")]
                panic!("Worker process not supported on WASM: {worker:?}")
            }
            rift_cli::Command::Completions { shell } => {
                return rift_cli::completions::generate_to_stdout(*shell);
            }
            rift_cli::Command::CommandLine(cmd) => {
                let (is_sandboxed, computer_use_override) = match cmd.as_ref() {
                    rift_cli::CliCommand::Agent(rift_cli::agent::AgentCommand::Run(run_args)) => (
                        run_args.sandboxed,
                        run_args.computer_use.computer_use_override(),
                    ),
                    _ => (false, None),
                };

                return run_internal(LaunchMode::CommandLine {
                    command: cmd.as_ref().clone(),
                    global_options: GlobalOptions {
                        output_format: args.output_format(),
                        api_key: args.api_key().cloned(),
                    },
                    debug: args.debug(),
                    is_sandboxed,
                    computer_use_override,
                });
            }
            rift_cli::Command::DumpDebugInfo => {
                return debug_dump::run();
            }
            #[cfg(not(target_family = "wasm"))]
            rift_cli::Command::PrintTelemetryEvents => {
                return TelemetryEvent::print_telemetry_events_json();
            }
        }
    }

    // If running as a standalone CLI binary or invoked as "oz", print help
    // instead of launching the GUI app.
    let is_cli_binary = cfg!(feature = "standalone")
        || rift_cli::binary_name().is_some_and(|name| name.starts_with("oz"))
        || std::env::var_os("RIFT_CLI_MODE").is_some();
    if is_cli_binary {
        rift_cli::Args::clap_command().print_help()?;
        return Ok(());
    }

    let api_key = args.api_key().cloned();
    run_internal(LaunchMode::App {
        args: args.into_app_args(),
        api_key,
    })
}

/// Runs an integration test using the provided test driver.
pub fn run_integration_test(driver: TestDriver) -> Result<()> {
    let is_integration_test = std::env::var("RIFT_INTEGRATION").is_ok();
    let launch = LaunchMode::Test {
        driver: Box::new(Some(driver)),
        is_integration_test,
    };
    run_internal(launch)
}

/// Runs the app (or CLI / daemon).
fn run_internal(mut launch_mode: LaunchMode) -> Result<()> {
    let mut timer = IntervalTimer::new();

    // ── Early initialization (pre-AppBuilder) ──────────────────────
    // These steps run before the platform event loop is started.
    // They must not depend on AppContext.

    #[cfg(windows)]
    dynamic_libraries::configure_library_loading();

    if launch_mode.needs_profiling() {
        profiling::init();
    }

    // The `run` function already initializes feature flags, but ensure they're initialized here
    // for other entrypoints.
    features::init_feature_flags();

    #[cfg(feature = "crash_reporting")]
    if launch_mode.needs_crash_reporting() {
        // Ensure that the main/root Sentry hub is initialized on the main
        // thread.  PtySpawner creates a background thread to receive logs from
        // the terminal server process, and we don't want it to be the host of
        // the primary sentry::Hub.
        sentry::Hub::main();
    }

    if launch_mode.needs_profiling() {
        tracing::init()?;
    }

    let log_destination = launch_mode.log_destination();
    let is_cli = log_destination.is_some();

    cfg_if::cfg_if! {
        if #[cfg(enable_crash_recovery)] {
            if crash_recovery::is_crash_recovery_process(launch_mode.args().as_ref()) {
                rift_logging::init_for_crash_recovery_process()?;
            } else {
                rift_logging::init(rift_logging::LogConfig {
                    is_cli,
                    log_destination,
                    ..Default::default()
                })?;
            }
        } else {
            rift_logging::init(rift_logging::LogConfig {
                is_cli,
                log_destination,
                ..Default::default()
            })?;
        }
    }

    timer.mark_interval_end("LOG_FILE_SETUP_COMPLETE");

    #[cfg(windows)]
    platform::windows::check_redirection_guard();

    // Adjust resource limits early, before doing other work, to ensure that
    // any children we spawn (like the terminal server) inherit our adjusted
    // rlimits.
    resource_limits::adjust_resource_limits();

    // Configure rustls to use its default crypto provider.  This MUST be called
    // before making any network requests that use TLS, otherwise rustls will
    // panic.
    #[cfg(not(target_family = "wasm"))]
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("must be able to initialize crypto provider for TLS support");

    // Collect errors that occur in run_internal() before the Sentry client is initialized,
    // so they can be replayed to Sentry once it's ready.
    #[cfg_attr(
        not(all(
            feature = "release_bundle",
            any(windows, any(target_os = "linux", target_os = "freebsd"))
        )),
        expect(unused_mut)
    )]
    let mut pre_sentry_errors: Vec<anyhow::Error> = Vec::new();

    #[cfg(all(
        feature = "release_bundle",
        any(target_os = "linux", target_os = "freebsd")
    ))]
    if let LaunchMode::App { .. } = launch_mode {
        match app_services::linux::pass_startup_args_to_existing_instance(
            launch_mode.args().as_ref(),
        ) {
            // If we were able to contact an existing application instance, quit -
            // we only want to run a single instance of Rift at a time.
            Ok(_) => std::process::exit(0),
            // If Rift isn't already running, we're good to go.
            Err(app_services::linux::StartupArgsForwardingError::NoExistingInstance) => {}
            // If we just finished an auto-update, we should continue running.
            Err(app_services::linux::StartupArgsForwardingError::IgnoredAfterAutoUpdate) => {}
            // If we were unable to perform the forwarding for an unknown reason,
            // it's better to run a second instance than potentially end up in a
            // state where Rift refuses to run even a first instance.
            Err(err) => {
                let err = anyhow::Error::from(err).context("Failed to forward startup args");
                log::error!("{err:#}");
                pre_sentry_errors.push(err);
            }
        }
    }

    #[cfg(all(feature = "release_bundle", windows))]
    if let LaunchMode::App { .. } = launch_mode {
        match app_services::windows::pass_startup_args_to_existing_instance(
            launch_mode.args().as_ref(),
        ) {
            // If we were able to contact an existing application instance, quit -
            // we only want to run a single instance of Rift at a time.
            Ok(_) => std::process::exit(0),
            // If Rift isn't already running, we're good to go.
            Err(app_services::windows::StartupArgsForwardingError::NoExistingInstance) => {}
            // If we just finished an auto-update, we should continue running.
            Err(app_services::windows::StartupArgsForwardingError::IgnoredAfterAutoUpdate) => {}
            // If we were unable to perform the forwarding for an unknown reason,
            // it's better to run a second instance than potentially end up in a
            // state where Rift refuses to run even a first instance.
            Err(err) => {
                let err = anyhow::Error::from(err).context("Failed to forward startup args");
                log::error!("{err:#}");
                pre_sentry_errors.push(err);
            }
        }
    }

    // Sets up a Job Object that we associate with the Rift process to handle
    // shared fate with its child processes. This should be called before we
    // start spawning any child processes.
    #[cfg(windows)]
    command::windows::init();

    let private_preferences = settings::init_private_user_preferences();
    let (public_preferences, startup_toml_parse_error) = settings::init_public_user_preferences();

    // When the SettingsFile feature flag is enabled, public settings live in
    // the TOML-backed store. When disabled, they live in the platform-native
    // store (same backend as private). Use the correct one for pre-app reads.
    #[cfg_attr(
        not(any(enable_crash_recovery, any(target_os = "linux", target_os = "freebsd"))),
        expect(unused)
    )]
    let prefs_for_public_settings: &dyn riftui_extras::user_preferences::UserPreferences =
        if FeatureFlag::SettingsFile.is_enabled() {
            public_preferences.as_ref()
        } else {
            private_preferences.deref()
        };

    #[cfg(enable_crash_recovery)]
    let crash_recovery =
        crash_recovery::CrashRecovery::new(&launch_mode, prefs_for_public_settings);

    // Set up the pty spawner before doing any meaningful work. We want to
    // ensure that the process is in the cleanest possible state (minimal opened
    // files, modified signal handlers, etc.) to avoid unexpected effects on
    // spawned ptys.
    #[cfg(feature = "local_tty")]
    let pty_spawner =
        terminal::local_tty::spawner::PtySpawner::new().context("Failed to create pty spawner")?;

    let mut app_builder = if launch_mode.is_headless() {
        riftui::platform::AppBuilder::new_headless(
            app_callbacks(launch_mode.is_integration_test()),
            Box::new(ASSETS),
            launch_mode.take_test_driver(),
        )
    } else {
        riftui::platform::AppBuilder::new(
            app_callbacks(launch_mode.is_integration_test()),
            Box::new(ASSETS),
            launch_mode.take_test_driver(),
        )
    };

    #[cfg(target_os = "macos")]
    {
        use riftui::platform::mac::AppExt;
        use riftui::AssetProvider as _;

        let activate_on_launch = !launch_mode.is_integration_test()
            || std::env::var("RIFTUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS").is_ok();
        app_builder.set_activate_on_launch(activate_on_launch);

        let dev_icon = ASSETS.get("bundled/png/local.png")?;
        app_builder.set_dev_icon(dev_icon);

        app_builder.set_menu_bar_builder(app_menus::menu_bar);
        app_builder.set_dock_menu_builder(|_| app_menus::dock_menu());
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        use riftui::platform::linux::{self, AppBuilderExt};

        use crate::settings::ForceX11;

        app_builder.set_window_class(ChannelState::app_id().to_string());

        let force_x11 = ForceX11::read_from_preferences(prefs_for_public_settings)
            .unwrap_or(ForceX11::default_value());
        // Force use of wayland if the user has passed the `RIFT_ENABLE_WAYLAND` env var.
        let allow_wayland = linux::is_wayland_env_var_set() || !force_x11;
        app_builder.force_x11(!allow_wayland);
    }

    #[cfg(target_os = "windows")]
    {
        use riftui::platform::windows::AppBuilderExt;
        app_builder.set_app_user_model_id(ChannelState::app_id().to_string());

        // Only use DXC for DirectX shader compilation if we're not running in a Parallels VM
        // Parallels VMs can have issues with DXC shader compilation
        let is_parallels_vm = crate::util::vm_detection::is_running_in_windows_parallels_vm();
        if !is_parallels_vm {
            log::info!("Using DXC for DirectX shader compilation");
            use riftui::platform::windows::DXCPath;

            app_builder.use_dxc_for_directx_shader_compilation(DXCPath {
                dxc_path: "dxcompiler.dll".to_string(),
                dxil_path: "dxil.dll".to_string(),
            });
        } else {
            log::info!("Skipping DXC for DirectX shader compilation; running in a Parallels VM");
        }
    }

    // Override any bindings that have a `Custom` trigger to a `Keystroke`-based trigger. In theory,
    // this should be a noop on Mac (since the keystrokes registered via the  Mac menus first
    // intercept the binding), but just to be safe we only enable this in cases where we don't
    // include mac menus.
    #[cfg(not(target_os = "macos"))]
    app_builder.convert_custom_triggers_to_keystroke_triggers(
        crate::util::bindings::custom_tag_to_keystroke,
    );

    #[cfg(target_os = "macos")]
    app_builder.register_default_keystroke_triggers_for_custom_actions(
        crate::util::bindings::custom_tag_to_keystroke,
    );

    app_builder.run(move |ctx| {
        #[cfg(not(target_family = "wasm"))]
        // Rotate the log files in the background.
        ctx.background_executor()
            .spawn(rift_logging::rotate_log_files())
            .detach();

        ctx.add_singleton_model(|ctx| {
            AppExecutionMode::new(
                launch_mode.execution_mode(),
                launch_mode.is_sandboxed(),
                ctx,
            )
        });
        #[cfg(feature = "crash_reporting")]
        crate::crash_reporting::set_client_type_tag(launch_mode.execution_mode().client_id());

        // Add the terminal server singleton to the application.
        #[cfg(feature = "local_tty")]
        ctx.add_singleton_model(move |_ctx| pty_spawner);

        // Register user preferences.  This must be done before initializing
        // feature flags or experiments, both of which check user preferences for
        // overrides.
        ctx.add_singleton_model(move |_ctx| ::settings::PublicPreferences::new(public_preferences));
        ctx.add_singleton_model(move |_ctx| private_preferences);
        let startup_toml_parse_error = startup_toml_parse_error;

        #[cfg(enable_crash_recovery)]
        ctx.add_singleton_model(move |_ctx| crash_recovery);

        #[cfg(feature = "plugin_host")]
        ctx.add_singleton_model(move |ctx| {
            plugin::PluginHost::new(ctx).expect("Could not instantiate PluginHost")
        });
        let app_state = initialize_app(
            &launch_mode,
            timer,
            startup_toml_parse_error,
            ctx,
            pre_sentry_errors,
        );

        launch(ctx, app_state, launch_mode);
    })
}

pub struct UpdateQuakeModeEventArg {
    active_window_id: Option<WindowId>,
}

pub(crate) fn initialize_app(
    launch_mode: &LaunchMode,
    mut timer: IntervalTimer,
    startup_toml_parse_error: Option<riftui_extras::user_preferences::Error>,
    ctx: &mut riftui::AppContext,
    _pre_sentry_errors: impl IntoIterator<Item = anyhow::Error>,
) -> Option<AppState> {
    // WARNING: Errors that happen here before crash_reporting::init will not be collected in
    // Sentry. Only the dependencies of crash_reporting should be initialized here. Avoid adding
    // any other stuff here, as failures will be silent. Push them to pre_sentry_errors instead.
    let data_domain = ChannelState::data_domain();

    // Register an implementation of the secure storage service.
    cfg_if::cfg_if! {
        if #[cfg(feature = "integration_tests")] {
            riftui_extras::secure_storage::register_noop(&data_domain, ctx);
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            riftui_extras::secure_storage::register_with_fallback(&data_domain, rift_core::paths::state_dir(), ctx)
        } else if #[cfg(target_os = "windows")] {
            riftui_extras::secure_storage::register_with_dir(&data_domain, rift_core::paths::state_dir(), ctx)
        } else {
            riftui_extras::secure_storage::register(&data_domain, ctx);
        }
    }

    // One-time migration: give Preview its own config directory by
    // symlinking contents from the shared ~/.rift location. Must run
    // before ensure_rift_watch_roots_exist() creates the new directory.
    #[cfg(target_os = "macos")]

    ensure_rift_watch_roots_exist();
    ctx.add_singleton_model(RiftManagedPathsWatcher::new);

    ctx.add_singleton_model(RiftConfig::new);
    ctx.add_singleton_model(|_ctx| SettingsManager::default());

    let user_defaults_on_startup = settings::init(startup_toml_parse_error, ctx);
    timer.mark_interval_end("READ_USER_DEFAULTS_AND_INITIALIZE_SETTINGS");

    if FeatureFlag::UIZoom.is_enabled() {
        ctx.set_zoom_factor(WindowSettings::as_ref(ctx).zoom_level.as_zoom_factor());
    }

    // Extract API key from command line options, if applicable.
    let api_key = match launch_mode {
        LaunchMode::CommandLine { global_options, .. } => global_options.api_key.clone(),
        LaunchMode::App { api_key, .. } if ChannelState::channel().is_dogfood() => api_key.clone(),
        _ => None,
    };
    let api_key = if FeatureFlag::APIKeyAuthentication.is_enabled() {
        api_key
    } else {
        None
    };

    let auth_state = Arc::new(AuthState::initialize(ctx, api_key));
    timer.mark_interval_end("AUTH_MANAGER_SET_USER");

    ctx.add_singleton_model(|_ctx| AuthStateProvider::new(auth_state.clone()));

    ctx.add_singleton_model(AppTelemetryContextProvider::new_context_provider);

    ctx.add_singleton_model(AuthManager::new);

    ctx.add_singleton_model(|_ctx| GPUState::new());

    PrivacySettings::register_singleton(ctx);

    // If any part of sqlite initialization fails, we just don't do session restoration (i.e.
    // feature degradation).
    let (sqlite_data, writer_handles) =
        persistence::initialize(ctx, persistence::PersistenceScope::App);
    timer.mark_interval_end("SQLITE_INITIALIZED");

    let persistence_writer = PersistenceWriter::new(writer_handles);

    let model_event_sender = persistence_writer.sender();

    let tips_handle = ctx.add_model(|_| user_defaults_on_startup.tips_data);
    let user_default_shell_unsupported_banner_model_handle =
        ctx.add_model(|_| user_defaults_on_startup.user_default_shell_unsupported_banner_state);
    // Extract the full-file parse error (if any) before the settings_file_error
    // value is moved below. Only FileParseFailed gates the broken-file guard
    // in `initialize_cloud_preferences_syncer`; InvalidSettings means TOML
    // parsed but individual values were wrong, which doesn't mean local
    // state is unusable.
    let _startup_toml_parse_error_for_syncer = user_defaults_on_startup
        .settings_file_error
        .as_ref()
        .and_then(|err| match err {
            settings::SettingsFileError::FileParseFailed(msg) => Some(msg.clone()),
            settings::SettingsFileError::InvalidSettings(_) => None,
        });
    let settings_file_error = user_defaults_on_startup.settings_file_error;
    ctx.add_singleton_model(move |_ctx| {
        GlobalResourceHandlesProvider::new(GlobalResourceHandles {
            model_event_sender,
            tips_completed: tips_handle,
            user_default_shell_unsupported_banner_model_handle,
            settings_file_error,
        })
    });

    let (
        cached_workspaces,
        current_workspace_uid,
        app_state,
        command_history,
        restored_user_profiles,
        experiments,
        workspace_language_servers,
        persisted_projects,
        persisted_ignored_suggestions,
    ) = sqlite_data
        .map(|sqlite_data| {
            (
                sqlite_data.workspaces,
                sqlite_data.current_workspace_uid,
                Some(sqlite_data.app_state),
                sqlite_data.command_history,
                sqlite_data.user_profiles,
                sqlite_data.experiments,
                sqlite_data.workspace_language_servers,
                sqlite_data.projects,
                sqlite_data.ignored_suggestions,
            )
        })
        .unwrap_or_else(|| {
            (
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            )
        });

    let _ = experiments;

    ctx.add_singleton_model(|ctx| {
        UserWorkspaces::new(cached_workspaces, current_workspace_uid, ctx)
    });

    ctx.add_singleton_model(AntivirusInfo::new);

    cfg_if::cfg_if! {
        if #[cfg(feature = "crash_reporting")] {
            let is_crash_reporting_enabled = crash_reporting::init(ctx);
        } else {
            let is_crash_reporting_enabled = false;
        }
    }
    // Send buffered pre-init errors to Sentry now that the client is ready.
    #[cfg(feature = "crash_reporting")]
    for err in _pre_sentry_errors {
        sentry::integrations::anyhow::capture_anyhow(&err);
    }
    timer.mark_interval_end("INIT_CRASH_REPORTING");

    ctx.set_fallback_font_source_provider(|url| ::asset_cache::url_source(url));

    ctx.set_default_binding_validator(is_binding_cross_platform);


    // Initialize timestamp for session id and last active event
    App::record_last_active_timestamp();

    ctx.add_singleton_model(|_| SettingsPaneManager::new());
    ctx.add_singleton_model(|_| pricing::PricingInfoModel::new());

    #[cfg(target_os = "macos")]
    if !launch_mode.is_headless() {
        AppearanceManager::as_ref(ctx).set_app_icon(ctx);
    }

    #[cfg(feature = "local_tty")]
    terminal::available_shells::register(ctx);

    // Add truly global actions that don't depend on the existence of any view here
    ctx.add_global_action("app:toggle_user_ps1", move |_args: &(), ctx| {
        SessionSettings::handle(ctx).update(ctx, |session_settings, ctx| {
            report_if_error!(session_settings.honor_ps1.toggle_and_save_value(ctx));
        });
    });
    ctx.add_global_action("app:toggle_copy_on_select", move |_args: &(), ctx| {
        SelectionSettings::handle(ctx).update(ctx, |selection_settings, ctx| {
            report_if_error!(selection_settings.copy_on_select.toggle_and_save_value(ctx));
        });
    });

    ctx.add_singleton_model(|_ctx| SyncedInputState::new());

    #[cfg(not(target_family = "wasm"))]

    log::info!(
        "Starting rift with channel state {} and version {:?}",
        ChannelState::debug_str(),
        ChannelState::app_version()
    );

    // Teach our app that sometimes option means meta.
    ctx.set_event_munger(move |event, ctx| {
        let extra_meta_keys = *KeysSettings::as_ref(ctx).extra_meta_keys;
        apply_extra_meta_keys(event, extra_meta_keys);
        apply_scroll_multiplier(event, ctx);
    });

    ctx.set_a11y_verbosity(*AccessibilitySettings::as_ref(ctx).a11y_verbosity);

    #[cfg(enable_crash_recovery)]
    ctx.on_draw_frame_error(|ctx, window_id| {
        crash_recovery::CrashRecovery::handle(ctx).update(ctx, |crash_recovery, _ctx| {
            crash_recovery.on_draw_frame_error(window_id);
        });
    });

    let user_is_logged_in = auth_state.is_logged_in();

    if user_is_logged_in {
        // Skip refresh_user for CLI mode — the CLI handles auth refresh in
        // ensure_auth_state so it can detect invalid credentials before running
        // a command.
        if !matches!(launch_mode, LaunchMode::CommandLine { .. }) {
            AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                auth_manager.refresh_user(ctx);
            });
        }

        // Set the first frame callback to record the app's startup time.
        // This is only sent for logged-in users so that new users don't skew performance metrics.
        let is_screen_reader_enabled = ctx.is_screen_reader_enabled();
        let from_relaunch = launch_mode.args().finish_update;
        ctx.on_first_frame_drawn(move |ctx| {
            let timing_data = IntervalTimer::handle(ctx).update(ctx, |timer, _| {
                timer.mark_interval_end("FIRST_FRAME_DRAWN");
                timer.compute_stats()
            });
            let _event = TelemetryEvent::AppStartup(AppStartupInfo {
                is_session_restoration_on: user_defaults_on_startup.should_restore_session,
                is_screen_reader_enabled,
                from_relaunch,
                is_crash_reporting_enabled,
                timing_data,
            });

            GPUState::handle(ctx).update(ctx, |gpu_state, ctx| {
                gpu_state
                    .set_has_lower_power_gpu(riftui::rendering::is_low_power_gpu_available(), ctx);
            });

            for window_id in ctx.window_ids().collect_vec() {
                SettingsPaneManager::handle(ctx)
                    .read(ctx, |model, _| model.settings_view(window_id))
                    .update(ctx, |settings, ctx| {
                        settings.refresh_preferred_graphics_backend_dropdown(ctx);
                    })
            }

            send_telemetry_from_app_ctx!(event, ctx);
        });

        #[cfg(enable_crash_recovery)]
        ctx.on_frame_drawn(|ctx, window_id| {
            crash_recovery::CrashRecovery::handle(ctx).update(ctx, |crash_recovery, ctx| {
                crash_recovery.on_frame_drawn(window_id, ctx);
            });
        })
    } else {
        // If the app was opened while logged out, record an event for measuring new users.
        // This is sent immediately in case they quit the app on the signup screen.
        send_telemetry_sync_from_app_ctx!(TelemetryEvent::LoggedOutStartup, ctx);
        download_method::determine_and_report(
            auth_state.clone(),
            ctx.background_executor().clone(),
        );
    }

    #[cfg(not(target_family = "wasm"))]
    {
        ctx.add_singleton_model(DirectoryWatcher::new);
        ctx.add_singleton_model(|_| DetectedRepositories::default());
        if let Some(home_dir) = dirs::home_dir() {
            ctx.add_singleton_model(|ctx| HomeDirectoryWatcher::new(home_dir, ctx));
        } else {
            log::info!("Home directory not found; skipping HomeDirectoryWatcher registration");
        }
    }

    #[cfg(feature = "local_fs")]
    {
        let imported_config_model = ctx.add_singleton_model(ImportedConfigModel::new);

        if ChannelState::channel() != rift_core::channel::Channel::Integration {
            imported_config_model.update(ctx, |model, ctx| {
                model.search_for_settings_to_import(ctx);
            });
        }

        ctx.add_singleton_model(|ctx| {
            let model = RepoMetadataModel::new(ctx);

            model
        });
    }

    ctx.add_singleton_model(|ctx| {
        ProjectManagementModel::new(persisted_projects, persistence_writer.sender(), ctx)
    });

    ctx.add_singleton_model(move |_| History::new(command_history));

    ctx.add_singleton_model(CustomSecretRegexUpdater::new);

    // Register the `TelemetryCollection` singleton model.
    ctx.add_singleton_model(|ctx| {
        let telemetry_collector = TelemetryCollector::new();
        telemetry_collector.initialize_telemetry_collection(ctx);
        telemetry_collector
    });
    timer.mark_interval_end("INITIALIZE_TELEMETRY_COLLECTION");

    // Register initial keybindings prior to creating menus
    app_services::init(ctx);
    // // TODO: Temporarily disabling keybindings for WASM builds. Will be implemented in future WASM support.
    #[cfg(not(target_family = "wasm"))]
    workspace::init(ctx);
    pane_group::init(ctx);
    terminal::init(ctx);
    input::init(ctx);
    editor::init(ctx);
    menu::init(ctx);
    tips::tip_view::init(ctx);
    launch_configs::init(ctx);
    themes::theme_chooser::init(ctx);
    themes::theme_creator_modal::init(ctx);
    themes::theme_deletion_modal::init(ctx);
    root_view::init(ctx);
    auth::init(ctx);
    crate::view_components::find::init(ctx);
    prompt::editor_modal::init(ctx);
    undo_close::init(ctx);
    tab_configs::new_worktree_modal::init(ctx);
    tab_configs::params_modal::init(ctx);
    terminal::ssh::error::init(ctx);
    context_chips::display_menu::init(ctx);
    context_chips::node_version_popup::init(ctx);

    let display_count = ctx.windows().display_count();
    ctx.add_singleton_model(|_| DisplayCount(display_count));

    ctx.add_singleton_model(|_| NetworkStatus::new());
    ctx.add_singleton_model(|_| SystemStats::new());
    ctx.add_singleton_model(|_| KeybindingChangedNotifier::new());
    ctx.add_singleton_model(|_| search::command_palette::SelectedItems::new());
    ctx.add_singleton_model(search::files::model::FileSearchModel::new);
    ctx.add_singleton_model(|_| VimRegisters::new());
    ctx.add_singleton_model(UndoCloseStack::new);
    ctx.add_singleton_model(|_| ToastStack);
    #[cfg(feature = "local_fs")]
    ctx.add_singleton_model(FileModel::new);
    #[cfg(windows)]
    ctx.add_singleton_model(util::traffic_lights::windows::RendererState::new);

    ctx.add_singleton_model(|_| UserProfiles::new(restored_user_profiles));


    ctx.add_singleton_model(|_| AudibleBell::new());

    // This model has to be registered after the user workspaces model because it relies on it,
    // and before the UpdateManager models because they rely on the TeamTester model.
    ctx.add_singleton_model(TeamTesterStatus::new);

    ctx.add_singleton_model(TeamUpdateManager::new);



    // LogManager must be registered before any subsystem (e.g. MCP, LSP) that creates file-based loggers.
    ctx.add_singleton_model(|_| simple_logger::manager::LogManager::new());









    // ByoLlmAuthBannerSessionState tracks dismissal of the BYO LLM auth banner (e.g., AWS Bedrock login).
    ctx.add_singleton_model(ByoLlmAuthBannerSessionState::new);

    ctx.add_singleton_model(TerminalKeybindings::new);
    ctx.add_singleton_model(|_| ActiveSession::default());

    #[cfg(all(not(target_family = "wasm"), feature = "local_tty"))]
    {
        ctx.add_singleton_model(LocalShellState::new);
        ctx.add_singleton_model(system::SystemInfo::new);
    }

    // Add a singleton model that holds the current prompt configuration.
    ctx.add_singleton_model(Prompt::new);

    // Add a singleton model for resizable modals whose size should be persisted through restarts.
    ctx.add_singleton_model(|_| ResizableData::default());

    timer.mark_interval_end("SINGLETON_MODELS_REGISTERED");

    ctx.add_singleton_model(move |_| timer);


    ctx.add_singleton_model(DefaultTerminal::new);

    let _ = workspace_language_servers;
    ctx.add_singleton_model(move |_| persistence_writer);

    ctx.add_singleton_model(input_classifier::InputClassifierModel::new);

    ctx.add_singleton_model(move |_| IgnoredSuggestionsModel::new(persisted_ignored_suggestions));

    // Subscribe WorkflowAliases to the UpdateManager so that it can be notified when objects are
    // trashed.

    // When running natively, add the http server singleton to the application.
    #[cfg(not(target_family = "wasm"))]
    ctx.add_singleton_model(move |ctx| {
        let routers = vec![profiling::make_router()];
        http_server::HttpServer::new(routers, ctx)
    });

    app_state
}

pub(crate) fn app_callbacks(is_integration_test: bool) -> riftui::platform::AppCallbacks {
    riftui::platform::AppCallbacks {
        on_internet_reachability_changed: Some(Box::new(move |reachable, ctx| {
            NetworkStatus::handle(ctx)
                .update(ctx, move |me, ctx| me.reachability_changed(reachable, ctx));
        })),
        on_become_active: Some(Box::new(move |ctx| {
            let auth_state = AuthStateProvider::as_ref(ctx).get();
            ctx.record_app_focus(
                auth_state.user_id().map(|uid| uid.as_string()),
                auth_state.anonymous_id(),
            );
        })),
        on_screen_changed: Some(Box::new(move |ctx| {
            ctx.dispatch_global_action(
                "root_view:move_quake_mode_window_from_screen_change",
                &KeysSettings::as_ref(ctx)
                    .quake_mode_settings
                    .value()
                    .clone(),
            );

            let new_display_count = ctx.windows().display_count();
            DisplayCount::handle(ctx).update(ctx, |display_count, ctx| {
                display_count.0 = new_display_count;
                ctx.notify();
            });
        })),
        on_cpu_awakened: Some(Box::new(move |ctx| {
            SystemStats::handle(ctx).update(ctx, move |system, ctx| {
                log::info!("System has returned from sleep");
                system.dispatch_cpu_was_awakened(ctx);
            });
        })),
        on_cpu_will_sleep: Some(Box::new(move |ctx| {
            SystemStats::handle(ctx).update(ctx, move |system, ctx| {
                log::info!("System is going to sleep...");
                system.dispatch_cpu_will_sleep(ctx);
            });
        })),
        on_resigned_active: Some(Box::new(move |ctx| {
            let active_window_id = ctx.windows().active_window();
            let update_quake_mode_arg = UpdateQuakeModeEventArg { active_window_id };

            ctx.dispatch_global_action("root_view:update_quake_mode_state", &update_quake_mode_arg);

            let auth_state = AuthStateProvider::as_ref(ctx).get();
            ctx.record_app_blur(
                auth_state.user_id().map(|uid| uid.as_string()),
                auth_state.anonymous_id(),
            );
        })),
        on_will_terminate: Some(Box::new(move |ctx| {
            PersistenceWriter::handle(ctx).update(ctx, |writer, _ctx| {
                writer.terminate();
            });

            let auth_state = AuthStateProvider::as_ref(ctx).get();
            ctx.try_record_daily_app_focus_duration(
                auth_state.user_id().map(|uid| uid.as_string()),
                auth_state.anonymous_id(),
            );
            TelemetryCollector::handle(ctx).update(ctx, |telemetry_collector, ctx| {
                telemetry_collector.flush_telemetry_events_for_shutdown(ctx);
            });

            // Shutdown all LSP servers gracefully before app termination
            lsp::LspManagerModel::handle(ctx).update(ctx, |manager, ctx| {
                manager.terminate(ctx);
            });

            // We want to tear down the terminal server before relaunching for
            // autoupdate, to ensure we're not running any extra Rift processes
            // when we bring up the new process.  Additionally, this must occur
            // after terminating the persistence writer, so we don't keep track
            // of the fact that the shell sessions terminated.
            #[cfg(feature = "local_tty")]
            terminal::local_tty::spawner::PtySpawner::handle(ctx).update(ctx, |pty_spawner, _| {
                pty_spawner.prepare_for_app_termination();
            });

            #[cfg(all(feature = "local_tty", windows))]
            terminal::local_tty::shutdown_all_pty_event_loops(ctx);

            // Tear down app services before spawning the new process, to
            // ensure that the new process doesn't find the old process while
            // attempting to enforce our single-instance policy on Linux.
            app_services::teardown(ctx);

            // Tear down any application profilers that are running, writing
            // results to disk.
            profiling::teardown();

            #[cfg(enable_crash_recovery)]
            crash_recovery::CrashRecovery::handle(ctx).update(ctx, |crash_recovery, _ctx| {
                crash_recovery.teardown();
            });

            // Tear down crash reporting as the last thing we do before the application
            // terminates.
            #[cfg(feature = "crash_reporting")]
            crash_reporting::uninit_sentry();
        })),
        on_should_close_window: Some(Box::new(move |window_id, ctx| {
            let general_settings = GeneralSettings::as_ref(ctx);
            // On Linux or Windows, if we're about to close the final window, we should quit the app instead.
            // On Mac, we do this conditionally based on a user setting.
            let quit_on_last_window_closed =
                cfg!(any(target_os = "linux", target_os = "freebsd", windows))
                    || *general_settings.quit_on_last_window_closed;
            if ctx.window_ids().count() == 1 && quit_on_last_window_closed {
                log::info!("No windows left, terminating app");
                ctx.terminate_app(TerminationMode::Cancellable, None);
                return ApproveTerminateResult::Cancel;
            }

            let summary = UnsavedStateSummary::for_window(window_id, ctx);

            send_telemetry_from_app_ctx!(
                TelemetryEvent::UserInitiatedClose {
                    initiated_on: CloseTarget::Window,
                },
                ctx
            );

            // Don't show dialog on integration test. Machine can't press buttons.
            if !is_integration_test && summary.should_display_warning(ctx) {
                let shown = summary
                    .dialog()
                    .on_confirm(move |ctx| {
                        ctx.windows()
                            .close_window(window_id, TerminationMode::ForceTerminate);
                    })
                    .on_cancel(move |ctx| {
                        on_close_window_cancelled(window_id, false, ctx);
                    })
                    .on_show_processes(move |ctx| {
                        on_close_window_cancelled(window_id, true, ctx);
                    })
                    .show(ctx);
                if shown {
                    ApproveTerminateResult::Cancel
                } else {
                    ApproveTerminateResult::Terminate
                }
            } else {
                ApproveTerminateResult::Terminate
            }
        })),
        on_should_terminate_app: Some(Box::new(move |ctx| {
            send_telemetry_from_app_ctx!(
                TelemetryEvent::UserInitiatedClose {
                    initiated_on: CloseTarget::App,
                },
                ctx
            );

            let summary = UnsavedStateSummary::for_app(ctx);
            // Don't show dialog on integration test. Machine can't press buttons.
            if !is_integration_test && summary.should_display_warning(ctx) {
                let shown = summary
                    .dialog()
                    .on_confirm(|ctx| ctx.terminate_app(TerminationMode::ForceTerminate, None))
                    .on_show_processes(|ctx| on_close_app_cancelled(true, ctx))
                    .on_cancel(|ctx| on_close_app_cancelled(false, ctx))
                    .show(ctx);
                if shown {
                    return ApproveTerminateResult::Cancel;
                }
            }

            ApproveTerminateResult::Terminate
        })),
        on_disable_warning_modal: Some(Box::new(move |ctx| {
            GeneralSettings::handle(ctx).update(ctx, |general_settings, ctx| {
                report_if_error!(general_settings
                    .show_warning_before_quitting
                    .toggle_and_save_value(ctx));
            });
            send_telemetry_from_app_ctx!(TelemetryEvent::QuitModalDisabled, ctx);
        })),
        on_notification_clicked: Some(Box::new(move |notification_response, ctx| {
            if let Some(notification_data) = notification_response.data() {
                let context: serde_json::Result<NotificationContext> =
                    serde_json::from_str(notification_data);
                if let Ok(NotificationContext::BlockOrigin {
                    window_id,
                    pane_group_id,
                    pane_id,
                }) = context
                {
                    // Ensure the window ID exists, if so dispatch an action to focus
                    // the correct pane.
                    if ctx.window_ids().contains(&window_id) {
                        if let Some(root_view_id) = ctx.root_view_id(window_id) {
                            ctx.dispatch_action(
                                window_id,
                                &[root_view_id],
                                "root_view:handle_notification_click",
                                &PaneViewLocator {
                                    pane_group_id,
                                    pane_id,
                                },
                                log::Level::Info,
                            );
                        }
                    }
                }
            }
        })),
        on_new_window_requested: Some(Box::new(move |ctx| {
            // This one is called when the app is requested to open a new window,
            // e.g. clicking on the Dock icon. It is NOT called from the New Window
            // menu item.
            App::record_last_active_timestamp();
            ctx.dispatch_global_action("root_view:open_new", &());
            ctx.dispatch_global_action("workspace:save_app", &());
        })),
        on_open_urls: Some(Box::new(move |urls, ctx| {
            for url in &urls {
                let parsed_url = Url::parse(url);
                match parsed_url {
                    Ok(url) => uri::handle_incoming_uri(&url, ctx),
                    Err(e) => log::warn!("Unable to parse received url: {e}"),
                }
            }
        })),
        on_os_appearance_changed: Some(Box::new(move |ctx| {
            AppearanceManager::handle(ctx).update(ctx, |appearance_manager, ctx| {
                appearance_manager.refresh_theme_state(ctx);
            });
        })),
        on_active_window_changed: Some(Box::new(move |ctx| {
            let windowing_model = ctx.windows();
            let active_window_id = windowing_model.active_window();
            let key_window_is_modal_panel = windowing_model.key_window_is_modal_panel();

            if !key_window_is_modal_panel {
                let update_quake_mode_arg = UpdateQuakeModeEventArg { active_window_id };
                ctx.dispatch_global_action(
                    "root_view:update_quake_mode_state",
                    &update_quake_mode_arg,
                );
            }

            ctx.dispatch_global_action("workspace:save_app", &());
        })),
        on_window_will_close: Some(Box::new(move |closed_window_data, ctx| {
            if ctx.windows().stage() == ApplicationStage::Terminating {
                return;
            }

            if let Some(window_data) = closed_window_data {
                UndoCloseStack::handle(ctx).update(ctx, |stack, ctx| {
                    stack.handle_window_closed(window_data, ctx);
                });
            }
            ctx.dispatch_global_action("workspace:save_app", &());
        })),
        on_window_moved: Some(Box::new(move |ctx| {
            ctx.dispatch_global_action("workspace:save_app", &());
        })),
        on_window_resized: Some(Box::new(move |ctx| {
            ctx.dispatch_global_action("workspace:save_app", &());
        })),
        ..Default::default()
    }
}

fn on_close_app_cancelled(open_navigation_palette: bool, ctx: &mut AppContext) {
    send_telemetry_from_app_ctx!(
        TelemetryEvent::QuitModalCancel {
            nav_palette: open_navigation_palette,
            modal_for: CloseTarget::App,
        },
        ctx
    );

    let sessions = SessionNavigationData::all_sessions(ctx).collect_vec();
    let sessions_summary = RunningSessionSummary::new(&sessions);

    // If open_navigation_palette is false, return early. Otherwise, we honor the open_navigation_palette
    // param which is true if the user clicked the modal button for that. However, if the running
    // processes in this window have finished since the modal popped, there is nothing to do now and we
    // can return early
    if !open_navigation_palette || sessions_summary.long_running_cmds.is_empty() {
        return;
    }

    let windowing_model = ctx.windows();
    let active_window_id = windowing_model.active_window();
    // show the nav palette in the active window. if there is no active window,
    // arbitrarily pick one of the windows having a running process
    let window_id_to_focus = active_window_id.unwrap_or_else(|| {
        *sessions_summary
            .windows_running()
            .iter()
            .next()
            .expect("already checked len > 0")
    });

    windowing_model.show_window_and_focus_app(window_id_to_focus);

    // open the nav palette in the selected window
    if let Some(workspaces) = ctx.views_of_type::<Workspace>(window_id_to_focus) {
        if let Some(handle) = workspaces.first() {
            ctx.dispatch_typed_action_for_view(
                window_id_to_focus,
                handle.id(),
                &WorkspaceAction::OpenPalette {
                    mode: PaletteMode::Navigation,
                    source: PaletteSource::QuitModal,
                    query: Some("running".to_owned()),
                },
            );
        }
    }
}

fn on_close_window_cancelled(
    window_id: WindowId,
    open_navigation_palette: bool,
    ctx: &mut AppContext,
) {
    send_telemetry_from_app_ctx!(
        TelemetryEvent::QuitModalCancel {
            nav_palette: open_navigation_palette,
            modal_for: CloseTarget::Window,
        },
        ctx
    );

    let sessions = SessionNavigationData::all_sessions(ctx).collect_vec();
    let sessions_summary = RunningSessionSummary::new(&sessions);
    let num_processes_in_window = sessions_summary.processes_in_window(&window_id).len();

    // If open_navigation_palette is false, return early. Otherwise, we honor the
    // open_navigation_palette param which is true if the user clicked the modal
    // button for that. However, if the running processes in this window have finished
    // since the modal popped, there is nothing to do now and we can return early
    if !open_navigation_palette || num_processes_in_window == 0 {
        return;
    }

    ctx.windows().show_window_and_focus_app(window_id);

    // if we haven't returned early, it means open_navigation_palette is true as the
    // user pressed the modal button for opening the navigation palette to show their
    // running processes
    if let Some(workspaces) = ctx.views_of_type::<Workspace>(window_id) {
        if let Some(handle) = workspaces.first() {
            ctx.dispatch_typed_action_for_view(
                window_id,
                handle.id(),
                &WorkspaceAction::OpenPalette {
                    mode: PaletteMode::Navigation,
                    source: PaletteSource::QuitModal,
                    query: Some("running".to_owned()),
                },
            );
        }
    }
}

fn is_cloud_agent_web_home_launch_url(url: &Url) -> bool {
    url.scheme() == ChannelState::url_scheme()
        && url.host_str() == Some("action")
        && url.path() == "/new_cloud_agent_conversation"
        && url
            .query_pairs()
            .any(|(key, value)| key == "source" && value == "web_home")
}
fn launch(ctx: &mut riftui::AppContext, app_state: Option<AppState>, launch_mode: LaunchMode) {
    IntervalTimer::handle(ctx).update(ctx, |timer, _ctx| {
        timer.mark_interval_end("APP_LAUNCHED");
    });

    keyboard::load_custom_keybindings(ctx);

    IntervalTimer::handle(ctx).update(ctx, |timer, _ctx| {
        timer.mark_interval_end("KEYBINDINGS_LOADED");
    });

    match launch_mode {
        LaunchMode::App { .. } | LaunchMode::Test { .. } => {
            let should_skip_restore = launch_mode
                .args()
                .urls
                .iter()
                .any(is_cloud_agent_web_home_launch_url);
            let app_state = if should_skip_restore { None } else { app_state };
            // Attempt to restore windows from the persisted application state.
            let arg = OpenFromRestoredArg { app_state };
            ctx.dispatch_global_action("root_view:open_from_restored", &arg);

            // Process any URLs that were provided on the command line (which may be
            // file:// URLs or ones using our custom URL scheme).
            for url in launch_mode.args().urls.iter() {
                uri::handle_incoming_uri(url, ctx);
            }

            // If, after session restoration and command-line argument handling, we
            // haven't opened any windows, open a new window.
            if ctx.window_ids().count() == 0 {
                ctx.dispatch_global_action("root_view:open_new", &());
            }

            IntervalTimer::handle(ctx).update(ctx, |timer, _| {
                timer.mark_interval_end("WINDOWS_CREATED");
            });

            // TODO(ben): We should skip this for LaunchMode::Test.
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            {
                use crate::login_item::maybe_register_app_as_login_item;
                use crate::terminal::general_settings::GeneralSettingsChangedEvent;
                // Note that we put this here because it depends on settings already having been initialized.
                ctx.subscribe_to_model(&GeneralSettings::handle(ctx), |_, event, ctx| {
                    if matches!(event, GeneralSettingsChangedEvent::LoginItem { .. }) {
                        maybe_register_app_as_login_item(ctx);
                    }
                });
                maybe_register_app_as_login_item(ctx);
            }
        }
        #[cfg_attr(target_family = "wasm", allow(unused_variables))]
        LaunchMode::CommandLine {
            command,
            global_options,
            ..
        } => {
            let _ = (command, global_options);
            eprintln!("CLI agent commands are not supported in this build.");
            std::process::exit(1);
        }
        // Proxy should never reach launch() — it's a thin byte bridge.
    }
}

/// Initializes the logger before running tests.
///
/// The `ctor` attribute here means that this runs BEFORE main(), whenever the
/// binary is executed. For this reason, we need to ensure that this function
/// only exists within unit test code. Production bundles and integration tests
/// also initialize the logging system, and initializing it twice causes a panic.
///
/// Additionally, we must not write anything to stdout in this function, as it
/// can interfere with test harnesses collecting the set of tests to run. (This
/// is why we're not simply calling the init() function above.)
#[ctor::ctor]
#[cfg(test)]
fn init_logging_for_unit_tests_glue() {
    // Initialize terminal-friendly logging for tests from the shared logger crate.
    rift_logging::init_logging_for_unit_tests();
}
