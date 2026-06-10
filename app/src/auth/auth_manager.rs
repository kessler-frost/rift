//! Offline auth manager stub.
//!
//! Rift is a fully-offline terminal with no cloud account, login, token refresh, or anonymous-user
//! provisioning. This module retains the `AuthManager` singleton and the public method surface that
//! the rest of the app calls, but every method that used to talk to the server (`fetch_user`,
//! `create_anonymous_user`, token minting, onboarding sync, telemetry-login flush, etc.) is now a
//! local no-op.

use riftui::{Entity, ModelContext, SingletonEntity};


pub type LoginGatedFeature = &'static str;

type URLConstructorCallback = Box<dyn FnOnce(Option<&str>) -> String>;

/// Offline auth manager. Holds no server clients and performs no network access.
pub struct AuthManager;

impl AuthManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn new_for_test(_ctx: &mut ModelContext<Self>) -> Self {
        Self
    }

    /// Offline no-op.
    #[cfg(target_family = "wasm")]
    pub fn initialize_user_from_session_cookie(&self, _ctx: &mut ModelContext<Self>) {}

    /// Offline no-op: there is no server to refresh user state from.
    pub fn refresh_user(&self, _ctx: &mut ModelContext<Self>) {}

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

    /// Offline no-op: the local user is always onboarded; there is no server to update.
    pub fn set_user_onboarded(&self, _ctx: &mut ModelContext<Self>) {}
}

#[derive(Clone, Debug)]
pub struct PersistedCurrentUserInformation {
    pub email: String,
}

impl Entity for AuthManager {
    type Event = ();
}

impl SingletonEntity for AuthManager {}
