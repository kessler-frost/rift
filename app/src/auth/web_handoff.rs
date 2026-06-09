use anyhow::anyhow;
use riftui::ui_components::components::UiComponent as _;
use riftui::{AppContext, Element, Entity, SingletonEntity, View, ViewContext};
use wasm_bindgen::prelude::*;

use super::auth_manager::{AuthManager, AuthManagerEvent};
use crate::auth::auth_view_modal::AuthRedirectPayload;
use crate::auth::login_error_modal::LoginErrorModal;
use crate::platform::wasm::{user_handoff, AuthHandoffError};
use crate::report_error;

#[wasm_bindgen]
extern "C" {}

pub struct WebHandoffView {
    state: HandoffState,
}

#[derive(Debug, Clone)]
pub enum WebHandoffEvent {
    /// Web auth handoff is unavailable, so the app should fall back to the login screen.
    Unsupported,
}

enum HandoffState {
    /// We have retrieved a refresh token from the host application and are fetching the user's
    /// profile.
    LoadingFromHost,
    /// We are deriving authentication from an ambient browser session cookie.
    LoadingFromSessionCookie,
    /// There was an error using the provided refresh token. In practice, this should never happen,
    /// as the host application would have recently used the token successfully.
    Failed,
}

impl WebHandoffView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        ctx.subscribe_to_model(&AuthManager::handle(ctx), |me, _, event, ctx| {
            me.handle_auth_manager_event(event, ctx);
        });

        Self {
            state: HandoffState::Failed,
        }
    }

    fn import_user_from_session_cookie(&mut self, ctx: &mut ViewContext<Self>) {
        log::debug!("Attempting to derive auth from browser session cookie");
        AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
            auth_manager.initialize_user_from_session_cookie(ctx);
        });
        self.state = HandoffState::LoadingFromSessionCookie;
    }

    /// Import the authenticated user from the host React app, if available.
    pub fn import_user(&mut self, ctx: &mut ViewContext<Self>) {
        match user_handoff() {
            Ok(Some(_refresh_token)) => {
                log::debug!("Attempting to retrieve refresh token from host app");
                let payload = AuthRedirectPayload;

                AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                    // No need to validate state for web handoff, since everything's happening
                    // on same web page.
                    auth_manager.initialize_user_from_auth_payload(payload, false, ctx);
                });
                self.state = HandoffState::LoadingFromHost;
            }
            Ok(None) => {
                self.import_user_from_session_cookie(ctx);
            }
            Err(AuthHandoffError::Unsupported) => {
                self.import_user_from_session_cookie(ctx);
            }
            Err(AuthHandoffError::Unexpected(err)) => {
                report_error!(anyhow!("Web user handoff failed: {err:?}"));
                self.state = HandoffState::Failed;
                ctx.notify();
            }
        }
        ctx.notify();
    }

    fn handle_auth_manager_event(&mut self, event: &AuthManagerEvent, _ctx: &mut ViewContext<Self>) {
        // Offline: the only auth event is `SkippedLogin`. The cloud auth-complete/failed events that
        // drove the web handoff flow no longer exist.
        match event {
            AuthManagerEvent::SkippedLogin => {}
        }
    }
}

impl Entity for WebHandoffView {
    type Event = WebHandoffEvent;
}

impl View for WebHandoffView {
    fn ui_name() -> &'static str {
        "WebHandoffView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let label = match &self.state {
            HandoffState::LoadingFromHost | HandoffState::LoadingFromSessionCookie => "Loading...",
            HandoffState::Failed => "Error authenticating - please refresh the page",
        };

        LoginErrorModal::new(app)
            .with_detail(label)
            .build()
            .finish()
    }
}
