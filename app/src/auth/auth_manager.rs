//! Offline auth manager stub.
//!
//! Rift is a fully-offline terminal with no cloud account, login, token refresh, or anonymous-user
//! provisioning. This module retains the `AuthManager` singleton and the public method surface that
//! the rest of the app calls, but every method that used to talk to the server (`fetch_user`,
//! `create_anonymous_user`, token minting, onboarding sync, telemetry-login flush, etc.) is now a
//! local no-op.

use riftui::{Entity, ModelContext, SingletonEntity};
use uuid::Uuid;

use super::auth_view_modal::{AuthRedirectPayload, AuthViewVariant};
use crate::channel::ChannelState;
use crate::server::telemetry::AnonymousUserSignupEntrypoint;

#[derive(Debug)]
pub enum AuthManagerEvent {
    /// The user chose to skip login entirely. This is the only event ever emitted in the offline
    /// build (login itself is a no-op, so there is never an auth-complete/failed/override event).
    SkippedLogin,
}

pub type LoginGatedFeature = &'static str;

type URLConstructorCallback = Box<dyn FnOnce(Option<&str>) -> String>;

/// Offline auth manager. Holds no server clients and performs no network access.
pub struct AuthManager {
    pending_auth_state: Option<String>,
}

impl AuthManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {
            pending_auth_state: None,
        }
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn new_for_test(_ctx: &mut ModelContext<Self>) -> Self {
        Self {
            pending_auth_state: None,
        }
    }

    /// Offline no-op: there is no server to fetch a user from. The local user is always present.
    pub fn initialize_user_from_auth_payload(
        &mut self,
        _auth_payload: AuthRedirectPayload,
        _enforce_state_validation: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    /// Offline no-op.
    pub fn resume_interrupted_auth_payload(
        &mut self,
        _auth_payload: AuthRedirectPayload,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    /// Offline no-op.
    #[cfg(target_family = "wasm")]
    pub fn initialize_user_from_session_cookie(&self, _ctx: &mut ModelContext<Self>) {}

    /// Offline no-op: there is no server to refresh user state from.
    pub fn refresh_user(&self, _ctx: &mut ModelContext<Self>) {}

    /// Offline no-op: logout simply clears any dangling pending auth state.
    pub(super) fn log_out(&mut self, _ctx: &mut ModelContext<Self>) {
        self.pending_auth_state = None;
    }

    /// Offline no-op: anonymous users are not created in the offline build.
    pub fn create_anonymous_user(
        &self,
        _referral_code: Option<String>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    /// Offline no-op: there are no login-gated features in the offline build (the local user is
    /// always treated as fully logged in).
    pub fn attempt_login_gated_feature(
        &self,
        _feature: LoginGatedFeature,
        _auth_view_variant: AuthViewVariant,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    /// Offline no-op.
    pub fn initiate_anonymous_user_linking(
        &self,
        _entrypoint: AnonymousUserSignupEntrypoint,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    /// Opens a URL. In the offline build there is never an anonymous token to attach, so this
    /// simply opens the constructed URL.
    pub fn open_url_maybe_with_anonymous_token(
        &self,
        ctx: &mut ModelContext<Self>,
        construct_url: URLConstructorCallback,
    ) {
        let url: String = construct_url(None);
        ctx.open_url(&url);
    }

    /// Offline no-op: there is no anonymous user linking URL to copy.
    pub fn copy_anonymous_user_linking_url_to_clipboard(&self, _ctx: &mut ModelContext<Self>) {}

    /// Generates a unique state parameter for the (local) URL flow.
    fn generate_auth_state(&mut self) -> String {
        let state = Uuid::new_v4().to_string();
        self.pending_auth_state = Some(state.clone());
        state
    }

    pub fn sign_up_url(&mut self) -> String {
        let state = self.generate_auth_state();
        format!(
            "{}/signup/remote?scheme={}&state={}&public_beta=true",
            ChannelState::server_root_url(),
            ChannelState::url_scheme(),
            state,
        )
    }

    pub fn sign_in_url(&mut self) -> String {
        let state = self.generate_auth_state();
        format!(
            "{}/login/remote?scheme={}&state={}",
            ChannelState::server_root_url(),
            ChannelState::url_scheme(),
            state,
        )
    }

    pub fn link_sso_url(&mut self, email: &str) -> String {
        let state = self.generate_auth_state();
        format!(
            "{}/link_sso?email={}&state={}",
            ChannelState::server_root_url(),
            email,
            state,
        )
    }

    /// Offline no-op: the local user is always onboarded; there is no server to update.
    pub fn set_user_onboarded(&self, _ctx: &mut ModelContext<Self>) {}
}

#[derive(Clone, Debug)]
pub struct PersistedCurrentUserInformation {
    pub email: String,
}

impl Entity for AuthManager {
    type Event = AuthManagerEvent;
}

impl SingletonEntity for AuthManager {}
