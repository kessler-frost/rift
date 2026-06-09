//! Local, offline auth data types.
//!
//! Rift is a fully-offline terminal with no cloud account, login, or token refresh. These types are
//! minimal local stand-ins for the user identifier, credentials, and user/persisted-user data.
//! They carry no network code; they exist only so the handful of caller files that reference a
//! user identifier, credentials, or persisted-user placeholder keep compiling.
//!
//! The submodules `user_uid`, `credentials`, and `user` are re-exported from `auth/mod.rs` to
//! preserve the old `crate::auth::{user_uid, credentials, user}` paths.

#[cfg(feature = "crash_reporting")]
use rift_core::user_preferences::GetUserPreferences;
#[cfg(feature = "crash_reporting")]
use uuid::Uuid;

/// Key used to persist the anonymous id to user defaults. Kept identical to the prior
/// implementation so an existing persisted id is reused.
#[cfg(feature = "crash_reporting")]
const ANONYMOUS_ID_KEY: &str = "ExperimentId";

/// Reads the persisted anonymous id from user defaults, if it exists and is a valid uuid.
#[cfg(feature = "crash_reporting")]
fn get_persisted_anonymous_id(ctx: &dyn GetUserPreferences) -> Option<Uuid> {
    let anonymous_id = ctx
        .private_user_preferences()
        .read_value(ANONYMOUS_ID_KEY)
        .unwrap_or_default()?;
    Uuid::parse_str(&anonymous_id).ok()
}

/// Gets the persisted anonymous id if possible, otherwise generates a new uuid and saves it.
/// Fully local: this writes only to on-disk user preferences and performs no network access.
#[cfg(feature = "crash_reporting")]
pub fn get_or_create_anonymous_id(ctx: &dyn GetUserPreferences) -> Uuid {
    get_persisted_anonymous_id(ctx).unwrap_or_else(|| {
        let uuid = Uuid::new_v4();
        let _ = ctx
            .private_user_preferences()
            .write_value(ANONYMOUS_ID_KEY, uuid.to_string());
        uuid
    })
}

/// User identifier types.
pub mod user_uid {
    use std::fmt;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Test user uid constant, retained for test helpers.
    #[cfg_attr(
        not(any(test, feature = "integration_tests", feature = "test-util")),
        allow(dead_code)
    )]
    pub const TEST_USER_UID: &str = "test_user_uid";

    /// `UserUid` is the unique identifier for the (single, local) user.
    #[derive(Clone, PartialEq, Eq, Hash, Default)]
    pub struct UserUid(String);

    impl UserUid {
        pub fn new(uid: &str) -> Self {
            Self(uid.to_owned())
        }

        pub fn as_str(&self) -> &str {
            self.0.as_str()
        }

        pub fn as_string(&self) -> String {
            self.0.clone()
        }
    }

    impl fmt::Display for UserUid {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.as_str())
        }
    }

    impl fmt::Debug for UserUid {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "UserUid({})", self.as_str())
        }
    }

    impl Serialize for UserUid {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(self.as_str())
        }
    }

    impl<'de> Deserialize<'de> for UserUid {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            String::deserialize(deserializer).map(|s| UserUid::new(&s))
        }
    }
}

/// Credential types. In the offline build no real credentials ever exist; these are minimal local
/// placeholders so that the auth state / paste-auth UI keep compiling. Nothing in the default build
/// ever constructs them (only used as types in the auth-state signatures), hence the module-wide
/// `allow(dead_code)`.
#[allow(dead_code)]
pub mod credentials {
    use super::user::FirebaseAuthTokens;

    /// Represents the different ways a user could authenticate.
    #[derive(Clone, Debug)]
    pub enum Credentials {
        Firebase(FirebaseAuthTokens),
        ApiKey { key: String },
    }

    /// Long-lived refresh token placeholder.
    #[derive(Debug, Clone)]
    pub struct RefreshToken(String);

    impl RefreshToken {
        pub fn new(token: impl Into<String>) -> Self {
            Self(token.into())
        }

        pub fn get(&self) -> &str {
            self.0.as_str()
        }
    }
}

/// User types. In the offline build there is only the single local user; these are minimal local
/// placeholders that nothing ever constructs (only used as types in the auth-state signatures),
/// hence the module-wide `allow(dead_code)`.
#[allow(dead_code)]
pub mod user {
    

    /// Firebase auth tokens placeholder.
    #[derive(Clone, Debug)]
    pub struct FirebaseAuthTokens {
        pub id_token: String,
        pub refresh_token: String,
    }

    /// The (single, local) user. Carries no real account data.
    #[derive(Clone, Debug, Default)]
    pub struct User;

    /// Persisted-user placeholder. In the offline build there is never a cloud user to persist.
    pub mod persistence {
        #[derive(Clone, Debug, Default)]
        pub struct PersistedUser;
    }
}
