pub mod auth_manager;
pub mod auth_state;
mod auth_override_warning_body;
pub mod auth_override_warning_modal;
mod auth_view_body;
pub mod auth_view_modal;
mod auth_view_shared_helpers;
mod login_error_modal;
mod login_failure_notification;
pub mod needs_sso_link_view;
pub mod paste_auth_token_modal;
// `auth_state` is now a local, offline implementation (see `auth_state.rs`). The remaining
// re-exports are pure local data types (User/Credentials/UserUid) and carry no network code.
pub use rift_server_auth::{credentials, user, user_uid};
#[cfg(target_family = "wasm")]
pub mod web_handoff;

use ::settings::{Setting, ToggleableSetting};
pub use auth_manager::AuthManager;
pub use auth_state::AuthStateProvider;
use itertools::Itertools;
pub use login_failure_notification::LoginFailureReason;
use riftui::modals::{AlertDialogWithCallbacks, ModalButton};
use riftui::{AppContext, SingletonEntity};
pub use user_uid::UserUid;

use crate::palette::PaletteMode;
use crate::server::telemetry::PaletteSource;
use crate::session_management::{RunningSessionSummary, SessionNavigationData};
use crate::terminal::general_settings::GeneralSettings;
use crate::workspace::{Workspace, WorkspaceAction};
use crate::{
    focus_running_window_and_show_native_modal, persistence, report_if_error,
    send_telemetry_sync_from_app_ctx, GlobalResourceHandlesProvider,
};

pub fn init(app: &mut AppContext) {
    auth_view_modal::init(app);
    auth_view_body::init(app);
    auth_override_warning_body::init(app);
    paste_auth_token_modal::init(app);
}

/// If the app has running processes or dirty objects, we'll show a confirmation modal before logging out.
/// If the user aborts, the user will not be logged out.
pub fn maybe_log_out(app: &mut AppContext) {
    send_telemetry_sync_from_app_ctx!(TelemetryEvent::UserInitiatedLogOut, app);

    let sessions = SessionNavigationData::all_sessions(app).collect_vec();
    let num_long_running_commands = RunningSessionSummary::new(&sessions)
        .long_running_cmds
        .len();
    let num_shared_sessions = crate::session_management::num_shared_sessions(app);
    let num_unsaved_objects = 0;
    let num_unsaved_files = 0;

    let show_warning_before_log_out = *GeneralSettings::as_ref(app)
        .show_warning_before_quitting
        .value();
    if show_warning_before_log_out
        && (num_long_running_commands > 0
            || num_shared_sessions > 0
            || num_unsaved_objects > 0
            || num_unsaved_files > 0)
    {
        send_telemetry_sync_from_app_ctx!(TelemetryEvent::LogOutModalShown, app);
        let mut button_data = vec![ModalButton::for_app("Yes, log out", |ctx| {
            log_out(ctx);
        })];

        let mut info_text_vec: Vec<String> = vec![];
        if num_long_running_commands > 0 {
            let plural = if num_long_running_commands > 1 {
                "processes"
            } else {
                "process"
            };
            info_text_vec.push(format!(
                "You have {num_long_running_commands} {plural} running."
            ));

            button_data.push(ModalButton::for_app("Show running processes", move |ctx| {
                send_telemetry_sync_from_app_ctx!(
                    TelemetryEvent::LogOutModalCancel { nav_palette: true },
                    ctx
                );
                let windowing_model = ctx.windows();
                let window_id = if let Some(active_window_id) = windowing_model.active_window() {
                    active_window_id
                } else if let Some(window_id) = ctx.window_ids().collect_vec().first() {
                    let window_id = *window_id;
                    windowing_model.show_window_and_focus_app(window_id);
                    window_id
                } else {
                    return;
                };

                if let Some(workspaces) = ctx.views_of_type::<Workspace>(window_id) {
                    if let Some(handle) = workspaces.first() {
                        ctx.dispatch_typed_action_for_view(
                            window_id,
                            handle.id(),
                            &WorkspaceAction::OpenPalette {
                                mode: PaletteMode::Navigation,
                                source: PaletteSource::LogOutModal,
                                query: Some("running".to_owned()),
                            },
                        );
                    }
                }
            }))
        }

        if num_shared_sessions > 0 {
            let plural = if num_shared_sessions > 1 {
                "sessions"
            } else {
                "session"
            };
            info_text_vec.push(format!("You have {num_shared_sessions} shared {plural}."));
        }

        if num_unsaved_objects > 0 {
            let plural = if num_unsaved_objects > 1 {
                "objects"
            } else {
                "object"
            };
            info_text_vec.push(format!(
                "You have {num_unsaved_objects} unsynced Warp Drive {plural}. \
            Logging out will cause you to lose the {plural}."
            ));
        }

        if num_unsaved_files > 0 {
            let plural = if num_unsaved_files > 1 {
                "files"
            } else {
                "file"
            };
            info_text_vec.push(format!(
                "You have {num_unsaved_files} unsaved {plural}. \
            Logging out will cause you to lose the {plural}."
            ));
        }

        button_data.push(ModalButton::for_app("Cancel", move |_ctx| {
            send_telemetry_sync_from_app_ctx!(
                TelemetryEvent::LogOutModalCancel { nav_palette: false },
                ctx
            );
        }));

        let alert_data = AlertDialogWithCallbacks::for_app(
            "Log out?",
            info_text_vec.join("\n"),
            button_data,
            move |ctx| {
                GeneralSettings::handle(ctx).update(ctx, |general_settings, ctx| {
                    report_if_error!(general_settings
                        .show_warning_before_quitting
                        .toggle_and_save_value(ctx));
                });
            },
        );

        // On mac, we show the native platform modal. On platforms that don't support a native modal,
        // we show the custom warp modal.
        if cfg!(all(not(target_family = "wasm"), target_os = "macos")) {
            app.show_native_platform_modal(alert_data);
        } else {
            let sessions = SessionNavigationData::all_sessions(app).collect_vec();
            let sessions_summary = RunningSessionSummary::new(&sessions);
            focus_running_window_and_show_native_modal(sessions_summary, alert_data, app);
        }
    } else {
        log_out(app);
    }
}

// Log out the user, clears workspace state, stops running processes, and deletes database.
pub fn log_out(app: &mut AppContext) {
    send_telemetry_sync_from_app_ctx!(TelemetryEvent::LogOut, app);

    let global_resource_handles = GlobalResourceHandlesProvider::as_ref(app).get();

    // As part of Logout v0, we remove sqlite3 so sessions and cloud objects don't persist between accounts.
    // TODO: Implement per-user scoping of sqlite3.
    persistence::remove(&global_resource_handles.model_event_sender);

    AuthManager::handle(app).update(app, |auth_manager, ctx| {
        auth_manager.log_out(ctx);
    });
    // Dispatch action on root view of every open window so the state can be updated
    // correctly.
    let window_ids = app.window_ids().collect_vec();
    for window_id in window_ids {
        if let Some(root_view_id) = app.root_view_id(window_id) {
            app.dispatch_action(
                window_id,
                &[root_view_id],
                "root_view:log_out",
                &(),
                log::Level::Info,
            );
        }
    }

    #[cfg(target_family = "wasm")]
    crate::platform::wasm::emit_event(crate::platform::wasm::WarpEvent::LoggedOut);
}

