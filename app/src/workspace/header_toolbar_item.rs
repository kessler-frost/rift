use riftui::{AppContext, SingletonEntity};
use serde::{Deserialize, Serialize};

use crate::features::FeatureFlag;
use crate::ui_components::icons::Icon;
use crate::workspace::tab_settings::TabSettings;

/// A configurable item in the vertical tabs header toolbar.
///
/// Each variant represents a panel toggle button that can be placed on either
/// the left or right side of the toolbar. The side determines which side of the
/// main content area the panel opens on.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(rename_all = "snake_case")]
pub enum HeaderToolbarItemKind {
    TabsPanel,
}

impl HeaderToolbarItemKind {
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::TabsPanel => "Tabs Panel",
        }
    }

    pub fn icon(&self) -> Icon {
        match self {
            Self::TabsPanel => Icon::Menu,
        }
    }

    /// Whether this item is supported on the current platform/configuration
    /// (feature flags, compile-time features).
    pub fn is_supported(&self, app: &AppContext) -> bool {
        match self {
            Self::TabsPanel => {
                FeatureFlag::VerticalTabs.is_enabled()
                    && *TabSettings::as_ref(app).use_vertical_tabs
            }
        }
    }

    pub fn default_left() -> Vec<Self> {
        vec![Self::TabsPanel]
    }

    pub fn default_right() -> Vec<Self> {
        vec![]
    }

    /// All toolbar item variants (availability filtering is done at the call site).
    pub fn all_items() -> Vec<Self> {
        vec![Self::TabsPanel]
    }
}
