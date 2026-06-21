pub mod auth_manager;
pub mod auth_state;
pub mod local_types;
// `auth_state` is now a local, offline implementation (see `auth_state.rs`). The data types
// (User/Credentials/UserUid) are now pure local placeholders (see `local_types.rs`) and carry no
// network code.
pub use auth_state::AuthStateProvider;
pub use local_types::{credentials, user, user_uid};
use riftui::AppContext;
pub use user_uid::UserUid;

pub fn init(_app: &mut AppContext) {
    // Login/auth UI was removed; the local user is always signed in.
}
