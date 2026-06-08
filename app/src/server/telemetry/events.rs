use std::time::Duration;

use rift_completer::completer::MatchType;
use rift_core::command::ExitCode;
use rift_core::interval_timer::TimingDataPoint;
use rift_core::telemetry::{
    EnablementState, TelemetryEvent as TelemetryEventTrait, TelemetryEventDesc,
};
use riftui::keymap::Keystroke;
use riftui::notification::{NotificationSendError, RequestPermissionsOutcome};
use riftui::rendering::ThinStrokes;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use session_sharing_protocol::common::{ParticipantId, Role, SessionId as SharedSessionId};
use session_sharing_protocol::sharer::SessionSourceType;
use strum_macros::{EnumDiscriminants, EnumIter};

use crate::auth::auth_manager::LoginGatedFeature;
use crate::channel::Channel;
use crate::features::FeatureFlag;
use crate::launch_configs::save_modal::SaveState;
use crate::palette::PaletteMode;
use crate::pane_group::PaneDragDropLocation;
use crate::prompt::editor_modal::OpenSource as PromptEditorOpenSource;
use crate::search::command_search::searcher::CommandSearchItemAction;
use crate::search::QueryFilter;
use crate::server::block::DisplaySetting;
use crate::server::ids::ServerId;
use crate::settings::import::config::{ParsedTerminalSetting, SettingType};
use crate::settings::import::model::TerminalType;
use crate::settings::AgentModeCodingPermissionsType;
use crate::tab::TabTelemetryAction;
use crate::terminal::block_list_viewport::InputMode;
use crate::terminal::cli_agent_sessions::{CLIAgentInputEntrypoint, CLIAgentRichInputCloseReason};
use crate::terminal::input::TelemetryInputSuggestionsMode;
use crate::terminal::model::ansi::WarpificationUnavailableReason;
use crate::terminal::model::session::SessionId;
use crate::terminal::model::terminal_model::{BlockSelectionCardinality, TmuxInstallationState};
use crate::terminal::settings::AltScreenPaddingMode;
use crate::terminal::shell::ShellType;
use crate::terminal::ssh::ssh_detection::SshInteractiveSessionDetected;
use crate::terminal::view::{
    BlockEntity, BlockSelectionDetails, ContextMenuInfo, GridHighlightedLink,
    NotificationsDiscoveryBannerAction, NotificationsErrorBannerAction, NotificationsTrigger,
    PromptPart,
};
use crate::terminal::ShareBlockType;
use crate::tips::WelcomeTipFeature;
#[cfg(feature = "local_fs")]
use crate::util::openable_file_type::FileTarget;
use crate::workspace::tab_settings::{TabCloseButtonPosition, WorkspaceDecorationVisibility};
use crate::workspace::TabMovement;

#[derive(Clone, Serialize, Deserialize)]
pub struct BootstrappingInfo {
    pub shell: &'static str,
    pub is_ssh: bool,
    pub is_subshell: bool,
    pub is_wsl: bool,
    pub is_msys2: bool,
    /// `true` if the bootstrapping process was triggered by an RC file snippet.
    ///
    /// This should only be true if `is_subshell` is true.
    pub was_triggered_by_rc_file: bool,
    /// The total time it took to bootstrap the shell, in seconds.
    pub bootstrap_duration_seconds: Option<f64>,
    /// The time it took to source the user's rcfiles, in seconds.  May be None
    /// if we weren't able to get that information from the shell.
    pub rcfiles_duration_seconds: Option<f64>,
    /// The difference between the total bootstrap time and the rcfile sourcing
    /// time, which roughly equals the time cost of running our bootstrap
    /// script.  Will be None if `bootstrap_duration_seconds` or
    /// `rcfiles_duration_seconds` is None.
    pub warp_attributed_bootstrap_duration_seconds: Option<f64>,
    pub shell_version: Option<String>,
    pub terminal_session_id: Option<SessionId>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SlowBootstrapInfo {
    pub shell: &'static str,
    pub is_ssh: bool,
    pub is_subshell: bool,
    pub is_wsl: bool,
    pub is_msys2: bool,
    /// Contents of the bootstrap block when the slow bootstrap was detected.
    /// This includes both command and output content from the block.
    pub bootstrap_block_contents: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppStartupInfo {
    pub is_session_restoration_on: bool,
    /// Whether or not a screen reader is enabled at the time the app is
    /// launched.  Should be set to None if we do not know for sure.
    pub is_screen_reader_enabled: Option<bool>,
    pub from_relaunch: bool,
    pub is_crash_reporting_enabled: bool,
    pub timing_data: Vec<TimingDataPoint>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum DownloadSource {
    Website,
    Homebrew,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlockLatencyInfo {
    pub command: &'static str,
    pub shell: &'static str,
    pub is_ssh: bool,
    pub execution_ms: u64,
}












/// How the user opened the Warp Drive sharing dialog.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SharingDialogSource {
    /// The sharing button in the pane header.
    PaneHeader,
    /// The per-pane command palette entry (includes keybindings).
    CommandPalette,
    /// The Warp Drive index context menu.
    DriveIndex,
    /// The sharing dialog was auto-opened from shared session creation.
    StartedSessionShare,
    /// The user intented into Warp with an email address to invite.
    InviteeRequest,
    /// The user jumped from an inherited ACL to its definition on a parent object.
    InheritedPermission,
    /// The onboarding block shown after users create new personal objects.
    OnboardingBlock,
    /// The conversation list overflow menu.
    ConversationList,
    /// The AI block context menu.
    AIBlockContextMenu,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum TabRenameEvent {
    OpenedEditor,
    CustomNameSet,
    CustomNameCleared,
}

/// The possible sources notifications can turned on from.
#[derive(Clone, Serialize, Deserialize)]
pub enum NotificationsTurnedOnSource {
    Settings,
    Banner,
}

/// The possible types of toggles in the find bar
#[derive(Clone, Serialize, Deserialize)]
pub enum FindOption {
    CaseSensitive,
    FindInBlock,
    Regex,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum LinkOpenMethod {
    CmdClick,
    ToolTip,
    MiddleClick,
}

/// The possible ways to trigger command x-ray
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandXRayTrigger {
    Hover,
    Keystroke,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum PaletteSource {
    PrefixChange,
    Keybinding,
    CtrlTab { shift_pressed_initially: bool },
    WarpDrive,
    QuitModal,
    LogOutModal,
    IntegrationTest,
    ConversationManager,
    ContextChip,
    PaneHeader,
    AgentTip,
    TitleBarSearchBar,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum FileTreeSource {
    /// Opened from the pane header toolbelt button.
    PaneHeader,
    Keybinding,
    LeftPanelToolbelt,
    ForceOpened,
    /// Opened from the CLI agent view footer (e.g., Claude Code).
    CLIAgentView,
}

#[cfg(feature = "local_fs")]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodePanelsFileOpenEntrypoint {
    CodeReview,
    ProjectExplorer,
    GlobalSearch,
}

/// The CLI agent being used (for telemetry purposes).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CLIAgentType {
    Claude,
    Gemini,
    Codex,
    Amp,
    Droid,
    OpenCode,
    Copilot,
    Pi,
    Auggie,
    Cursor,
    Goose,
    Hermes,
    Vibe,
    Unknown,
}

/// The kind of plugin chip shown or dismissed (for telemetry purposes).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginChipTelemetryKind {
    Install,
    Update,
}



/// The action taken on a plugin chip (for telemetry purposes).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginChipTelemetryAction {
    /// User clicked the auto-install button.
    Install,
    /// User clicked the auto-update button.
    Update,
    /// User clicked the manual install instructions button.
    InstallInstructions,
    /// User clicked the manual update instructions button.
    UpdateInstructions,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WarpDriveSource {
    Legacy,
    LeftPanelToolbelt,
    ForceOpened,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CommandCorrectionAcceptedType {
    /// TODO: We don't use the Autosuggestion variant yet. We need to wire through
    /// when an autosuggestion is accepted to be able to check this.
    Autosuggestion,
    Banner,
    Keybinding,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CommandCorrectionEvent {
    Proposed {
        rule: &'static str,
    },
    Accepted {
        via: CommandCorrectionAcceptedType,
        rule: &'static str,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CommandSearchResultType {
    History,
    Workflow,
    OpenWarpAI,
    TranslateUsingWarpAI,
    Notebook,
    EnvVarCollection,
    ViewInWarpDrive,
    AIQuery,
    Project,
}

impl From<&CommandSearchItemAction> for CommandSearchResultType {
    fn from(action: &CommandSearchItemAction) -> Self {
        use crate::search::command_search::searcher::CommandSearchItemAction::*;
        match action {
            AcceptHistory(_) | ExecuteHistory(_) => Self::History,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CloseTarget {
    App,
    Window,
    Tab,
    Pane,
    EditorTab,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum PtySpawnMode {
    /// The pty was spawned using the terminal server.
    TerminalServer,
    /// We tried to spawn the pty using the terminal server, but something went
    /// wrong so we fell back to spawning it directly.
    FallbackToDirect,
    /// The terminal server is not in use, and we spawned the pty directly
    /// (in tests, for example).
    Direct,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum OpenedWarpAISource {
    GlobalEntryButton,
    HelpWithBlock,
    HelpWithTextSelection,
    FromAICommandSearch,
    WarmWelcome,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WarpAIRequestResult {
    Succeeded { latency_ms: i64, truncated: bool },
    OutOfRequests,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WarpAIActionType {
    CopyTranscript,
    Restart,
    CopyAnswer,
    CopyCode,
    InsertIntoInput,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SaveAsWorkflowModalSource {
    Block,
    Input,
    WarpAIWorkflowCard,
    WarpAIPanel,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum LaunchConfigUiLocation {
    CommandPalette,
    AppMenu,
    TabMenu,
    Uri,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AICommandSearchEntrypoint {
    ShortHandTrigger,
    Keybinding,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SecretInteraction {
    RevealSecret,
    HideSecret,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AnonymousUserSignupEntrypoint {
    HitDriveObjectLimit,
    LoginGatedFeature,
    SignUpButton,
    RenotificationBlock,
    SignUpAIPrompt,
    NextCommandSuggestionsUpgradeBanner,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum UndoCloseItemType {
    Window,
    Tab,
    Pane,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PromptChoice {
    PS1,
    Default,
    Custom { builtin_chips: Vec<String> },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ToggleBlockFilterSource {
    /// This includes the keybinding and the command palette items.
    Binding,
    ContextMenu,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TierLimitHitEvent {
    pub team_uid: ServerId,
    pub feature: String,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum KnowledgePaneEntrypoint {
    /// Triggered by either the command palette or the mac menus
    #[serde(rename = "global")]
    Global,

    #[serde(rename = "settings")]
    Settings,

    #[serde(rename = "warp_drive")]
    WarpDrive,

    #[serde(rename = "ai_blocklist")]
    AIBlocklist,

    #[serde(rename = "slash_command")]
    SlashCommand,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeEntrypointSelectionType {
    /// User entered Agent Mode by taking action on a blocklist text selection.
    Text,

    /// User entered Agent Mode by taking action on a block selection.
    Block,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeEntrypoint {
    /// The stars icon button in the tab bar.
    #[serde(rename = "tab_bar")]
    TabBar,

    /// This corresponds to _both_ triggering from the command palette and via keybinding.
    ///
    /// Unfortunately due to the way the command palette automatically surfaces any editable
    /// keybinding as an action, we don't have enough information to discern if the binding was
    /// triggered by the palette or keyboard.
    #[serde(rename = "new_pane_binding")]
    NewPaneBinding,

    /// The stars button in the hoverable block "toolbelt".
    #[serde(rename = "block_toolbelt")]
    BlockToolbelt,

    /// The "Ask Agent Mode" option from AI command search.
    #[serde(rename = "ai_command_search")]
    AICommandSearch,

    /// Context menu item(s) that attach a blocklist selection as context to an Agent Mode query.
    #[serde(rename = "context_menu")]
    ContextMenu {
        selection_type: AgentModeEntrypointSelectionType,
    },

    /// The Agent Mode chip in the prompt.
    #[serde(rename = "prompt_chip")]
    PromptChip,

    /// The Agent Management popup, where you can see all the most recent tasks for each terminal
    /// pane across all windows/tabs/panes.
    #[serde(rename = "agent_management_popup")]
    AgentManagementPopup,

    /// User manually switched between terminal and AI input modes in UDI interface
    #[serde(rename = "udi_terminal_input_switcher")]
    UDITerminalInputSwitcher,

    /// The agent management view, where you can see both local interactive and ambient agent tasks
    #[serde(rename = "agent_management_view")]
    AgentManagementView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AutonomySettingToggleSource {
    Speedbump,
    SettingsPage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToggleCodeSuggestionsSettingSource {
    Speedbump,
    Settings,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum InteractionSource {
    Button,
    Keybinding,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PromptSuggestionViewType {
    TerminalView,
    AgentView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeAttachContextMethod {
    #[serde(rename = "keyboard")]
    Keyboard,

    #[serde(rename = "mouse")]
    Mouse,
}

/// The entrypoint from which the rewind dialog was opened.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AgentModeRewindEntrypoint {
    /// The rewind button in the AI block header.
    Button,
    /// The context menu item "Rewind to before here".
    ContextMenu,
    /// The /rewind slash command.
    SlashCommand,
}

/// Reasons why we fell back to a prompt suggestion from a suggested code diff.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum PromptSuggestionFallbackReason {
    /// Code file had too many lines, hence we stopped triggering the suggested code diff.
    #[serde(rename = "file_too_many_lines")]
    FileTooManyLines,
    /// Code file had too many bytes, hence we stopped triggering the suggested code diff.
    #[serde(rename = "file_too_many_bytes")]
    FileTooManyBytes,
    /// Missing file, when looking up filepaths in local file system.
    #[serde(rename = "missing_file")]
    MissingFile,
    /// Failed to retrieve file from local file system.
    #[serde(rename = "failed_to_retrieve_file")]
    FailedToRetrieveFile,
    /// In an SSH/remote session.
    #[serde(rename = "ssh_remote_session")]
    SSHRemoteSession,
    /// No read files permission.
    #[serde(rename = "no_read_files_permission")]
    NoReadFilesPermission,
    /// AI query timeout.
    #[serde(rename = "ai_query_timeout")]
    AIQueryTimeout,
    /// Failed to send AI request.
    #[serde(rename = "failed_to_send_ai_request")]
    FailedToSendAIRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeSetupProjectScopedRulesActionType {
    #[serde(rename = "link_from_existing")]
    LinkFromExisting(String),
    #[serde(rename = "generate_warp_md")]
    GenerateWarpMd,
    #[serde(rename = "skip_rules")]
    SkipRules,
    #[serde(rename = "regenerate_warp_md")]
    RegenerateWarpMd,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeSetupCodebaseContextActionType {
    #[serde(rename = "index_codebase")]
    IndexCodebase,
    #[serde(rename = "skip_indexing")]
    SkipIndexing,
    #[serde(rename = "view_index_status")]
    ViewIndexStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentModeSetupCreateEnvironmentActionType {
    #[serde(rename = "create_environment")]
    CreateEnvironment,
    #[serde(rename = "skip_environment")]
    SkipEnvironment,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CpuUsageStats {
    /// The number of logical CPUs on the system.
    pub num_cpus: usize,

    /// The maximum CPU usage over the measurement interval.
    ///
    /// This number is in the range [0, num_cpus].  The CPU utilization, as a
    /// percentage, can be determined via `max_usage / num_cpus * 100`.
    pub max_usage: f32,

    /// The average CPU usage over the measurement interval.
    ///
    /// This number is in the range [0, num_cpus].  The CPU utilization, as a
    /// percentage, can be determined via `avg_usage / num_cpus * 100`.
    pub avg_usage: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryUsageStats {
    pub total_application_usage_bytes: usize,
    pub total_blocks: usize,
    pub total_lines: usize,

    /// Statistics about blocks that have been seen in the past 5 minutes.
    pub active_block_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [5m, 1h).
    pub inactive_5m_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [1h, 24h).
    pub inactive_1h_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [24h, ..).
    pub inactive_24h_stats: BlockMemoryUsageStats,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockMemoryUsageStats {
    pub num_blocks: usize,
    pub num_lines: usize,
    pub estimated_memory_usage_bytes: usize,
}

/// Entrypoints to toggle the input auto-detection setting for Agent Mode.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AgentModeAutoDetectionSettingOrigin {
    /// The "speed bump" banner shown that's shown to the user when input is autodetected.
    #[serde(rename = "banner")]
    Banner,

    /// The AI settings page.
    #[serde(rename = "settings_page")]
    SettingsPage,
}

/// Payload for the [`AgentModePotentialAutodetectionFalsePositive`] event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentModeAutoDetectionFalsePositivePayload {
    /// Payload includes input text for dogfood channels.
    InternalDogfoodUsers { input_text: String },

    /// Do not include the misclassified input text in stable channels due to privacy concerns.
    ExternalUsers,
}

/// How the user triggered the [`AddTabWithShell`] event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum AddTabWithShellSource {
    CommandPalette,
    ShellSelectorMenu,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeContextDestination {
    Pty,
    AgentInput,
    RichInput,
}


#[derive(Clone, Copy, Debug, Serialize)]
pub enum ImageProtocol {
    Kitty,
    ITerm,
}

#[derive(Clone, Copy, Debug, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InputUXChangeOrigin {
    #[default]
    Settings,
    ADELaunchModal,
}





#[derive(Clone, Copy, Debug, Serialize)]
pub enum SlashMenuSource {
    SlashButton,
    UserTyped,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginEventSource {
    OnboardingSlide,
    AuthModal,
}



/// Details about which type of slash command was accepted
#[derive(Clone, Debug, Serialize)]
pub enum SlashCommandAcceptedDetails {
    /// A built-in static command with its specific name (e.g., "/init", "/diff-review")
    StaticCommand { command_name: String },
    /// A user-created saved prompt/workflow
    SavedPrompt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AutoReloadModalAction {
    #[serde(rename = "dismissed")]
    Dismissed,
    #[serde(rename = "enabled_auto_reload")]
    EnabledAutoReload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OutOfCreditsBannerAction {
    #[serde(rename = "dismissed")]
    Dismissed,
    #[serde(rename = "credits_purchased")]
    CreditsPurchased,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteCodebaseIndexStatusTelemetrySource {
    Snapshot,
    PushUpdate,
    MutationResponse,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteCodebaseAutoIndexTrigger {
    NavigatedToGitRepo,
    CodebaseContextEnablementChanged,
}

#[derive(Clone, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter))]
pub enum TelemetryEvent {
    AutosuggestionInserted {
        insertion_length: usize,
        buffer_length: usize,
    },
    BlockCompleted {
        block_finished_to_precmd_delay_ms: u64,
        honor_ps1_enabled: bool,
        num_secrets_redacted: usize,
        /// The number of lines in the block's output grid when it was
        /// finished.
        num_output_lines: u64,
        /// The number of lines of output that were truncated while the block
        /// was active and receiving output.
        num_output_lines_truncated: u64,
        terminal_session_id: Option<SessionId>,
        is_udi_enabled: bool,
        /// Whether the command was executed while in an active agent view.
        is_in_agent_view: bool,
    },
    /// This is identical to the `BlockCompleted` event, but includes extra fields for
    /// the command run / time it took the block to complete / exit code.
    /// That sort of telemetry should *NEVER* be sent in production, so
    /// DO NOT SEND THIS IN NON-DOGFOOD ENVIRONMENTS!
    BlockCompletedOnDogfoodOnly {
        block_finished_to_precmd_delay_ms: u64,
        honor_ps1_enabled: bool,
        num_secrets_redacted: usize,
        /// The number of lines in the block's output grid when it was
        /// finished.
        num_output_lines: u64,
        /// The number of lines of output that were truncated while the block
        /// was active and receiving output.
        num_output_lines_truncated: u64,
        command: String,
        duration: Duration,
        exit_code: ExitCode,
        terminal_session_id: Option<SessionId>,
    },
    /// A new block of background output was started and added to the block list.
    BackgroundBlockStarted,
    /// User-perceptible latency (i.e. from hitting enter to first frame after command finishes) for
    /// a number of commands that perform minimal work we use as a baseline.
    BaselineCommandLatency(BlockLatencyInfo),
    SessionCreation,
    Login,
    OpenSuggestionsMenu(TelemetryInputSuggestionsMode),
    ConfirmSuggestion {
        mode: TelemetryInputSuggestionsMode,
        match_type: MatchType,
    },
    OpenContextMenu {
        context_menu_info: ContextMenuInfo,
    },
    /// Copy command, output or both for some number of blocks.
    ContextMenuCopy(BlockEntity, BlockSelectionCardinality),
    ContextMenuOpenShareModal(BlockSelectionCardinality),
    ContextMenuFindWithinBlocks(BlockSelectionCardinality),
    ContextMenuCopyPrompt {
        part: PromptPart,
    },
    ContextMenuToggleGitPromptDirtyIndicator {
        enabled: bool,
    },
    ContextMenuInsertSelectedText,
    ContextMenuCopySelectedText,
    /// The user opened the prompt editor modal.
    OpenPromptEditor {
        entrypoint: PromptEditorOpenSource,
    },
    /// The user's prompt was edited via the prompt editor modal.
    PromptEdited {
        prompt: PromptChoice,
        entrypoint: String,
    },
    ReinputCommands(BlockSelectionCardinality),
    JumpToPreviousCommand,
    CopyBlockSharingLink(ShareBlockType),
    GenerateBlockSharingLink {
        share_type: ShareBlockType,
        display_setting: DisplaySetting,
        show_prompt: bool,
        redact_secrets: bool,
    },
    BlockSelection(BlockSelectionDetails),
    BootstrappingSlow(BootstrappingInfo),
    BootstrappingSlowContents(SlowBootstrapInfo),
    /// Logged when a pending session is abandoned before it hits Bootstrapped.
    SessionAbandonedBeforeBootstrap {
        pending_shell: Option<ShellType>,
        has_pending_ssh_session: bool,
        was_ever_visible: bool,
        duration_since_start: Duration,
    },
    BootstrappingSucceeded(BootstrappingInfo),
    /// The user accepted a completion suggestion when it was the only one in the suggestions menu.
    /// This event is named with 'Tab' to maintain backwards compatibility; the completion
    /// suggestions menu may be triggered with a keybinding other than tab.
    TabSingleResultAutocompletion,
    EditorUnhandledModifierKey(String),
    CopyInviteLink,
    OpenThemeChooser,
    ThemeSelection {
        theme: String,
        entrypoint: String,
    },
    AppIconSelection {
        icon: String,
    },
    CursorDisplayType {
        cursor: String,
    },
    OpenThemeCreatorModal,
    CreateCustomTheme,
    DeleteCustomTheme,
    SplitPane,
    UnableToAutoUpdateToNewVersion,
    /// An update was successfully installed, and we're attempting to relaunch the app.
    AutoupdateRelaunchAttempt {
        new_version: String,
    },
    SkipOnboardingSurvey,
    ToggleRestoreSession(bool),
    DatabaseStartUpError(String),
    DatabaseReadError(String),
    DatabaseWriteError(String),
    AppStartup(AppStartupInfo),
    /// The native app was opened while logged out. Since Warp requires login,
    /// this usually means a new user.
    LoggedOutStartup,
    /// The download source, if it can be determined. Will only be sent when
    /// the app is launched while logged out.
    DownloadSource(DownloadSource),
    /// We attempted to bootstrap an SSH session via the SSH wrapper.  The
    /// argument is the name of the remote shell.
    SSHBootstrapAttempt(String),
    SSHControlMasterError {
        has_remote_server: bool,
    },
    KeybindingChanged {
        action: String,
        keystroke: Keystroke,
    },
    KeybindingResetToDefault {
        action: String,
    },
    KeybindingRemoved {
        action: String,
    },
    FeaturesPageAction {
        action: String,
        value: String,
    },
    OpenWorkflowSearch,
    OpenQuakeModeWindow,
    OpenWelcomeTips,
    CompleteWelcomeTipFeature {
        total_completed_count: usize,
        tip_name: WelcomeTipFeature,
    },
    DismissWelcomeTips,
    ShowNotificationsDiscoveryBanner,
    NotificationsDiscoveryBannerAction(NotificationsDiscoveryBannerAction),
    ShowNotificationsErrorBanner,
    NotificationsErrorBannerAction(NotificationsErrorBannerAction),
    NotificationPermissionsRequested {
        source: NotificationsTurnedOnSource,
        trigger: Option<NotificationsTrigger>,
    },
    NotificationsRequestPermissionsOutcome {
        outcome: RequestPermissionsOutcome,
    },
    NotificationFailedToSend {
        error: NotificationSendError,
    },
    NotificationClicked,
    ToggleFindOption {
        option: FindOption,
        enabled: bool,
    },
    SignUpButtonClicked,
    LoginButtonClicked {
        source: LoginEventSource,
    },
    LoginLaterButtonClicked {
        source: LoginEventSource,
    },
    LoginLaterConfirmationButtonClicked {
        source: LoginEventSource,
    },
    OpenNewSessionFromFilePath,
    OpenTeamFromURI,
    SelectNavigationPaletteItem,
    SelectCommandPaletteOption(String),
    PaletteSearchOpened {
        mode: PaletteMode,
        source: PaletteSource,
    },
    PaletteSearchResultAccepted {
        result_type: &'static str,
        filter: Option<QueryFilter>,
        buffer_length: usize,
    },
    PaletteSearchExited {
        filter: Option<QueryFilter>,
        buffer_length: usize,
    },
    AuthCommonQuestionClicked {
        question: &'static str,
    },
    AuthToggleFAQ {
        open: bool,
    },
    OpenAuthPrivacySettings {
        source: LoginEventSource,
    },
    TabRenamed(TabRenameEvent),
    MoveActiveTab {
        direction: TabMovement,
    },
    MoveTab {
        direction: TabMovement,
    },
    DragAndDropTab,
    DragAndDropTabGroup,
    TabOperations {
        action: TabTelemetryAction,
    },
    EditedInputBeforePrecmd,
    TriedToExecuteBeforePrecmd,
    ThinStrokesSettingChanged {
        new_value: ThinStrokes,
    },
    BookmarkBlockToggled {
        enable_bookmark: bool,
    },
    JumpToBookmark,
    JumpToLatestAgentMessage,
    JumpToBottomofBlockButtonClicked,
    ToggleJumpToBottomofBlockButton {
        enabled: bool,
    },
    ToggleShowBlockDividers {
        enabled: bool,
    },
    OpenLink {
        link: GridHighlightedLink,
        open_with: LinkOpenMethod,
    },
    OpenChangelogLink {
        url: String,
    },
    ShowInFileExplorer,
    CommandXRayTriggered {
        trigger: CommandXRayTrigger,
    },
    OpenLaunchConfigSaveModal,
    SaveLaunchConfig {
        state: SaveState,
    },
    OpenLaunchConfigFile,
    OpenLaunchConfig {
        ui_location: LaunchConfigUiLocation,
        open_in_active_window: bool,
    },
    TeamCreated,
    TeamJoined,
    TeamLeft,
    ToggleSettingsSync {
        is_settings_sync_enabled: bool,
    },
    TeamLinkCopied,
    RemovedUserFromTeam,
    DeletedWorkflow,
    DeletedNotebook,
    ToggleApprovalsModal,
    SendEmailInvites,
    CommandCorrection {
        event: CommandCorrectionEvent,
    },
    SetLineHeight {
        new_value: f32,
    },
    ResourceCenterOpened,
    ResourceCenterTipsCompleted,
    ResourceCenterTipsSkipped,
    KeybindingsPageOpened,
    CommandSearchOpened {
        has_initial_query: bool,
    },
    CommandSearchExited {
        query_filter: Option<QueryFilter>,
        buffer_length: usize,
    },
    CommandSearchResultAccepted {
        result_index: usize,
        result_type: CommandSearchResultType,
        query_filter: Option<QueryFilter>,
        buffer_length: usize,
        was_immediately_executed: bool,
    },
    CommandSearchFilterChanged {
        new_filter: Option<QueryFilter>,
    },
    GlobalSearchOpened,
    GlobalSearchQueryStarted,
    AICommandSearchOpened {
        entrypoint: AICommandSearchEntrypoint,
    },
    OpenedAltScreenFind,
    UserInitiatedClose {
        initiated_on: CloseTarget,
    },
    QuitModalShown {
        running_processes: u32,
        shared_sessions: u32,
        modal_for: CloseTarget,
    },
    QuitModalCancel {
        nav_palette: bool,
        modal_for: CloseTarget,
    },
    QuitModalDisabled,
    UserInitiatedLogOut,
    LogOutModalShown,
    LogOutModalCancel {
        nav_palette: bool,
    },
    SetOpacity {
        // Represented in percentages from 1-100.
        opacity: u8,
    },
    SetBlurRadius {
        // The radius value from 1-18.
        blur_radius: u8,
    },
    ToggleDimInactivePanes {
        enabled: bool,
    },
    InputModeChanged {
        old_mode: InputMode,
        new_mode: InputMode,
    },
    PtySpawned {
        mode: PtySpawnMode,
    },
    InitialWorkingDirectoryConfigurationChanged {
        advanced_mode_enabled: bool,
    },
    /// Opened legacy Warp AI.
    OpenedWarpAI {
        source: OpenedWarpAISource,
    },
    /// Issued legacy Warp AI request.
    WarpAIRequestIssued {
        result: WarpAIRequestResult,
    },
    WarpAIAction {
        action_type: WarpAIActionType,
    },
    /// This is purely for static prompts! Do not send user-written prompts with this event.
    UsedWarpAIPreparedPrompt {
        prompt: &'static str,
    },
    ToggleFocusPaneOnHover {
        enabled: bool,
    },
    WarpAICharacterLimitExceeded,
    OpenInputContextMenu,
    InputCutSelectedText,
    InputCopySelectedText,
    InputSelectAll,
    InputPaste,
    InputCommandSearch,
    InputAICommandSearch,
    InputAskWarpAI,
    SaveAsWorkflowModal {
        source: SaveAsWorkflowModalSource,
    },
    ExperimentTriggered {
        experiment: &'static str,
        layer: &'static str,
        group_assignment: &'static str,
    },
    ToggleSyncAllPanesInAllTabs {
        enabled: bool,
    },
    ToggleSyncAllPanesInTab {
        enabled: bool,
    },
    ToggleSameLinePrompt {
        enabled: bool,
    },
    ToggleNewWindowsAtCustomSize {
        enabled: bool,
    },
    SetNewWindowsAtCustomSize,
    DisableInputSync,
    ToggleTabIndicators {
        enabled: bool,
    },
    TogglePreserveActiveTabColor {
        enabled: bool,
    },
    ShowSubshellBanner,
    DeclineSubshellBootstrap {
        remember: bool,
    },
    TriggerSubshellBootstrap {
        triggered_by_rc_file_snippet: bool,
    },
    AddDenylistedSubshellCommand,
    RemoveDenylistedSubshellCommand,
    AddAddedSubshellCommand,
    RemoveAddedSubshellCommand,
    ReceivedSubshellRcFileDcs,
    AddDenylistedSshTmuxWrapperHost,
    RemoveDenylistedSshTmuxWrapperHost,
    /// User Setting for enabling SSH Tmux Wrapper changed.
    ToggleSshTmuxWrapper {
        enabled: bool,
    },
    ToggleSshWarpification {
        enabled: bool,
    },
    /// User changed the SSH extension install mode.
    SetSshExtensionInstallMode {
        mode: &'static str,
    },
    /// User toggled the "Don't ask me this again" checkbox on the SSH
    /// remote-server choice block.
    SshRemoteServerChoiceDoNotAskAgainToggled {
        checked: bool,
    },
    /// An ssh interactive session was detected.
    SshInteractiveSessionDetected(SshInteractiveSessionDetected),
    SshTmuxWarpifyBannerDisplayed,
    /// A SSH Warpify Block was accepted
    SshTmuxWarpifyBlockAccepted,
    /// A SSH Warpify Block was dismissed
    SshTmuxWarpifyBlockDismissed,
    WarpifyFooterShown {
        is_ssh: bool,
    },
    AgentToolbarDismissed,
    WarpifyFooterAcceptedWarpify {
        is_ssh: bool,
    },
    /// How long until the warpify process succeeded
    SshTmuxWarpificationSuccess {
        tmux_installation: Option<TmuxInstallationState>,
        duration_ms: u64,
    },
    /// An SSH Error block was displayed to the user.
    SshTmuxWarpificationErrorBlock {
        error: WarpificationUnavailableReason,
        tmux_installation: Option<TmuxInstallationState>,
    },
    /// A SSH Install Tmux Block was displayed.
    SshInstallTmuxBlockDisplayed,
    /// A SSH Install Tmux Block was accepted.
    SshInstallTmuxBlockAccepted,
    /// A SSH Install Tmux Block was dismissed.
    SshInstallTmuxBlockDismissed,
    ShowAliasExpansionBanner,
    EnableAliasExpansionFromBanner,
    DismissAliasExpansionBanner,
    ShowVimKeybindingsBanner,
    EnableVimKeybindingsFromBanner,
    DismissVimKeybindingsBanner,
    InitiateReauth,
    InitiateAnonymousUserSignup {
        entrypoint: AnonymousUserSignupEntrypoint,
    },
    AnonymousUserExpirationLockout,
    AnonymousUserLinkedFromBrowser,
    AnonymousUserAttemptLoginGatedFeature {
        feature: LoginGatedFeature,
    },
    AnonymousUserHitCloudObjectLimit,
    NeedsReauth,
    WarpDriveOpened {
        source: WarpDriveSource,
        is_code_mode_v2: bool,
    },
    // Toggled the legacy Warp AI side panel.
    ToggleWarpAI {
        opened: bool,
    },
    ToggleSecretRedaction {
        enabled: bool,
    },
    CustomSecretRegexAdded,
    ToggleObfuscateSecret {
        interaction: SecretInteraction,
    },
    CopySecret,
    AutoGenerateMetadataSuccess,
    AutoGenerateMetadataError {
        error_payload: Value,
    },
    UndoClose {
        item_type: UndoCloseItemType,
    },
    /// This event is used to measure PTY throughput.
    /// NOTE: this event is only meant to be used for WarpDev.
    PtyThroughput {
        /// The maximum PTY throughput in bytes/sec, aggregated over a 10 minute period.
        max_bytes_per_second: usize,
    },
    DriveSharingOnboardingBlockShown,
    CommandFileRun,
    PageUpDownInEditorPressed {
        // Key pressed when nothing is in the editor (no-op)
        is_empty_editor: bool,
        // Is PageDown. Otherwise is PageUp
        is_down: bool,
    },
    JoinedSharedSession {
        session_id: SharedSessionId,
        source_type: SessionSourceType,
    },
    SharedSessionModalUpgradePressed,
    /// Emitted when a shared session sharer cancels granting a role
    /// (currently only applies when granting executor mode).
    SharerCancelledGrantRole {
        role: Role,
    },
    /// Emitted when a shared session sharer checks "dont show again"
    /// in confirmation modal when granting a role.
    SharerGrantModalDontShowAgain,
    JumpToSharedSessionParticipant {
        jumped_to: ParticipantId,
    },
    UnsupportedShell {
        shell: String,
    },
    LogOut,
    SettingsImportInitiated,
    InviteTeammates {
        num_teammates: usize,
        team_uid: ServerId,
    },
    OpenAndWarpifyDockerSubshell {
        /// Some variant if we support this shell type, and None otherwise.
        shell_type: Option<ShellType>,
    },
    /// Represents an update to a block filter query that goes from empty to non-empty.
    UpdateBlockFilterQuery,
    UpdateBlockFilterQueryContextLines {
        num_context_lines: u16,
    },
    ToggleBlockFilterQuery {
        enabled: bool,
        source: ToggleBlockFilterSource,
    },
    ToggleBlockFilterCaseSensitivity {
        enabled: bool,
    },
    ToggleBlockFilterRegex {
        enabled: bool,
    },
    ToggleBlockFilterInvert {
        enabled: bool,
    },
    BlockFilterToolbeltButtonClicked,
    ToggleSnackbarInActivePane {
        show_snackbar: bool,
    },
    PaneDragInitiated,
    PaneDropped {
        drop_location: PaneDragDropLocation,
    },
    ObjectLinkCopied {
        link: String,
    },
    FileTreeToggled {
        source: FileTreeSource,
        is_code_mode_v2: bool,
        /// The CLI agent type if opened from a CLI agent footer (e.g., Claude Code).
        cli_agent: Option<CLIAgentType>,
    },
    /// User attached a file or directory as context from the file tree
    FileTreeItemAttachedAsContext {
        is_directory: bool,
    },
    /// User added selected code as context from the code editor.
    CodeSelectionAddedAsContext {
        destination: CodeContextDestination,
    },
    /// User created a new file from the file tree
    FileTreeItemCreated,
    /// Conversation list view was opened
    ConversationListViewOpened,
    /// User opened a conversation from the conversation list
    ConversationListItemOpened {
        /// Whether the conversation is an ambient agent task (vs a local conversation)
        is_ambient_agent: bool,
    },
    /// User deleted a conversation from the conversation list
    ConversationListItemDeleted,
    /// User copied a conversation link from the conversation list
    ConversationListLinkCopied {
        /// Whether the conversation is an ambient agent task (vs a local conversation)
        is_ambient_agent: bool,
    },
    /// The user tried to send an Agent Mode query but they have already reached their AI request
    /// limit. Note that this limit is for all AI requests, not Agent Mode alone.
    AgentModeUserAttemptedQueryAtRequestLimit {
        /// The AI request limit for the user's current plan.
        limit: usize,
    },
    AgentModeClickedEntrypoint {
        entrypoint: AgentModeEntrypoint,
    },


    /// User opened the rewind confirmation dialog.
    AgentModeRewindDialogOpened {
        entrypoint: AgentModeRewindEntrypoint,
    },

    /// User executed a conversation rewind.
    AgentModeRewindExecuted {
        /// The number of AI blocks that were reverted.
        num_blocks_reverted: usize,
    },

    /// Emitted when a user explicitly attaches a block as context to an Agent Mode query.
    ///
    /// This is only emitted for the initial attachment -- its intended to express user's intent to
    /// attach context.
    ///
    /// For example, this is emitted when a user selects a block to attach as context and has no
    /// prior blocks selected. If a user has already selected a block as context and is merely
    /// changing or adding to the existing selection, this is not emitted.
    ///
    /// Also note this is not emitted if the user clicks an entrypoint that automatically attaches
    /// context (like clicking the block toolbelt stars button, for instance).
    AgentModeAttachedBlockContext {
        method: AgentModeAttachContextMethod,
    },

    /// Emitted when the user toggles the "Input Auto-detection" setting in the AI settings page or
    /// in the auto-detection "speed bump" banner.
    AgentModeToggleAutoDetectionSetting {
        is_autodetection_enabled: bool,
        origin: AgentModeAutoDetectionSettingOrigin,
    },


    /// Emitted when the user manually toggles the terminal input from AI mode to shell mode when
    /// the current input text has been auto-detected as AI input -- this is likely a natural
    /// language auto-detection false-positive.
    AgentModePotentialAutoDetectionFalsePositive(AgentModeAutoDetectionFalsePositivePayload),


    /// Keeps track of number of times the user is presented with a Prompt Suggestions banner.
    PromptSuggestionShown {
        id: String,
        request_duration_ms: u64,
        block_id: Option<String>,
        view: PromptSuggestionViewType,
        /// Server-assigned request token from the `/passive-suggestion`
        /// request that generated this suggestion. Used to join client-side
        /// telemetry with server-side logs. `None` on the legacy code path.
        server_request_token: Option<String>,
    },


    /// Keeps track of number of times the user falls back to a prompt suggestion from a suggested code diff banner.
    SuggestedCodeDiffFailed {
        prompt_suggestion_id: String,
        reason: PromptSuggestionFallbackReason,
    },

    /// Keeps track of number of times the user accepts & runs a query from the Prompt Suggestions banner.
    PromptSuggestionAccepted {
        id: String,
        view: PromptSuggestionViewType,
        interaction_source: InteractionSource,
    },

    /// Keeps track of number of times the user is presented with a Static Prompt Suggestions banner.
    StaticPromptSuggestionsBannerShown {
        id: String,
        block_id: String,
        static_prompt_suggestion_name: String,
        // The below fields are only collected if telemetry is enabled.
        query: Option<String>,
        block_command: Option<String>,
        request_duration_ms: u64,
        view: PromptSuggestionViewType,
    },

    /// Keeps track of number of times the user accepts a Static Prompt Suggestion.
    StaticPromptSuggestionAccepted {
        id: String,
        view: PromptSuggestionViewType,
        interaction_source: InteractionSource,
    },








    /// Emitted when the user toggles the "Intelligent autosuggestions" setting in the AI settings page.
    ToggleIntelligentAutosuggestionsSetting {
        is_intelligent_autosuggestions_enabled: bool,
    },

    /// Emitted when the user toggles global AI.
    ToggleGlobalAI {
        is_ai_enabled: bool,
    },

    /// Emitted when the user toggles codebase context.
    ToggleCodebaseContext {
        is_codebase_context_enabled: bool,
    },

    ToggleAutoIndexing {
        is_autoindexing_enabled: bool,
    },

    ActiveIndexedReposChanged {
        updated_number_of_codebase_indices: usize,
        hit_max_indices: bool,
    },

    /// Emitted when the user toggles active AI.
    ToggleActiveAI {
        is_active_ai_enabled: bool,
    },

    /// Emitted when the user toggles the "Prompt Suggestions" setting in the AI settings page.
    TogglePromptSuggestionsSetting {
        is_prompt_suggestions_enabled: bool,
    },

    /// Emitted when the user toggles the "Code Suggestions" setting.
    ToggleCodeSuggestionsSetting {
        source: ToggleCodeSuggestionsSettingSource,
        is_code_suggestions_enabled: bool,
    },

    /// Emitted when the user toggles the "Natural Language Autosuggestions" setting in the AI settings page.
    ToggleNaturalLanguageAutosuggestionsSetting {
        is_natural_language_autosuggestions_enabled: bool,
    },

    /// Emitted when the user toggles the "Shared Block Title Auto Generation" setting in the AI settings page.
    ToggleSharedBlockTitleGenerationSetting {
        is_shared_block_title_generation_enabled: bool,
    },

    /// Emitted when the user toggles the "Git Operations Autogen" setting in the AI settings page.
    ToggleGitOperationsAutogenSetting {
        is_git_operations_autogen_enabled: bool,
    },

    /// Emitted when the user toggles the "Voice Input" setting in the AI settings page.
    ToggleVoiceInputSetting {
        is_voice_input_enabled: bool,
    },

    /// Emitted when the user toggles the "Show Agent Tips" setting in the AI settings page.
    ToggleShowAgentTips {
        is_enabled: bool,
    },

    TierLimitHit(TierLimitHitEvent),
    SharedObjectLimitHitBannerViewPlansButtonClicked,
    ResourceUsageStats {
        cpu: CpuUsageStats,
        mem: MemoryUsageStats,
    },
    MemoryUsageStats {
        total_application_usage_bytes: usize,
        total_blocks: usize,
        total_lines: usize,

        /// Statistics about blocks that have been seen in the past 5 minutes.
        active_block_stats: BlockMemoryUsageStats,
        /// Statistics about blocks that haven't been seen since [5m, 1h).
        inactive_5m_stats: BlockMemoryUsageStats,
        /// Statistics about blocks that haven't been seen since [1h, 24h).
        inactive_1h_stats: BlockMemoryUsageStats,
        /// Statistics about blocks that haven't been seen since [24h, ..).
        inactive_24h_stats: BlockMemoryUsageStats,
    },
    MemoryUsageHigh {
        total_application_usage_bytes: u64,
        /// Platform-specific memory breakdown (JSON object with keys that
        /// vary by OS).  See `memory_footprint::memory_breakdown()`.
        memory_breakdown: serde_json::Value,
    },

    /// The user imported settings from another terminal.
    CompletedSettingsImport {
        terminal_type: TerminalType,
        imported_settings: Vec<ParsedTerminalSetting>,
    },
    /// The user focused a terminal option to import settings from.
    SettingsImportConfigFocused(TerminalType),
    /// The user clicked the "Reset to defaults" button in the settings import onboarding block.
    SettingsImportResetButtonClicked,
    /// Completed parsing a terminal for its settings to import.
    SettingsImportConfigParsed {
        timing_data: Vec<TimingDataPoint>,
        terminal_type: TerminalType,
        settings_shown_to_user: Option<Vec<SettingType>>,
    },
    /// When parsing iTerm for settings it contained multiple hotkey bindings.
    ITermMultipleHotkeys,
    UserMenuUpgradeClicked,
    ToggleWorkspaceDecorationVisibility {
        previous_value: WorkspaceDecorationVisibility,
        new_value: WorkspaceDecorationVisibility,
    },
    UpdateAltScreenPaddingMode {
        new_mode: AltScreenPaddingMode,
    },
    AddTabWithShell {
        source: AddTabWithShellSource,
        shell: String,
    },
    ToggleLigatureRendering {
        enabled: bool,
    },

    ToggledAgentModeAutoexecuteReadonlyCommandsSetting {
        src: AutonomySettingToggleSource,
        enabled: bool,
    },
    ChangedAgentModeCodingPermissions {
        src: AutonomySettingToggleSource,
        new: AgentModeCodingPermissionsType,
    },
    RepoOutlineConstructionSuccess {
        total_parse_seconds: usize,
        file_count: usize,
    },
    RepoOutlineConstructionFailed {
        error: String,
    },
    KnowledgePaneOpened {
        entrypoint: KnowledgePaneEntrypoint,
    },
    #[cfg(feature = "local_fs")]
    CodePanelsFileOpened {
        entrypoint: CodePanelsFileOpenEntrypoint,
        target: FileTarget,
    },
    #[cfg(feature = "local_fs")]
    PreviewPanePromoted,
    AttachedImagesToAgentModeQuery {
        num_images: usize,
        /// Whether or not Universal Developer Input mode is enabled
        is_udi_enabled: bool,
    },
    /// An error was encountered fetching available WSL distributions from the Registry.
    /// This typically means the user hasn't installed or enabled WSL.
    #[cfg(windows)]
    WSLRegistryError,
    #[cfg(windows)]
    AutoupdateUnableToCloseApplications,
    #[cfg(windows)]
    AutoupdateFileInUse,
    #[cfg(windows)]
    AutoupdateMutexTimeout,
    #[cfg(windows)]
    AutoupdateForcekillFailed {
        exit_code: i32,
    },
    #[cfg(windows)]
    AutoupdateMinidumpCleanupFailed {
        exit_code: i32,
    },
    ImageReceived {
        image_protocol: ImageProtocol,
    },
    GrepToolSucceeded,
    FileGlobToolSucceeded,
    ShellTerminatedPrematurely {
        shell_type: Option<ShellType>,
        shell_path: Option<String>,
        reason: String,
        reason_details: Option<String>,
        antivirus_name: Option<String>,
        long_os_version: Option<String>,
        exit_reason: Option<String>,
    },
    /// User changed the input UX mode (e.g. Universal Developer Input, UDI, mode or Classic)
    InputUXModeChanged {
        is_udi_enabled: bool,
        origin: InputUXChangeOrigin,
    },
    /// User interacted with context chips (git branch, working directory, etc.)
    ContextChipInteracted {
        chip_type: String,
        /// "opened"
        action: String,
        /// Whether or not Universal Developer Input mode is enabled
        is_udi_enabled: bool,
    },
    TabCloseButtonPositionUpdated {
        position: TabCloseButtonPosition,
    },
    AIExecutionProfileCreated,
    AIExecutionProfileDeleted,
    AIExecutionProfileSettingUpdated {
        setting_type: String,
        setting_value: String,
    },
    AIExecutionProfileAddedToAllowlist {
        list_type: String,
        value: String,
    },
    AIExecutionProfileAddedToDenylist {
        list_type: String,
        value: String,
    },
    AIExecutionProfileRemovedFromAllowlist {
        list_type: String,
        value: String,
    },
    AIExecutionProfileRemovedFromDenylist {
        list_type: String,
        value: String,
    },
    AIExecutionProfileModelSelected {
        model_type: String,
        model_value: String,
    },
    AIExecutionProfileContextWindowSelected {
        tokens: Option<u32>,
        model_id: String,
    },
    OpenSlashMenu {
        source: SlashMenuSource,
        /// Whether the inline slash commands UI is enabled.
        is_inline_ui_enabled: bool,
        /// Whether the menu was opened in the agent view vs terminal mode.
        is_in_agent_view: bool,
    },
    SlashCommandAccepted {
        command_details: SlashCommandAcceptedDetails,
        /// Whether the command was accepted in the agent view vs terminal mode.
        is_in_agent_view: bool,
    },
    AgentModeSetupBannerAccepted,
    AgentModeSetupBannerDismissed,
    AgentModeSetupProjectScopedRulesAction {
        action: AgentModeSetupProjectScopedRulesActionType,
    },

    AgentModeSetupCodebaseContextAction {
        action: AgentModeSetupCodebaseContextActionType,
    },
    AgentModeSetupCreateEnvironmentAction {
        action: AgentModeSetupCreateEnvironmentActionType,
    },
    /// User submitted a prompt from the create project view - metadata (non-UGC)
    CreateProjectPromptSubmitted {
        /// Whether this was a custom prompt or a predefined suggestion
        is_custom_prompt: bool,
        /// For suggested prompts, this is always collected. For custom prompts, this is None.
        suggested_prompt: Option<String>,
        /// Whether this was from the FTUX
        is_ftux: bool,
    },
    /// User submitted a custom prompt from the create project view - content (UGC)
    CreateProjectPromptSubmittedContent {
        /// The custom prompt content - only collected when UGC is enabled
        custom_prompt: String,
    },
    /// User submitted a repository URL from the clone repo view
    CloneRepoPromptSubmitted {
        is_ftux: bool,
    },
    /// From the first-time user "get started" page, skip straight to terminal without
    /// creating/opening a project/repository.
    GetStartedSkipToTerminal,

    /// User selected an item from the "Recent" list on the new tab zero state
    RecentMenuItemSelected {
        // The kind of recent menu item selected
        kind: &'static str,
    },

    /// User selected a folder to open as a repo from the "Open repository" button
    OpenRepoFolderSubmitted {
        is_ftux: bool,
    },

    /// User closed the "Out of credits" banner (dismissed or purchased credits)
    OutOfCreditsBannerClosed {
        action: OutOfCreditsBannerAction,
        selected_credits: Option<i32>,
        auto_reload_checkbox_enabled: bool,
        banner_toggle_flag_enabled: bool,
        post_purchase_modal_flag_enabled: bool,
    },

    /// User closed the auto-reload modal (either dismissed or enabled auto-reload)
    AutoReloadModalClosed {
        action: AutoReloadModalAction,
        selected_credits: Option<i32>,
        banner_toggle_flag_enabled: bool,
        post_purchase_modal_flag_enabled: bool,
    },

    /// User toggled auto-reload in Billing & Usage settings
    AutoReloadToggledFromBillingSettings {
        enabled: bool,
        banner_toggle_flag_enabled: bool,
        post_purchase_modal_flag_enabled: bool,
    },

    /// Emitted when the user toggles the Agent Management View.
    AgentManagementViewToggled {
        is_open: bool,
    },
    /// Emitted when the user opens a session from the Agent Management View.
    AgentManagementViewOpenedSession,
    /// Emitted when the user copies a session link from the Agent Management View.
    AgentManagementViewCopiedSessionLink,
    /// Detected that Warp is running in an isolated sandbox.
    DetectedIsolationPlatform {
        platform: rift_isolation_platform::IsolationPlatformType,
    },

    AgentTipShown {
        tip: String,
    },
    AgentTipClicked {
        tip: String,
        click_target: String,
    },
    /// Emitted when the user uses voice input from the CLI agent footer.
    CLIAgentToolbarVoiceInputUsed {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
    },
    /// Emitted when the user attaches an image from the CLI agent footer.
    CLIAgentToolbarImageAttached {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
    },
    /// Emitted when the CLI agent footer is shown.
    CLIAgentToolbarShown {
        /// The CLI agent being shown.
        cli_agent: CLIAgentType,
    },
    /// Emitted when the user opens the CLI agent rich input editor.
    CLIAgentRichInputOpened {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// How the editor was opened (Ctrl-G or footer button).
        entrypoint: CLIAgentInputEntrypoint,
    },
    /// Emitted when the CLI agent rich input editor is closed.
    CLIAgentRichInputClosed {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Why the editor was closed.
        reason: CLIAgentRichInputCloseReason,
    },
    /// Emitted when the user submits a prompt via the CLI agent rich input editor.
    CLIAgentRichInputSubmitted {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Length of the submitted prompt in characters.
        prompt_length: usize,
    },
    /// Emitted when the user clicks a plugin chip (install, update, or instructions).
    CLIAgentPluginChipClicked {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// The specific action taken.
        action: PluginChipTelemetryAction,
    },
    /// Emitted when the user dismisses the plugin chip.
    CLIAgentPluginChipDismissed {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Whether this was the install or update chip.
        chip_kind: PluginChipTelemetryKind,
    },
    /// Emitted when auto plugin install or update succeeds.
    CLIAgentPluginOperationSucceeded {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Whether this was an install or update operation.
        operation: PluginChipTelemetryKind,
    },
    /// Emitted when auto plugin install or update fails.
    CLIAgentPluginOperationFailed {
        /// The CLI agent being used.
        cli_agent: CLIAgentType,
        /// Whether this was an install or update operation.
        operation: PluginChipTelemetryKind,
    },
    /// Emitted when a CLI agent plugin is first recognized (SessionStart event received).
    CLIAgentPluginDetected {
        /// The CLI agent whose plugin was detected.
        cli_agent: CLIAgentType,
    },
    /// Emitted when the user toggles the CLI agent footer setting.
    ToggleCLIAgentToolbarSetting {
        /// Whether the setting is enabled or disabled.
        is_enabled: bool,
    },
    /// Emitted when the user toggles the "Use Agent" footer setting.
    ToggleUseAgentToolbarSetting {
        /// Whether the setting is enabled or disabled.
        is_enabled: bool,
    },
    /// Emitted when the inline conversation menu is opened.
    InlineConversationMenuOpened {
        /// Whether the menu was opened in the agent view vs terminal mode.
        is_in_agent_view: bool,
    },
    /// Emitted when an item is selected from the inline conversation menu.
    InlineConversationMenuItemSelected {
        /// Whether the item was selected in the agent view vs terminal mode.
        is_in_agent_view: bool,
    },
    /// Emitted when the agent shortcuts view visibility is toggled.
    AgentShortcutsViewToggled {
        /// Whether the shortcuts view is now visible.
        is_visible: bool,
    },
    /// Emitted when the Codex modal is opened.
    CodexModalOpened,
    /// Emitted when the user clicks "Use Codex" in the Codex modal.
    CodexModalUseCodexClicked,
    /// Emitted when the cloud agent capacity modal is opened.
    CloudAgentCapacityModalOpened,
    /// Emitted when the cloud agent capacity modal is dismissed.
    CloudAgentCapacityModalDismissed,
    /// Emitted when the user clicks the upgrade button in the cloud agent capacity modal.
    CloudAgentCapacityModalUpgradeClicked,
    /// Emitted when a warp://linear deeplink is opened.
    LinearIssueLinkOpened,
    /// Emitted when the free tier limit hit interstitial is displayed.
    FreeTierLimitHitInterstitialDisplayed,
    /// Emitted when the user clicks the "Upgrade" button in the free tier limit hit interstitial.
    FreeTierLimitHitInterstitialUpgradeButtonClicked,
    /// Emitted when the user clicks close on the free tier limit hit interstitial.
    FreeTierLimitHitInterstitialClosed,
    /// Emitted when the remote server binary check completes.
    RemoteServerBinaryCheck {
        found: bool,
        error: Option<String>,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when the remote server binary installation completes.
    /// `error` is `None` on success, `Some(reason)` on failure.
    RemoteServerInstallation {
        error: Option<String>,
        install_source: Option<remote_server::transport::InstallSource>,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when the remote server connection + initialization completes.
    /// `error` is `None` on success, `Some(reason)` on failure.
    RemoteServerInitialization {
        phase: remote_server::manager::RemoteServerInitPhase,
        error: Option<String>,
        remote_os: Option<String>,
        remote_arch: Option<String>,
        /// Exit code of the SSH subprocess, if available.
        /// Helps distinguish proxy crashes from transport failures.
        exit_code: Option<i32>,
        /// Whether the SSH subprocess was killed by a signal.
        signal_killed: Option<bool>,
        /// Last lines from the proxy's stderr, if available.
        /// Provides server-side context for why the proxy exited.
        proxy_stderr: Option<String>,
    },
    /// Emitted when an established remote server connection drops.
    RemoteServerDisconnection {
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when a client request to the remote server fails.
    RemoteServerClientRequestError {
        operation: remote_server::manager::RemoteServerOperation,
        error_type: remote_server::manager::RemoteServerErrorKind,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when a server message cannot be decoded (no parseable request_id).
    RemoteServerMessageDecodingError {
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when the full remote server setup flow completes successfully.
    RemoteServerSetupDuration {
        duration_ms: u64,
        installed_binary: bool,
        remote_os: Option<String>,
        remote_arch: Option<String>,
        /// Short description of the remote libc (e.g. "glibc 2.35",
        /// "musl", "unknown"). `None` when the preinstall check did
        /// not run (e.g. macOS hosts).
        remote_libc: Option<String>,
    },
    /// Emitted when the preinstall check classifies the remote host as
    /// unsupported by the prebuilt remote-server binary, so the controller
    /// silently falls back to the legacy SSH/`RemoteCommandExecutor`
    /// flow without surfacing an install prompt.
    RemoteServerHostUnsupported {
        remote_os: Option<String>,
        remote_arch: Option<String>,
        /// Typed unsupported reason. Converted into stable telemetry
        /// fields in `payload()`.
        unsupported_reason: remote_server::setup::UnsupportedReason,
        /// Detected libc on the remote host, e.g. `"glibc 2.28"`,
        /// `"musl"`, `"unknown"`.
        detected_libc: String,
    },
    /// Emitted when a reconnection attempt succeeds after a spontaneous disconnect.
    RemoteServerReconnection {
        attempt: u32,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when the remote server daemon process finishes startup and
    /// binds its Unix domain socket.  Reports the same `IntervalTimer`
    /// data that `AppStartup` reports for the GUI, but from the headless
    /// daemon process on the remote host.  Only emitted on success — if
    /// the daemon crashes before binding, no event is sent.
    RemoteServerDaemonStartup {
        timing_data: Vec<rift_core::interval_timer::TimingDataPoint>,
    },
    /// Emitted when all reconnection attempts are exhausted.
    RemoteServerReconnectExhausted {
        attempts: u32,
        remote_os: Option<String>,
        remote_arch: Option<String>,
        exit_code: Option<i32>,
        signal_killed: Option<bool>,
    },
    /// Emitted when the remote codebase index status changes.
    RemoteCodebaseIndexStatusChanged {
        state: remote_server::codebase_index_proto::RemoteCodebaseIndexState,
        previous_state: Option<remote_server::codebase_index_proto::RemoteCodebaseIndexState>,
        has_root_hash: bool,
        has_failure_message: bool,
        progress_completed: Option<u64>,
        progress_total: Option<u64>,
        mutation_kind: Option<remote_server::manager::RemoteCodebaseIndexUpdateOperation>,
        source: RemoteCodebaseIndexStatusTelemetrySource,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when auto-indexing requests one or more remote codebases.
    RemoteCodebaseAutoIndexRequested {
        trigger: RemoteCodebaseAutoIndexTrigger,
        requested_count: usize,
        remote_os: Option<String>,
        remote_arch: Option<String>,
    },
    /// Emitted when the user toggles the queued prompts panel collapse state.
    QueuedPromptPanelCollapseToggled {
        collapsed: bool,
    },
}

impl TelemetryEventTrait for TelemetryEvent {
    fn name(&self) -> &'static str {
        self.name()
    }

    fn payload(&self) -> Option<Value> {
        self.payload()
    }

    fn description(&self) -> &'static str {
        let discriminant: TelemetryEventDiscriminants = self.into();
        discriminant.description()
    }

    fn contains_ugc(&self) -> bool {
        self.contains_ugc()
    }

    fn enablement_state(&self) -> EnablementState {
        self.enablement_state()
    }

    fn event_descs() -> impl Iterator<Item = Box<dyn TelemetryEventDesc>> {
        rift_core::telemetry::enum_events::<Self>()
    }
}

impl TelemetryEvent {
    pub fn name(&self) -> &'static str {
        let discriminant: TelemetryEventDiscriminants = self.into();
        discriminant.name()
    }

    pub fn enablement_state(&self) -> EnablementState {
        let discriminant: TelemetryEventDiscriminants = self.into();
        discriminant.enablement_state()
    }

    pub fn payload(&self) -> Option<Value> {
        match self {
            TelemetryEvent::AutosuggestionInserted {
                insertion_length,
                buffer_length,
            } => {
                Some(json!({"insertion_length": insertion_length, "buffer_length": buffer_length}))
            }
            TelemetryEvent::AgentModeRewindDialogOpened { entrypoint } => {
                Some(json!({"entrypoint": entrypoint}))
            }
            TelemetryEvent::AgentModeRewindExecuted {
                num_blocks_reverted,
            } => Some(json!({"num_blocks_reverted": num_blocks_reverted})),
            TelemetryEvent::BootstrappingSlow(info) => Some(json!(info)),
            TelemetryEvent::BootstrappingSlowContents(info) => Some(json!(info)),
            TelemetryEvent::ToggleSettingsSync {
                is_settings_sync_enabled,
            } => Some(json!({ "is_settings_sync_enabled": is_settings_sync_enabled })),
            TelemetryEvent::SessionAbandonedBeforeBootstrap {
                pending_shell,
                has_pending_ssh_session,
                was_ever_visible,
                duration_since_start,
            } => Some(json!({
                "pending_shell": pending_shell.map(|shell| shell.name()),
                "has_pending_ssh_session": has_pending_ssh_session,
                "was_ever_visible": was_ever_visible,
                "duration_since_start_secs": duration_since_start.as_secs_f32(),
            })),
            TelemetryEvent::BlockCompleted {
                block_finished_to_precmd_delay_ms,
                honor_ps1_enabled,
                num_secrets_redacted,
                num_output_lines,
                num_output_lines_truncated,
                terminal_session_id,
                is_udi_enabled,
                is_in_agent_view,
            } => Some(json!({
                "block_finished_to_precmd_delay_ms": block_finished_to_precmd_delay_ms,
                "honor_ps1_enabled": honor_ps1_enabled,
                "num_secrets_redacted": num_secrets_redacted,
                "num_output_lines": num_output_lines,
                "num_output_lines_truncated": num_output_lines_truncated,
                "terminal_session_id": terminal_session_id,
                "is_udi_enabled": is_udi_enabled,
                "is_in_agent_view": is_in_agent_view,
            })),
            TelemetryEvent::ToggleFocusPaneOnHover { enabled } => Some(json!({
                "enabled": enabled,
            })),
            TelemetryEvent::BlockCompletedOnDogfoodOnly {
                block_finished_to_precmd_delay_ms,
                honor_ps1_enabled,
                num_secrets_redacted,
                num_output_lines,
                num_output_lines_truncated,
                command,
                duration,
                exit_code,
                terminal_session_id,
            } => Some(json!({
                "block_finished_to_precmd_delay_ms": block_finished_to_precmd_delay_ms,
                "honor_ps1_enabled": honor_ps1_enabled,
                "num_secrets_redacted": num_secrets_redacted,
                "num_output_lines": num_output_lines,
                "num_output_lines_truncated": num_output_lines_truncated,
                "command": command,
                "duration": duration,
                "exit_code": exit_code,
                "terminal_session_id": terminal_session_id,
            })),
            TelemetryEvent::BootstrappingSucceeded(info) => Some(json!(info)),
            TelemetryEvent::SSHBootstrapAttempt(remote_shell) => {
                Some(json!({ "shell": remote_shell.as_str() }))
            }
            TelemetryEvent::OpenContextMenu { context_menu_info } => Some(
                json!({ "type": context_menu_info.type_for_telemetry(), "open_method": context_menu_info.open_method_for_telemetry() }),
            ),
            TelemetryEvent::ContextMenuCopy(entity, cardinality) => {
                Some(json!({ "entity": entity.as_str(), "cardinality": cardinality }))
            }
            TelemetryEvent::ContextMenuFindWithinBlocks(cardinality) => {
                Some(json!({ "cardinality": cardinality }))
            }
            TelemetryEvent::ContextMenuOpenShareModal(cardinality) => {
                Some(json!({ "cardinality": cardinality }))
            }
            TelemetryEvent::ContextMenuCopyPrompt { part } => Some(json!({ "part": part })),
            TelemetryEvent::ReinputCommands(cardinality) => {
                Some(json!({ "cardinality": cardinality }))
            }
            TelemetryEvent::ContextMenuToggleGitPromptDirtyIndicator { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::BlockSelection(details) => Some(json!(details)),
            TelemetryEvent::OpenSuggestionsMenu(mode) => Some(json!(mode)),
            TelemetryEvent::ConfirmSuggestion { mode, match_type } => {
                Some(json!({ "mode": mode, "match_type": match_type }))
            }
            TelemetryEvent::EditorUnhandledModifierKey(normalized_keystroke) => {
                Some(json!(normalized_keystroke.as_str()))
            }
            TelemetryEvent::ThemeSelection { theme, entrypoint } => {
                Some(json!({ "theme": theme, "entrypoint": entrypoint }))
            }
            TelemetryEvent::AppIconSelection { icon } => Some(json!({"icon": icon})),
            TelemetryEvent::CursorDisplayType {
                cursor: cursor_display_type,
            } => Some(json!({"cursor": cursor_display_type})),
            TelemetryEvent::ObjectLinkCopied { link } => Some(json!({"link": link})),
            TelemetryEvent::FileTreeToggled {
                source,
                is_code_mode_v2,
                cli_agent,
            } => Some(
                json!({"source": source, "is_code_mode_v2": is_code_mode_v2, "cli_agent": cli_agent}),
            ),
            TelemetryEvent::FileTreeItemAttachedAsContext { is_directory } => {
                Some(json!({"is_directory": is_directory}))
            }
            TelemetryEvent::ToggleRestoreSession(enabled) => Some(json!({ "enabled": enabled })),
            TelemetryEvent::DatabaseStartUpError(error) => Some(json!(error)),
            TelemetryEvent::DatabaseReadError(error) => Some(json!(error)),
            TelemetryEvent::DatabaseWriteError(error) => Some(json!(error)),
            TelemetryEvent::AppStartup(info) => Some(json!(info)),
            TelemetryEvent::DownloadSource(source) => Some(json!(source)),
            TelemetryEvent::BaselineCommandLatency(info) => Some(json!(info)),
            TelemetryEvent::KeybindingChanged { action, keystroke } => {
                Some(json!({ "action": action, "keystroke": keystroke.normalized() }))
            }
            TelemetryEvent::KeybindingResetToDefault { action } => {
                Some(json!({ "action": action }))
            }
            TelemetryEvent::KeybindingRemoved { action } => Some(json!({ "action": action })),
            TelemetryEvent::FeaturesPageAction { action, value } => {
                Some(json!({"action": action, "value": value}))
            }
            TelemetryEvent::CompleteWelcomeTipFeature {
                total_completed_count,
                tip_name,
            } => Some(
                json!({ "total_completed_count": total_completed_count, "tip_name": tip_name }),
            ),
            TelemetryEvent::NotificationsDiscoveryBannerAction(action) => {
                Some(json!({ "action": action }))
            }
            TelemetryEvent::InputModeChanged { old_mode, new_mode } => {
                Some(json!({ "old_mode": old_mode, "new_mode": new_mode }))
            }
            TelemetryEvent::NotificationsErrorBannerAction(action) => {
                Some(json!({ "action": action }))
            }
            TelemetryEvent::NotificationPermissionsRequested { source, trigger } => {
                Some(json!({ "source": source, "trigger": trigger }))
            }
            TelemetryEvent::NotificationFailedToSend { error } => Some(json!({ "error": error })),
            TelemetryEvent::NotificationsRequestPermissionsOutcome { outcome } => {
                Some(json!({ "outcome": outcome }))
            }
            TelemetryEvent::ToggleFindOption { option, enabled } => {
                Some(json!({ "option": option, "enabled": enabled }))
            }
            TelemetryEvent::SelectCommandPaletteOption(option) => Some(json!({ "option": option })),
            TelemetryEvent::PaletteSearchOpened { mode, source } => {
                Some(json!({ "mode": mode, "source": source }))
            }
            TelemetryEvent::PaletteSearchResultAccepted {
                result_type,
                filter: mode,
                buffer_length,
            } => Some(
                json!({ "result_type": result_type, "mode": mode, "buffer_length": buffer_length }),
            ),
            TelemetryEvent::PaletteSearchExited {
                filter: mode,
                buffer_length,
            } => Some(json!({ "mode": mode, "buffer_length": buffer_length })),
            TelemetryEvent::AuthCommonQuestionClicked { question } => Some(json!(question)),
            TelemetryEvent::AuthToggleFAQ { open } => {
                let payload = if *open { "open" } else { "close" };
                Some(json!(payload))
            }
            TelemetryEvent::TabRenamed(rename_event) => Some(json!(rename_event)),
            TelemetryEvent::MoveActiveTab { direction } => Some(json!({ "direction": direction })),
            TelemetryEvent::MoveTab { direction } => Some(json!({ "direction": direction })),
            TelemetryEvent::TabOperations { action } => Some(json!({ "action": action })),
            TelemetryEvent::ThinStrokesSettingChanged { new_value } => {
                Some(json!({ "new_value": new_value }))
            }
            TelemetryEvent::BookmarkBlockToggled { enable_bookmark } => {
                Some(json!({ "enable_bookmark": enable_bookmark }))
            }
            TelemetryEvent::OpenLink { link, open_with } => {
                Some(json!({"link_type": link, "open_with": open_with}))
            }
            TelemetryEvent::OpenChangelogLink { url } => Some(json!({ "url": url })),
            TelemetryEvent::CommandXRayTriggered { trigger } => Some(json!({ "trigger": trigger })),
            TelemetryEvent::SaveLaunchConfig { state } => Some(json!({ "state": state })),
            TelemetryEvent::SaveAsWorkflowModal { source } => Some(json!({ "source": source })),
            TelemetryEvent::CommandCorrection { event } => Some(json!({ "event": event })),
            TelemetryEvent::SetLineHeight { new_value } => Some(json!({ "new_value": new_value })),
            TelemetryEvent::CommandSearchOpened { has_initial_query } => {
                Some(json!({ "has_initial_query": has_initial_query }))
            }
            TelemetryEvent::CommandSearchExited {
                buffer_length,
                query_filter,
            } => Some(json!({ "buffer_length": buffer_length, "query_filter": query_filter })),
            TelemetryEvent::CommandSearchResultAccepted {
                result_index,
                result_type,
                query_filter,
                buffer_length,
                was_immediately_executed,
            } => Some(json!({
                "result_index": result_index,
                "result_type": result_type,
                "query_filter": query_filter,
                "buffer_length": buffer_length,
                "was_immediately_executed": was_immediately_executed
            })),
            TelemetryEvent::CommandSearchFilterChanged { new_filter } => {
                Some(json!({ "new_filter": new_filter }))
            }
            TelemetryEvent::AICommandSearchOpened { entrypoint } => {
                Some(json!({ "entrypoint": entrypoint }))
            }
            TelemetryEvent::UserInitiatedClose { initiated_on } => {
                Some(json!({ "initiated_on": initiated_on }))
            }
            TelemetryEvent::QuitModalShown {
                running_processes,
                shared_sessions,
                modal_for,
            } => Some(
                json!({ "running_processes": running_processes, "shared_sessions": shared_sessions, "modal_for": modal_for }),
            ),
            TelemetryEvent::QuitModalCancel {
                nav_palette,
                modal_for,
            } => Some(json!({ "nav_palette": nav_palette, "modal_for": modal_for })),
            TelemetryEvent::LogOutModalCancel { nav_palette } => {
                Some(json!({ "nav_palette": nav_palette }))
            }
            TelemetryEvent::SetBlurRadius { blur_radius } => {
                Some(json!({ "blur_radius": blur_radius }))
            }
            TelemetryEvent::SetOpacity { opacity } => Some(json!({ "opacity": opacity })),
            TelemetryEvent::ToggleDimInactivePanes { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleJumpToBottomofBlockButton { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::PtySpawned { mode } => Some(json!({ "mode": mode })),
            TelemetryEvent::InitialWorkingDirectoryConfigurationChanged {
                advanced_mode_enabled,
            } => Some(json!({ "advanced_mode_enabled": advanced_mode_enabled })),
            TelemetryEvent::OpenedWarpAI { source } => Some(json!({ "source": source })),
            TelemetryEvent::WarpAIRequestIssued { result } => Some(json!({ "result": result })),
            TelemetryEvent::WarpAIAction { action_type } => {
                Some(json!({ "action_type": action_type }))
            }
            TelemetryEvent::KnowledgePaneOpened { entrypoint } => {
                Some(json!({ "entrypoint": entrypoint }))
            }
            #[cfg(feature = "local_fs")]
            TelemetryEvent::CodePanelsFileOpened { entrypoint, target } => {
                let (target, layout, editor) = match target {
                    FileTarget::MarkdownViewer(layout) => {
                        ("warp_markdown_viewer", Some(*layout), None)
                    }
                    FileTarget::CodeEditor(layout) => ("warp_code_editor", Some(*layout), None),
                    FileTarget::EnvEditor => ("env_editor", None, None),
                    FileTarget::SystemDefault => ("system_default", None, None),
                    FileTarget::SystemGeneric => ("system_generic", None, None),
                    FileTarget::ExternalEditor(editor) => ("external_editor", None, Some(*editor)),
                };

                Some(json!({
                    "entrypoint": entrypoint,
                    "target": target,
                    "layout": layout,
                    "editor": editor,
                }))
            }
            #[cfg(feature = "local_fs")]
            TelemetryEvent::PreviewPanePromoted => None,
            TelemetryEvent::CodeSelectionAddedAsContext { destination } => Some(json!({
                "destination": destination,
            })),
            TelemetryEvent::UsedWarpAIPreparedPrompt { prompt } => {
                Some(json!({ "prompt": prompt }))
            }
            TelemetryEvent::ExperimentTriggered {
                experiment,
                layer,
                group_assignment,
            } => Some(
                json!({ "experiment": experiment, "layer": layer, "group_assignment": group_assignment }),
            ),
            TelemetryEvent::ToggleSyncAllPanesInAllTabs { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleSyncAllPanesInTab { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleTabIndicators { enabled } => Some(json!({ "enabled": enabled })),
            TelemetryEvent::TogglePreserveActiveTabColor { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::DeclineSubshellBootstrap { remember } => {
                Some(json!({ "remember": remember }))
            }
            TelemetryEvent::AgentToolbarDismissed => None,
            TelemetryEvent::WarpifyFooterShown { is_ssh }
            | TelemetryEvent::WarpifyFooterAcceptedWarpify { is_ssh } => {
                Some(json!({ "is_ssh": is_ssh }))
            }
            TelemetryEvent::ToggleSameLinePrompt { enabled } => Some(json!({ "enabled": enabled })),
            TelemetryEvent::TriggerSubshellBootstrap {
                triggered_by_rc_file_snippet,
            } => Some(json!({
                "triggered_by_rc_file_snippet": triggered_by_rc_file_snippet
            })),
            TelemetryEvent::OpenLaunchConfig {
                ui_location,
                open_in_active_window,
            } => Some(
                json!({ "ui_location": ui_location, "open_in_active_window": open_in_active_window }),
            ),
            TelemetryEvent::ToggleWarpAI { opened } => Some(json!({ "opened": opened })),
            TelemetryEvent::ToggleSecretRedaction { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleObfuscateSecret { interaction } => {
                Some(json!({ "interaction": interaction }))
            }
            TelemetryEvent::AutoGenerateMetadataError { error_payload } => {
                Some(json!({ "error": error_payload }))
            }
            TelemetryEvent::UndoClose { item_type } => Some(json!({ "item_type": item_type })),
            TelemetryEvent::PromptEdited { prompt, entrypoint } => Some(json!({
                "prompt": prompt,
                "entrypoint": entrypoint
            })),
            TelemetryEvent::OpenPromptEditor { entrypoint } => {
                Some(json!({ "entrypoint": entrypoint }))
            }
            TelemetryEvent::PtyThroughput {
                max_bytes_per_second,
            } => Some(json!({
                "max_bytes_per_second": max_bytes_per_second,
            })),
            TelemetryEvent::GenerateBlockSharingLink {
                share_type,
                display_setting,
                show_prompt,
                redact_secrets,
            } => Some(
                json!({"share_type": share_type, "display_setting": display_setting, "show_prompt": show_prompt, "redact_secrets": redact_secrets}),
            ),
            TelemetryEvent::CopyBlockSharingLink(share_type) => {
                Some(json!({ "share_type": share_type }))
            }
            TelemetryEvent::PageUpDownInEditorPressed {
                is_empty_editor,
                is_down,
            } => Some(json!({"is_empty_editor": is_empty_editor, "is_down": is_down})),
            TelemetryEvent::UnsupportedShell { shell } => Some(json!({ "shell": shell })),
            TelemetryEvent::OpenAndWarpifyDockerSubshell { shell_type } => {
                Some(json!({ "shell_type": shell_type }))
            }
            TelemetryEvent::ToggleBlockFilterQuery { enabled, source } => {
                Some(json!({"enabled": enabled, "source": source}))
            }
            TelemetryEvent::ToggleBlockFilterRegex { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleShowBlockDividers { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleBlockFilterCaseSensitivity { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::ToggleBlockFilterInvert { enabled } => {
                Some(json!({ "enabled": enabled }))
            }
            TelemetryEvent::UpdateBlockFilterQueryContextLines { num_context_lines } => {
                Some(json!({ "num_context_lines": num_context_lines }))
            }
            TelemetryEvent::ToggleNewWindowsAtCustomSize { enabled } => {
                Some(json!({"enabled": enabled}))
            }
            TelemetryEvent::ToggleSshTmuxWrapper { enabled } => Some(json!({"enabled": enabled})),
            TelemetryEvent::ToggleSshWarpification { enabled } => Some(json!({"enabled": enabled})),
            TelemetryEvent::SetSshExtensionInstallMode { mode } => Some(json!({"mode": mode})),
            TelemetryEvent::SshRemoteServerChoiceDoNotAskAgainToggled { checked } => {
                Some(json!({"checked": checked}))
            }
            TelemetryEvent::SshInteractiveSessionDetected(ssh_interactive_session_detected) => {
                Some(json!({"ssh_interactive_session": ssh_interactive_session_detected}))
            }
            TelemetryEvent::SshTmuxWarpificationSuccess {
                duration_ms,
                tmux_installation,
            } => Some(json!({
                "duration_ms": duration_ms,
                "tmux_installation": *tmux_installation,
            })),
            TelemetryEvent::SshTmuxWarpificationErrorBlock {
                error,
                tmux_installation,
            } => Some(json!({
                "error": error,
                "tmux_installation": *tmux_installation,
            })),
            TelemetryEvent::JoinedSharedSession {
                session_id,
                source_type,
            } => Some(json!({
                "session_id": session_id,
                "source_type": source_type,
            })),
            TelemetryEvent::SharerCancelledGrantRole { role } => Some(json!({ "role": role })),
            TelemetryEvent::JumpToSharedSessionParticipant { jumped_to } => {
                Some(json!({ "jumped_to": jumped_to }))
            }
            TelemetryEvent::ToggleSnackbarInActivePane { show_snackbar } => {
                Some(json!({ "show_snackbar": show_snackbar }))
            }
            TelemetryEvent::PaneDropped { drop_location } => {
                Some(json!({ "location": drop_location }))
            }
            TelemetryEvent::InviteTeammates {
                num_teammates,
                team_uid,
            } => Some(json!({"num_teammates": num_teammates, "team_uid": team_uid})),
            TelemetryEvent::TierLimitHit(event) => Some(json!(event)),
            TelemetryEvent::AgentModeUserAttemptedQueryAtRequestLimit { limit } => {
                Some(json!({"limit": limit}))
            }
            TelemetryEvent::AgentModeClickedEntrypoint { entrypoint } => {
                Some(json!({"entrypoint": entrypoint}))
            }
            TelemetryEvent::AgentModeAttachedBlockContext { method } => {
                Some(json!({"method": method}))
            }
            TelemetryEvent::AgentModeToggleAutoDetectionSetting {
                is_autodetection_enabled,
                origin,
            } => Some(
                json!({"is_autodetection_enabled": is_autodetection_enabled, "origin": origin }),
            ),
            TelemetryEvent::ToggleIntelligentAutosuggestionsSetting {
                is_intelligent_autosuggestions_enabled,
            } => Some(
                json!({"is_intelligent_autosuggestions_enabled": is_intelligent_autosuggestions_enabled}),
            ),
            // Using legacy name to avoid breaking telemetry.
            TelemetryEvent::TogglePromptSuggestionsSetting {
                is_prompt_suggestions_enabled,
            } => Some(
                json!({"is_agent_mode_query_suggestions_enabled": is_prompt_suggestions_enabled}),
            ),
            TelemetryEvent::ToggleCodeSuggestionsSetting {
                source,
                is_code_suggestions_enabled,
            } => Some(
                json!({"source": source, "is_code_suggestions_enabled": is_code_suggestions_enabled}),
            ),
            TelemetryEvent::ToggleNaturalLanguageAutosuggestionsSetting {
                is_natural_language_autosuggestions_enabled,
            } => Some(
                json!({"is_natural_language_autosuggestions_enabled": is_natural_language_autosuggestions_enabled}),
            ),
            TelemetryEvent::ToggleSharedBlockTitleGenerationSetting {
                is_shared_block_title_generation_enabled,
            } => Some(
                json!({"is_shared_block_title_generation_enabled": is_shared_block_title_generation_enabled}),
            ),
            TelemetryEvent::ToggleGitOperationsAutogenSetting {
                is_git_operations_autogen_enabled,
            } => Some(
                json!({"is_git_operations_autogen_enabled": is_git_operations_autogen_enabled}),
            ),
            TelemetryEvent::ToggleVoiceInputSetting {
                is_voice_input_enabled,
            } => Some(json!({"is_voice_input_enabled": is_voice_input_enabled})),
            TelemetryEvent::AgentModePotentialAutoDetectionFalsePositive(
                AgentModeAutoDetectionFalsePositivePayload::InternalDogfoodUsers { input_text },
            ) => Some(json!({"input_text": input_text})),
            TelemetryEvent::PromptSuggestionShown {
                id,
                request_duration_ms,
                block_id,
                view,
                server_request_token,
            } => Some(json!({
                "id": id,
                "request_duration_ms": request_duration_ms,
                "block_id": block_id,
                "view": view,
                "server_request_token": server_request_token,
            })),
            TelemetryEvent::SuggestedCodeDiffFailed {
                prompt_suggestion_id,
                reason,
            } => Some(json!({
                "prompt_suggestion_id": prompt_suggestion_id,
                "reason": reason,
            })),
            TelemetryEvent::PromptSuggestionAccepted {
                id,
                view,
                interaction_source,
            } => Some(json!({
                "id": id,
                "view": view,
                "interaction_source": interaction_source,
            })),
            TelemetryEvent::StaticPromptSuggestionsBannerShown {
                id,
                query,
                block_id,
                block_command,
                static_prompt_suggestion_name,
                request_duration_ms,
                view,
            } => Some(json!({
                "id": id,
                "query": query,
                "block_id": block_id,
                "block_command": block_command,
                "static_prompt_suggestion_name": static_prompt_suggestion_name,
                "request_duration_ms": request_duration_ms,
                "view": view,
            })),
            TelemetryEvent::StaticPromptSuggestionAccepted {
                id,
                view,
                interaction_source,
            } => Some(json!({
                "id": id,
                "view": view,
                "interaction_source": interaction_source,
            })),
            TelemetryEvent::ResourceUsageStats { cpu, mem } => Some(json!({
                "cpu": cpu,
                "mem": {
                    // Only report the total application usage; skip sending
                    // the additional, more detailed usage information.
                    "total_application_usage_bytes": mem.total_application_usage_bytes,
                },
            })),
            TelemetryEvent::MemoryUsageStats {
                total_application_usage_bytes,
                total_blocks,
                total_lines,
                active_block_stats,
                inactive_5m_stats,
                inactive_1h_stats,
                inactive_24h_stats,
            } => Some(json!({
                "total_application_usage_bytes": total_application_usage_bytes,
                "total_blocks": total_blocks,
                "total_lines": total_lines,
                "active_block_stats": active_block_stats,
                "inactive_5m_stats": inactive_5m_stats,
                "inactive_1h_stats": inactive_1h_stats,
                "inactive_24h_stats": inactive_24h_stats
            })),
            TelemetryEvent::MemoryUsageHigh {
                total_application_usage_bytes,
                memory_breakdown,
            } => Some(json!({
                "total_application_usage_bytes": total_application_usage_bytes,
                "memory_breakdown": memory_breakdown,
            })),
            TelemetryEvent::CompletedSettingsImport {
                terminal_type,
                imported_settings,
            } => Some(
                json!({ "terminal_type": terminal_type, "imported_settings": imported_settings}),
            ),
            TelemetryEvent::SettingsImportConfigParsed {
                timing_data,
                terminal_type,
                settings_shown_to_user,
            } => Some(
                json!({"timing_data": timing_data,  "terminal_type": terminal_type, "settings_shown_to_user": settings_shown_to_user}),
            ),
            TelemetryEvent::SettingsImportConfigFocused(terminal_type_and_profile) => {
                Some(json!({"terminal_and_type_profile": terminal_type_and_profile}))
            }
            TelemetryEvent::InitiateAnonymousUserSignup { entrypoint } => {
                Some(json!({"entrypoint": entrypoint}))
            }
            TelemetryEvent::AnonymousUserAttemptLoginGatedFeature { feature } => {
                Some(json!({"feature": feature}))
            }
            TelemetryEvent::ToggleWorkspaceDecorationVisibility {
                previous_value,
                new_value,
            } => Some(json!({
                "previous_value": previous_value,
                "new_value": new_value,
            })),
            TelemetryEvent::UpdateAltScreenPaddingMode { new_mode } => Some(json!({
                "new_mode": new_mode,
            })),
            TelemetryEvent::AddTabWithShell { source, shell } => {
                Some(json!({ "source": source, "shell": shell }))
            }
            TelemetryEvent::ToggleGlobalAI { is_ai_enabled } => {
                Some(json!({"is_ai_enabled": is_ai_enabled}))
            }
            TelemetryEvent::ToggleActiveAI {
                is_active_ai_enabled,
            } => Some(json!({"is_active_ai_enabled": is_active_ai_enabled})),
            TelemetryEvent::ToggleCodebaseContext {
                is_codebase_context_enabled,
            } => Some(json!( {
                "is_codebase_context_enabled": is_codebase_context_enabled
            })),
            TelemetryEvent::ToggleAutoIndexing {
                is_autoindexing_enabled,
            } => Some(json!({
                "is_autoindexing_enabled": is_autoindexing_enabled
            })),
            TelemetryEvent::ActiveIndexedReposChanged {
                updated_number_of_codebase_indices,
                hit_max_indices,
            } => Some(json!({
                "updated_number_of_codebase_indices": updated_number_of_codebase_indices,
                "hit_max_indices": hit_max_indices
            })),
            TelemetryEvent::ToggleLigatureRendering { enabled } => {
                Some(json!({"enabled": enabled}))
            }
            TelemetryEvent::AutoupdateRelaunchAttempt { new_version } => Some(json!({
                "new_version": new_version,
            })),
            TelemetryEvent::ToggledAgentModeAutoexecuteReadonlyCommandsSetting { src, enabled } => {
                Some(json!({
                    "source": src,
                    "enabled": enabled,
                }))
            }
            TelemetryEvent::ChangedAgentModeCodingPermissions { src, new } => Some(json!({
                "source": src,
                "new": new,
            })),
            TelemetryEvent::RepoOutlineConstructionSuccess {
                total_parse_seconds,
                file_count,
            } => Some(json!({
                "total_parse_seconds": total_parse_seconds,
                "file_count": file_count,
            })),
            TelemetryEvent::RepoOutlineConstructionFailed { error } => Some(json!({
                "error": error,
            })),
            TelemetryEvent::AttachedImagesToAgentModeQuery {
                num_images,
                is_udi_enabled,
            } => Some(json!({
                "num_images": num_images,
                "is_udi_enabled": is_udi_enabled,
            })),
            TelemetryEvent::ImageReceived { image_protocol } => Some(json!({
                "image_protocol": image_protocol,
            })),
            TelemetryEvent::ShellTerminatedPrematurely {
                shell_type,
                shell_path,
                reason,
                reason_details,
                antivirus_name,
                long_os_version,
                exit_reason,
            } => Some(json!({
                "shell_type": shell_type,
                "shell_path": shell_path,
                "reason": reason,
                "reason_details": reason_details,
                "antivirus_name": antivirus_name,
                "long_os_version": long_os_version,
                "exit_reason": exit_reason,
            })),
            TelemetryEvent::InputUXModeChanged {
                is_udi_enabled,
                origin,
            } => Some(json!({
                "is_udi_enabled": is_udi_enabled,
                "origin": origin,
            })),
            TelemetryEvent::ContextChipInteracted {
                chip_type,
                action,
                is_udi_enabled,
            } => Some(json!({
                "chip_type": chip_type,
                "action": action,
                "is_udi_enabled": is_udi_enabled,
            })),
            TelemetryEvent::TabCloseButtonPositionUpdated { position } => Some(json!({
                "position": position,
            })),
            TelemetryEvent::BackgroundBlockStarted
            | TelemetryEvent::SessionCreation
            | TelemetryEvent::Login
            | TelemetryEvent::ContextMenuInsertSelectedText
            | TelemetryEvent::ContextMenuCopySelectedText
            | TelemetryEvent::JumpToPreviousCommand
            | TelemetryEvent::TabSingleResultAutocompletion
            | TelemetryEvent::CopyInviteLink
            | TelemetryEvent::OpenThemeChooser
            | TelemetryEvent::OpenThemeCreatorModal
            | TelemetryEvent::CreateCustomTheme
            | TelemetryEvent::DeleteCustomTheme
            | TelemetryEvent::SplitPane
            | TelemetryEvent::UnableToAutoUpdateToNewVersion
            | TelemetryEvent::SkipOnboardingSurvey
            | TelemetryEvent::LoggedOutStartup
            | TelemetryEvent::OpenWorkflowSearch
            | TelemetryEvent::OpenQuakeModeWindow
            | TelemetryEvent::OpenWelcomeTips
            | TelemetryEvent::DismissWelcomeTips
            | TelemetryEvent::ShowNotificationsDiscoveryBanner
            | TelemetryEvent::ShowNotificationsErrorBanner
            | TelemetryEvent::NotificationClicked
            | TelemetryEvent::SignUpButtonClicked
            | TelemetryEvent::OpenNewSessionFromFilePath
            | TelemetryEvent::OpenTeamFromURI
            | TelemetryEvent::SelectNavigationPaletteItem
            | TelemetryEvent::DragAndDropTab
            | TelemetryEvent::DragAndDropTabGroup
            | TelemetryEvent::EditedInputBeforePrecmd
            | TelemetryEvent::TriedToExecuteBeforePrecmd
            | TelemetryEvent::JumpToBookmark
            | TelemetryEvent::JumpToLatestAgentMessage
            | TelemetryEvent::JumpToBottomofBlockButtonClicked
            | TelemetryEvent::ShowInFileExplorer
            | TelemetryEvent::OpenLaunchConfigSaveModal
            | TelemetryEvent::OpenLaunchConfigFile
            | TelemetryEvent::TeamCreated
            | TelemetryEvent::TeamJoined
            | TelemetryEvent::TeamLeft
            | TelemetryEvent::TeamLinkCopied
            | TelemetryEvent::RemovedUserFromTeam
            | TelemetryEvent::DeletedWorkflow
            | TelemetryEvent::DeletedNotebook
            | TelemetryEvent::ToggleApprovalsModal
            | TelemetryEvent::SendEmailInvites
            | TelemetryEvent::ResourceCenterOpened
            | TelemetryEvent::ResourceCenterTipsCompleted
            | TelemetryEvent::ResourceCenterTipsSkipped
            | TelemetryEvent::KeybindingsPageOpened
            | TelemetryEvent::OpenedAltScreenFind
            | TelemetryEvent::QuitModalDisabled
            | TelemetryEvent::UserInitiatedLogOut
            | TelemetryEvent::LogOutModalShown
            | TelemetryEvent::WarpAICharacterLimitExceeded
            | TelemetryEvent::OpenInputContextMenu
            | TelemetryEvent::InputCutSelectedText
            | TelemetryEvent::InputCopySelectedText
            | TelemetryEvent::InputSelectAll
            | TelemetryEvent::InputPaste
            | TelemetryEvent::InputCommandSearch
            | TelemetryEvent::InputAICommandSearch
            | TelemetryEvent::InputAskWarpAI
            | TelemetryEvent::SetNewWindowsAtCustomSize
            | TelemetryEvent::DisableInputSync
            | TelemetryEvent::ShowSubshellBanner
            | TelemetryEvent::SshTmuxWarpifyBannerDisplayed
            | TelemetryEvent::AddDenylistedSubshellCommand
            | TelemetryEvent::RemoveDenylistedSubshellCommand
            | TelemetryEvent::AddAddedSubshellCommand
            | TelemetryEvent::RemoveAddedSubshellCommand
            | TelemetryEvent::ReceivedSubshellRcFileDcs
            | TelemetryEvent::AddDenylistedSshTmuxWrapperHost
            | TelemetryEvent::RemoveDenylistedSshTmuxWrapperHost
            | TelemetryEvent::SshTmuxWarpifyBlockAccepted
            | TelemetryEvent::SshTmuxWarpifyBlockDismissed
            | TelemetryEvent::SshInstallTmuxBlockDisplayed
            | TelemetryEvent::SshInstallTmuxBlockAccepted
            | TelemetryEvent::SshInstallTmuxBlockDismissed
            | TelemetryEvent::ShowAliasExpansionBanner
            | TelemetryEvent::EnableAliasExpansionFromBanner
            | TelemetryEvent::DismissAliasExpansionBanner
            | TelemetryEvent::ShowVimKeybindingsBanner
            | TelemetryEvent::EnableVimKeybindingsFromBanner
            | TelemetryEvent::DismissVimKeybindingsBanner
            | TelemetryEvent::InitiateReauth
            | TelemetryEvent::NeedsReauth
            | TelemetryEvent::AnonymousUserExpirationLockout
            | TelemetryEvent::AnonymousUserLinkedFromBrowser
            | TelemetryEvent::AnonymousUserHitCloudObjectLimit
            | TelemetryEvent::CustomSecretRegexAdded
            | TelemetryEvent::CopySecret
            | TelemetryEvent::AutoGenerateMetadataSuccess
            | TelemetryEvent::CommandFileRun
            | TelemetryEvent::SharerGrantModalDontShowAgain
            | TelemetryEvent::LogOut
            | TelemetryEvent::UpdateBlockFilterQuery
            | TelemetryEvent::BlockFilterToolbeltButtonClicked
            | TelemetryEvent::PaneDragInitiated
            | TelemetryEvent::SharedObjectLimitHitBannerViewPlansButtonClicked
            | TelemetryEvent::SharedSessionModalUpgradePressed
            | TelemetryEvent::AgentModePotentialAutoDetectionFalsePositive(
                AgentModeAutoDetectionFalsePositivePayload::ExternalUsers,
            )
            | TelemetryEvent::SettingsImportResetButtonClicked
            | TelemetryEvent::ITermMultipleHotkeys
            | TelemetryEvent::DriveSharingOnboardingBlockShown
            | TelemetryEvent::SettingsImportInitiated
            | TelemetryEvent::GrepToolSucceeded
            | TelemetryEvent::FileGlobToolSucceeded
            | TelemetryEvent::UserMenuUpgradeClicked
            | TelemetryEvent::AIExecutionProfileCreated
            | TelemetryEvent::AIExecutionProfileDeleted
            | TelemetryEvent::FileTreeItemCreated
            | TelemetryEvent::ConversationListItemDeleted
            | TelemetryEvent::ConversationListViewOpened
            | TelemetryEvent::GlobalSearchOpened
            | TelemetryEvent::GlobalSearchQueryStarted
            | TelemetryEvent::GetStartedSkipToTerminal => None,
            TelemetryEvent::SSHControlMasterError { has_remote_server } => Some(json!({
                "has_remote_server": has_remote_server,
            })),
            TelemetryEvent::RemoteServerBinaryCheck {
                found,
                error,
                remote_os,
                remote_arch,
            } => Some(json!({
                "found": found,
                "error": error,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerInstallation {
                error,
                install_source,
                remote_os,
                remote_arch,
            } => Some(json!({
                "error": error,
                "install_source": install_source,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerInitialization {
                phase,
                error,
                remote_os,
                remote_arch,
                exit_code,
                signal_killed,
                proxy_stderr,
            } => Some(json!({
                "phase": phase,
                "error": error,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
                "exit_code": exit_code,
                "signal_killed": signal_killed,
                "proxy_stderr": proxy_stderr,
            })),
            TelemetryEvent::RemoteServerDisconnection {
                remote_os,
                remote_arch,
            } => Some(json!({
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerReconnection {
                attempt,
                remote_os,
                remote_arch,
            } => Some(json!({
                "attempt": attempt,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerReconnectExhausted {
                attempts,
                remote_os,
                remote_arch,
                exit_code,
                signal_killed,
            } => Some(json!({
                "attempts": attempts,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
                "exit_code": exit_code,
                "signal_killed": signal_killed,
            })),
            TelemetryEvent::RemoteServerClientRequestError {
                operation,
                error_type,
                remote_os,
                remote_arch,
            } => Some(json!({
                "operation": operation,
                "error_type": error_type,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerMessageDecodingError {
                remote_os,
                remote_arch,
            } => Some(json!({
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteCodebaseIndexStatusChanged {
                state,
                previous_state,
                has_root_hash,
                has_failure_message,
                progress_completed,
                progress_total,
                mutation_kind,
                source,
                remote_os,
                remote_arch,
            } => Some(json!({
                "state": state,
                "previous_state": previous_state,
                "has_root_hash": has_root_hash,
                "has_failure_message": has_failure_message,
                "progress_completed": progress_completed,
                "progress_total": progress_total,
                "mutation_kind": mutation_kind,
                "source": source,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteCodebaseAutoIndexRequested {
                trigger,
                requested_count,
                remote_os,
                remote_arch,
            } => Some(json!({
                "trigger": trigger,
                "requested_count": requested_count,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
            })),
            TelemetryEvent::RemoteServerDaemonStartup { timing_data } => {
                Some(json!({ "timing_data": timing_data }))
            }
            TelemetryEvent::RemoteServerSetupDuration {
                duration_ms,
                installed_binary,
                remote_os,
                remote_arch,
                remote_libc,
            } => Some(json!({
                "duration_ms": duration_ms,
                "installed_binary": installed_binary,
                "remote_os": remote_os,
                "remote_arch": remote_arch,
                "remote_libc": remote_libc,
            })),
            TelemetryEvent::RemoteServerHostUnsupported {
                remote_os,
                remote_arch,
                unsupported_reason,
                detected_libc,
            } => {
                let unsupported_os = match unsupported_reason {
                    remote_server::setup::UnsupportedReason::UnsupportedOs { os } => {
                        Some(os.clone())
                    }
                    remote_server::setup::UnsupportedReason::GlibcTooOld { .. }
                    | remote_server::setup::UnsupportedReason::NonGlibc { .. }
                    | remote_server::setup::UnsupportedReason::UnsupportedArch { .. } => None,
                };
                let unsupported_arch = match unsupported_reason {
                    remote_server::setup::UnsupportedReason::UnsupportedArch { arch } => {
                        Some(arch.clone())
                    }
                    remote_server::setup::UnsupportedReason::GlibcTooOld { .. }
                    | remote_server::setup::UnsupportedReason::NonGlibc { .. }
                    | remote_server::setup::UnsupportedReason::UnsupportedOs { .. } => None,
                };
                Some(json!({
                    "remote_os": remote_os,
                    "remote_arch": remote_arch,
                    "reason": unsupported_reason.as_telemetry_reason(),
                    "detected_libc": detected_libc,
                    "unsupported_os": unsupported_os,
                    "unsupported_arch": unsupported_arch,
                }))
            }
            TelemetryEvent::ConversationListItemOpened { is_ambient_agent } => Some(json!({
                "is_ambient_agent": is_ambient_agent,
            })),
            TelemetryEvent::ConversationListLinkCopied { is_ambient_agent } => Some(json!({
                "is_ambient_agent": is_ambient_agent,
            })),
            TelemetryEvent::AIExecutionProfileSettingUpdated {
                setting_type,
                setting_value,
            } => Some(json!({
                "setting_type": setting_type,
                "setting_value": setting_value,
            })),
            TelemetryEvent::AIExecutionProfileAddedToAllowlist { list_type, value } => {
                Some(json!({
                    "list_type": list_type,
                    "value": value,
                }))
            }
            TelemetryEvent::AIExecutionProfileAddedToDenylist { list_type, value } => Some(json!({
                "list_type": list_type,
                "value": value,
            })),
            TelemetryEvent::AIExecutionProfileRemovedFromAllowlist { list_type, value } => {
                Some(json!({
                    "list_type": list_type,
                    "value": value,
                }))
            }
            TelemetryEvent::AIExecutionProfileRemovedFromDenylist { list_type, value } => {
                Some(json!({
                    "list_type": list_type,
                    "value": value,
                }))
            }
            TelemetryEvent::AIExecutionProfileModelSelected {
                model_type,
                model_value,
            } => Some(json!({
                "model_type": model_type,
                "model_value": model_value,
            })),
            TelemetryEvent::AIExecutionProfileContextWindowSelected { tokens, model_id } => {
                Some(json!({
                    "tokens": tokens,
                    "model_id": model_id,
                }))
            }
            TelemetryEvent::OpenSlashMenu {
                source,
                is_inline_ui_enabled,
                is_in_agent_view,
            } => Some(json!({
                "source": source,
                "is_inline_ui_enabled": is_inline_ui_enabled,
                "is_in_agent_view": is_in_agent_view,
            })),
            TelemetryEvent::SlashCommandAccepted {
                command_details,
                is_in_agent_view,
            } => Some(json!({
                "command_details": command_details,
                "is_in_agent_view": is_in_agent_view,
            })),
            TelemetryEvent::AgentModeSetupBannerAccepted => None,
            TelemetryEvent::AgentModeSetupBannerDismissed => None,
            TelemetryEvent::AgentModeSetupProjectScopedRulesAction { action } => Some(json!({
                "action": action,
            })),
            TelemetryEvent::AgentModeSetupCodebaseContextAction { action } => Some(json!({
                "action": action,
            })),
            TelemetryEvent::AgentModeSetupCreateEnvironmentAction { action } => Some(json!({
                "action": action,
            })),
            #[cfg(windows)]
            TelemetryEvent::WSLRegistryError
            | TelemetryEvent::AutoupdateUnableToCloseApplications
            | TelemetryEvent::AutoupdateFileInUse
            | TelemetryEvent::AutoupdateMutexTimeout => None,
            #[cfg(windows)]
            TelemetryEvent::AutoupdateForcekillFailed { exit_code } => Some(json!({
                "exit_code": exit_code,
            })),
            #[cfg(windows)]
            TelemetryEvent::AutoupdateMinidumpCleanupFailed { exit_code } => Some(json!({
                "exit_code": exit_code,
            })),
            TelemetryEvent::CreateProjectPromptSubmitted {
                is_custom_prompt,
                suggested_prompt,
                is_ftux,
            } => Some(json!({
                "is_custom_prompt": is_custom_prompt,
                "suggested_prompt": suggested_prompt,
                "is_ftux": is_ftux,
            })),
            TelemetryEvent::CreateProjectPromptSubmittedContent { custom_prompt } => Some(json!({
                "custom_prompt": custom_prompt
            })),
            TelemetryEvent::CloneRepoPromptSubmitted { is_ftux } => Some(json!({
                "is_ftux": is_ftux,
            })),
            TelemetryEvent::RecentMenuItemSelected { kind } => Some(json!({
                "kind": kind,
            })),
            TelemetryEvent::OpenRepoFolderSubmitted { is_ftux } => Some(json!({
                "is_ftux": is_ftux,
            })),
            TelemetryEvent::OutOfCreditsBannerClosed {
                action,
                selected_credits,
                auto_reload_checkbox_enabled,
                banner_toggle_flag_enabled,
                post_purchase_modal_flag_enabled,
            } => Some(json!({
                "action": action,
                "selected_credits": selected_credits,
                "auto_reload_checkbox_enabled": auto_reload_checkbox_enabled,
                "banner_toggle_flag_enabled": banner_toggle_flag_enabled,
                "post_purchase_modal_flag_enabled": post_purchase_modal_flag_enabled,
            })),
            TelemetryEvent::AutoReloadModalClosed {
                action,
                selected_credits,
                banner_toggle_flag_enabled,
                post_purchase_modal_flag_enabled,
            } => Some(json!({
                "action": action,
                "selected_credits": selected_credits,
                "banner_toggle_flag_enabled": banner_toggle_flag_enabled,
                "post_purchase_modal_flag_enabled": post_purchase_modal_flag_enabled,
            })),
            TelemetryEvent::AutoReloadToggledFromBillingSettings {
                enabled,
                banner_toggle_flag_enabled,
                post_purchase_modal_flag_enabled,
            } => Some(json!({
                "enabled": enabled,
                "banner_toggle_flag_enabled": banner_toggle_flag_enabled,
                "post_purchase_modal_flag_enabled": post_purchase_modal_flag_enabled,
            })),
            TelemetryEvent::WarpDriveOpened {
                source,
                is_code_mode_v2,
            } => Some(json!({
                "source": source,
                "is_code_mode_v2": is_code_mode_v2,
            })),
            TelemetryEvent::AgentTipShown { tip } => Some(json!({
                "tip": tip,
            })),
            TelemetryEvent::AgentTipClicked { tip, click_target } => Some(json!({
                "tip": tip,
                "click_target": click_target,
            })),
            TelemetryEvent::ToggleShowAgentTips { is_enabled } => Some(json!({
                "is_enabled": is_enabled,
            })),
            TelemetryEvent::AgentManagementViewToggled { is_open } => Some(json!({
                "is_open": is_open,
            })),
            TelemetryEvent::AgentManagementViewOpenedSession => None,
            TelemetryEvent::AgentManagementViewCopiedSessionLink => None,
            TelemetryEvent::DetectedIsolationPlatform { platform } => Some(json!({
                "platform": platform,
            })),
            TelemetryEvent::CLIAgentToolbarVoiceInputUsed { cli_agent } => Some(json!({
                "agent_name": cli_agent,
            })),
            TelemetryEvent::CLIAgentToolbarImageAttached { cli_agent } => Some(json!({
                "agent_name": cli_agent,
            })),
            TelemetryEvent::CLIAgentToolbarShown { cli_agent } => Some(json!({
                "agent_name": cli_agent,
            })),
            TelemetryEvent::CLIAgentRichInputOpened {
                cli_agent,
                entrypoint,
            } => Some(json!({
                "agent_name": cli_agent,
                "entrypoint": entrypoint,
            })),
            TelemetryEvent::CLIAgentRichInputClosed { cli_agent, reason } => Some(json!({
                "agent_name": cli_agent,
                "reason": reason,
            })),
            TelemetryEvent::CLIAgentRichInputSubmitted {
                cli_agent,
                prompt_length,
            } => Some(json!({
                "agent_name": cli_agent,
                "prompt_length": prompt_length,
            })),
            TelemetryEvent::CLIAgentPluginChipClicked { cli_agent, action } => Some(json!({
                "agent_name": cli_agent,
                "action": action,
            })),
            TelemetryEvent::CLIAgentPluginChipDismissed {
                cli_agent,
                chip_kind,
            } => Some(json!({
                "agent_name": cli_agent,
                "chip_kind": chip_kind,
            })),
            TelemetryEvent::CLIAgentPluginOperationSucceeded {
                cli_agent,
                operation,
            } => Some(json!({
                "agent_name": cli_agent,
                "operation": operation,
            })),
            TelemetryEvent::CLIAgentPluginOperationFailed {
                cli_agent,
                operation,
            } => Some(json!({
                "agent_name": cli_agent,
                "operation": operation,
            })),
            TelemetryEvent::CLIAgentPluginDetected { cli_agent } => Some(json!({
                "agent_name": cli_agent,
            })),
            TelemetryEvent::ToggleCLIAgentToolbarSetting { is_enabled } => Some(json!({
                "is_enabled": is_enabled,
            })),
            TelemetryEvent::ToggleUseAgentToolbarSetting { is_enabled } => Some(json!({
                "is_enabled": is_enabled,
            })),
            TelemetryEvent::InlineConversationMenuOpened { is_in_agent_view } => Some(json!({
                "is_in_agent_view": is_in_agent_view,
            })),
            TelemetryEvent::InlineConversationMenuItemSelected { is_in_agent_view } => {
                Some(json!({
                    "is_in_agent_view": is_in_agent_view,
                }))
            }
            TelemetryEvent::AgentShortcutsViewToggled { is_visible } => Some(json!({
                "is_visible": is_visible,
            })),
            TelemetryEvent::CodexModalOpened => None,
            TelemetryEvent::CodexModalUseCodexClicked => None,
            TelemetryEvent::LinearIssueLinkOpened => None,
            TelemetryEvent::CloudAgentCapacityModalOpened => None,
            TelemetryEvent::CloudAgentCapacityModalDismissed => None,
            TelemetryEvent::CloudAgentCapacityModalUpgradeClicked => None,
            TelemetryEvent::FreeTierLimitHitInterstitialDisplayed => None,
            TelemetryEvent::FreeTierLimitHitInterstitialUpgradeButtonClicked => None,
            TelemetryEvent::FreeTierLimitHitInterstitialClosed => None,
            TelemetryEvent::LoginButtonClicked { source }
            | TelemetryEvent::LoginLaterButtonClicked { source }
            | TelemetryEvent::LoginLaterConfirmationButtonClicked { source }
            | TelemetryEvent::OpenAuthPrivacySettings { source } => Some(json!({
                "source": source,
            })),
            TelemetryEvent::QueuedPromptPanelCollapseToggled { collapsed } => Some(json!({
                "collapsed": collapsed,
            })),
        }
    }

    /// Returns whether the event contains user generated content, indicating it should
    /// be sent to a dedicated rudderstack source.
    pub fn contains_ugc(&self) -> bool {
        match self {
            TelemetryEvent::BootstrappingSlowContents { .. } => true,
            TelemetryEvent::CreateProjectPromptSubmitted { .. } => false,
            TelemetryEvent::CreateProjectPromptSubmittedContent { .. } => true,
            TelemetryEvent::AgentModePotentialAutoDetectionFalsePositive(payload) => {
                // For internal dogfood users, the payload contains UGC.
                matches!(
                    payload,
                    AgentModeAutoDetectionFalsePositivePayload::InternalDogfoodUsers { .. }
                )
            }
            // Telemetry events do not contain user-generated content unless
            // explicitly marked above. (The original enumerated every variant;
            // that exhaustive list was removed during the AI/cloud strip.)
            _ => false,
        }
    }

    /// Prints a JSON containing all telemetry events enabled for the current build.
    /// The keys are the event name and the values are the event description.
    #[cfg(not(target_family = "wasm"))]
    pub fn print_telemetry_events_json() -> anyhow::Result<()> {
        // We initialize the feature flags so that we can determine which telemetry events to print.
        crate::features::init_feature_flags();

        let events: serde_json::Map<String, Value> = rift_core::telemetry::all_events()
            .filter_map(|event| {
                if !event.enablement_state().is_enabled() {
                    return None;
                }

                Some((
                    event.name().to_string(),
                    Value::String(event.description().to_string()),
                ))
            })
            .collect();

        let json_pretty_print_string = serde_json::to_string_pretty(&events)?;
        println!("{json_pretty_print_string}");
        Ok(())
    }
}

impl TelemetryEventDesc for TelemetryEventDiscriminants {
    fn enablement_state(&self) -> EnablementState {
        // We disallow the wildcard statement to prevent us from accidentally ignoring any
        // variants added in the future. Going forward, we should associate all new telemetry events
        // with a feature flag when appropriate.
        #[deny(clippy::wildcard_enum_match_arm)]
        match self {
            Self::RepoOutlineConstructionSuccess { .. } => {
                EnablementState::Flag(FeatureFlag::AgentModeAnalytics)
            }
            Self::RepoOutlineConstructionFailed { .. } => {
                EnablementState::Flag(FeatureFlag::AgentModeAnalytics)
            }
            Self::ObjectLinkCopied => EnablementState::Always,
            Self::FileTreeToggled => EnablementState::Flag(FeatureFlag::FileTree),
            Self::FileTreeItemAttachedAsContext => EnablementState::Flag(FeatureFlag::FileTree),
            Self::CodeSelectionAddedAsContext => EnablementState::Flag(FeatureFlag::HoaCodeReview),
            Self::FileTreeItemCreated => EnablementState::Flag(FeatureFlag::FileTree),
            Self::ConversationListViewOpened
            | Self::ConversationListItemOpened
            | Self::ConversationListItemDeleted
            | Self::ConversationListLinkCopied => {
                EnablementState::Flag(FeatureFlag::AgentViewConversationListView)
            }
            Self::InlineConversationMenuOpened
            | Self::InlineConversationMenuItemSelected
            | Self::AgentShortcutsViewToggled => EnablementState::Flag(FeatureFlag::AgentView),
            Self::CreateProjectPromptSubmitted => EnablementState::Flag(FeatureFlag::GetStartedTab),
            Self::CreateProjectPromptSubmittedContent => {
                EnablementState::Flag(FeatureFlag::GetStartedTab)
            }
            Self::CloneRepoPromptSubmitted => EnablementState::Flag(FeatureFlag::GetStartedTab),
            Self::GetStartedSkipToTerminal => EnablementState::Flag(FeatureFlag::GetStartedTab),
            Self::PtyThroughput => EnablementState::Flag(FeatureFlag::RecordPtyThroughput),
            Self::KnowledgePaneOpened { .. } => EnablementState::Flag(FeatureFlag::AIRules),
            #[cfg(feature = "local_fs")]
            Self::CodePanelsFileOpened { .. } => EnablementState::Always,
            #[cfg(feature = "local_fs")]
            Self::PreviewPanePromoted => EnablementState::Always,
            Self::ToggleFocusPaneOnHover { .. } => EnablementState::Always,
            Self::InitiateAnonymousUserSignup { .. }
            | Self::LoginLaterButtonClicked
            | Self::LoginLaterConfirmationButtonClicked
            | Self::AnonymousUserExpirationLockout
            | Self::AnonymousUserLinkedFromBrowser
            | Self::AnonymousUserAttemptLoginGatedFeature
            | Self::AnonymousUserHitCloudObjectLimit => EnablementState::Always,

            Self::SharedSessionModalUpgradePressed => {
                EnablementState::Flag(FeatureFlag::CreatingSharedSessions)
            }
            Self::JoinedSharedSession => EnablementState::Flag(FeatureFlag::ViewingSharedSessions),
            Self::ToggleSettingsSync { .. } => EnablementState::Always,
            Self::AgentTipShown | Self::AgentTipClicked | Self::ToggleShowAgentTips => {
                EnablementState::Flag(FeatureFlag::AgentTips)
            }
            Self::AutosuggestionInserted => EnablementState::Always,
            Self::BlockCompleted => EnablementState::Always,
            Self::BackgroundBlockStarted => EnablementState::Always,
            Self::BaselineCommandLatency => EnablementState::Always,
            Self::SessionCreation => EnablementState::Always,
            Self::Login => EnablementState::Always,
            Self::OpenSuggestionsMenu => EnablementState::Always,
            Self::ConfirmSuggestion => EnablementState::Always,
            Self::OpenContextMenu => EnablementState::Always,
            Self::ContextMenuCopy => EnablementState::Always,
            Self::ContextMenuOpenShareModal => EnablementState::Always,
            Self::ContextMenuFindWithinBlocks => EnablementState::Always,
            Self::ContextMenuCopyPrompt => EnablementState::Always,
            Self::ContextMenuToggleGitPromptDirtyIndicator => EnablementState::Always,
            Self::ContextMenuInsertSelectedText => EnablementState::Always,
            Self::ContextMenuCopySelectedText => EnablementState::Always,
            Self::OpenPromptEditor => EnablementState::Always,
            Self::PromptEdited => EnablementState::Always,
            Self::ReinputCommands => EnablementState::Always,
            Self::JumpToPreviousCommand => EnablementState::Always,
            Self::CopyBlockSharingLink => EnablementState::Always,
            Self::GenerateBlockSharingLink => EnablementState::Always,
            Self::BlockSelection => EnablementState::Always,
            Self::BootstrappingSlow => EnablementState::Always,
            Self::BootstrappingSlowContents => EnablementState::Always,
            Self::SessionAbandonedBeforeBootstrap => EnablementState::Always,
            Self::BootstrappingSucceeded => EnablementState::Always,
            Self::TabSingleResultAutocompletion => EnablementState::Always,
            Self::EditorUnhandledModifierKey => EnablementState::Always,
            Self::CopyInviteLink => EnablementState::Always,
            Self::OpenThemeChooser => EnablementState::Always,
            Self::ThemeSelection => EnablementState::Always,
            Self::AppIconSelection => EnablementState::Always,
            Self::CursorDisplayType => EnablementState::Always,
            Self::OpenThemeCreatorModal => EnablementState::Always,
            Self::CreateCustomTheme => EnablementState::Always,
            Self::DeleteCustomTheme => EnablementState::Always,
            Self::SplitPane => EnablementState::Always,
            Self::UnableToAutoUpdateToNewVersion | Self::AutoupdateRelaunchAttempt => {
                EnablementState::Always
            }
            Self::SkipOnboardingSurvey => EnablementState::Always,
            Self::ToggleRestoreSession => EnablementState::Always,
            Self::DatabaseStartUpError => EnablementState::Always,
            Self::DatabaseReadError => EnablementState::Always,
            Self::DatabaseWriteError => EnablementState::Always,
            Self::AppStartup => EnablementState::Always,
            Self::LoggedOutStartup => EnablementState::Always,
            Self::DownloadSource => EnablementState::Always,
            Self::SSHBootstrapAttempt => EnablementState::Always,
            Self::SSHControlMasterError => EnablementState::Always,
            Self::KeybindingChanged => EnablementState::Always,
            Self::KeybindingResetToDefault => EnablementState::Always,
            Self::KeybindingRemoved => EnablementState::Always,
            Self::FeaturesPageAction => EnablementState::Always,
            Self::OpenWorkflowSearch => EnablementState::Always,
            Self::OpenQuakeModeWindow => EnablementState::Always,
            Self::OpenWelcomeTips => EnablementState::Always,
            Self::CompleteWelcomeTipFeature => EnablementState::Always,
            Self::DismissWelcomeTips => EnablementState::Always,
            Self::ShowNotificationsDiscoveryBanner => EnablementState::Always,
            Self::NotificationsDiscoveryBannerAction => EnablementState::Always,
            Self::ShowNotificationsErrorBanner => EnablementState::Always,
            Self::NotificationsErrorBannerAction => EnablementState::Always,
            Self::NotificationPermissionsRequested => EnablementState::Always,
            Self::NotificationsRequestPermissionsOutcome => EnablementState::Always,
            Self::NotificationFailedToSend => EnablementState::Always,
            Self::NotificationClicked => EnablementState::Always,
            Self::ToggleFindOption => EnablementState::Always,
            Self::SignUpButtonClicked => EnablementState::Always,
            Self::LoginButtonClicked => EnablementState::Always,
            Self::OpenNewSessionFromFilePath => EnablementState::Always,
            Self::OpenTeamFromURI => EnablementState::Always,
            Self::SelectCommandPaletteOption => EnablementState::Always,
            Self::PaletteSearchOpened => EnablementState::Always,
            Self::PaletteSearchResultAccepted => EnablementState::Always,
            Self::PaletteSearchExited => EnablementState::Always,
            Self::SelectNavigationPaletteItem => EnablementState::Always,
            Self::AuthCommonQuestionClicked => EnablementState::Always,
            Self::AuthToggleFAQ => EnablementState::Always,
            Self::OpenAuthPrivacySettings => EnablementState::Always,
            Self::TabRenamed => EnablementState::Always,
            Self::MoveActiveTab => EnablementState::Always,
            Self::MoveTab => EnablementState::Always,
            Self::DragAndDropTab => EnablementState::Always,
            Self::DragAndDropTabGroup => EnablementState::Always,
            Self::TabOperations => EnablementState::Always,
            Self::EditedInputBeforePrecmd => EnablementState::Always,
            Self::TriedToExecuteBeforePrecmd => EnablementState::Always,
            Self::ThinStrokesSettingChanged => EnablementState::Always,
            Self::BookmarkBlockToggled => EnablementState::Always,
            Self::JumpToBookmark => EnablementState::Always,
            Self::JumpToLatestAgentMessage => EnablementState::Always,
            Self::JumpToBottomofBlockButtonClicked => EnablementState::Always,
            Self::ToggleJumpToBottomofBlockButton => EnablementState::Always,
            Self::OpenLink => EnablementState::Always,
            Self::OpenChangelogLink => EnablementState::Always,
            Self::ShowInFileExplorer => EnablementState::Always,
            Self::CommandXRayTriggered => EnablementState::Always,
            Self::OpenLaunchConfigSaveModal => EnablementState::Always,
            Self::SaveLaunchConfig => EnablementState::Always,
            Self::OpenLaunchConfigFile => EnablementState::Always,
            Self::OpenLaunchConfig => EnablementState::Always,
            Self::TeamCreated => EnablementState::Always,
            Self::TeamJoined => EnablementState::Always,
            Self::TeamLeft => EnablementState::Always,
            Self::TeamLinkCopied => EnablementState::Always,
            Self::RemovedUserFromTeam => EnablementState::Always,
            Self::DeletedWorkflow => EnablementState::Always,
            Self::DeletedNotebook => EnablementState::Always,
            Self::ToggleApprovalsModal => EnablementState::Always,
            Self::SendEmailInvites => EnablementState::Always,
            Self::CommandCorrection => EnablementState::Always,
            Self::SetLineHeight => EnablementState::Always,
            Self::ResourceCenterOpened => EnablementState::Always,
            Self::ResourceCenterTipsCompleted => EnablementState::Always,
            Self::ResourceCenterTipsSkipped => EnablementState::Always,
            Self::KeybindingsPageOpened => EnablementState::Always,
            Self::GlobalSearchOpened => EnablementState::Always,
            Self::GlobalSearchQueryStarted => EnablementState::Always,
            Self::CommandSearchOpened => EnablementState::Always,
            Self::CommandSearchExited => EnablementState::Always,
            Self::CommandSearchResultAccepted => EnablementState::Always,
            Self::CommandSearchFilterChanged => EnablementState::Always,
            Self::AICommandSearchOpened => EnablementState::Always,
            Self::OpenedAltScreenFind => EnablementState::Always,
            Self::UserInitiatedClose => EnablementState::Always,
            Self::QuitModalShown => EnablementState::Always,
            Self::QuitModalCancel => EnablementState::Always,
            Self::QuitModalDisabled => EnablementState::Always,
            Self::UserInitiatedLogOut => EnablementState::Always,
            Self::LogOutModalShown => EnablementState::Always,
            Self::LogOutModalCancel => EnablementState::Always,
            Self::SetOpacity => EnablementState::Always,
            Self::SetBlurRadius => EnablementState::Always,
            Self::ToggleDimInactivePanes => EnablementState::Always,
            Self::InputModeChanged => EnablementState::Always,
            Self::PtySpawned => EnablementState::Always,
            Self::InitialWorkingDirectoryConfigurationChanged => EnablementState::Always,
            Self::OpenedWarpAI => EnablementState::Always,
            Self::WarpAIRequestIssued => EnablementState::Always,
            Self::WarpAIAction => EnablementState::Always,
            Self::UsedWarpAIPreparedPrompt => EnablementState::Always,
            Self::WarpAICharacterLimitExceeded => EnablementState::Always,
            Self::OpenInputContextMenu => EnablementState::Always,
            Self::InputCutSelectedText => EnablementState::Always,
            Self::InputCopySelectedText => EnablementState::Always,
            Self::InputSelectAll => EnablementState::Always,
            Self::InputPaste => EnablementState::Always,
            Self::InputCommandSearch => EnablementState::Always,
            Self::InputAICommandSearch => EnablementState::Always,
            Self::InputAskWarpAI => EnablementState::Always,
            Self::SaveAsWorkflowModal => EnablementState::Always,
            Self::ExperimentTriggered => EnablementState::Always,
            Self::ToggleSyncAllPanesInAllTabs => EnablementState::Always,
            Self::ToggleSyncAllPanesInTab => EnablementState::Always,
            Self::ToggleSameLinePrompt => EnablementState::Always,
            Self::ToggleNewWindowsAtCustomSize => EnablementState::Always,
            Self::SetNewWindowsAtCustomSize => EnablementState::Always,
            Self::DisableInputSync => EnablementState::Always,
            Self::ToggleTabIndicators => EnablementState::Always,
            Self::TogglePreserveActiveTabColor => EnablementState::Always,
            Self::ShowSubshellBanner => EnablementState::Always,
            Self::SshTmuxWarpifyBannerDisplayed => EnablementState::Always,
            Self::DeclineSubshellBootstrap => EnablementState::Always,
            Self::TriggerSubshellBootstrap => EnablementState::Always,
            Self::AddDenylistedSubshellCommand => EnablementState::Always,
            Self::RemoveDenylistedSubshellCommand => EnablementState::Always,
            Self::ToggleSshTmuxWrapper => EnablementState::Always,
            Self::ToggleSshWarpification => EnablementState::Always,
            Self::SetSshExtensionInstallMode => EnablementState::Always,
            Self::SshRemoteServerChoiceDoNotAskAgainToggled => EnablementState::Always,
            Self::AddDenylistedSshTmuxWrapperHost => EnablementState::Always,
            Self::RemoveDenylistedSshTmuxWrapperHost => EnablementState::Always,
            Self::SshInteractiveSessionDetected => EnablementState::Always,
            Self::SshTmuxWarpifyBlockAccepted => EnablementState::Always,
            Self::SshTmuxWarpifyBlockDismissed => EnablementState::Always,
            Self::WarpifyFooterShown
            | Self::AgentToolbarDismissed
            | Self::WarpifyFooterAcceptedWarpify => EnablementState::Always,
            Self::SshTmuxWarpificationSuccess => EnablementState::Always,
            Self::SshTmuxWarpificationErrorBlock => EnablementState::Always,
            Self::SshInstallTmuxBlockDisplayed => EnablementState::Always,
            Self::SshInstallTmuxBlockAccepted => EnablementState::Always,
            Self::SshInstallTmuxBlockDismissed => EnablementState::Always,
            Self::AddAddedSubshellCommand => EnablementState::Always,
            Self::RemoveAddedSubshellCommand => EnablementState::Always,
            Self::ReceivedSubshellRcFileDcs => EnablementState::Always,
            Self::ShowAliasExpansionBanner => EnablementState::Always,
            Self::EnableAliasExpansionFromBanner => EnablementState::Always,
            Self::DismissAliasExpansionBanner => EnablementState::Always,
            Self::ShowVimKeybindingsBanner => EnablementState::Always,
            Self::EnableVimKeybindingsFromBanner => EnablementState::Always,
            Self::DismissVimKeybindingsBanner => EnablementState::Always,
            Self::InitiateReauth => EnablementState::Always,
            Self::NeedsReauth => EnablementState::Always,
            Self::WarpDriveOpened => EnablementState::Always,
            Self::ToggleWarpAI => EnablementState::Always,
            Self::ToggleSecretRedaction => EnablementState::Always,
            Self::CustomSecretRegexAdded => EnablementState::Always,
            Self::ToggleObfuscateSecret => EnablementState::Always,
            Self::CopySecret => EnablementState::Always,
            Self::AutoGenerateMetadataSuccess => EnablementState::Always,
            Self::AutoGenerateMetadataError => EnablementState::Always,
            Self::UndoClose => EnablementState::Always,
            Self::CommandFileRun => EnablementState::Always,
            Self::PageUpDownInEditorPressed => EnablementState::Always,
            Self::UnsupportedShell => EnablementState::Always,
            Self::LogOut => EnablementState::Always,
            Self::SettingsImportInitiated => EnablementState::Always,
            Self::InviteTeammates => EnablementState::Always,
            Self::OpenAndWarpifyDockerSubshell => EnablementState::Always,
            Self::UpdateBlockFilterQuery => EnablementState::Always,
            Self::UpdateBlockFilterQueryContextLines => EnablementState::Always,
            Self::ToggleBlockFilterQuery => EnablementState::Always,
            Self::ToggleBlockFilterCaseSensitivity => EnablementState::Always,
            Self::ToggleBlockFilterRegex => EnablementState::Always,
            Self::ToggleBlockFilterInvert => EnablementState::Always,
            Self::BlockFilterToolbeltButtonClicked => EnablementState::Always,
            Self::ToggleSnackbarInActivePane => EnablementState::Always,
            Self::PaneDragInitiated => EnablementState::Always,
            Self::PaneDropped => EnablementState::Always,
            Self::TierLimitHit => EnablementState::Always,
            Self::SharerCancelledGrantRole => EnablementState::Always,
            Self::SharerGrantModalDontShowAgain => EnablementState::Always,
            Self::JumpToSharedSessionParticipant => EnablementState::Always,
            Self::ToggleShowBlockDividers => EnablementState::Flag(FeatureFlag::MinimalistUI),
            Self::DriveSharingOnboardingBlockShown => EnablementState::Always,
            Self::SharedObjectLimitHitBannerViewPlansButtonClicked => EnablementState::Always,
            Self::ResourceUsageStats => EnablementState::Always,
            Self::ToggleGlobalAI => EnablementState::Always,
            Self::ToggleActiveAI => EnablementState::Always,
            Self::MemoryUsageStats => EnablementState::ChannelSpecific {
                channels: vec![Channel::Local, Channel::Dev],
            },
            Self::MemoryUsageHigh => EnablementState::Always,
            Self::AgentModeUserAttemptedQueryAtRequestLimit
            | Self::AgentModeClickedEntrypoint
            | Self::AgentModeAttachedBlockContext
            | Self::AgentModeToggleAutoDetectionSetting
            | Self::AgentModePotentialAutoDetectionFalsePositive => {
                EnablementState::Flag(FeatureFlag::AgentMode)
            }
            Self::BlockCompletedOnDogfoodOnly => EnablementState::ChannelSpecific {
                channels: vec![Channel::Local, Channel::Dev],
            },
            Self::CompletedSettingsImport
            | Self::SettingsImportConfigFocused
            | Self::SettingsImportConfigParsed
            | Self::SettingsImportResetButtonClicked
            | Self::ITermMultipleHotkeys => EnablementState::Always,
            Self::ToggleIntelligentAutosuggestionsSetting => {
                EnablementState::Always
            }
            Self::PromptSuggestionShown
            | Self::SuggestedCodeDiffFailed
            | Self::PromptSuggestionAccepted
            | Self::StaticPromptSuggestionsBannerShown
            | Self::StaticPromptSuggestionAccepted
            | Self::TogglePromptSuggestionsSetting
            | Self::ToggleCodeSuggestionsSetting => EnablementState::Always,
            Self::ToggleNaturalLanguageAutosuggestionsSetting => {
                EnablementState::Flag(FeatureFlag::PredictAMQueries)
            }
            Self::ToggleSharedBlockTitleGenerationSetting => {
                EnablementState::Flag(FeatureFlag::SharedBlockTitleGeneration)
            }
            Self::ToggleGitOperationsAutogenSetting => {
                EnablementState::Flag(FeatureFlag::GitOperationsInCodeReview)
            }
            Self::ToggleVoiceInputSetting => EnablementState::Always,

            Self::ToggleWorkspaceDecorationVisibility => {
                EnablementState::Flag(FeatureFlag::FullScreenZenMode)
            }
            Self::UpdateAltScreenPaddingMode => EnablementState::Always,
            Self::AddTabWithShell => EnablementState::Flag(FeatureFlag::ShellSelector),
            Self::ToggleLigatureRendering => EnablementState::Flag(FeatureFlag::Ligatures),
            Self::ToggledAgentModeAutoexecuteReadonlyCommandsSetting
            | Self::ChangedAgentModeCodingPermissions => EnablementState::Always,
            Self::AttachedImagesToAgentModeQuery => {
                EnablementState::Flag(FeatureFlag::ImageAsContext)
            }
            #[cfg(windows)]
            Self::WSLRegistryError
            | Self::AutoupdateUnableToCloseApplications
            | Self::AutoupdateFileInUse
            | Self::AutoupdateMutexTimeout
            | Self::AutoupdateForcekillFailed { .. }
            | Self::AutoupdateMinidumpCleanupFailed { .. } => EnablementState::Always,
            Self::ToggleCodebaseContext => EnablementState::Always,
            Self::ToggleAutoIndexing => EnablementState::Always,
            Self::ImageReceived => EnablementState::Always,
            Self::GrepToolSucceeded => EnablementState::Always,
            Self::FileGlobToolSucceeded => EnablementState::Always,
            Self::ShellTerminatedPrematurely { .. } => EnablementState::Always,
            Self::InputUXModeChanged { .. } => EnablementState::Always,
            Self::ContextChipInteracted { .. } => EnablementState::Always,
            Self::ActiveIndexedReposChanged { .. } => {
                EnablementState::Flag(FeatureFlag::FullSourceCodeEmbedding)
            }
            Self::UserMenuUpgradeClicked => EnablementState::Always,
            Self::TabCloseButtonPositionUpdated { .. } => EnablementState::Always,
            Self::AIExecutionProfileCreated
            | Self::AIExecutionProfileDeleted
            | Self::AIExecutionProfileSettingUpdated { .. }
            | Self::AIExecutionProfileAddedToAllowlist { .. }
            | Self::AIExecutionProfileAddedToDenylist { .. }
            | Self::AIExecutionProfileRemovedFromAllowlist { .. }
            | Self::AIExecutionProfileRemovedFromDenylist { .. }
            | Self::AIExecutionProfileModelSelected { .. }
            | Self::AIExecutionProfileContextWindowSelected { .. } => {
                EnablementState::Flag(FeatureFlag::MultiProfile)
            }
            Self::OpenSlashMenu { .. } => EnablementState::Always,
            Self::SlashCommandAccepted { .. } => EnablementState::Always,
            Self::AgentModeSetupBannerAccepted { .. } => EnablementState::Always,
            Self::AgentModeSetupBannerDismissed => EnablementState::Always,
            Self::AgentModeSetupProjectScopedRulesAction { .. } => EnablementState::Always,
            Self::AgentModeSetupCodebaseContextAction { .. } => EnablementState::Always,
            Self::AgentModeSetupCreateEnvironmentAction { .. } => EnablementState::Always,
            Self::AgentModeRewindDialogOpened { .. } => {
                EnablementState::Flag(FeatureFlag::RevertToCheckpoints)
            }
            Self::AgentModeRewindExecuted { .. } => {
                EnablementState::Flag(FeatureFlag::RevertToCheckpoints)
            }
            Self::RecentMenuItemSelected => EnablementState::Always,
            Self::OpenRepoFolderSubmitted => EnablementState::Always,
            Self::OutOfCreditsBannerClosed => EnablementState::Always,
            Self::AutoReloadModalClosed => EnablementState::Always,
            Self::AutoReloadToggledFromBillingSettings => EnablementState::Always,
            Self::AgentManagementViewToggled { .. }
            | Self::AgentManagementViewOpenedSession
            | Self::AgentManagementViewCopiedSessionLink => {
                EnablementState::Flag(FeatureFlag::AgentManagementView)
            }
            Self::DetectedIsolationPlatform { .. } => EnablementState::Always,
            Self::CLIAgentToolbarVoiceInputUsed { .. } => EnablementState::Always,
            Self::CLIAgentToolbarImageAttached { .. } => EnablementState::Always,
            Self::CLIAgentToolbarShown { .. } => EnablementState::Always,
            Self::CLIAgentPluginChipClicked { .. }
            | Self::CLIAgentPluginChipDismissed { .. }
            | Self::CLIAgentPluginOperationSucceeded { .. }
            | Self::CLIAgentPluginOperationFailed { .. } => {
                EnablementState::Flag(FeatureFlag::HOANotifications)
            }
            Self::CLIAgentPluginDetected { .. } => EnablementState::Always,
            Self::CLIAgentRichInputOpened { .. }
            | Self::CLIAgentRichInputClosed { .. }
            | Self::CLIAgentRichInputSubmitted { .. } => {
                EnablementState::Flag(FeatureFlag::CLIAgentRichInput)
            }
            Self::ToggleCLIAgentToolbarSetting { .. } => EnablementState::Always,
            Self::ToggleUseAgentToolbarSetting { .. } => EnablementState::Always,
            Self::CodexModalOpened | Self::CodexModalUseCodexClicked => EnablementState::Always,
            Self::LinearIssueLinkOpened => EnablementState::Always,
            Self::CloudAgentCapacityModalOpened
            | Self::CloudAgentCapacityModalDismissed
            | Self::CloudAgentCapacityModalUpgradeClicked => {
                EnablementState::Flag(FeatureFlag::CloudMode)
            }
            Self::FreeTierLimitHitInterstitialDisplayed { .. } => EnablementState::Always,
            Self::FreeTierLimitHitInterstitialUpgradeButtonClicked { .. } => {
                EnablementState::Always
            }
            Self::FreeTierLimitHitInterstitialClosed { .. } => EnablementState::Always,
            Self::RemoteServerBinaryCheck
            | Self::RemoteServerInstallation
            | Self::RemoteServerInitialization
            | Self::RemoteServerDaemonStartup
            | Self::RemoteServerDisconnection
            | Self::RemoteServerClientRequestError
            | Self::RemoteServerMessageDecodingError
            | Self::RemoteServerSetupDuration
            | Self::RemoteServerHostUnsupported
            | Self::RemoteServerReconnection
            | Self::RemoteServerReconnectExhausted
            | Self::RemoteCodebaseIndexStatusChanged
            | Self::RemoteCodebaseAutoIndexRequested => {
                EnablementState::Flag(FeatureFlag::SshRemoteServer)
            }
            Self::QueuedPromptPanelCollapseToggled => {
                EnablementState::Flag(FeatureFlag::QueueSlashCommand)
            }
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::RepoOutlineConstructionSuccess => "Repo Outline Built Successfully",
            Self::RepoOutlineConstructionFailed => "Repo Outline Construction Failed",
            Self::AutosuggestionInserted => "Autosuggestion Inserted",
            // Although this event is sent when the block completes rather than
            // when it's created, we are still naming it "Block Creation" to
            // preserve our historical telemetry data.
            Self::BlockCompleted => "Block Creation",
            Self::BlockCompletedOnDogfoodOnly => "Block Completed (dogfood only)",
            Self::BackgroundBlockStarted => "Background Block Started",
            Self::SessionCreation => "Tab Creation",
            Self::Login => "Logged in to native app",
            Self::AgentModeRewindDialogOpened { .. } => "Opened Rewind Confirmation Dialog",
            Self::AgentModeRewindExecuted { .. } => "Executed Conversation Rewind",
            Self::ReinputCommands => "Context Menu: Reinput Commands",
            Self::ToggleSettingsSync => "Toggle Settings Sync",
            Self::ToggleFocusPaneOnHover => "Toggle Focus Pane On Hover",
            Self::LoginLaterButtonClicked => "Login Later Button Clicked",
            Self::LoginLaterConfirmationButtonClicked => "Login Later Confirmation Button Clicked",
            Self::JumpToPreviousCommand => "Jumped to Previous Command",
            Self::OpenContextMenu => "Open Context Menu",
            Self::ContextMenuFindWithinBlocks => "Context Menu: Find Within Blocks",
            Self::ContextMenuOpenShareModal => "Context Menu: Initiate Block Sharing",
            Self::ContextMenuCopy => "Context Menu Copy",
            Self::CopyBlockSharingLink => "Copy Block Sharing Link",
            Self::GenerateBlockSharingLink => "Generate Block Sharing Link",
            Self::BlockSelection => "Block Selection",
            Self::BootstrappingSlow => "Bootstrapping Slow",
            Self::BootstrappingSlowContents => "Bootstrap Slow Contents",
            Self::ObjectLinkCopied => "Object Link Copied",
            Self::FileTreeToggled => "File Tree Toggled",
            Self::FileTreeItemAttachedAsContext => "FileTree.AttachedAsContext",
            Self::CodeSelectionAddedAsContext => "CodeView.SelectionAddedAsContext",
            Self::FileTreeItemCreated => "FileTree.ItemCreated",
            Self::ConversationListViewOpened => "ConversationList.Opened",
            Self::ConversationListItemOpened => "ConversationList.ItemOpened",
            Self::ConversationListItemDeleted => "ConversationList.ItemDeleted",
            Self::ConversationListLinkCopied => "ConversationList.LinkCopied",
            Self::InlineConversationMenuOpened => "AgentView.InlineConversationMenuOpened",
            Self::InlineConversationMenuItemSelected => {
                "AgentView.InlineConversationMenuItemSelected"
            }
            Self::AgentShortcutsViewToggled => "AgentView.ShortcutsViewToggled",
            Self::CreateProjectPromptSubmitted => "Create Project Prompt Submitted",
            Self::CreateProjectPromptSubmittedContent => "Create Project Prompt Submitted Content",
            Self::CloneRepoPromptSubmitted => "Clone Repo Prompt Submitted",
            Self::GetStartedSkipToTerminal => "Get Started Skip to Terminal",
            Self::InitiateAnonymousUserSignup => "Anonymous User Initiated Signup",
            Self::AnonymousUserExpirationLockout => "Anonymous User Expiration Lockout",
            Self::AnonymousUserLinkedFromBrowser => "Anonymous User Linked from Browser",
            Self::AnonymousUserAttemptLoginGatedFeature => {
                "Anonymous User Attempted Login-Gated Feature"
            }
            Self::KnowledgePaneOpened { .. } => "Knowledge Pane Opened",
            #[cfg(feature = "local_fs")]
            Self::CodePanelsFileOpened { .. } => "CodePanels.FileOpened",
            #[cfg(feature = "local_fs")]
            Self::PreviewPanePromoted => "Preview Pane Promoted",
            Self::AnonymousUserHitCloudObjectLimit => "Anonymous User Hit Cloud Object Limit",
            Self::BootstrappingSucceeded => "Bootstrapping Succeeded",
            Self::SessionAbandonedBeforeBootstrap => "Session Abandoned Before Bootstrap",
            Self::TabSingleResultAutocompletion => "Tab Single Result Autocompletion",
            Self::OpenSuggestionsMenu => "Open Suggestions Menu",
            Self::ConfirmSuggestion => "Confirm Suggestion",
            Self::ContextMenuInsertSelectedText => "Context Menu Insert Selected Text into Input",
            Self::ContextMenuCopySelectedText => "Context Menu Copy Selected Text",
            Self::ContextMenuCopyPrompt => "Context Menu Copy Prompt",
            Self::ContextMenuToggleGitPromptDirtyIndicator => {
                "Context Menu Toggle Git Prompt Dirty Indicator"
            }
            Self::EditorUnhandledModifierKey => "Unhandled Editor Modifier Key",
            Self::CopyInviteLink => "Copy Invite Link",
            Self::OpenThemeChooser => "Open Theme Chooser",
            Self::ThemeSelection => "Select Theme",
            Self::AppIconSelection => "Select App Icon",
            Self::CursorDisplayType => "Select Cursor Type",
            Self::OpenThemeCreatorModal => "Open Theme Creator Modal",
            Self::CreateCustomTheme => "Create Custom Theme",
            Self::DeleteCustomTheme => "Delete Custom Theme",
            Self::UnableToAutoUpdateToNewVersion => "Unable to Update To New Version",
            Self::AutoupdateRelaunchAttempt => "Attempting to Relaunch for Update",
            Self::SplitPane => "Split Pane",
            Self::SkipOnboardingSurvey => "Skip Onboarding Survey",
            Self::ToggleRestoreSession => "Toggle Restore Session",
            Self::DatabaseStartUpError => "Database Startup Error",
            Self::DatabaseWriteError => "Database Write Error",
            Self::DatabaseReadError => "Database Read Error",
            Self::AppStartup => "App Startup",
            Self::LoggedOutStartup => "Logged-out App Startup",
            Self::DownloadSource => "App Download Source",
            Self::BaselineCommandLatency => "BaselineCommand Latency",
            Self::SSHBootstrapAttempt => "SSH Bootstrap Attempt",
            Self::SSHControlMasterError => "SSH ControlMaster Error",
            Self::SetNewWindowsAtCustomSize => "Set New Windows at Custom Size",
            Self::ToggleNewWindowsAtCustomSize => "Toggle New Windows at Custom Size",
            Self::KeybindingChanged => "Keybinding Changed",
            Self::KeybindingResetToDefault => "Keybinding Reset to Default",
            Self::KeybindingRemoved => "Keybinding Removed",
            Self::OpenWorkflowSearch => "Open Workflows Search",
            Self::FeaturesPageAction => "Features Page Action",
            Self::OpenQuakeModeWindow => "Open Quake Mode Window",
            Self::OpenWelcomeTips => "Open Welcome Tips",
            Self::CompleteWelcomeTipFeature => "Complete Welcome Tip",
            Self::DismissWelcomeTips => "Dismiss Welcome Tips",
            Self::ShowNotificationsDiscoveryBanner => "ShowNotificationsDiscoveryBanner",
            Self::NotificationsDiscoveryBannerAction => "Notifications Discovery Banner Action",
            Self::ShowNotificationsErrorBanner => "ShowNotificationsErrorBanner",
            Self::NotificationsErrorBannerAction => "Notifications Error Banner Action",
            Self::NotificationPermissionsRequested => "Notification Permissions Requested",
            Self::NotificationFailedToSend => "Notification Failed to Send",
            Self::NotificationClicked => "Notification Clicked",
            Self::NotificationsRequestPermissionsOutcome => {
                "Notification Request Permissions Outcome"
            }
            Self::ToggleFindOption => "Find Option Toggled",
            Self::SignUpButtonClicked => "Sign Up Button Clicked in App",
            Self::LoginButtonClicked => "Log In Button Clicked in App",
            Self::OpenNewSessionFromFilePath => "New Session From Directory",
            Self::OpenTeamFromURI => "Open Team from URI",
            Self::SelectCommandPaletteOption => "Select Command Palette Option",
            Self::PaletteSearchOpened => "Open Palette",
            Self::PaletteSearchResultAccepted => "Command Palette Search Accepted",
            Self::PaletteSearchExited => "Command Palette Search Exited",
            Self::AuthCommonQuestionClicked => "Auth Common Question Clicked in App",
            Self::AuthToggleFAQ => "Auth: Toggle Common Questions",
            Self::OpenAuthPrivacySettings => "Auth: Open Privacy Settings Overlay",
            Self::TabRenamed => "Tab Renamed",
            Self::MoveActiveTab => "Move Active Tab",
            Self::MoveTab => "Move Tab",
            Self::DragAndDropTab => "Drag and Drop Tab",
            Self::DragAndDropTabGroup => "Drag and Drop Tab Group",
            Self::TabOperations => "Tab Operations",
            Self::EditedInputBeforePrecmd => "Edited Input Before Precmd",
            Self::TriedToExecuteBeforePrecmd => "Tried to Execute Before Precmd",
            Self::ThinStrokesSettingChanged => "Thin Strokes Setting Changed",
            Self::BookmarkBlockToggled => "Toggled Bookmark Block",
            Self::JumpToBookmark => "Jumped to Bookmark Block",
            Self::JumpToLatestAgentMessage => "Jumped to Latest Agent Message",
            Self::JumpToBottomofBlockButtonClicked => "Jumped to Bottom of Block Button Clicked",
            Self::OpenLink => "Opened Link",
            Self::OpenChangelogLink => "Opened Changelog Link",
            Self::ShowInFileExplorer => "Showed File in File Explorer",
            Self::CommandXRayTriggered => "Triggered Command XRay",
            Self::OpenLaunchConfigSaveModal => "Open Save Config Modal",
            Self::SaveLaunchConfig => "Save Launch Config",
            Self::OpenLaunchConfigFile => "Open Launch Config File",
            Self::OpenLaunchConfig => "Open Launch Config",
            Self::LogOut => "Log Out",
            Self::SelectNavigationPaletteItem => "Select Navigation Palette Item",
            Self::CommandCorrection => "Command Correction Event",
            Self::SetLineHeight => "Set Line Height",
            Self::ResourceCenterOpened => "Resource Center Opened",
            Self::ResourceCenterTipsCompleted => "Resource Center Tips Completed",
            Self::ResourceCenterTipsSkipped => "Resource Center Tips Skipped",
            Self::KeybindingsPageOpened => "Resource Center Keybindings Page Opened",
            Self::GlobalSearchOpened => "Global Search Opened",
            Self::GlobalSearchQueryStarted => "Global Search Query Started",
            Self::CommandSearchOpened => "Command Search Opened",
            Self::CommandSearchExited => "Command Search Exited",
            Self::CommandSearchResultAccepted => "Command Search Result Accepted",
            Self::CommandSearchFilterChanged => "Command Search Filter Changed",
            Self::AICommandSearchOpened => "AI Command Search opened",
            Self::OpenedAltScreenFind => "Opened alt screen find bar",
            Self::UserInitiatedClose => "User Initiated Closing Something",
            Self::QuitModalShown => "Quit Modal Shown",
            Self::QuitModalCancel => "Quit Modal Cancel Pressed",
            Self::QuitModalDisabled => "Quit Modal Disabled",
            Self::UserInitiatedLogOut => "User Initiated Log Out",
            Self::LogOutModalShown => "Log Out Modal Shown",
            Self::LogOutModalCancel => "Log Out Modal Cancel Pressed",
            Self::SetBlurRadius => "Set Window Blur Radius",
            Self::SetOpacity => "Set Window Opacity",
            Self::ToggleDimInactivePanes => "Toggle Dim Inactive Panes",
            Self::ToggleJumpToBottomofBlockButton => "Toggle Jump to Bottom of Block Button",
            Self::ToggleShowBlockDividers => "Toggle Show Block Dividers",
            Self::PtySpawned => "Pty Spawned",
            Self::InitialWorkingDirectoryConfigurationChanged => {
                "InitialWorkingDirectoryConfigurationChanged"
            }
            Self::InputModeChanged => "Input Mode Changed",
            Self::OpenedWarpAI => "Opened Warp AI",
            Self::WarpAIRequestIssued => "Warp AI Request Issued",
            Self::WarpAIAction => "Warp AI Action",
            Self::UsedWarpAIPreparedPrompt => "Used Warp AI Prepared Prompt",
            Self::WarpAICharacterLimitExceeded => "Warp AI Character Limit Exceeded",
            Self::OpenInputContextMenu => "OpenInputBoxContextMenu",
            Self::InputCutSelectedText => "InputBoxCutSelectedText",
            Self::InputCopySelectedText => "InputBoxCutSelectedText",
            Self::InputSelectAll => "InputBoxSelectAll",
            Self::InputPaste => "InputBoxPaste",
            Self::InputCommandSearch => "InputBoxCommandSearch",
            Self::InputAICommandSearch => "InputBoxAICommandSearch",
            Self::InputAskWarpAI => "InputBoxAskWarpAI",
            Self::SaveAsWorkflowModal => "Opened Save As Workflow Modal",
            Self::ExperimentTriggered => "experiments.client.enroll_client",
            Self::ToggleSyncAllPanesInAllTabs => "Toggle Sync Inputs Across All Panes in All Tabs",
            Self::ToggleSyncAllPanesInTab => "Toggle Sync Inputs Across All Panes in Current Tab",
            Self::ToggleSameLinePrompt => "Toggle Same Line Prompt",
            Self::DisableInputSync => "Disable Input Sync Inputs",
            Self::ToggleTabIndicators => "Toggle Tab Indicators",
            Self::TogglePreserveActiveTabColor => "Toggle Preserve Active Tab Color",
            Self::ShowSubshellBanner => "Show Subshell Banner",
            Self::SshTmuxWarpifyBannerDisplayed => "Show Warpify SSH Banner",
            Self::DeclineSubshellBootstrap => "Decline Subshell Bootstrap",
            Self::TriggerSubshellBootstrap => "Trigger Subshell Bootstrap",
            Self::AddDenylistedSubshellCommand => "Add Denylisted Subshell Command",
            Self::RemoveDenylistedSubshellCommand => "Remove Denylisted Subshell Command",
            Self::AddAddedSubshellCommand => "Add Added Subshell Command",
            Self::RemoveAddedSubshellCommand => "Remove Added Subshell Command",
            Self::ReceivedSubshellRcFileDcs => "Received Subshell RC File DCS",
            Self::ToggleSshTmuxWrapper => "Toggle SSH Tmux Wrapper",
            Self::ToggleSshWarpification => "Toggle SSH Warpification",
            Self::SetSshExtensionInstallMode => "Set SSH Extension Install Mode",
            Self::SshRemoteServerChoiceDoNotAskAgainToggled => {
                "SSH Remote Server Choice Do Not Ask Again Toggled"
            }
            Self::AddDenylistedSshTmuxWrapperHost => "Add Denylisted SSH Tmux Wrapper Host",
            Self::RemoveDenylistedSshTmuxWrapperHost => "Remove Denylisted SSH Tmux Wrapper Host",
            Self::SshInteractiveSessionDetected => "SSH Interactive Session Detected",
            Self::SshTmuxWarpifyBlockAccepted => "SSH Tmux Warpify Block Accepted",
            Self::SshTmuxWarpifyBlockDismissed => "SSH Tmux Warpify Block Dismissed",
            Self::WarpifyFooterShown => "Warpify Footer Shown",
            Self::AgentToolbarDismissed => "Agent Toolbar Dismissed",
            Self::WarpifyFooterAcceptedWarpify => "Warpify Footer Accepted Warpify",
            Self::SshTmuxWarpificationSuccess => "SSH Tmux Warpification Succeeded",
            Self::SshTmuxWarpificationErrorBlock => "SSH Tmux Warpification Error Block",
            Self::SshInstallTmuxBlockDisplayed => "SSH Install Tmux Block Displayed",
            Self::SshInstallTmuxBlockAccepted => "SSH Install Tmux Block Accepted",
            Self::SshInstallTmuxBlockDismissed => "SSH Install Tmux Block Dismissed",
            Self::ShowAliasExpansionBanner => "Show Alias Expansion Banner",
            Self::DismissAliasExpansionBanner => "Dismiss Alias Expansion Banner",
            Self::EnableAliasExpansionFromBanner => "Enable Alias Expansion From Banner",
            Self::InitiateReauth => "Initiate Reauth",
            Self::NeedsReauth => "Needs Reauth",
            Self::WarpDriveOpened => "Warp Drive Opened",
            Self::ToggleWarpAI => "Toggle Warp AI",
            Self::ToggleSecretRedaction => "Toggle Secret Redaction",
            Self::CustomSecretRegexAdded => "Custom Secret Regex Added",
            Self::ToggleObfuscateSecret => "Toggle Obfuscate Secret",
            Self::CopySecret => "Copy Obfuscated Secret",
            Self::AutoGenerateMetadataSuccess => "Generate Metadata For Workflow Success",
            Self::AutoGenerateMetadataError => "Generate Metadata For Workflow Error",
            Self::UndoClose => "Undo Close",
            Self::OpenPromptEditor => "Prompt Editor Opened",
            Self::PromptEdited => "Prompt Edited",
            Self::PtyThroughput => "PTY Throughput",
            Self::CommandFileRun => "Command File Run",
            Self::PageUpDownInEditorPressed => "Page Up/Down In Editor Pressed",
            Self::JoinedSharedSession => "Joined Shared Session",
            Self::SharedSessionModalUpgradePressed => "Shared Session Modal Upgrade Pressed",
            Self::SharerCancelledGrantRole => "Sharer Cancelled Grant Role",
            Self::SharerGrantModalDontShowAgain => "Don't Show Sharer Grant Modal Again",
            Self::JumpToSharedSessionParticipant { .. } => "Jumped to Shared Session Participant",
            Self::DriveSharingOnboardingBlockShown => "Warp Drive Sharing onboarding block shown",
            Self::UnsupportedShell => "Unsupported Shell",
            Self::SettingsImportInitiated => "Settings Import Initiated",
            Self::InviteTeammates => "Invited Teammates",
            Self::OpenAndWarpifyDockerSubshell => "OpenAndWarpifyDockerSubshell",
            Self::UpdateBlockFilterQuery => "Update Block Filter Query",
            Self::ToggleBlockFilterQuery => "Toggle Block Filter Query",
            Self::ToggleBlockFilterCaseSensitivity => "Toggle Block Filter Case Sensitivity",
            Self::ToggleBlockFilterRegex => "Toggle Block Filter Regex",
            Self::ToggleBlockFilterInvert => "Toggle Block Filter Invert",
            Self::BlockFilterToolbeltButtonClicked => "Block Filter Toolbelt Button Clicked",
            Self::ShowVimKeybindingsBanner => "Vim Keybindings Banner Displayed",
            Self::EnableVimKeybindingsFromBanner => "Vim Keybindings Enabled from Banner",
            Self::DismissVimKeybindingsBanner => "Vim Keybindings Banner Dismissed",
            Self::UpdateBlockFilterQueryContextLines => {
                "Update Block Filter Query With Context Lines"
            }
            Self::ToggleSnackbarInActivePane => "Toggle Sticky Command Header in Active Pane",
            Self::PaneDragInitiated => "Pane Drag Inititiated",
            Self::PaneDropped => "Pane Drag Ended",
            Self::TeamCreated => "Team Created",
            Self::TeamJoined => "Team Joined",
            Self::TeamLeft => "Team Left",
            Self::TeamLinkCopied => "Team Link Copied",
            Self::RemovedUserFromTeam => "Removed user from team",
            Self::DeletedWorkflow => "Deleted Workflow",
            Self::DeletedNotebook => "Deleted Notebook",
            Self::ToggleApprovalsModal => "Toggle Approvals Modal",
            Self::SendEmailInvites => "Sent email invites",
            Self::TierLimitHit => "Tier Limit Hit",
            Self::SharedObjectLimitHitBannerViewPlansButtonClicked => {
                "Shared Object Limit Hit Banner View Plans Button Clicked"
            }
            Self::AgentModeUserAttemptedQueryAtRequestLimit => "AgentMode.QueryAttemptAtLImit",
            Self::AgentModeClickedEntrypoint => "AgentMode.ClickedEntrypoint",
            Self::AgentModeAttachedBlockContext => "AgentMode.AttachedContext",
            Self::ResourceUsageStats => "perf_metrics.resource_usage",
            Self::MemoryUsageStats => "perf_metrics.memory_usage",
            Self::MemoryUsageHigh => "perf_metrics.memory_usage_high",
            Self::AgentModeToggleAutoDetectionSetting => "AgentMode.ToggleAutoDetectionSetting",
            Self::AgentModePotentialAutoDetectionFalsePositive => {
                "AgentMode.PotentialAutoDetectionFalsePositive"
            }
            // Agent Mode Query Suggestions is the legacy name for Prompt Suggestions - we avoid renaming
            // the event to avoid breaking historical telemetry data.
            Self::PromptSuggestionShown => "Agent Mode Query Suggestions Banner Shown",
            Self::SuggestedCodeDiffFailed => "Suggested Code Diff Failed",
            Self::PromptSuggestionAccepted => "Agent Mode Query Suggestion Accepted",
            Self::StaticPromptSuggestionsBannerShown => "Static Prompt Suggestions Banner Shown",
            Self::StaticPromptSuggestionAccepted => "Static Prompt Suggestion Accepted",
            Self::TogglePromptSuggestionsSetting => "Toggle Agent Mode Query Suggestions Setting",
            Self::ToggleCodeSuggestionsSetting => "Toggle Code Suggestions Setting",
            Self::ToggleNaturalLanguageAutosuggestionsSetting => {
                "Toggle Natural Language Autosuggestions Setting"
            }
            Self::ToggleSharedBlockTitleGenerationSetting => "Toggle SharedBlock Title Generation",
            Self::ToggleGitOperationsAutogenSetting => "Toggle Git Operations Autogen Setting",
            Self::ToggleIntelligentAutosuggestionsSetting => {
                "Toggle Intelligent Autosuggestions Setting"
            }
            Self::ToggleVoiceInputSetting => "Toggle Voice Input Setting",
            Self::CompletedSettingsImport => "Completed Settings Import",
            Self::SettingsImportConfigFocused => "Focused Config in Settings Import",
            Self::SettingsImportConfigParsed => "Parsed Config in Settings Import",
            Self::SettingsImportResetButtonClicked => {
                "Clicked Reset to Defaults Button in Settings Import"
            }
            Self::ITermMultipleHotkeys => "ITerm Profile has Multiple Hotkeys",
            Self::ToggleWorkspaceDecorationVisibility => "Toggled Tab Bar Visibility",
            Self::UpdateAltScreenPaddingMode => "Updated Alt Screen Padding Mode",
            Self::AddTabWithShell => "Add Tab With Shell",
            Self::ToggleGlobalAI => "Toggle Global AI Enablement",
            Self::ToggleActiveAI => "Toggle Active AI Enablement",
            Self::ToggleLigatureRendering => "Toggle Ligature Rendering",

            Self::ToggledAgentModeAutoexecuteReadonlyCommandsSetting => {
                "AIAutonomy.ToggledAutoexecuteReadonlyCommandsSetting"
            }
            Self::ChangedAgentModeCodingPermissions => {
                "AIAutonomy.ChangedAgentModeCodingPermissions"
            }
            Self::RemoteServerBinaryCheck => "RemoteServer.BinaryCheck",
            Self::RemoteServerInstallation => "RemoteServer.Installation",
            Self::RemoteServerInitialization => "RemoteServer.Initialization",
            Self::RemoteServerDaemonStartup => "RemoteServer.DaemonStartup",
            Self::RemoteServerDisconnection => "RemoteServer.Disconnection",
            Self::RemoteServerClientRequestError => "RemoteServer.ClientRequestError",
            Self::RemoteServerMessageDecodingError => "RemoteServer.MessageDecodingError",
            Self::RemoteServerSetupDuration => "RemoteServer.SetupDuration",
            Self::RemoteServerHostUnsupported => "RemoteServer.HostUnsupported",
            Self::RemoteServerReconnection => "RemoteServer.Reconnection",
            Self::RemoteServerReconnectExhausted => "RemoteServer.ReconnectExhausted",
            Self::RemoteCodebaseIndexStatusChanged => "RemoteCodebaseIndex.StatusChanged",
            Self::RemoteCodebaseAutoIndexRequested => "RemoteCodebaseIndex.AutoIndexRequested",
            Self::QueuedPromptPanelCollapseToggled => "QueuedPrompt.PanelCollapseToggled",
            #[cfg(windows)]
            Self::WSLRegistryError => "WSL Distribution Registry Error",
            #[cfg(windows)]
            Self::AutoupdateUnableToCloseApplications => {
                "Windows Autoupdate: Setup Unable to Close Applications"
            }
            #[cfg(windows)]
            Self::AutoupdateFileInUse => "Windows Autoupdate: File In Use Error",
            #[cfg(windows)]
            Self::AutoupdateMutexTimeout => "Windows Autoupdate: Mutex Timeout",
            #[cfg(windows)]
            Self::AutoupdateForcekillFailed { .. } => "Windows Autoupdate: Forcekill Failed",
            #[cfg(windows)]
            Self::AutoupdateMinidumpCleanupFailed { .. } => {
                "Windows Autoupdate: Minidump Cleanup Failed"
            }
            Self::ToggleCodebaseContext => "Toggle Agent Mode Codebase Context",
            Self::ToggleAutoIndexing => "Toggle Codebase Context Autoindexing",
            Self::ActiveIndexedReposChanged => "Active Indexed Repos Changed",
            Self::AttachedImagesToAgentModeQuery => "AgentMode.AttachedImages",
            Self::ImageReceived => "Image Received",
            Self::GrepToolSucceeded => "AgentMode.Grep.Succeeded",
            Self::FileGlobToolSucceeded => "AgentMode.FileGlob.Succeeded",
            Self::ShellTerminatedPrematurely { .. } => "Shell Terminated Prematurely",
            Self::InputUXModeChanged { .. } => "Input.InputUXModeChanged",
            Self::ContextChipInteracted { .. } => "Input.ContextChipInteracted",
            Self::UserMenuUpgradeClicked => "User Menu Upgrade Clicked",
            Self::TabCloseButtonPositionUpdated { .. } => "Update Tab Close Button Position",
            Self::AIExecutionProfileCreated => "AI Execution Profile Created",
            Self::AIExecutionProfileDeleted => "AI Execution Profile Deleted",
            Self::AIExecutionProfileSettingUpdated { .. } => {
                "AI Execution Profile: Setting Updated"
            }
            Self::AIExecutionProfileAddedToAllowlist { .. } => {
                "AI Execution Profile: Added To Allowlist"
            }
            Self::AIExecutionProfileAddedToDenylist { .. } => {
                "AI Execution Profile: Added To Denylist"
            }
            Self::AIExecutionProfileRemovedFromAllowlist { .. } => {
                "AI Execution Profile: Removed From Allowlist"
            }
            Self::AIExecutionProfileRemovedFromDenylist { .. } => {
                "AI Execution Profile: Removed From Denylist"
            }
            Self::AIExecutionProfileModelSelected { .. } => "AI Execution Profile: Model Selected",
            Self::AIExecutionProfileContextWindowSelected { .. } => {
                "AI Execution Profile: Context Window Selected"
            }
            Self::OpenSlashMenu { .. } => "Open Slash Menu",
            Self::SlashCommandAccepted { .. } => "Slash Command Accepted",
            Self::AgentModeSetupBannerAccepted => "Agent Mode Setup Banner Accepted",
            Self::AgentModeSetupBannerDismissed => "Agent Mode Setup Banner Dismissed",
            Self::AgentModeSetupProjectScopedRulesAction { .. } => {
                "Agent Mode Setup Project Scoped Rules Action"
            }
            Self::AgentModeSetupCodebaseContextAction { .. } => {
                "Agent Mode.Setup Codebase Context Action"
            }
            Self::AgentModeSetupCreateEnvironmentAction { .. } => {
                "AgentMode.SetupCreateEnvironmentAction"
            }
            Self::RecentMenuItemSelected { .. } => "Recent Menu Item Selected",
            Self::OpenRepoFolderSubmitted { .. } => "Open Repo Folder Submitted",
            Self::OutOfCreditsBannerClosed => "revenue.OutOfCreditsBannerClosed",
            Self::AutoReloadModalClosed => "revenue.AutoReloadModalClosed",
            Self::AutoReloadToggledFromBillingSettings => {
                "revenue.AutoReloadToggledFromBillingSettings"
            }
            Self::AgentManagementViewToggled { .. } => "Agent Management View Toggled",
            Self::AgentManagementViewOpenedSession => "Agent Management View Opened Session",
            Self::AgentManagementViewCopiedSessionLink => {
                "Agent Management View Copied Session Link"
            }
            Self::DetectedIsolationPlatform { .. } => "Isolation.DetectedIsolationPlatform",
            Self::AgentTipShown => "AgentTip Shown",
            Self::AgentTipClicked => "AgentTip Clicked",
            Self::ToggleShowAgentTips => "Toggle Show Agent Tips",
            Self::CLIAgentToolbarVoiceInputUsed { .. } => "CLIAgentFooter.VoiceInputUsed",
            Self::CLIAgentToolbarImageAttached { .. } => "CLIAgentFooter.ImageAttached",
            Self::CLIAgentToolbarShown { .. } => "CLIAgentFooter.Shown",
            Self::CLIAgentPluginChipClicked { .. } => "CLIAgentPlugin.ChipClicked",
            Self::CLIAgentPluginChipDismissed { .. } => "CLIAgentPlugin.ChipDismissed",
            Self::CLIAgentPluginOperationSucceeded { .. } => "CLIAgentPlugin.OperationSucceeded",
            Self::CLIAgentPluginOperationFailed { .. } => "CLIAgentPlugin.OperationFailed",
            Self::CLIAgentPluginDetected { .. } => "CLIAgentPlugin.Detected",
            Self::CLIAgentRichInputOpened { .. } => "CLIAgentRichInput.Opened",
            Self::CLIAgentRichInputClosed { .. } => "CLIAgentRichInput.Closed",
            Self::CLIAgentRichInputSubmitted { .. } => "CLIAgentRichInput.Submitted",
            Self::ToggleCLIAgentToolbarSetting { .. } => "CLIAgentFooter.SettingToggled",
            Self::ToggleUseAgentToolbarSetting { .. } => "UseAgentToolbar.SettingToggled",
            Self::CodexModalOpened => "CodexModal.Opened",
            Self::CodexModalUseCodexClicked => "CodexModal.UseCodexClicked",
            Self::LinearIssueLinkOpened => "Linear.IssueLinkOpened",
            Self::CloudAgentCapacityModalOpened => "AmbientAgent.ConcurrencyModal.Opened",
            Self::CloudAgentCapacityModalDismissed => "AmbientAgent.ConcurrencyModal.Dismissed",
            Self::CloudAgentCapacityModalUpgradeClicked => {
                "AmbientAgent.ConcurrencyModal.UpgradeClicked"
            }
            Self::FreeTierLimitHitInterstitialDisplayed { .. } => {
                "FreeTierLimitHitInterstitial.Displayed"
            }
            Self::FreeTierLimitHitInterstitialUpgradeButtonClicked { .. } => {
                "FreeTierLimitHitInterstitial.UpgradeButtonClicked"
            }
            Self::FreeTierLimitHitInterstitialClosed { .. } => {
                "FreeTierLimitHitInterstitial.Closed"
            }
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::AIExecutionProfileContextWindowSelected { .. } => {
                "Selected a context window limit for an execution profile's base model"
            }
            Self::RepoOutlineConstructionSuccess => {
                "Repository outline built successfully for providing codebase context"
            }
            Self::RepoOutlineConstructionFailed => "Repository outline built failed",
            Self::AutosuggestionInserted => "Accepted autosuggestion",
            Self::BlockCompleted => "Created Block",
            Self::AgentModeRewindDialogOpened { .. } => {
                "User opened the rewind confirmation dialog"
            }
            Self::AgentModeRewindExecuted { .. } => {
                "User executed a rewind to a previous conversation state"
            }
            Self::BlockCompletedOnDogfoodOnly => {
                "Completed a block, with extra information for dogfood only"
            }
            Self::InitiateAnonymousUserSignup => "An anonymous user initiated the sign up flow",
            Self::AnonymousUserExpirationLockout => {
                "An anonymous user opened Warp after their conversion deadline and was locked out"
            }
            Self::AnonymousUserLinkedFromBrowser => {
                "Received an auth payload from anonymous user after linking in browser"
            }
            Self::AnonymousUserAttemptLoginGatedFeature => {
                "Anonymous user attempted to access a login-gated feature"
            }
            Self::AnonymousUserHitCloudObjectLimit => {
                "Anonymous user attempted to create a cloud object past their personal object limit"
            }
            Self::BackgroundBlockStarted => {
                "Warp created a background-output Block (whenever a processes has been backgrounded and yields some output)"
            }
            Self::BaselineCommandLatency => "Command execution time",
            Self::SessionCreation => "Created a tab",
            Self::KnowledgePaneOpened { .. } => "Knowledge Pane Opened",
            #[cfg(feature = "local_fs")]
            Self::CodePanelsFileOpened { .. } => {
                "Opened a file from code review, project explorer, or global search"
            }
            #[cfg(feature = "local_fs")]
            Self::PreviewPanePromoted => "Promoted a preview code tab to a normal tab",
            Self::ToggleSettingsSync => "Toggle Settings Sync",
            Self::Login => "Login is successful",
            Self::LoginLaterButtonClicked => "Clicked \"Login later\" button",
            Self::LoginLaterConfirmationButtonClicked => {
                "Clicked \"Yes, skip login\" confirmation button"
            }
            Self::OpenSuggestionsMenu => "Opened a suggestion menus, such as with up arrow or tab",
            Self::ConfirmSuggestion => "Accepted tab completion suggestion",
            Self::OpenContextMenu => {
                "Opened context menu (such as right clicking, clicking on ellipses in the top right of a Block, etc.)"
            }
            Self::ContextMenuCopy => "Clicked \"Copy\" in context menu",
            Self::ContextMenuOpenShareModal => "Opened \"Share\" modal via context menu",
            Self::ContextMenuFindWithinBlocks => "Clicked \"find within blocks\" in context menu",
            Self::ContextMenuCopyPrompt => "Clicked  \"Copy Prompt\" in context menu",
            Self::ContextMenuToggleGitPromptDirtyIndicator => {
                "Toggled indicator of dirty git prompt"
            }
            Self::ContextMenuInsertSelectedText => "Clicked \"insert into input\" in context menu",
            Self::ContextMenuCopySelectedText => "Clicked \"Copy selected text\" in context menu",
            Self::OpenPromptEditor => "Opened the prompt editor",
            Self::PromptEdited => "Edited the prompt using the built-in prompt editor",
            Self::ReinputCommands => "Clicked \"reinput commands\" in context menu",
            Self::JumpToPreviousCommand => "Jumped to a previous command",
            Self::CopyBlockSharingLink => "Clicked \"Share block...\" in context menu",
            Self::GenerateBlockSharingLink => "Generated Block sharing link",
            Self::BlockSelection => "Selected Block",
            Self::BootstrappingSlow => "Slow bootstrap on session startup",
            Self::BootstrappingSlowContents => {
                "Contents of the bootstrap block if bootstrapping is slow"
            }
            Self::SessionAbandonedBeforeBootstrap => {
                "Abandoned session before the bootstrapping completes"
            }
            Self::BootstrappingSucceeded => "Successful bootstrap for session",
            Self::TabSingleResultAutocompletion => {
                "Accepted tab completion and inserted into Input Editor"
            }
            Self::EditorUnhandledModifierKey => {
                "Used modifier keybinding keystroke which is not currently supported"
            }
            Self::CopyInviteLink => "Clicked \"Copy Link\" on Referral Modal",
            Self::OpenThemeChooser => {
                "Opened theme chooser (list of different themes and visualizations of those themes)"
            }
            Self::ThemeSelection => "Selected theme",
            Self::AppIconSelection => "Selected app icon",
            Self::CursorDisplayType => "Selected cursor type",
            Self::OpenThemeCreatorModal => {
                "Opened theme creator modal (modal to create a new theme)"
            }
            Self::CreateCustomTheme => "Created a custom theme using the built-in theme creator",
            Self::DeleteCustomTheme => "Deleted a custom theme using the built-in theme creator",
            Self::SplitPane => "Split tab into multiple panes",
            Self::UnableToAutoUpdateToNewVersion => {
                "Update available but not authorized to install"
            }
            Self::AutoupdateRelaunchAttempt => {
                "Attempted to relaunch the app after installing an update"
            }
            Self::SkipOnboardingSurvey => "Skipped onboarding survey as a whole",
            Self::ToggleRestoreSession => {
                "Toggled session restoration (\"Restore windows, tabs, panes, on startup\")"
            }
            Self::DatabaseStartUpError => "Failed to initialize sqlite upon startup",
            Self::DatabaseReadError => {
                "Database read error when trying to get app state for session restoration"
            }
            Self::DatabaseWriteError => {
                "Database write error when trying to write app state for session restoration"
            }
            Self::AppStartup => "App is launched",
            Self::LoggedOutStartup => "Started Warp in the logged-out / signed-out state",
            Self::DownloadSource => {
                "Whether the Warp was installed from the home page or through homebrew"
            }
            Self::SSHBootstrapAttempt => "Attempted bootstrapping for an SSH session",
            Self::SSHControlMasterError => {
                "Encountered a ControlMaster error during an SSH session"
            }
            Self::KeybindingChanged => "Edited a custom keybinding",
            Self::KeybindingResetToDefault => "Reset a custom keybinding to its default",
            Self::KeybindingRemoved => "Removed / cleared a keybinding",
            Self::FeaturesPageAction => "Changed settings in Features Page",
            Self::OpenWorkflowSearch => "Opened workflows search in command search pane",
            Self::OpenQuakeModeWindow => {
                "Toggled quake mode window when previously hidden or closed"
            }
            Self::OpenWelcomeTips => "Opened welcome tips in app",
            Self::CompleteWelcomeTipFeature => "Completed all welcome tips items",
            Self::DismissWelcomeTips => "Dismissed Welcome tips",
            Self::ShowNotificationsDiscoveryBanner => {
                "Showed notifications discovery banner in the block list"
            }
            Self::NotificationsDiscoveryBannerAction => {
                "Showed banner introducing the notifications feature"
            }
            Self::ShowNotificationsErrorBanner => "Showed error banner for notifications feature",
            Self::NotificationsErrorBannerAction => "Showed error banner for notifications feature",
            Self::NotificationPermissionsRequested => {
                "Requested permission for desktop notification permissions"
            }
            Self::NotificationsRequestPermissionsOutcome => {
                "Recorded outcome of attempting to request desktop notification permissions"
            }
            Self::NotificationFailedToSend => "Failed to send desktop notification",
            Self::NotificationClicked => "Clicked desktop notification sent from Warp",
            Self::ToggleShowAgentTips => "Toggled the Show Agent Tips setting in AI settings",
            Self::ToggleFindOption => "Changed settings in Find Toggle",
            Self::SignUpButtonClicked => "Clicked \"Sign Up\" button",
            Self::LoginButtonClicked => "Clicked on \"Log in\" button",
            Self::OpenNewSessionFromFilePath => {
                "Dragged a file, folder, etc. into Warp to start a session"
            }
            Self::OpenTeamFromURI => {
                "Showed settings view of their newly joined team within the app"
            }
            Self::SelectCommandPaletteOption => "Selected option from command palette (i.e. CMD-P)",
            Self::PaletteSearchOpened => "Opened the palette",
            Self::PaletteSearchResultAccepted => "Accepted a command palette search result",
            Self::PaletteSearchExited => "Exited command palette search without accepting a result",
            Self::SelectNavigationPaletteItem => {
                "Selected session from the Session Navigation Palette (search across panes, tabs, and windows)"
            }
            Self::AuthCommonQuestionClicked => "Clicked on \"Common Question\" when logging in",
            Self::AuthToggleFAQ => "Toggled FAQ Page when logging in",
            Self::OpenAuthPrivacySettings => "Privacy settings are open during sign-in",
            Self::TabRenamed => "Changed tab title",
            Self::MoveActiveTab => "Move active tab left or right",
            Self::MoveTab => "Move tab left or right",
            Self::DragAndDropTab => "Tab dragged and dropped",
            Self::DragAndDropTabGroup => "Tab group dragged and dropped",
            Self::TabOperations => {
                "Took operation on a tab: change color, close tab, close adjacent tabs, etc."
            }
            Self::EditedInputBeforePrecmd => "Input edited before precmd hook completes",
            Self::TriedToExecuteBeforePrecmd => {
                "Attempted to execute command before precmd, a shell stage that has metadata on a command such as ssh, prompt info, etc."
            }
            Self::ThinStrokesSettingChanged => {
                "Changed thin strokes setting in settings -> Appearance"
            }
            Self::BookmarkBlockToggled => "Bookmarked or unbookmarked Block",
            Self::JumpToBookmark => "Jumped to bookmarked Block",
            Self::JumpToLatestAgentMessage => "Jumped to the latest agent message",
            Self::JumpToBottomofBlockButtonClicked => {
                "Used the button to jump to the bottom of a Block"
            }
            Self::ToggleJumpToBottomofBlockButton => {
                "Enabled or disabled the Jump to Bottom of Block Button"
            }
            Self::ToggleShowBlockDividers => "Enabled or disabled the Show Block Dividers Button",
            Self::OpenLink => "Opened a highlighted link within input or output",
            Self::OpenChangelogLink => "Opened the changelog link within the App",
            Self::ShowInFileExplorer => "Opened a file in Finder by using \"Show in Finder\"",
            Self::CommandXRayTriggered => {
                "Triggered Command X-Ray (hovering over a command for explanation)"
            }
            Self::OpenLaunchConfigSaveModal => "Opened save launch configuration modal",
            Self::SaveLaunchConfig => {
                "Saved current launch configuration of windows, tabs, and panes"
            }
            Self::OpenLaunchConfigFile => {
                "Opened the launch config YAML file from modal once saved successfully"
            }
            Self::OpenLaunchConfig => "Opened launch config for a session",
            Self::TeamCreated => "Created a Warp Drive team",
            Self::TeamJoined => "Joined a Warp Drive team",
            Self::TeamLeft => "Left a Warp Drive team",
            Self::TeamLinkCopied => "Copied a Warp Drive team link",
            Self::RemovedUserFromTeam => "Remove user from Warp Drive team",
            Self::DeletedWorkflow => "Deleted workflow from Warp Drive team",
            Self::DeletedNotebook => "Deleted notebook from Warp Drive team",
            Self::ToggleApprovalsModal => "Opened or closed teams modal",
            Self::SendEmailInvites => "Sent email invites for Warp Drive team",
            Self::CommandCorrection => "Accepted command correction",
            Self::SetLineHeight => "Set line height through Settings -> Appearance",
            Self::ResourceCenterOpened => "Opened Resource Center pane",
            Self::ResourceCenterTipsCompleted => "Completed resource center tips",
            Self::ResourceCenterTipsSkipped => "Skipped welcome tips for new users",
            Self::KeybindingsPageOpened => "Opened the keybinding page within the resource center",
            Self::CommandSearchOpened => "Opened command search (universal search panel to search)",
            Self::CommandSearchExited => {
                "Exited command search (universal search panel to search) without accepting a result"
            }
            Self::CommandSearchResultAccepted => "Accepted command search result",
            Self::CommandSearchFilterChanged => "Changed command search filter",
            Self::AICommandSearchOpened => {
                "Opened the modal for AI Command Search, where you can use natural language to search for commands"
            }
            Self::OpenedAltScreenFind => "Opened the Find bar in the Alt Screen",
            Self::UserInitiatedClose => "Attempted to either quit the app or close a window",
            Self::QuitModalShown => {
                "Showed an alert modal to warn the user about closing the app/window with a running process"
            }
            Self::QuitModalCancel => "`Cancel` button on the alert modal was pressed",
            Self::QuitModalDisabled => {
                "The quit modal dialog has been disabled and will not popup when a user closes Warp while a session is running"
            }
            Self::UserInitiatedLogOut => {
                "Confirms a user has explicitly logged out of the application"
            }
            Self::LogOutModalShown => "When the log out modal is displayed",
            Self::LogOutModalCancel => "Escaped the log out flow by canceling the log out modal",
            Self::SetOpacity => {
                "Changed the opacity (window transparency) from the `Settings -> Appearance` dialog"
            }
            Self::SetBlurRadius => {
                "Changed the blur radius from the `Settings -> Appearance` dialog"
            }
            Self::ToggleDimInactivePanes => {
                "Whether the dim inactive panes feature has been toggled"
            }
            Self::InputModeChanged => {
                "Changed the Input Editor Mode (Pinned to Bottom, Pinned to Top, Classic / Waterfall Mode)"
            }
            Self::PtySpawned => {
                "Tracks the manner by which we create a new shell process (new codepath vs. old codepath).  Used to ensure nothing breaks as we change parts of our infrastructure."
            }
            Self::InitialWorkingDirectoryConfigurationChanged => {
                "Replaced the default working directory with a different path"
            }
            Self::OpenedWarpAI => "Activated Warp AI",
            Self::WarpAIRequestIssued => "Issued a question to Warp AI",
            Self::WarpAIAction => "Executed a Warp AI action: Restart, Copy, Insert into terminal",
            Self::UsedWarpAIPreparedPrompt => {
                "Used one of the Warp-provided prompts, like \"Show examples\""
            }
            Self::WarpAICharacterLimitExceeded => {
                "Attempted to ask a question longer than 1k chars to Warp AI"
            }
            Self::OpenInputContextMenu => "Opened the Input Editor's context menu",
            Self::InputCutSelectedText => {
                "Cut the highlighted text via the Input Editor's context menu (right clicking the buffer)"
            }
            Self::InputCopySelectedText => "Copied selected text from Input Editor",
            Self::InputSelectAll => {
                "Selected all the text in the Input Editor via its context menu (right clicking the buffer)"
            }
            Self::InputPaste => {
                "Pasted text into the Input Editor's via its context menu (right clicking the buffer)"
            }
            Self::InputCommandSearch => {
                "Opened Command Search via the Input Editor's context menu (right clicking the buffer)"
            }
            Self::InputAICommandSearch => {
                "Opened AI Command Search via the Input Editor's context menu (right clicking the buffer)"
            }
            Self::InputAskWarpAI => "Clicked \"Ask Warp AI\" from the Input Editor's context menu",
            Self::SaveAsWorkflowModal => {
                "Opened the modal to create a new workflow using a Block's context--command, etc."
            }
            Self::ExperimentTriggered => "Client assigned to A/B test",
            Self::ToggleSyncAllPanesInAllTabs => {
                "Enable the synchronization of the Input Editor's buffer to all the panes in all the tabs"
            }
            Self::ToggleSyncAllPanesInTab => {
                "Enable the synchronization of the Input Editor's buffer to all the panes in the current tab"
            }
            Self::ToggleSameLinePrompt => "Toggled on/off same line prompt",
            Self::ToggleNewWindowsAtCustomSize => {
                "Whether the new windows at custom size feature has been toggled"
            }
            Self::ToggleFocusPaneOnHover => {
                "Toggled on/off focus pane on hover feature, which causes panes to automatically focus when hovering over them"
            }
            Self::SetNewWindowsAtCustomSize => {
                "Set new windows at custom size through Settings -> Appearance"
            }
            Self::DisableInputSync => {
                "Disabled / turn off the Input Synchronization (across editors)"
            }
            Self::ToggleTabIndicators => {
                "Enabled or disabled the tab indicators (failed command, etc.)"
            }
            Self::TogglePreserveActiveTabColor => {
                "Enabled or disabled preserving the active tab color"
            }
            Self::ShowSubshellBanner => {
                "Displayed the banner asking whether Warp should Warpify the current session via Warp's subshell wrapper"
            }
            Self::SshTmuxWarpifyBannerDisplayed => {
                "Displayed the banner asking whether Warp should Warpify the current SSH session via Warp's SSH Wrapper"
            }
            Self::DeclineSubshellBootstrap => {
                "Developer declined the Warp banner to Warpify the current session"
            }
            Self::TriggerSubshellBootstrap => {
                "Attempted to Warpify the current session via Warp's subshell wrapper"
            }
            Self::AddDenylistedSubshellCommand => {
                "Explicitly prevent a command from being Warpified via Warp's subshell wrapper"
            }
            Self::RemoveDenylistedSubshellCommand => {
                "Removed a command from the list of commands to IGNORE when trying to Warpify via Warp's subshell wrapper"
            }
            Self::AddAddedSubshellCommand => {
                "Added a command to be automatically Warpified via Warp's subshell wrapper"
            }
            Self::RemoveAddedSubshellCommand => {
                "Removed a command from the list of commands to automatically Warpify via Warp's subshell wrapper"
            }
            Self::ReceivedSubshellRcFileDcs => "Spawned a subshell to be automatically Warpified",
            Self::ToggleSshTmuxWrapper => {
                "Changed the setting for SSH sessions to prompt for Tmux Wrapper"
            }
            Self::ToggleSshWarpification => "Changed the setting for SSH sessions to be warified",
            Self::SetSshExtensionInstallMode => {
                "Changed the SSH extension install mode (always ask / always allow / always skip)"
            }
            Self::SshRemoteServerChoiceDoNotAskAgainToggled => {
                "Toggled the 'Don't ask me this again' checkbox on the SSH remote-server choice block"
            }
            Self::AddDenylistedSshTmuxWrapperHost => {
                "Added a SSH host to the denylist for prompting for Tmux Wrapper"
            }
            Self::RemoveDenylistedSshTmuxWrapperHost => {
                "Removed an SSH host from the denylist from prompting for Tmux Wrapper"
            }
            Self::SshInteractiveSessionDetected => "An interactive SSH session was detected",
            Self::SshTmuxWarpifyBlockAccepted => "User accepted an ssh tmux warpify block",
            Self::SshTmuxWarpifyBlockDismissed => "User dismissed an ssh tmux warpify block",
            Self::WarpifyFooterShown => {
                "Displayed the warpify footer for a detected subshell or SSH session"
            }
            Self::AgentToolbarDismissed => "User dismissed the use-agent toolbar",
            Self::WarpifyFooterAcceptedWarpify => "User clicked Warpify in the warpify footer",
            Self::SshTmuxWarpificationSuccess => "Ssh tmux warpification succeeded",
            Self::SshTmuxWarpificationErrorBlock => "Ssh tmux warpification errored out",
            Self::SshInstallTmuxBlockDisplayed => "Displayed an ssh install tmux block",
            Self::SshInstallTmuxBlockAccepted => "User accepted an ssh install tmux block",
            Self::SshInstallTmuxBlockDismissed => "User dismissed an ssh install tmux block",
            Self::ShowAliasExpansionBanner => {
                "Displayed the banner asking whether Warp should automatically expand aliases within the Input Editor"
            }
            Self::EnableAliasExpansionFromBanner => {
                "Enabled automatic alias expansion within the Input Editor from the banner"
            }
            Self::DismissAliasExpansionBanner => {
                "Dismissed the banner to enable automatic alias expansion within the Input Editor"
            }
            Self::ShowVimKeybindingsBanner => {
                "Displayed the banner asking whether Warp should enable Vim keybindings in the Input Editor"
            }
            Self::EnableVimKeybindingsFromBanner => {
                "Enabled Vim keybindings in the Input Editor from the banner"
            }
            Self::DismissVimKeybindingsBanner => {
                "Dismissed the banner to enable Vim keybindings in the Input Editor"
            }
            Self::InitiateReauth => "Started the flow to re-authenticate the client",
            Self::NeedsReauth => "User needs to re-authenticate",
            Self::WarpDriveOpened => "Opened Warp Drive panel",
            Self::ToggleWarpAI => {
                "Toggled Warp AI--an AI assistant to help you debug errors, look up forgotten commands and more"
            }
            Self::ToggleSecretRedaction => {
                "Toggled on/off the setting for Secret Redaction - attempts to redact secrets and sensitive information"
            }
            Self::CustomSecretRegexAdded => "Custom Secret Regex Added",
            Self::ToggleObfuscateSecret => "Revealed or hid a secret",
            Self::CopySecret => "Copied a secret's obfuscated contents to clipboard",
            Self::AutoGenerateMetadataSuccess => {
                "Successfully generated metadata for a workflow using Warp AI"
            }
            Self::AutoGenerateMetadataError => {
                "Failed to generate metadata for a workflow using Warp AI"
            }
            Self::UndoClose => "Re-opened a closed tab or window (undo closing a tab or window)",
            Self::PtyThroughput => "A sample of the max PTY throughput in bytes/sec",
            Self::CommandFileRun => {
                "Opened a .cmd or unix executable file and ran it directly in Warp"
            }
            Self::PageUpDownInEditorPressed => {
                "Pressed `PAGE-UP` or `PAGE-DOWN` within the Input Editor"
            }
            Self::JoinedSharedSession => {
                "When you join another instance of Warp using shared sessions"
            }
            Self::SharedSessionModalUpgradePressed => {
                "Pressed upgrade after reaching max session sharing limit"
            }
            Self::SharerCancelledGrantRole => {
                "When you cancel granting a role to a shared session participant"
            }
            Self::SharerGrantModalDontShowAgain => {
                "When you check don't show again on the confirmation modal for granting a role"
            }
            Self::JumpToSharedSessionParticipant => {
                "Clicked on a shared session participant avatar to jump to their location in the session"
            }
            Self::DriveSharingOnboardingBlockShown => {
                "Showed onboarding block for Warp Drive sharing"
            }
            Self::UnsupportedShell => "Booted Warp with a shell that isn't supported",
            Self::LogOut => "Logged out of the Warp client",
            Self::SettingsImportInitiated => "Started the import settings flow for new users",
            Self::InviteTeammates => "Sent emails to invite teammates to join Warp Drive team",
            Self::OpenAndWarpifyDockerSubshell => {
                "Warpifying a docker subshell from using the docker extension"
            }
            Self::UpdateBlockFilterQuery => "When a new filter is applied to a block",
            Self::UpdateBlockFilterQueryContextLines => {
                "When the number of context lines for a block filter query is updated"
            }
            Self::ToggleBlockFilterQuery => "Toggled on/off a block filter query",
            Self::ToggleBlockFilterCaseSensitivity => {
                "Toggled on/off case sensitivity within the block filter editor"
            }
            Self::ToggleBlockFilterRegex => "Toggled on/off regex within the block filter editor",
            Self::ToggleBlockFilterInvert => "Toggled on/off invert within the block filter editor",
            Self::BlockFilterToolbeltButtonClicked => {
                "Clicked the block filter icon in the top-right of a block"
            }
            Self::ToggleSnackbarInActivePane => {
                "Expanded or collapsed the sticky command header in the active pane"
            }
            Self::PaneDragInitiated => "Initiated dragging a pane via the header",
            Self::PaneDropped => "Ended dragging a pane via the pane header",
            Self::TierLimitHit => "User hit the tier limit for a feature",
            Self::SharedObjectLimitHitBannerViewPlansButtonClicked => {
                "Clicked the 'View Plans' button on the persistent drive banner"
            }
            Self::AgentModeUserAttemptedQueryAtRequestLimit => {
                "Tried to send an Agent Mode query but they already reached the query limit"
            }
            Self::AgentModeClickedEntrypoint => "Clicked on an Agent Mode entrypoint",
            Self::AgentModeAttachedBlockContext => {
                "Attached block as context to an Agent Mode query"
            }
            Self::ResourceUsageStats => "Periodic report on application resource usage statistics",
            Self::MemoryUsageStats => "Periodic report on application memory usage statistics",
            Self::MemoryUsageHigh => {
                "Total application memory usage exceeded a significant threshold"
            }
            Self::AgentModeToggleAutoDetectionSetting => {
                "Toggled the setting that enables or disables natural language auto-detection in the input. "
            }
            Self::AgentModePotentialAutoDetectionFalsePositive => {
                "Manually toggled input to shell mode after input was auto-detected as natural language."
            }
            Self::ToggleIntelligentAutosuggestionsSetting => {
                "Toggled on/off the intelligent autosuggestions setting"
            }
            Self::TogglePromptSuggestionsSetting => "Toggled on/off the prompt suggestions setting",
            Self::ToggleCodeSuggestionsSetting => "Toggled on/off the code suggestions setting",
            Self::ToggleNaturalLanguageAutosuggestionsSetting => {
                "Toggled on/off the natural language autosuggestions setting"
            }
            Self::ToggleSharedBlockTitleGenerationSetting => {
                "Toggled on/off the shared block title generation setting"
            }
            Self::ToggleGitOperationsAutogenSetting => {
                "Toggled on/off the git operations autogen setting"
            }
            Self::ToggleVoiceInputSetting => "Toggled on/off the voice input setting",
            Self::PromptSuggestionShown => "Prompt Suggestions banner shown",
            Self::SuggestedCodeDiffFailed => "Suggested Code Diff Failed",
            Self::PromptSuggestionAccepted => "Prompt Suggestion accepted",
            Self::StaticPromptSuggestionsBannerShown => "Static Prompt Suggestions banner shown",
            Self::StaticPromptSuggestionAccepted => "Static Prompt Suggestion accepted",
            Self::ObjectLinkCopied => "The web link to an object has been copied.",
            Self::FileTreeToggled => "Opened the file tree/project explorer",
            Self::GlobalSearchOpened => "Opened the global search view",
            Self::GlobalSearchQueryStarted => "Started a global search (rift_ripgrep) search",
            Self::FileTreeItemAttachedAsContext => {
                "Attached a file or directory as context from the file tree"
            }
            Self::CodeSelectionAddedAsContext => {
                "Added selected code as context from the code editor"
            }
            Self::FileTreeItemCreated => "Created a new file from the file tree",
            Self::ConversationListViewOpened => {
                "Opened the conversation list view in the left panel"
            }
            Self::ConversationListItemOpened => "Opened a conversation from the conversation list",
            Self::ConversationListItemDeleted => {
                "Deleted a conversation from the conversation list"
            }
            Self::ConversationListLinkCopied => {
                "Copied a conversation link from the conversation list"
            }
            Self::InlineConversationMenuOpened => {
                "User opened the inline conversation menu in Agent View"
            }
            Self::InlineConversationMenuItemSelected => {
                "User selected an item from the inline conversation menu"
            }
            Self::AgentShortcutsViewToggled => "User toggled the shortcuts view in Agent View",
            Self::CreateProjectPromptSubmitted => {
                "User submitted a prompt from the create project view"
            }
            Self::CreateProjectPromptSubmittedContent => {
                "User submitted custom prompt content from the create project view"
            }
            Self::CloneRepoPromptSubmitted => {
                "User submitted a repository URL from the clone repo view"
            }
            Self::GetStartedSkipToTerminal => "User clicked skip to terminal from get started view",
            Self::CompletedSettingsImport => {
                "Imported a terminal's settings via the settings import onboarding block"
            }
            Self::SettingsImportConfigFocused => {
                "Selected a terminal in the settings import onboarding block"
            }
            Self::SettingsImportResetButtonClicked => {
                "Reset the imported settings in the settings import onboarding block"
            }
            Self::SettingsImportConfigParsed => {
                "Parsed a terminal's settings as part of settings import"
            }
            Self::ITermMultipleHotkeys => {
                "Attempted to import an iTerm profile that contained multiple hotkey window bindings"
            }
            Self::ToggleWorkspaceDecorationVisibility => "Toggled when to display the tab bar",
            Self::UpdateAltScreenPaddingMode => {
                "Updated the custom padding setting for the alt-screen"
            }
            Self::AddTabWithShell => "Added a tab with specific shell",
            Self::ToggleGlobalAI => "Toggled global AI enablement.",
            Self::ToggleActiveAI => "Toggled active AI enablement.",
            Self::ToggleLigatureRendering => "Toggled ligature rendering",
            Self::ToggledAgentModeAutoexecuteReadonlyCommandsSetting => {
                "Toggled setting to autoexecute readonly Agent Mode requested commands"
            }
            Self::ChangedAgentModeCodingPermissions => {
                "Changed Agent Mode permissions for coding tasks"
            }
            Self::AttachedImagesToAgentModeQuery => "Attached images to an Agent Mode query",
            #[cfg(windows)]
            Self::WSLRegistryError => {
                "Encountered an error while fetching WSL distributions from the registry"
            }
            #[cfg(windows)]
            Self::AutoupdateUnableToCloseApplications => {
                "The Windows auto-update installer was unable to automatically close all applications before installing the update"
            }
            #[cfg(windows)]
            Self::AutoupdateFileInUse => {
                "The Windows auto-update installer encountered a file-in-use error during installation"
            }
            #[cfg(windows)]
            Self::AutoupdateMutexTimeout => {
                "The Windows auto-update installer timed out waiting for Warp to release its mutex; a force-kill was attempted"
            }
            #[cfg(windows)]
            Self::AutoupdateForcekillFailed { .. } => {
                "The Windows auto-update installer failed to force-kill Warp after the mutex timeout"
            }
            #[cfg(windows)]
            Self::AutoupdateMinidumpCleanupFailed { .. } => {
                "The Windows auto-update installer failed to clean up the orphaned minidump server process"
            }
            Self::ToggleCodebaseContext => {
                "Toggled on/off the enablement of codebase context usage for Agent Mode."
            }
            Self::ToggleAutoIndexing => {
                "Toggled on/off the enablement of autoindexing for codebase context."
            }
            Self::ActiveIndexedReposChanged => {
                "Active indexed repositories changed, affecting codebase context."
            }
            Self::ImageReceived => "Received an image through an image protocol over the pty",
            Self::GrepToolSucceeded => "The grep tool completed successfully",
            Self::FileGlobToolSucceeded => "The file glob tool completed successfully",
            Self::ShellTerminatedPrematurely { .. } => "The shell process terminated prematurely",
            Self::InputUXModeChanged { .. } => "Changed the input UX mode",
            Self::ContextChipInteracted { .. } => "Interacted with a context chip",
            Self::UserMenuUpgradeClicked => "Clicked the 'Upgrade' menu item in the user menu",
            Self::TabCloseButtonPositionUpdated { .. } => "Updated the tab close button position",
            Self::AIExecutionProfileCreated => "A new AI execution profile was created",
            Self::AIExecutionProfileDeleted => "An AI execution profile was deleted",
            Self::AIExecutionProfileSettingUpdated { .. } => {
                "An AI execution profile setting was updated"
            }
            Self::AIExecutionProfileAddedToAllowlist { .. } => {
                "An item was added to an AI execution profile allowlist"
            }
            Self::AIExecutionProfileAddedToDenylist { .. } => {
                "An item was added to an AI execution profile denylist"
            }
            Self::AIExecutionProfileRemovedFromAllowlist { .. } => {
                "An item was removed from an AI execution profile allowlist"
            }
            Self::AIExecutionProfileRemovedFromDenylist { .. } => {
                "An item was removed from an AI execution profile denylist"
            }
            Self::AIExecutionProfileModelSelected { .. } => {
                "An AI model was selected for an AI execution profile"
            }
            Self::OpenSlashMenu { .. } => "Opened the slash commands menu",
            Self::SlashCommandAccepted { .. } => "User accepted a slash command",
            Self::AgentModeSetupBannerAccepted { .. } => "Agent Mode setup banner accepted",
            Self::AgentModeSetupBannerDismissed => "Agent Mode setup banner dismissed",
            Self::AgentModeSetupProjectScopedRulesAction { .. } => {
                "User clicked a button in the Agent Mode setup project scoped rules step"
            }
            Self::AgentModeSetupCodebaseContextAction { .. } => {
                "User clicked a button in the Agent Mode setup codebase context step"
            }
            Self::AgentModeSetupCreateEnvironmentAction { .. } => {
                "User clicked a button in the Agent Mode setup create environment step"
            }
            Self::RecentMenuItemSelected { .. } => {
                "User selected an item from the recents list on the new tab zero state"
            }
            Self::OpenRepoFolderSubmitted { .. } => {
                "User selected a folder to open as a repo from the \"Open repository\" button"
            }
            Self::OutOfCreditsBannerClosed => {
                "User closed the 'Out of credits' banner (dismissed or purchased credits)"
            }
            Self::AutoReloadModalClosed => {
                "User closed the auto-reload modal (either dismissed or enabled auto-reload)"
            }
            Self::AutoReloadToggledFromBillingSettings => {
                "User toggled auto-reload in Billing & Usage settings"
            }
            Self::AgentManagementViewToggled { .. } => {
                "User toggled the Agent Management View open or closed"
            }
            Self::AgentManagementViewOpenedSession => {
                "User opened a session from the Agent Management View"
            }
            Self::AgentManagementViewCopiedSessionLink => {
                "User copied a session link from the Agent Management View"
            }
            Self::DetectedIsolationPlatform { .. } => {
                "Detected that Warp is running in an isolated sandbox"
            }
            Self::AgentTipShown => "Selected an Agent Tip to show in the Agent Mode status bar",
            Self::AgentTipClicked => "User clicked a link or action in an Agent Tip",
            Self::CLIAgentToolbarVoiceInputUsed { .. } => {
                "User used voice input from the CLI agent footer"
            }
            Self::CLIAgentToolbarImageAttached { .. } => {
                "User attached an image from the CLI agent footer"
            }
            Self::CLIAgentToolbarShown { .. } => "CLI agent footer was shown to the user",
            Self::CLIAgentPluginChipClicked { .. } => {
                "User clicked the plugin install or update chip"
            }
            Self::CLIAgentPluginChipDismissed { .. } => {
                "User dismissed the plugin install or update chip"
            }
            Self::CLIAgentPluginOperationSucceeded { .. } => {
                "Auto plugin install or update completed successfully"
            }
            Self::CLIAgentPluginOperationFailed { .. } => {
                "Auto plugin install or update failed"
            }
            Self::CLIAgentPluginDetected { .. } => {
                "A CLI agent plugin was detected via a SessionStart event"
            }
            Self::CLIAgentRichInputOpened { .. } => "User opened CLI agent Rich Input",
            Self::CLIAgentRichInputClosed { .. } => "CLI agent Rich Input was closed",
            Self::CLIAgentRichInputSubmitted { .. } => {
                "User submitted a prompt via CLI agent Rich Input"
            }
            Self::ToggleCLIAgentToolbarSetting { .. } => {
                "User toggled the CLI agent footer setting"
            }
            Self::ToggleUseAgentToolbarSetting { .. } => {
                "User toggled the Use Agent footer setting"
            }
            Self::CodexModalOpened => "User opened the Codex modal",
            Self::CodexModalUseCodexClicked => "User clicked 'Use Codex' in the Codex modal",
            Self::LinearIssueLinkOpened => {
                "User opened a warp://linear deeplink to work on an issue"
            }
            Self::CloudAgentCapacityModalOpened => "User opened the cloud agent capacity modal",
            Self::CloudAgentCapacityModalDismissed => {
                "User dismissed the cloud agent capacity modal"
            }
            Self::CloudAgentCapacityModalUpgradeClicked => {
                "User clicked the upgrade button in the cloud agent capacity modal"
            }
            Self::FreeTierLimitHitInterstitialDisplayed { .. } => {
                "The free tier limit hit interstitial was displayed"
            }
            Self::FreeTierLimitHitInterstitialUpgradeButtonClicked { .. } => {
                "User clicked the 'Upgrade' button in the free tier limit hit interstitial"
            }
            Self::FreeTierLimitHitInterstitialClosed { .. } => {
                "User closed the free tier limit hit interstitial"
            }
            Self::RemoteServerBinaryCheck => {
                "Remote server binary check completed (found, not found, or error)"
            }
            Self::RemoteServerInstallation => {
                "Remote server binary installation completed (success or failure)"
            }
            Self::RemoteServerInitialization => {
                "Remote server connection and initialization completed (success or failure)"
            }
            Self::RemoteServerDaemonStartup => {
                "Remote server daemon startup completed and socket bound"
            }
            Self::RemoteServerDisconnection => {
                "An established remote server connection was dropped"
            }
            Self::RemoteServerClientRequestError => {
                "A client request to the remote server failed"
            }
            Self::RemoteServerMessageDecodingError => {
                "A server message could not be decoded (no parseable request_id)"
            }
            Self::RemoteServerSetupDuration => {
                "End-to-end duration of the remote server setup flow"
            }
            Self::RemoteServerHostUnsupported => {
                "Preinstall check classified the remote host as unsupported, \
                 falling back to the legacy SSH flow"
            }
            Self::RemoteServerReconnection => {
                "A reconnection attempt succeeded after a spontaneous disconnect"
            }
            Self::RemoteServerReconnectExhausted => {
                "All reconnection attempts were exhausted after a spontaneous disconnect"
            }
            Self::RemoteCodebaseIndexStatusChanged => "The remote codebase index status changed",
            Self::RemoteCodebaseAutoIndexRequested => {
                "Remote codebase auto-indexing requested one or more repositories"
            }
            Self::QueuedPromptPanelCollapseToggled => {
                "User toggled the queued prompts panel collapse state"
            }
        }
    }
}

rift_core::register_telemetry_event!(TelemetryEvent);

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
