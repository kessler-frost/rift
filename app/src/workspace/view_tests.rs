use std::collections::HashMap;

use repo_metadata::repositories::DetectedRepositories;
use repo_metadata::watcher::DirectoryWatcher;
#[cfg(feature = "local_fs")]
use repo_metadata::RepoMetadataModel;
use riftui::platform::WindowStyle;
use riftui::{AddSingletonModel, App, ViewHandle};
use watcher::HomeDirectoryWatcher;

use super::*;
use crate::auth::AuthStateProvider;
use crate::context_chips::prompt::Prompt;
use crate::editor::Event;
use crate::gpu_state::GPUState;
use crate::network::NetworkStatus;
use crate::pane_group::{Direction, PaneGroupAction};
use crate::pricing::PricingInfoModel;
use crate::projects::ProjectManagementModel;
use crate::rift_managed_paths_watcher::RiftManagedPathsWatcher;
use crate::server::telemetry::context_provider::AppTelemetryContextProvider;
use crate::settings::PrivacySettings;
use crate::settings_view::keybindings::KeybindingChangedNotifier;
use crate::settings_view::DisplayCount;
use crate::suggestions::ignored_suggestions_model::IgnoredSuggestionsModel;
use crate::system::SystemStats;
use crate::tab_configs::tab_config::{TabConfigPaneNode, TabConfigPaneType};
use crate::terminal::history::History;
use crate::terminal::local_tty::spawner::PtySpawner;
use crate::test_util::settings::initialize_settings_for_tests;
use crate::undo_close::UndoCloseSettings;
#[cfg(windows)]
use crate::util::traffic_lights::windows::RendererState;
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::update_manager::TeamUpdateManager;
use crate::workspaces::user_profiles::UserProfiles;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::{workspace, GlobalResourceHandlesProvider};

fn initialize_app(app: &mut App) {
    initialize_settings_for_tests(app);

    // Add the necessary singleton models to the App
    app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(|_ctx| PtySpawner::new_for_test());
    app.add_singleton_model(|_| Prompt::mock());
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| SystemStats::new());
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(|_ctx| UserProfiles::new(Vec::new()));
    app.add_singleton_model(TeamTesterStatus::mock);
    app.add_singleton_model(TeamUpdateManager::mock);
    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(AppearanceManager::new);
    app.add_singleton_model(|_| DisplayCount::mock());
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(|_| KeybindingChangedNotifier::new());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(|ctx| ProjectManagementModel::new(Vec::new(), None, ctx));
    app.add_singleton_model(|_ctx| SyncedInputState::mock());
    app.add_singleton_model(|_| ResizableData::default());
    app.add_singleton_model(UndoCloseStack::new);
    app.add_singleton_model(|_| ActiveSession::default());
    app.add_singleton_model(|_| WorkspaceToastStack);
    app.add_singleton_model(|_| SettingsPaneManager::new());

    // Initialize file-based MCP dependencies.
    app.add_singleton_model(|_| DetectedRepositories::default());
    app.add_singleton_model(HomeDirectoryWatcher::new_for_test);
    app.add_singleton_model(DirectoryWatcher::new);
    app.add_singleton_model(RiftManagedPathsWatcher::new_for_testing);

    app.add_singleton_model(|_| GPUState::new());
    let global_resource_handles = GlobalResourceHandles::mock(app);
    app.add_singleton_model(|_| GlobalResourceHandlesProvider::new(global_resource_handles));
    app.add_singleton_model(DefaultTerminal::new);
    app.add_singleton_model(|_| IgnoredSuggestionsModel::new(vec![]));

    #[cfg(feature = "local_fs")]
    app.add_singleton_model(RepoMetadataModel::new);
    app.add_singleton_model(crate::search::files::model::FileSearchModel::new);

    #[cfg(windows)]
    {
        app.add_singleton_model(RendererState::new);
    }

    #[cfg(feature = "local_tty")]
    terminal::available_shells::register(app);
    AltScreenReporting::register(app);

    #[cfg(enable_crash_recovery)]
    crate::crash_recovery::CrashRecovery::register_for_test(app);

    app.add_singleton_model(|_| PricingInfoModel::new());
    app.add_singleton_model(|_| History::new(vec![]));

    // Make sure to initialize the keybindings so that they are available for subviews
    app.update(workspace::init);
}

fn mock_workspace(app: &mut App) -> ViewHandle<Workspace> {
    let global_resource_handles = GlobalResourceHandles::mock(app);
    let active_window_id = app.read(|ctx| ctx.windows().active_window());
    let (_, workspace) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
        Workspace::new(
            global_resource_handles,
            NewWorkspaceSource::Empty {
                previous_active_window: active_window_id,
                shell: None,
            },
            ctx,
        )
    });
    workspace
}

fn restored_workspace(
    app: &mut App,
    window_snapshot: crate::app_state::WindowSnapshot,
) -> ViewHandle<Workspace> {
    let global_resource_handles = GlobalResourceHandles::mock(app);
    let (_, workspace) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
        Workspace::new(
            global_resource_handles,
            NewWorkspaceSource::Restored {
                window_snapshot,
                block_lists: Arc::new(HashMap::new()),
            },
            ctx,
        )
    });
    workspace
}

fn transferred_tab_workspace(
    app: &mut App,
    vertical_tabs_panel_open: bool,
) -> ViewHandle<Workspace> {
    let global_resource_handles = GlobalResourceHandles::mock(app);
    let (_, workspace) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
        Workspace::new(
            global_resource_handles,
            NewWorkspaceSource::TransferredTab {
                tab_color: None,
                custom_title: None,
                left_panel_open: false,
                vertical_tabs_panel_open,
                right_panel_open: false,
                is_right_panel_maximized: false,
                is_tab_drag_preview: false,
            },
            ctx,
        )
    });
    workspace
}

#[test]
fn test_tab_bar_traffic_light_space_regression_for_resource_center_overlap() {
    // Regression for #10139: the Resource Center/right panel can be open on
    // Windows/Linux, but vertical-tabs and right-panel state should not decide
    // whether the tab bar reserves space for titlebar controls.
    let cases = [
        (TrafficLightSide::Left, false),
        (TrafficLightSide::Right, true),
    ];

    for (side, should_reserve_space) in cases {
        assert_eq!(
            should_reserve_traffic_light_space_in_tab_bar(side),
            should_reserve_space
        );
    }
}

fn new_session_menu_label(item: &MenuItem<WorkspaceAction>) -> String {
    match item {
        MenuItem::Item(fields) => fields.label().to_string(),
        MenuItem::Separator => "---".to_string(),
        MenuItem::ItemsRow { items } => items
            .iter()
            .map(|fields| fields.label().to_string())
            .collect::<Vec<_>>()
            .join(" | "),
        MenuItem::Submenu { fields, .. } => fields.label().to_string(),
        MenuItem::Header { fields, .. } => fields.label().to_string(),
    }
}

fn reopen_closed_session_menu_item(
    menu_items: &[MenuItem<WorkspaceAction>],
) -> &MenuItemFields<WorkspaceAction> {
    match menu_items.last() {
        Some(MenuItem::Item(fields)) if fields.label() == "Reopen closed session" => fields,
        _ => panic!("expected Reopen closed session to be the last new-session menu item"),
    }
}

#[test]
fn test_tab_renaming_editor_selections() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        // Add second tab and rename both of them to prepare for the test
        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            workspace.rename_tab_internal(0, "short_title", ctx);
            let selected_text = workspace
                .tab_rename_editor
                .read(ctx, |editor, ctx| editor.selected_text(ctx));
            assert_eq!("short_title", selected_text);

            // Ensure that whatever is selected, is the full title and not the leftover from
            // the previous, shorter one.
            workspace.rename_tab_internal(1, "very_long_title_this_is", ctx);
            let selected_text = workspace
                .tab_rename_editor
                .read(ctx, |editor, ctx| editor.selected_text(ctx));
            assert_eq!("very_long_title_this_is", selected_text);

            // Ensure that if we escape, the current editor's contents is going to be cleared
            // as well.
            workspace.handle_tab_rename_editor_event(&Event::Escape, ctx);
            let selected_text = workspace
                .tab_rename_editor
                .read(ctx, |editor, ctx| editor.selected_text(ctx));
            assert_eq!("", selected_text);
        });
    });
}

#[test]
fn test_tab_renaming_editor_reset() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            workspace.rename_tab_internal(0, "short_title", ctx);
            workspace.rename_tab_internal(1, "very_long_title_this_is", ctx);

            // Ensure that when the editor is initially not empty, it will be cleared before a user renames a tab
            workspace.tab_rename_editor.update(ctx, |editor, ctx| {
                editor.insert_selected_text("some-text", ctx);
            });
            workspace.rename_tab_internal(1, "new_very_long_title", ctx);
            let selected_text: String = workspace
                .tab_rename_editor
                .read(ctx, |editor, ctx| editor.selected_text(ctx));
            assert_eq!("new_very_long_title", selected_text);
        });
    });
}

#[test]
fn test_set_active_tab_name() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);

            workspace.handle_action(
                &WorkspaceAction::SetActiveTabName("  Backend API  ".to_string()),
                ctx,
            );
            assert_eq!(
                workspace
                    .active_tab_pane_group()
                    .as_ref(ctx)
                    .display_title(ctx),
                "Backend API"
            );
            assert_eq!(
                workspace
                    .active_tab_pane_group()
                    .as_ref(ctx)
                    .custom_title(ctx)
                    .as_deref(),
                Some("Backend API")
            );

            workspace.handle_action(&WorkspaceAction::ActivateTab(0), ctx);
            assert_ne!(
                workspace
                    .active_tab_pane_group()
                    .as_ref(ctx)
                    .custom_title(ctx)
                    .as_deref(),
                Some("Backend API")
            );

            workspace.handle_action(&WorkspaceAction::ActivateTab(1), ctx);
            workspace.handle_action(&WorkspaceAction::SetActiveTabName("   ".to_string()), ctx);
            assert_eq!(
                workspace
                    .active_tab_pane_group()
                    .as_ref(ctx)
                    .custom_title(ctx)
                    .as_deref(),
                Some("Backend API")
            );
        });
    });
}

#[test]
fn test_set_active_tab_name_clears_active_rename_editor_state() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.rename_tab_internal(0, "old title", ctx);
            assert!(workspace.current_workspace_state.is_tab_being_renamed());

            workspace.handle_action(
                &WorkspaceAction::SetActiveTabName("new title".to_string()),
                ctx,
            );

            assert!(!workspace.current_workspace_state.is_tab_being_renamed());
            assert_eq!(
                workspace
                    .active_tab_pane_group()
                    .as_ref(ctx)
                    .display_title(ctx),
                "new title"
            );
        });
    });
}

#[test]
fn test_set_active_tab_color() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            let active = workspace.active_tab_index;

            // Setting a color stores it as the manual selection and resolves to it.
            workspace.handle_action(
                &WorkspaceAction::SetActiveTabColor(SelectedTabColor::Color(
                    AnsiColorIdentifier::Magenta,
                )),
                ctx,
            );
            assert_eq!(
                workspace.tabs[active].selected_color,
                SelectedTabColor::Color(AnsiColorIdentifier::Magenta),
            );
            assert_eq!(
                workspace.tabs[active].color(),
                Some(AnsiColorIdentifier::Magenta),
            );

            // Replacing with a different color overwrites the previous selection.
            workspace.handle_action(
                &WorkspaceAction::SetActiveTabColor(SelectedTabColor::Color(
                    AnsiColorIdentifier::Green,
                )),
                ctx,
            );
            assert_eq!(
                workspace.tabs[active].selected_color,
                SelectedTabColor::Color(AnsiColorIdentifier::Green),
            );

            // `Cleared` explicitly suppresses any color (including a directory default).
            workspace.handle_action(
                &WorkspaceAction::SetActiveTabColor(SelectedTabColor::Cleared),
                ctx,
            );
            assert_eq!(
                workspace.tabs[active].selected_color,
                SelectedTabColor::Cleared,
            );
            assert_eq!(workspace.tabs[active].color(), None);

            // `Unset` removes the manual override so a directory default could apply.
            // With no directory default configured, the resolved color is still `None`.
            workspace.handle_action(
                &WorkspaceAction::SetActiveTabColor(SelectedTabColor::Unset),
                ctx,
            );
            assert_eq!(
                workspace.tabs[active].selected_color,
                SelectedTabColor::Unset,
            );
            assert_eq!(workspace.tabs[active].color(), None);

            // Action targets the active tab — switching to tab 0 leaves the second tab
            // unaffected.
            workspace.handle_action(&WorkspaceAction::ActivateTab(0), ctx);
            workspace.handle_action(
                &WorkspaceAction::SetActiveTabColor(SelectedTabColor::Color(
                    AnsiColorIdentifier::Blue,
                )),
                ctx,
            );
            assert_eq!(
                workspace.tabs[0].selected_color,
                SelectedTabColor::Color(AnsiColorIdentifier::Blue),
            );
            assert_eq!(
                workspace.tabs[active].selected_color,
                SelectedTabColor::Unset,
            );
        });
    });
}

#[test]
fn test_workspace_sessions_retrieves_tabs() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            let pane_id = workspace
                .get_pane_group_view(0)
                .map(|tab| tab.read(ctx, |tab, _ctx| tab.pane_id_by_index(0).unwrap()))
                .expect("WindowId was not retrieved.");

            assert!(workspace
                .workspace_sessions(ctx.window_id(), ctx)
                .any(|x| { x.pane_view_locator().pane_id == pane_id }));

            // Add a tab and check if workspace_sessions finds the second session from the new tab.
            workspace.add_terminal_tab(false, ctx);
            let new_pane_id = workspace
                .get_pane_group_view(1)
                .map(|tab| tab.read(ctx, |tab, _ctx| tab.pane_id_by_index(0).unwrap()))
                .expect("WindowId was not retrieved.");

            assert!(workspace
                .workspace_sessions(ctx.window_id(), ctx)
                .any(|x| { x.pane_view_locator().pane_id == new_pane_id }));
        });
    });
}

#[test]
fn test_workspace_sessions_retrieves_panes() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            // Add a new split pane to the right.
            if let Some(tab_view) = workspace.get_pane_group_view(0) {
                tab_view.update(ctx, |view, ctx| {
                    view.handle_action(&PaneGroupAction::Add(Direction::Right), ctx);
                })
            }

            // Get the EntityId of the new pane added to the current tab.
            let new_pane_id = workspace
                .get_pane_group_view(0)
                .map(|tab| tab.read(ctx, |tab, _ctx| tab.pane_id_by_index(1).unwrap()))
                .expect("WindowId was not retrieved.");
            assert!(workspace
                .workspace_sessions(ctx.window_id(), ctx)
                .any(|x| { x.pane_view_locator().pane_id == new_pane_id }));
        });
    });
}

#[test]
fn test_close_active_horizontal_tab_activates_tab_to_right() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(false, ctx));
            });
        });

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            workspace.add_terminal_tab(false, ctx);
            let tab_to_right_id = workspace.get_pane_group_view(2).unwrap().id();

            workspace.activate_tab(1, ctx);
            workspace.close_tab(1, true, true, ctx);

            assert_eq!(workspace.tab_count(), 2);
            assert_eq!(workspace.active_tab_index(), 1);
            assert_eq!(workspace.active_tab_pane_group().id(), tab_to_right_id);
        });
    });
}

#[test]
fn test_close_last_horizontal_tab_activates_tab_to_left() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(false, ctx));
            });
        });

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            workspace.add_terminal_tab(false, ctx);
            let tab_to_left_id = workspace.get_pane_group_view(1).unwrap().id();

            workspace.activate_tab(2, ctx);
            workspace.close_tab(2, true, true, ctx);

            assert_eq!(workspace.tab_count(), 2);
            assert_eq!(workspace.active_tab_index(), 1);
            assert_eq!(workspace.active_tab_pane_group().id(), tab_to_left_id);
        });
    });
}
#[test]
fn test_set_active_terminal_input_contents_and_focus_app() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            let initial_buffer_contents = workspace
                .get_active_input_view_handle(ctx)
                .map(|input_view_handle| input_view_handle.as_ref(ctx).buffer_text(ctx))
                .expect("There should be an active input view");
            assert_eq!(
                "", initial_buffer_contents,
                "initial active input should be empty"
            );

            workspace.set_active_terminal_input_contents_and_focus_app("foobar", ctx);

            assert_eq!(
                "foobar",
                workspace
                    .get_active_input_view_handle(ctx)
                    .map(|input_view_handle| input_view_handle.as_ref(ctx).buffer_text(ctx))
                    .expect("There should be an active input view")
            );
            assert!(ctx.windows().app_is_active());
        });
    });
}

/// Ensures that the terminal model is destroyed when it is no longer needed.
/// This is only a "workspace" test because we want to mimic what a normal
/// user would do and expect (e.g. close a tab and expect that its backing
/// data is correctly deallocated).
///
/// TODO(suraj): we may also want to investigate a more "real" integration test
/// that inspects the application process's overall memory consumption
/// instead of just the terminal model, but this is not easy because
/// 1. we want to measure non-shared memory (i.e. the "memory" value in Activity Monitor)
///    which is not easy; it's easier to measure "real memory" or RSS, but that includes
///    shared memory across processes.
/// 2. the test might be flaky depending on how much memory is actually allocated vs
///    freed up (not something easily controlled).
///
/// For now, this test is still useful because the terminal model is one of the largest data structures
/// maintained by our app, so we want to ensure we're not introducing regressions that cause it to not
/// be deallocated correctly.
#[test]
fn test_terminal_model_isnt_leaked() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        // Turn off undo-close so that we don't need to wait for deallocation.
        UndoCloseSettings::handle(&app).update(&mut app, |settings, ctx| {
            settings
                .enabled
                .set_value(false, ctx)
                .expect("Can turn off undo-close via settings.")
        });

        let workspace = mock_workspace(&mut app);

        let terminal_model = workspace.update(&mut app, |workspace, ctx| {
            // Add another tab so that the workspace isn't destroyed when we close the tab.
            workspace.add_terminal_tab(false, ctx);

            // Get a weak reference to the model.
            let model = workspace.get_active_session_terminal_model(ctx).unwrap();
            Arc::downgrade(&model)
        });

        workspace.update(&mut app, |workspace, ctx| {
            // Remove the tab. This should destroy the corresponding terminal view.
            workspace.remove_tab(workspace.active_tab_index(), true, true, ctx);
        });
        // For some reason, the update call above results in more pending effects, one of which
        // contains the actual logic that drops the `TerminalModel`.
        app.update(|_| ());

        // If we can't upgrade the weak reference, that means it was in fact destructed.
        assert!(
            terminal_model.upgrade().is_none(),
            "The terminal model should not exist once the tab is closed."
        )
    });
}

#[test]
fn test_vertical_tabs_panel_visibility_restores_from_window_snapshot() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(true, ctx));
            });
        });

        let workspace = mock_workspace(&mut app);

        let closed_snapshot = workspace.update(&mut app, |workspace, ctx| {
            workspace.vertical_tabs_panel_open = false;
            workspace.snapshot(ctx.window_id(), false, ctx)
        });
        let open_snapshot = workspace.update(&mut app, |workspace, ctx| {
            workspace.vertical_tabs_panel_open = true;
            workspace.snapshot(ctx.window_id(), false, ctx)
        });

        let restored_closed = restored_workspace(&mut app, closed_snapshot);
        let restored_open = restored_workspace(&mut app, open_snapshot);

        restored_closed.read(&app, |workspace, _| {
            assert!(!workspace.vertical_tabs_panel_open);
        });
        restored_open.read(&app, |workspace, _| {
            assert!(workspace.vertical_tabs_panel_open);
        });
    });
}

#[test]
fn test_vertical_tabs_panel_restored_open_when_show_in_restored_windows_enabled() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(true, ctx));
                report_if_error!(settings
                    .show_vertical_tab_panel_in_restored_windows
                    .set_value(true, ctx));
            });
        });

        let workspace = mock_workspace(&mut app);

        let closed_snapshot = workspace.update(&mut app, |workspace, ctx| {
            workspace.vertical_tabs_panel_open = false;
            workspace.snapshot(ctx.window_id(), false, ctx)
        });

        let restored = restored_workspace(&mut app, closed_snapshot);
        restored.read(&app, |workspace, _| {
            assert!(workspace.vertical_tabs_panel_open);
        });
    });
}

#[test]
fn test_vertical_tabs_panel_closed_when_disabled_even_if_persisted_open() {
    // Regression for #9505: when `vertical_tabs_panel_open=true` is persisted
    // and the user then disables vertical tabs, restoring the workspace must
    // not honor the stale snapshot — otherwise a dismiss underlay paints over
    // the window and silently swallows every click.
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        // Snapshot the workspace with the panel open while vertical tabs are enabled.
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(true, ctx));
            });
        });
        let workspace = mock_workspace(&mut app);
        let open_snapshot = workspace.update(&mut app, |workspace, ctx| {
            workspace.vertical_tabs_panel_open = true;
            workspace.snapshot(ctx.window_id(), false, ctx)
        });

        // Disable vertical tabs, then restore. The panel must stay closed.
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(false, ctx));
            });
        });
        let restored = restored_workspace(&mut app, open_snapshot);
        restored.read(&app, |workspace, _| {
            assert!(!workspace.vertical_tabs_panel_open);
        });
    });
}

#[test]
fn test_vertical_tabs_panel_defaults_open_for_new_window_when_vertical_tabs_enabled() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(true, ctx));
            });
        });

        let workspace = mock_workspace(&mut app);

        workspace.read(&app, |workspace, _| {
            assert!(workspace.vertical_tabs_panel_open);
        });
    });
}

#[test]
fn test_vertical_tabs_panel_inherits_transferred_tab_source_window_state() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(true, ctx));
            });
        });

        let transferred_closed = transferred_tab_workspace(&mut app, false);
        let transferred_open = transferred_tab_workspace(&mut app, true);

        transferred_closed.read(&app, |workspace, _| {
            assert!(!workspace.vertical_tabs_panel_open);
        });
        transferred_open.read(&app, |workspace, _| {
            assert!(workspace.vertical_tabs_panel_open);
        });
    });
}

#[test]
fn test_vertical_tabs_panel_auto_shows_when_setting_enabled() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.read(&app, |workspace, _| {
            assert!(!workspace.vertical_tabs_panel_open);
        });

        // Enabling vertical tabs should auto-open the panel.
        workspace.update(&mut app, |_, ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(true, ctx));
            });
        });
        workspace.read(&app, |workspace, _| {
            assert!(workspace.vertical_tabs_panel_open);
        });

        // Disabling vertical tabs should auto-close the panel.
        workspace.update(&mut app, |_, ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(false, ctx));
            });
        });
        workspace.read(&app, |workspace, _| {
            assert!(!workspace.vertical_tabs_panel_open);
        });
    });
}

#[test]
fn test_toggle_tab_configs_menu_opens_vertical_tabs_panel_and_menu() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(true, ctx));
            });
            workspace.vertical_tabs_panel_open = true;
        });
        workspace.update(&mut app, |workspace, ctx| {
            workspace.vertical_tabs_panel_open = false;
            workspace.show_new_session_dropdown_menu = None;

            workspace.handle_action(&WorkspaceAction::ToggleTabConfigsMenu, ctx);

            assert!(workspace.vertical_tabs_panel_open);
            assert!(workspace.show_new_session_dropdown_menu.is_some());
        });
    });
}

#[test]
fn test_toggle_tab_configs_menu_keyboard_shortcut_selects_top_item() {
    let _tab_configs_guard = FeatureFlag::TabConfigs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.show_new_session_dropdown_menu = None;

            workspace.handle_action(&WorkspaceAction::ToggleTabConfigsMenu, ctx);

            assert!(workspace.show_new_session_dropdown_menu.is_some());
            assert_eq!(
                workspace
                    .new_session_dropdown_menu
                    .read(ctx, |menu, _| menu.selected_index()),
                Some(0)
            );
        });
    });
}

#[test]
fn test_pointer_opened_tab_configs_menu_does_not_select_top_item() {
    let _tab_configs_guard = FeatureFlag::TabConfigs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.toggle_new_session_dropdown_menu(
                crate::workspace::action::NewSessionMenuAnchor::Pointer(Vector2F::zero()),
                ctx,
            );

            assert!(workspace.show_new_session_dropdown_menu.is_some());
            assert_eq!(
                workspace
                    .new_session_dropdown_menu
                    .read(ctx, |menu, _| menu.selected_index()),
                None
            );
        });
    });
}

#[test]
fn test_open_tab_config_with_params_does_not_use_worktree_branch_as_implicit_title() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);
        let tab_config = crate::tab_configs::TabConfig {
            name: "Untitled worktree".to_string(),
            title: None,
            color: None,
            panes: vec![TabConfigPaneNode {
                id: "main".to_string(),
                pane_type: Some(TabConfigPaneType::Terminal),
                split: None,
                children: None,
                is_focused: Some(true),
                directory: None,
                commands: Some(vec!["echo {{autogenerated_branch_name}}".to_string()]),
                shell: None,
            }],
            params: HashMap::new(),
            source_path: None,
        };

        workspace.update(&mut app, |workspace, ctx| {
            workspace.open_tab_config_with_params(
                tab_config.clone(),
                HashMap::new(),
                Some("mesa-coyote"),
                ctx,
            );
        });

        workspace.read(&app, |workspace, ctx| {
            assert_eq!(workspace.tab_count(), 2);
            assert_eq!(
                workspace
                    .active_tab_pane_group()
                    .as_ref(ctx)
                    .custom_title(ctx),
                None
            );
        });
    });
}

#[test]
fn test_open_tab_config_with_params_uses_explicit_title_template() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);
        let tab_config = crate::tab_configs::TabConfig {
            name: "Titled worktree".to_string(),
            title: Some("{{autogenerated_branch_name}}".to_string()),
            color: None,
            panes: vec![TabConfigPaneNode {
                id: "main".to_string(),
                pane_type: Some(TabConfigPaneType::Terminal),
                split: None,
                children: None,
                is_focused: Some(true),
                directory: None,
                commands: Some(vec!["echo {{autogenerated_branch_name}}".to_string()]),
                shell: None,
            }],
            params: HashMap::new(),
            source_path: None,
        };

        workspace.update(&mut app, |workspace, ctx| {
            workspace.open_tab_config_with_params(
                tab_config.clone(),
                HashMap::new(),
                Some("mesa-coyote"),
                ctx,
            );
        });

        workspace.read(&app, |workspace, ctx| {
            assert_eq!(workspace.tab_count(), 2);
            assert_eq!(
                workspace
                    .active_tab_pane_group()
                    .as_ref(ctx)
                    .custom_title(ctx),
                Some("mesa-coyote".to_string())
            );
        });
    });
}
#[test]
fn test_toggle_tab_configs_menu_does_not_change_vertical_tabs_panel_in_horizontal_mode() {
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings.use_vertical_tabs.set_value(false, ctx));
            });
            workspace.vertical_tabs_panel_open = true;
            workspace.show_new_session_dropdown_menu = None;

            workspace.handle_action(&WorkspaceAction::ToggleTabConfigsMenu, ctx);

            assert!(workspace.vertical_tabs_panel_open);
            assert!(workspace.show_new_session_dropdown_menu.is_some());
        });
    });
}

#[test]
fn test_unified_new_session_menu_uses_new_worktree_config_label_and_order() {
    let _tab_configs_guard = FeatureFlag::TabConfigs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            let labels = workspace
                .unified_new_session_menu_items(ctx)
                .iter()
                .map(new_session_menu_label)
                .collect::<Vec<_>>();

            assert!(!labels.iter().any(|label| label == "Worktree in"));

            let separator_index = labels
                .iter()
                .position(|label| label == "---")
                .expect("expected a separator in the new-session menu");

            assert_eq!(
                labels.get(separator_index + 1),
                Some(&"New worktree config".to_string())
            );
            assert_eq!(
                labels.get(separator_index + 2),
                Some(&"New tab config".to_string())
            );
        });
    });
}

#[test]
fn test_unified_new_session_menu_includes_reopen_closed_session() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            let menu_items = workspace.unified_new_session_menu_items(ctx);
            assert!(matches!(
                menu_items.get(menu_items.len() - 2),
                Some(MenuItem::Separator)
            ));

            let reopen_item = reopen_closed_session_menu_item(&menu_items);
            assert!(reopen_item.is_disabled());
            assert!(matches!(
                reopen_item.on_select_action(),
                Some(action) if matches!(action, WorkspaceAction::ReopenClosedSession)
            ));

            workspace.add_terminal_tab(false, ctx);
            workspace.remove_tab(workspace.active_tab_index(), true, true, ctx);

            let menu_items = workspace.unified_new_session_menu_items(ctx);
            let reopen_item = reopen_closed_session_menu_item(&menu_items);
            assert!(!reopen_item.is_disabled());
        });
    });
}

#[test]
fn test_vertical_tabs_context_menu_does_not_show_hover_only_tab_bar() {
    let _full_screen_zen_mode_guard = FeatureFlag::FullScreenZenMode.override_enabled(true);
    let _vertical_tabs_guard = FeatureFlag::VerticalTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings
                    .workspace_decoration_visibility
                    .set_value(WorkspaceDecorationVisibility::OnHover, ctx));
                report_if_error!(settings.use_vertical_tabs.set_value(true, ctx));
            });
            workspace.vertical_tabs_panel_open = true;

            workspace.show_tab_right_click_menu =
                Some((0, TabContextMenuAnchor::Pointer(Vector2F::zero())));

            assert_eq!(workspace.tab_bar_mode(ctx), ShowTabBar::Hidden);
        });
    });
}

#[test]
fn test_standard_tab_context_menu_shows_hover_only_tab_bar() {
    let _full_screen_zen_mode_guard = FeatureFlag::FullScreenZenMode.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings
                    .workspace_decoration_visibility
                    .set_value(WorkspaceDecorationVisibility::OnHover, ctx));
            });

            workspace.show_tab_right_click_menu =
                Some((0, TabContextMenuAnchor::Pointer(Vector2F::zero())));

            assert_eq!(workspace.tab_bar_mode(ctx), ShowTabBar::Stacked);
        });
    });
}

#[test]
fn test_tab_mru_order() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            workspace.add_terminal_tab(false, ctx);

            let id_a = workspace.tabs[0].pane_group.id();
            let id_b = workspace.tabs[1].pane_group.id();
            let id_c = workspace.tabs[2].pane_group.id();

            workspace.handle_action(&WorkspaceAction::ActivateTab(0), ctx);
            workspace.handle_action(&WorkspaceAction::ActivateTab(1), ctx);
            workspace.handle_action(&WorkspaceAction::ActivateTab(2), ctx);
            workspace.handle_action(&WorkspaceAction::ActivateTab(0), ctx);

            assert_eq!(workspace.tab_mru_order(), &[id_a, id_c, id_b]);
        });
    });
}

#[test]
fn test_create_new_tab_group_groups_active_tab() {
    let _grouped_tabs_guard = FeatureFlag::GroupedTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);
        workspace.update(&mut app, |workspace, ctx| {
            // Workspace starts with one tab from `Empty` source. Create a tab
            // group and verify the active tab is assigned to it.
            assert_eq!(workspace.tab_count(), 1);
            assert!(workspace.tabs[0].group_id.is_none());
            assert!(workspace.tab_groups.is_empty());

            workspace.handle_action(
                &WorkspaceAction::SelectNewSessionMenuItem(NewSessionMenuItem::CreateNewTabGroup),
                ctx,
            );

            assert_eq!(workspace.tab_groups.len(), 1);
            let group_id = workspace.tabs[0]
                .group_id
                .expect("active tab should be assigned to the new group");
            assert!(workspace.tab_groups.contains_key(&group_id));
            // New groups start expanded so members are visible.
            assert!(!workspace.tab_groups[&group_id].collapsed);
        });
    });
}

#[test]
fn test_toggle_tab_group_collapsed_flips_state() {
    let _grouped_tabs_guard = FeatureFlag::GroupedTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);
        workspace.update(&mut app, |workspace, ctx| {
            workspace.handle_action(
                &WorkspaceAction::SelectNewSessionMenuItem(NewSessionMenuItem::CreateNewTabGroup),
                ctx,
            );
            let group_id = workspace.tabs[0]
                .group_id
                .expect("active tab should be in a group");
            assert!(!workspace.tab_groups[&group_id].collapsed);

            workspace.handle_action(&WorkspaceAction::ToggleTabGroupCollapsed(group_id), ctx);
            assert!(workspace.tab_groups[&group_id].collapsed);

            workspace.handle_action(&WorkspaceAction::ToggleTabGroupCollapsed(group_id), ctx);
            assert!(!workspace.tab_groups[&group_id].collapsed);
        });
    });
}

#[test]
fn test_close_tab_group_removes_group_and_members() {
    let _grouped_tabs_guard = FeatureFlag::GroupedTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);
        workspace.update(&mut app, |workspace, ctx| {
            // Create a group, then add another tab which inherits the
            // active tab's group_id via `add_tab_with_pane_layout`.
            workspace.handle_action(
                &WorkspaceAction::SelectNewSessionMenuItem(NewSessionMenuItem::CreateNewTabGroup),
                ctx,
            );
            let group_id = workspace.tabs[workspace.active_tab_index()]
                .group_id
                .expect("active tab should be in a group");

            workspace.add_terminal_tab(false, ctx);

            let group_members: Vec<usize> = workspace
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, tab)| tab.group_id == Some(group_id))
                .map(|(idx, _)| idx)
                .collect();
            assert_eq!(
                group_members.len(),
                2,
                "new tab should inherit the active tab's group_id"
            );

            workspace.handle_action(&WorkspaceAction::CloseTabGroup(group_id), ctx);

            // All group members are closed and the group entry is removed.
            assert!(!workspace.tab_groups.contains_key(&group_id));
            assert!(workspace
                .tabs
                .iter()
                .all(|tab| tab.group_id != Some(group_id)));
        });
    });
}

#[test]
fn test_new_tab_with_after_all_tabs_setting_lands_at_group_end() {
    // With `new_tab_placement = AfterAllTabs` and the active tab in a
    // group, a new tab should land at the end of the group's contiguous
    // run instead of at the workspace's global end so group contiguity
    // is preserved while honoring the user's "end" placement preference.
    let _grouped_tabs_guard = FeatureFlag::GroupedTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings
                    .new_tab_placement
                    .set_value(NewTabPlacement::AfterAllTabs, ctx));
            });
        });

        let workspace = mock_workspace(&mut app);
        workspace.update(&mut app, |workspace, ctx| {
            // Create a group and add a second tab so the group has two
            // contiguous members.
            workspace.handle_action(
                &WorkspaceAction::SelectNewSessionMenuItem(NewSessionMenuItem::CreateNewTabGroup),
                ctx,
            );
            let group_id = workspace.tabs[workspace.active_tab_index()]
                .group_id
                .expect("active tab should be in a group");
            workspace.add_terminal_tab(false, ctx);

            // Add an ungrouped tab past the end of the group by first
            // activating the trailing ungrouped tab.
            let ungrouped_idx = workspace
                .tabs
                .iter()
                .position(|t| t.group_id.is_none())
                .expect("expected at least one ungrouped tab");
            workspace.activate_tab(ungrouped_idx, ctx);
            workspace.add_terminal_tab(false, ctx);

            // Now activate the first grouped tab and add a new tab. With
            // `AfterAllTabs`, the new tab must land at the end of the
            // group's contiguous run rather than past the trailing
            // ungrouped tabs.
            let first_grouped_idx = workspace
                .tabs
                .iter()
                .position(|t| t.group_id == Some(group_id))
                .expect("expected at least one grouped tab");
            workspace.activate_tab(first_grouped_idx, ctx);

            let group_run_end_before = workspace
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, t)| t.group_id == Some(group_id))
                .map(|(idx, _)| idx)
                .max()
                .expect("group should be non-empty")
                + 1;

            workspace.add_terminal_tab(false, ctx);

            // The new tab lands at the prior group-run end, inherits the
            // group_id, and keeps the group's run contiguous.
            assert_eq!(workspace.active_tab_index(), group_run_end_before);
            assert_eq!(
                workspace.tabs[group_run_end_before].group_id,
                Some(group_id)
            );

            let group_indices: Vec<usize> = workspace
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, t)| t.group_id == Some(group_id))
                .map(|(idx, _)| idx)
                .collect();
            assert!(
                group_indices.windows(2).all(|w| w[1] == w[0] + 1),
                "group's tab indices should be contiguous, got {group_indices:?}"
            );
        });
    });
}

#[test]
fn test_new_tab_with_after_current_tab_setting_lands_after_active_tab_in_group() {
    // With `new_tab_placement = AfterCurrentTab` and the active tab in the
    // middle of a group, a new tab should land immediately after the active
    // tab and inherit the group_id, preserving group contiguity rather than
    // jumping to the end of the group or past it.
    let _grouped_tabs_guard = FeatureFlag::GroupedTabs.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(settings
                    .new_tab_placement
                    .set_value(NewTabPlacement::AfterCurrentTab, ctx));
            });
        });

        let workspace = mock_workspace(&mut app);
        workspace.update(&mut app, |workspace, ctx| {
            // Create a group and grow it to two contiguous members so we can
            // activate the first one (i.e. a member that isn't at the end of
            // the group's run).
            workspace.handle_action(
                &WorkspaceAction::SelectNewSessionMenuItem(NewSessionMenuItem::CreateNewTabGroup),
                ctx,
            );
            let group_id = workspace.tabs[workspace.active_tab_index()]
                .group_id
                .expect("active tab should be in a group");
            workspace.add_terminal_tab(false, ctx);

            // Activate the first grouped tab so the next insertion happens in
            // the middle of the group's contiguous run.
            let first_grouped_idx = workspace
                .tabs
                .iter()
                .position(|t| t.group_id == Some(group_id))
                .expect("expected at least one grouped tab");
            workspace.activate_tab(first_grouped_idx, ctx);

            let expected_new_idx = first_grouped_idx + 1;

            workspace.add_terminal_tab(false, ctx);

            // The new tab lands immediately after the previously-active
            // grouped tab, inherits its group_id, and keeps the group's run
            // contiguous.
            assert_eq!(workspace.active_tab_index(), expected_new_idx);
            assert_eq!(
                workspace.tabs[expected_new_idx].group_id,
                Some(group_id),
                "new tab should inherit the active tab's group_id"
            );

            let group_indices: Vec<usize> = workspace
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, t)| t.group_id == Some(group_id))
                .map(|(idx, _)| idx)
                .collect();
            assert_eq!(
                group_indices.len(),
                3,
                "group should have grown to three members"
            );
            assert!(
                group_indices.windows(2).all(|w| w[1] == w[0] + 1),
                "group's tab indices should be contiguous, got {group_indices:?}"
            );
        });
    });
}
