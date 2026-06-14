use rift_core::user_preferences::GetUserPreferences as _;
use riftui::{App, SingletonEntity};

use super::{RootView, HAS_COMPLETED_ONBOARDING_KEY};
use crate::auth::auth_manager::AuthManager;
use crate::auth::AuthStateProvider;

fn initialize_app(app: &mut App) {
    app.update(crate::settings::init_and_register_user_preferences);
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
}

fn set_local_onboarding_completed(app: &mut App, completed: bool) {
    app.update(|ctx| {
        ctx.private_user_preferences()
            .write_value(
                HAS_COMPLETED_ONBOARDING_KEY,
                serde_json::to_string(&completed).unwrap(),
            )
            .unwrap();
    });
}

// NOTE: the two `test_sync_flips_server_is_onboarded…` / `…noop_when_local_
// onboarding_not_completed` tests were removed during the auth/login strip.
// They drove the server-side `is_onboarded` flip, but login was removed and the
// local user is permanently onboarded, so `set_is_onboarded(false)` is now a
// no-op and `is_onboarded()` is a constant `Some(true)` — there is no server
// flag left to flip. The remaining test documents that permanent state.

/// The server-side flag should also be left untouched when it is already set,
/// even if local onboarding is complete — avoids redundant server calls.
#[test]
fn test_sync_noop_when_already_onboarded_on_server() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        set_local_onboarding_completed(&mut app, true);
        app.update(|ctx| {
            // User::test() defaults to is_onboarded = true; assert that and
            // leave it in place.
            assert_eq!(
                AuthStateProvider::as_ref(ctx).get().is_onboarded(),
                Some(true)
            );
        });

        app.update(|ctx| {
            let auth_state = AuthStateProvider::as_ref(ctx).get().clone();
            RootView::sync_local_onboarding_to_server(&auth_state, ctx);
        });

        app.read(|ctx| {
            assert_eq!(
                AuthStateProvider::as_ref(ctx).get().is_onboarded(),
                Some(true)
            );
        });
    });
}
