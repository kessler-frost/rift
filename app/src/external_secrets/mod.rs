// Most of this module is dead code on web as it is not possible to retrieve
// external secrets from the browser.
#![cfg_attr(target_family = "wasm", allow(dead_code, unused_variables))]

use serde::{Deserialize, Serialize};

use crate::ui_components::icons::Icon;

/// A reference to a secret stored in an external password manager.
///
/// This was previously sourced from `cloud_object_models::ExternalSecret`. It is a pure local data
/// type with no network dependency, so it is now defined directly in the app.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ExternalSecret {
    OnePassword(OnePasswordSecret),
    LastPass(LastPassSecret),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OnePasswordSecret {
    pub name: String,
    pub reference: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LastPassSecret {
    pub name: String,
    pub reference: String,
}

impl ExternalSecret {
    pub fn get_display_name(&self) -> String {
        match self {
            ExternalSecret::OnePassword(secret) => secret.name.clone(),
            ExternalSecret::LastPass(secret) => secret.name.clone(),
        }
    }
}

pub trait ExternalSecretManager {
    fn icon(&self) -> Icon;
}

impl ExternalSecretManager for ExternalSecret {
    fn icon(&self) -> Icon {
        match self {
            ExternalSecret::OnePassword(_) => Icon::OnePassword,
            ExternalSecret::LastPass(_) => Icon::LastPass,
        }
    }
}
