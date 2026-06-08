// Most of this module is dead code on web as it is not possible to retrieve
// external secrets from the browser.
#![cfg_attr(target_family = "wasm", allow(dead_code, unused_variables))]

pub use cloud_object_models::ExternalSecret;

use crate::ui_components::icons::Icon;

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
