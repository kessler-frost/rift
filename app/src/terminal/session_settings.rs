pub mod new_session_shell;
pub mod startup_shell;
pub mod working_directory_config;

use instant::Duration;
use lazy_static::lazy_static;
pub use new_session_shell::*;
use rift_core::settings::macros::define_settings_group;
use rift_core::settings::{RespectUserSyncSetting, SupportedPlatforms, SyncToCloud};
use serde::{Deserialize, Serialize};
pub use startup_shell::*;
pub use working_directory_config::*;

use crate::context_chips::prompt::PromptSelection;

lazy_static! {
    pub static ref DEFAULT_THRESHOLD_FOR_LONG_RUNNING_NOTIFICATION: Duration =
        Duration::from_secs(30);
}

#[derive(
    Copy,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Whether the user has enabled or disabled notifications.",
    rename_all = "snake_case"
)]
pub enum NotificationsMode {
    // User has not been shown notifications banner before or has seen it before but decided not to dismiss it.
    #[schemars(description = "Notifications have not been configured yet.")]
    Unset,

    // User has asked not to be shown notifications banner again.
    #[schemars(description = "The notifications banner has been dismissed.")]
    Dismissed,

    // User has enabled system notifications and wants to receive notifications.
    #[schemars(description = "Notifications are enabled.")]
    Enabled,

    // User had previously enabled notifications, but has now disabled them.
    #[schemars(description = "Notifications are disabled.")]
    Disabled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, settings_value::SettingsValue)]
/**
 * Added [serde(default)] to ensure that new notification settings are backwards compatible with old clients.
 * Otherwise, clients will fail to deserialize existing settings after updating.
 *
 * @see upstream PR 14596/files#diff-90221c7ecae01c6faf8f170158dea3e49d34d40225a306da42ccc76489d1f84cR43-R44
 *
 * Alternative considered: Using Option<bool> fields would have required more
 * complex defaulting logic to set the default value to true.
 *
 */
#[serde(default)]
#[derive(schemars::JsonSchema)]
#[schemars(description = "Notification preferences for terminal events.")]
pub struct NotificationsSettings {
    #[schemars(
        description = "Whether notifications are enabled, disabled, or not yet configured."
    )]
    pub mode: NotificationsMode,

    #[schemars(description = "Whether to notify when a long-running command completes.")]
    pub is_long_running_enabled: bool,
    #[schemars(
        with = "u64",
        description = "Threshold in seconds for long-running command notifications."
    )]
    pub long_running_threshold: Duration,

    /// Legacy. To be combined with `is_needs_attention_enabled` when desktop notifs are unflagged.
    #[schemars(description = "Whether to notify when a password prompt is detected.")]
    pub is_password_prompt_enabled: bool,

    #[schemars(description = "Whether to notify when an agent task completes.")]
    pub is_agent_task_completed_enabled: bool,
    #[schemars(description = "Whether to notify when a session needs attention.")]
    pub is_needs_attention_enabled: bool,

    #[schemars(description = "Whether to play a sound with notifications.")]
    pub play_notification_sound: bool,
}

impl Default for NotificationsSettings {
    fn default() -> Self {
        Self {
            mode: NotificationsMode::Unset,
            is_long_running_enabled: true,
            long_running_threshold: *DEFAULT_THRESHOLD_FOR_LONG_RUNNING_NOTIFICATION,
            is_password_prompt_enabled: true,
            is_agent_task_completed_enabled: true,
            is_needs_attention_enabled: true,
            play_notification_sound: true,
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
pub enum GithubPrPromptChipDefaultValidation {
    #[default]
    Unvalidated,
    Validated,
    Suppressed,
}

impl GithubPrPromptChipDefaultValidation {
    pub fn is_suppressed(self) -> bool {
        matches!(self, Self::Suppressed)
    }
}

define_settings_group!(SessionSettings, settings: [
    working_directory_config: WorkingDirectoryConfig,
    startup_shell_override: StartupShellOverride {
        type: StartupShell,
        default: StartupShell::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "session.startup_shell_override",
        description: "The shell to use when Rift starts up.",
    },
    new_session_shell_override: NewSessionShellOverride {
        type: Option<NewSessionShell>,
        default: None,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: false,
        toml_path: "session.new_session_shell_override",
        description: "The shell to use when opening a new session.",
    }
    honor_ps1: HonorPS1 {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "terminal.input.honor_ps1",
        description: "Whether to use your shell's PS1 prompt instead of the Rift prompt.",
    },
    saved_prompt: SavedPrompt {
        type: PromptSelection,
        default: PromptSelection::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    },
    should_add_agent_mode_chip: ShouldAddAgentModeChip {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    },
    should_confirm_close_session: ShouldConfirmCloseSession {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "general.should_confirm_close_session",
        description: "Whether to show a confirmation dialog when closing a session.",
    },
    // Value is saved here but not shown in ui (can't be toggled in settings)
    should_confirm_shared_session_edit_access: ShouldConfirmSharedSessionEditAccess {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
    }
    notifications: Notifications {
        type: NotificationsSettings,
        default: NotificationsSettings::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "notifications.preferences",
        max_table_depth: 1,
        description: "Notification preferences for terminal events.",
    }
    // This is a legacy setting that we no longer allow users to toggle after
    // context chips were introduced. We keep it only to respect users who
    // had previously disabled the dirty files chip via this setting.
    git_prompt_dirty_indicator: LegacyGitPromptDirtyIndicator {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: true,
        storage_key: "GitPromptDirtyIndicator",
    },
    notification_toast_duration_secs: NotificationToastDurationSecs {
        type: u64,
        default: 8,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "notifications.toast_duration_secs",
        description: "How long notification toasts are displayed, in seconds.",
    },
    // Tracks whether the `gh` CLI is installed and authenticated on this machine.
    // Not synced because `gh` CLI availability is machine-specific.
    github_pr_chip_default_validation: GithubPrChipDefaultValidation {
        type: GithubPrPromptChipDefaultValidation,
        default: GithubPrPromptChipDefaultValidation::Unvalidated,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
    },
]);

settings::macros::implement_setting_for_enum!(
    WorkingDirectoryConfig,
    SessionSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Never,
    private: false,
    toml_path: "session.working_directory_config",
    max_table_depth: 1,
    description: "Controls the working directory used when opening new sessions.",
);
