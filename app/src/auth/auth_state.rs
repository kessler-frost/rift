//! Local, offline auth state.
//!
//! Rift is a fully-offline terminal: there is no cloud account, login, or token refresh. This
//! module is the local, offline auth-state implementation that the ~30 caller files use unchanged.
//!
//! The terminal is always treated as a single local user. Accordingly:
//! * `is_anonymous_or_logged_out()` returns `false` (treated as a normal local user — no login
//!   walls).
//! * `is_logged_in()` returns `true`.
//! * `is_onboarded()` returns `Some(true)`.
//! * `user_id()` / `username_for_display()` return a fixed local default.
//!
//! The `User`/`Credentials` data types are local placeholders (see `local_types.rs`); nothing in
//! this module ever performs a network request.

use std::sync::Arc;

use riftui::{AppContext, Entity, SingletonEntity};

use super::credentials::Credentials;
use super::user::persistence::PersistedUser;
use super::user::User;
use super::UserUid;

/// The fixed local user identifier used throughout the offline build.
const LOCAL_USER_ID: &str = "local_user";
/// The fixed local display name used throughout the offline build.
const LOCAL_USER_DISPLAY_NAME: &str = "Local User";

/// Describes what persistence action to take. In the offline build there is never any cloud user
/// to persist, so this is always `DoNothing`.
pub enum PersistAction {
    Persist(Box<PersistedUser>),
    Remove,
    DoNothing,
}

/// AuthState holds information about the (single, local) user. In the offline build it carries no
/// real credentials and performs no network access. Methods return fixed local-user values.
pub struct AuthState {
    anonymous_id: uuid::Uuid,
}

impl AuthState {
    fn new() -> Self {
        Self {
            anonymous_id: uuid::Uuid::new_v4(),
        }
    }

    /// Creates and initializes the local auth state. The `api_key` argument is ignored in the
    /// offline build.
    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    pub fn initialize(_ctx: &AppContext, _api_key: Option<String>) -> Self {
        Self::new()
    }

    #[cfg(any(test, feature = "integration_tests", feature = "test-util"))]
    pub fn new_for_test() -> Self {
        Self::new()
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn new_logged_out_for_test() -> Self {
        Self::new()
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn new_anonymous_for_test() -> Self {
        Self::new()
    }

    /// Offline: there is never a cloud user to persist.
    pub fn persist_action(&self) -> PersistAction {
        PersistAction::DoNothing
    }

    /// No-op: the local user cannot be changed.
    pub fn set_user(&self, _user: Option<User>) {}

    /// Offline: no real credentials exist.
    pub fn credentials(&self) -> Option<Credentials> {
        None
    }

    /// No-op in the offline build.
    pub fn set_credentials(&self, _credentials: Option<Credentials>) {}

    /// No-op: remote-server cloud auth handshake is not used in the offline build.
    #[cfg(any(not(target_family = "wasm"), test, feature = "test-util"))]
    pub fn apply_remote_server_auth_context(
        &self,
        _auth_token: String,
        _user_id: String,
        _user_email: String,
    ) {
    }

    /// No-op in the offline build.
    #[cfg(any(not(target_family = "wasm"), test, feature = "test-util"))]
    pub fn set_remote_server_bearer_token(&self, _auth_token: String) {}

    /// The local user is always considered logged in.
    pub fn is_logged_in(&self) -> bool {
        true
    }

    /// The local user is always treated as a normal (non-anonymous, logged-in) user.
    pub fn is_anonymous_or_logged_out(&self) -> bool {
        false
    }

    /// Offline: no access token exists.
    pub fn get_access_token_ignoring_validity(&self) -> Option<String> {
        None
    }

    pub fn username_for_display(&self) -> Option<String> {
        Some(LOCAL_USER_DISPLAY_NAME.to_owned())
    }

    pub fn display_name(&self) -> Option<String> {
        Some(LOCAL_USER_DISPLAY_NAME.to_owned())
    }

    pub fn user_email(&self) -> Option<String> {
        Some(String::new())
    }

    /// The local user is always onboarded.
    pub fn is_onboarded(&self) -> Option<bool> {
        Some(true)
    }

    pub fn user_email_domain(&self) -> Option<String> {
        Some(String::new())
    }

    /// The local user is never anonymous.
    pub fn is_user_anonymous(&self) -> Option<bool> {
        Some(false)
    }

    pub fn is_user_web_anonymous_user(&self) -> Option<bool> {
        Some(false)
    }

    pub fn is_anonymous_user_feature_gated(&self) -> Option<bool> {
        Some(false)
    }

    pub fn user_photo_url(&self) -> Option<String> {
        None
    }

    /// The local user never needs an SSO link.
    pub fn needs_sso_link(&self) -> Option<bool> {
        Some(false)
    }

    /// No-op in the offline build (the local user is always onboarded).
    pub fn set_is_onboarded(&self, _is_onboarded: bool) {}

    /// Returns the fixed local user id.
    pub fn user_id(&self) -> Option<UserUid> {
        Some(UserUid::new(LOCAL_USER_ID))
    }

    pub fn anonymous_id(&self) -> String {
        self.anonymous_id.to_string()
    }

    /// Offline: reauth is never required.
    pub fn needs_reauth(&self) -> bool {
        false
    }

    /// No-op: reauth is never required in the offline build. Always returns `false`
    /// (state never transitions to "needs reauth").
    pub fn set_needs_reauth(&self, _new_needs_reauth: bool) -> bool {
        false
    }

    pub fn anonymous_user_renotification_block_expired(
        &self,
        _last_time_opt: Option<String>,
    ) -> bool {
        false
    }

    pub fn is_on_work_domain(&self) -> Option<bool> {
        Some(false)
    }

    pub fn is_api_key_authenticated(&self) -> bool {
        false
    }

    pub fn api_key(&self) -> Option<String> {
        None
    }

    pub fn is_service_account(&self) -> bool {
        false
    }

    pub fn global_skills(&self) -> Vec<String> {
        Vec::new()
    }
}

/// AuthStateProvider is a singleton model which provides a reference to the global AuthState.
pub struct AuthStateProvider {
    auth_state: Arc<AuthState>,
}

impl AuthStateProvider {
    pub fn new(auth_state: Arc<AuthState>) -> Self {
        Self { auth_state }
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn new_for_test() -> Self {
        Self {
            auth_state: Arc::new(AuthState::new_for_test()),
        }
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn new_logged_out_for_test() -> Self {
        Self {
            auth_state: Arc::new(AuthState::new_logged_out_for_test()),
        }
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn new_anonymous_for_test() -> Self {
        Self {
            auth_state: Arc::new(AuthState::new_anonymous_for_test()),
        }
    }

    pub fn get(&self) -> &Arc<AuthState> {
        &self.auth_state
    }
}

impl Entity for AuthStateProvider {
    type Event = ();
}

impl SingletonEntity for AuthStateProvider {}
