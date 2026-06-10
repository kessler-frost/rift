//! Default new-session behavior: plain terminal or a user tab config.
//!
//! Extracted from the removed AI settings group; the agent/cloud session
//! modes are gone, so this is the minimal surviving surface.

use riftui::{AppContext, SingletonEntity};
use settings::{
    define_settings_group, RespectUserSyncSetting, Setting, SupportedPlatforms, SyncToCloud,
};
use strum_macros::EnumIter;

#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    EnumIter,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Default mode for new sessions.",
    rename_all = "snake_case"
)]
pub enum DefaultSessionMode {
    /// New sessions start in the terminal mode (default).
    #[default]
    Terminal,
    /// New sessions open a user-defined tab config.
    /// The specific config is identified by the companion `default_tab_config_path` setting.
    TabConfig,
}

settings::macros::implement_setting_for_enum!(
    DefaultSessionMode,
    SessionModeSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    private: false,
    toml_path: "general.default_session_mode",
    description: "The default mode for new terminal sessions.",
);

impl DefaultSessionMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            DefaultSessionMode::Terminal => "Terminal",
            DefaultSessionMode::TabConfig => "Tab Config",
        }
    }
}

define_settings_group!(SessionModeSettings, settings: [
    // The raw stored default mode for new sessions. Use `default_session_mode()` to retrieve the
    // effective mode.
    default_session_mode_internal: DefaultSessionMode,
    // The file path of the tab config used when default_session_mode_internal is TabConfig.
    default_tab_config_path: DefaultTabConfigPath {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "general.default_tab_config_path",
        description: "Path of the tab config used when the default session mode is tab_config.",
    },
]);

impl SessionModeSettings {
    pub fn default_session_mode(&self) -> DefaultSessionMode {
        *self.default_session_mode_internal.value()
    }

    /// Returns the stored default tab config path (only meaningful when mode is `TabConfig`).
    pub fn default_tab_config_path(&self) -> &str {
        &self.default_tab_config_path
    }

    /// Looks up the `TabConfig` matching the stored `default_tab_config_path`.
    /// Returns `None` if the path is empty or no loaded config matches.
    pub fn resolved_default_tab_config(
        &self,
        app: &AppContext,
    ) -> Option<crate::tab_configs::TabConfig> {
        let path_str = self.default_tab_config_path.as_str();
        if path_str.is_empty() {
            return None;
        }
        let path = std::path::Path::new(path_str);
        crate::user_config::RiftConfig::as_ref(app)
            .tab_configs()
            .iter()
            .find(|config| config.source_path.as_deref().is_some_and(|p| p == path))
            .cloned()
    }
}
