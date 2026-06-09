use std::sync::Arc;

use remote_server::auth::RemoteServerAuthContext;
use riftui::r#async::BoxFuture;

use crate::auth::auth_state::AuthState;

/// Builds the app-wide auth context used by remote-server connections.
///
/// Rift is fully offline: there is no cloud account or access token to fetch, so the access-token
/// callback always yields `None`. Identity is derived from the local user/anonymous id only.
pub fn server_api_auth_context(
    auth_state: Arc<AuthState>,
    crash_reporting_enabled: bool,
) -> RemoteServerAuthContext {
    let identity_auth_state = auth_state.clone();
    let user_id = auth_state
        .user_id()
        .map(|uid| uid.as_string())
        .unwrap_or_default();
    let user_email = auth_state.user_email().unwrap_or_default();

    RemoteServerAuthContext::new(
        // No cloud access token exists in the offline build.
        move || -> BoxFuture<'static, Option<String>> { Box::pin(async { None }) },
        move || remote_server_identity_key(&identity_auth_state),
        user_id,
        user_email,
        crash_reporting_enabled,
    )
}

fn remote_server_identity_key(auth_state: &AuthState) -> String {
    auth_state
        .user_id()
        .map(|uid| uid.as_string())
        .unwrap_or_else(|| auth_state.anonymous_id())
}
