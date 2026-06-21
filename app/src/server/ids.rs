//! Local, offline ID types. Decoupled from any cloud/GraphQL machinery. The
//! terminal only needs `SyncId` (block identity), `ServerId`, and `ClientId`.

use std::fmt;

use itertools::Itertools;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// Convert ID types into and from a hashed string form.
pub trait HashableId: Sized + Send + Sync {
    fn to_hash(&self) -> String;
    fn from_hash(hash: &str) -> Option<Self>;
}

/// A client-generated unique identifier.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ClientId(Uuid);

impl HashableId for ClientId {
    fn to_hash(&self) -> String {
        self.to_string()
    }

    fn from_hash(hash: &str) -> Option<ClientId> {
        hash.strip_prefix("Client-")
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(ClientId)
    }
}

impl ClientId {
    pub fn new() -> ClientId {
        Self(Uuid::new_v4())
    }

    pub fn sqlite_hash(&self) -> String {
        self.to_string()
    }
}

impl Default for ClientId {
    fn default() -> Self {
        ClientId::new()
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Client-{}", self.0)
    }
}

impl From<String> for ClientId {
    fn from(s: String) -> Self {
        ClientId::from_hash(&s).unwrap_or_default()
    }
}

/// The value stored in the database for a given object id.
pub type ObjectUid = String;

/// ID of an object that may be local (client-generated) or, historically,
/// server-assigned. Rift is offline, so in practice these are `ClientId`s.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum SyncId {
    ClientId(ClientId),
    ServerId(ServerId),
}

impl SyncId {
    pub fn uid(&self) -> ObjectUid {
        match self {
            Self::ClientId(id) => id.to_string(),
            Self::ServerId(id) => id.uid(),
        }
    }

    pub fn into_server(self) -> Option<ServerId> {
        match self {
            Self::ServerId(id) => Some(id),
            Self::ClientId(_) => None,
        }
    }

    pub fn into_client(self) -> Option<ClientId> {
        match self {
            Self::ServerId(_) => None,
            Self::ClientId(id) => Some(id),
        }
    }
}

impl settings_value::SettingsValue for SyncId {}

impl fmt::Display for SyncId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ServerId(id) => id.fmt(f),
            Self::ClientId(id) => id.fmt(f),
        }
    }
}

impl From<ServerId> for SyncId {
    fn from(id: ServerId) -> SyncId {
        SyncId::ServerId(id)
    }
}

impl Serialize for SyncId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SyncId::ServerId(server_id) => server_id.serialize(serializer),
            SyncId::ClientId(client_id) => client_id.to_hash().serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SyncId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        if let Some(hashed) = ClientId::from_hash(s.as_str()) {
            Ok(SyncId::ClientId(hashed))
        } else {
            Ok(SyncId::ServerId(ServerId::from_string_lossy(s)))
        }
    }
}

/// Length of a `ServerId` (kept for backwards-compat with persisted ids).
const SERVER_ID_LENGTH: usize = 22;

/// A fixed-length string identifier. Retained for backwards-compatibility with
/// previously-persisted ids.
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq)]
pub struct ServerId([char; SERVER_ID_LENGTH]);

#[derive(Debug, thiserror::Error)]
pub enum ParseServerIdError {
    #[error("ServerId must be exactly {SERVER_ID_LENGTH} characters, got {len}")]
    InvalidLength { len: usize },
}

impl ServerId {
    /// Convert a string to a `ServerId`, padding/truncating to the fixed length.
    pub fn from_string_lossy(id: impl AsRef<str>) -> Self {
        let id = id.as_ref();
        Self::try_from(id).unwrap_or_else(|err| {
            if cfg!(debug_assertions) {
                panic!("{err}");
            }
            let normalized = Self::normalize_id_str(id, 0);
            Self::try_from(normalized).expect("id should convert")
        })
    }

    fn normalize_id_str(input: &str, prefix_length: usize) -> String {
        let available_len = SERVER_ID_LENGTH - prefix_length;
        let truncated = if input.len() > available_len {
            &input[input.len() - available_len..]
        } else {
            input
        };
        format!("{truncated:0>available_len$}")
    }

    pub fn uid(&self) -> ObjectUid {
        (*self).into()
    }
}

impl TryFrom<&str> for ServerId {
    type Error = ParseServerIdError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.chars().collect_array() {
            Some(chars) => Ok(Self(chars)),
            None => Err(ParseServerIdError::InvalidLength {
                len: s.chars().count(),
            }),
        }
    }
}

impl TryFrom<String> for ServerId {
    type Error = ParseServerIdError;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::try_from(id.as_str())
    }
}

#[cfg(any(test, feature = "test-util"))]
impl From<i64> for ServerId {
    fn from(id: i64) -> Self {
        let prefix = "test_uid";
        let id_str = id.abs().to_string();
        let normalized = format!(
            "{}{}",
            prefix,
            Self::normalize_id_str(&id_str, prefix.len())
        );
        Self::try_from(normalized).expect("normalized string should always be valid")
    }
}

impl From<ServerId> for String {
    fn from(id: ServerId) -> String {
        String::from_iter(id.0)
    }
}

impl Serialize for ServerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s: String = (*self).into();
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for ServerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        ServerId::try_from(s.as_str()).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;
        for ch in self.0.iter() {
            f.write_char(*ch)?;
        }
        Ok(())
    }
}

impl fmt::Debug for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServerId({self})")
    }
}

/// server_id_traits generates `From`/`Display`/`HashableId` impls for newtype
/// aliases over `ServerId`. Kept for any in-crate id aliases.
#[macro_export]
macro_rules! server_id_traits {
    ($t:ty, $prefix:literal) => {
        #[cfg(test)]
        impl From<i64> for $t {
            fn from(id: i64) -> Self {
                Self(id.into())
            }
        }

        impl From<String> for $t {
            fn from(id: String) -> Self {
                Self($crate::server::ids::ServerId::from_string_lossy(id))
            }
        }

        impl From<$t> for String {
            fn from(id: $t) -> String {
                id.0.into()
            }
        }

        impl std::fmt::Display for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                write!(f, "{}", self.0)
            }
        }

        impl From<$t> for $crate::server::ids::ServerId {
            fn from(id: $t) -> Self {
                id.0
            }
        }

        impl $crate::server::ids::HashableId for $t {
            fn to_hash(&self) -> String {
                format!("{}-{}", $prefix, self)
            }

            fn from_hash(hash: &str) -> Option<$t> {
                hash.strip_prefix(&format!("{}-", $prefix))
                    .map(|s| s.to_string().into())
            }
        }

        impl From<$crate::server::ids::ServerId> for $t {
            fn from(id: $crate::server::ids::ServerId) -> Self {
                Self(id)
            }
        }
    };
}
