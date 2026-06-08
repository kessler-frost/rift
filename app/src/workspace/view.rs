mod build_plan_migration_modal;
pub(crate) mod cloud_agent_capacity_modal;
pub(crate) mod codex_modal;
#[cfg(enable_crash_recovery)]
mod crash_recovery;
pub(crate) mod free_tier_limit_hit_modal;
pub mod global_search;
pub(crate) mod launch_modal;
pub(crate) mod left_panel;
pub(crate) mod onboarding;
pub(crate) mod openwarp_launch_modal;
pub(crate) mod orchestration_launch_modal;
mod startup_directory;
#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
mod vertical_tabs;
#[cfg(target_family = "wasm")]
mod wasm_view;

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
#[cfg(feature = "local_fs")]
use std::convert::TryFrom;
#[cfg(target_os = "macos")]
use std::env;
use std::fmt::Write;
#[cfg(all(target_os = "macos", feature = "crash_reporting"))]
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::{SystemTime, UNIX_EPOCH};

use ::settings::{Setting, ToggleableSetting};
use ai::index::full_source_code_embedding::manager::CodebaseIndexManager;
use autoupdate::AutoupdateStage;
#[cfg(target_os = "macos")]
use command::blocking::Command;
use futures::Future;
use itertools::Itertools;
use lazy_static::lazy_static;
pub(crate) use onboarding::OnboardingTutorial;
use parking_lot::FairMutex;
use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
#[cfg(feature = "local_fs")]
use repo_metadata::repositories::DetectedRepositories;
#[cfg(feature = "local_fs")]
use repo_metadata::RemoteRepositoryIdentifier;
use rift_cli::agent::Harness;
use rift_core::context_flag::ContextFlag;
use rift_core::execution_mode::AppExecutionMode;
use rift_core::features::FeatureFlag;
use rift_core::semantic_selection::SemanticSelection;
use rift_core::ui::color::coloru_with_opacity;
use rift_core::ui::theme::color::internal_colors;
use rift_core::ui::theme::phenomenon::PhenomenonStyle;
use rift_core::ui::theme::Fill;
use rift_core::ui::Icon;
use rift_core::user_preferences::GetUserPreferences as _;
use rift_editor::editor::NavigationKey;
use rift_server_client::auth::AuthEvent;
use rift_util::path::{user_friendly_path, LineAndColumnArg};
#[cfg(all(feature = "local_fs", not(target_family = "wasm")))]
use rift_util::standardized_path::StandardizedPath;
use riftui::accessibility::{
    AccessibilityContent, AccessibilityVerbosity, ActionAccessibilityContent, WarpA11yRole,
};
use riftui::clipboard::ClipboardContent;
#[cfg(target_family = "wasm")]
use riftui::elements::Percentage;
use riftui::elements::{
    Align, Border, CacheOption, ChildAnchor, ChildView, Clipped, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, Dismiss, DispatchEventResult, DraggableState, DropTarget,
    Element, Empty, EventHandler, Expanded, Fill as ElementFill, Flex, Highlight, Hoverable,
    Icon as WarpUiIcon, Image, MainAxisAlignment, MainAxisSize, MouseInBehavior, MouseStateHandle,
    OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, PositionedElementAnchor,
    PositionedElementOffsetBounds, Radius, Rect, SavePosition, Shrinkable, Stack, Text,
};
use riftui::fonts::{Properties, Weight};
use riftui::geometry::vector::{vec2f, Vector2F};
use riftui::keymap::Context;
use riftui::modals::{AlertDialogWithCallbacks, AppModalCallback};
use riftui::notification::{NotificationSendError, RequestPermissionsOutcome, UserNotification};
use riftui::platform::{
    Cursor, FilePickerConfiguration, FullscreenState, SystemTheme, TerminationMode,
};
use riftui::text_layout::ClipConfig;
use riftui::ui_components::button::{Button, ButtonVariant};
use riftui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use riftui::windowing::state::ApplicationStage;
use riftui::windowing::{StateEvent, WindowManager};
use riftui::{
    AppContext, Entity, EntityId, FocusContext, ModelHandle, SingletonEntity, TypedActionView,
    UpdateModel, UpdateView, View, ViewAsRef, ViewContext, ViewHandle, WeakViewHandle, WindowId,
};
#[cfg(all(target_os = "macos", feature = "crash_reporting"))]
use sentry::protocol::{Attachment, AttachmentType};
use serde_json;
use session_sharing_protocol::common::SessionId as SharedSessionId;
#[cfg(target_family = "wasm")]
use url::Url;

use self::vertical_tabs::telemetry::{VerticalTabsDisplayOption, VerticalTabsTelemetryEvent};
use self::vertical_tabs::{
    render_detail_sidecar, render_settings_popup, vtab_group_position_id, VerticalTabsPanelState,
    VERTICAL_TABS_SETTINGS_BUTTON_POSITION_ID,
};
#[cfg(all(feature = "local_fs", not(target_family = "wasm")))]
use super::action::AutoCloudHandoffTrigger;
use super::action::{
    InitContent, NewSessionMenuAnchor, RestoreConversationLayout, TabContextMenuAnchor,
    VerticalTabsPaneContextMenuTarget, WorkspaceAction,
};
use super::close_session_confirmation_dialog::{
    CloseSessionConfirmationDialog, CloseSessionConfirmationEvent, OpenDialogSource,
};
use super::hoa_onboarding::{
    mark_hoa_onboarding_completed, HoaOnboardingFlow, HoaOnboardingFlowEvent, HoaOnboardingStep,
};
use super::lightbox_view::{LightboxParams, LightboxView, LightboxViewEvent};
use super::native_modal::{NativeModal, NativeModalEvent};
use super::one_time_modal_model::OneTimeModalEvent;
use super::tab_settings::{
    HeaderToolbarChipSelection, NewTabPlacement, TabSettings, TabSettingsChangedEvent,
    VerticalTabsDisplayGranularity, WorkspaceDecorationVisibility,
};
use super::util::{
    PaneViewLocator, TabMovement, TerminalSessionFallbackBehavior, WelcomeTipsViewState,
    WorkspaceMouseStates, WorkspaceState,
};
use super::{util, ActiveSession, TabBarDropTargetData, TabBarLocation, WorkspaceRegistry};
use crate::app_state::{
    LeafContents, LeafSnapshot, LeftPanelDisplayedTab, LeftPanelSnapshot,
    PaneNodeSnapshot, PaneUuid, RightPanelSnapshot, SettingsPaneSnapshot, TabSnapshot,
    TerminalPaneSnapshot, WindowSnapshot,
};
use crate::appearance::{Appearance, AppearanceManager};
use crate::auth::auth_manager::{AuthManager, AuthManagerEvent};
use crate::auth::auth_override_warning_modal::{
    AuthOverrideWarningModal, AuthOverrideWarningModalEvent, AuthOverrideWarningModalVariant,
};
use crate::auth::auth_state::AuthState;
use crate::auth::auth_view_modal::{AuthRedirectPayload, AuthView, AuthViewEvent, AuthViewVariant};
use crate::auth::AuthStateProvider;
use crate::autoupdate::{
    is_incoming_version_past_current, AutoupdateState, AutoupdateStateEvent, RelaunchModel,
};
use crate::banner::BannerState;
use crate::changelog_model::{ChangelogModel, ChangelogRequestType, Event as ChangelogEvent};
use crate::channel::{Channel, ChannelState};
use rift_util::local_or_remote_path::LocalOrRemotePath;
use crate::coding_panel_enablement_state::CodingPanelEnablementState;
use crate::context_chips::ChipRuntimeCapabilities;
use crate::default_terminal::DefaultTerminal;
use crate::editor::{
    EditorView, Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions,
    TextOptions,
};
use crate::experiments::{BlockOnboarding, Experiment};
use crate::launch_configs::launch_config::WindowTemplate;
use crate::launch_configs::save_modal::{LaunchConfigModalEvent, LaunchConfigSaveModal};
use crate::menu::{Event as MenuEvent, Menu, MenuItem, MenuItemFields, MenuSelectionSource};
use crate::modal::{Modal, ModalEvent, ModalViewState};
use crate::network::{NetworkStatus, NetworkStatusEvent};
use crate::notification::NotificationContext;
use crate::palette::PaletteMode;
use crate::pane_group::pane::ActionOrigin;
use crate::pane_group::{
    self, AnyPaneContent,
    Direction as PaneGroupDirection, Direction,
    NetworkLogPane, NewTerminalOptions, PaneGroup, PaneId, PanesLayout,
    TabBarHoverIndex, TerminalPaneId,
};
use crate::persistence::ModelEvent;
use crate::projects::ProjectManagementModel;
use crate::prompt::editor_modal::{
    EditorModal as PromptEditorModal, EditorModalEvent as PromptEditorModalEvent,
    OpenSource as PromptEditorOpenSource,
};
use crate::quit_warning::UnsavedStateSummary;
use crate::remote_server::manager::RemoteServerManager;
use crate::resource_center::{
    mark_feature_used_and_write_to_user_defaults, skip_tips_and_write_to_user_defaults,
    ResourceCenterEvent, ResourceCenterPage, ResourceCenterView, Tip, TipAction, TipsCompleted,
};
use crate::root_view::{quake_mode_window_id, NewWorkspaceSource, OpenLaunchConfigArg};
use crate::search::command_palette::view::{
    Event as CommandPaletteEvent, NavigationMode, View as CommandPalette,
};
use crate::search::command_search::searcher::{
    AcceptedHistoryItem, CommandSearchItemAction,
};
use crate::search::command_search::settings::CommandSearchSettings;
use crate::search::slash_command_menu::static_commands::commands;
use crate::search::{self, QueryFilter};
use crate::server::ids::{ObjectUid, ServerId, SyncId};
use crate::server::network_log_pane_manager::NetworkLogPaneManager;
use crate::server::server_api::{ServerApi, ServerApiProvider, ServerTime};
use crate::server::telemetry::{
    AddTabWithShellSource, AnonymousUserSignupEntrypoint, CloseTarget,
    FileTreeSource, KnowledgePaneEntrypoint, LaunchConfigUiLocation,
    MCPServerCollectionPaneEntrypoint, NotificationsTurnedOnSource, OpenedWarpAISource,
    PaletteSource, SharingDialogSource, TabRenameEvent, WarpDriveSource,
};
use crate::session_management::{SessionNavigationData, SessionSource, TabNavigationData};
use crate::settings::{
    active_theme_kind, respect_system_theme, AISettings, AISettingsChangedEvent,
    AccessibilitySettings, AliasExpansionSettings, AppEditorSettings, BlockVisibilitySettings,
    ChangelogSettings, CodeSettings, CodeSettingsChangedEvent, CtrlTabBehavior, CursorBlink,
    DebugSettings, DefaultSessionMode, FontSettings, GPUSettings, InputModeSettings, InputSettings,
    MonospaceFontSize, PaneSettings, PrivacySettings, SelectionSettings, Settings, SshSettings,
    ThemeSettings,
};
use crate::settings_view::keybindings::{KeybindingChangedEvent, KeybindingChangedNotifier};
use crate::settings_view::pane_manager::SettingsPaneManager;
use crate::settings_view::{flags, SettingsSection, SettingsView, SettingsViewEvent};
#[cfg(all(target_os = "windows", feature = "local_tty"))]
use crate::shell_indicator::ShellIndicatorType;
use crate::tab::{
    tab_position_id, uses_vertical_tabs, NewSessionMenuItem, PaneNameMenuTarget, SelectedTabColor,
    TabBarState, TabComponent, TabData, TabTelemetryAction, MOVE_TO_GROUP_LABEL,
    TAB_BAR_BORDER_HEIGHT,
};
use crate::tab_configs::action_sidecar::SidecarItemKind;
use crate::tab_configs::remove_confirmation_dialog::{
    RemoveTabConfigConfirmationDialog, RemoveTabConfigConfirmationEvent,
};
use crate::tab_configs::session_config_modal::{SessionConfigModal, SessionConfigModalEvent};
use crate::tab_configs::telemetry::{
    ExistingTabConfigOpenMode, GuidedModalSessionType, TabConfigsTelemetryEvent,
};
#[cfg(feature = "local_fs")]
use crate::tab_configs::telemetry::{NewWorktreeConfigOpenSource, WorktreeBranchNamingMode};
use crate::tab_configs::{
    NewWorktreeModal, NewWorktreeModalEvent, TabConfigParamsModal, TabConfigParamsModalEvent,
};
use crate::terminal::alt_screen_reporting::AltScreenReporting;
use crate::terminal::available_shells::AvailableShell;
#[cfg(target_os = "windows")]
use crate::terminal::available_shells::AvailableShells;
use crate::terminal::block_list_viewport::InputMode;
#[cfg(not(target_family = "wasm"))]
use crate::terminal::cli_agent_sessions::plugin_manager::{plugin_manager_for, PluginModalKind};
use crate::terminal::cli_agent_sessions::{CLIAgentSessionsModel, CLIAgentSessionsModelEvent};
use crate::terminal::general_settings::GeneralSettings;
use crate::terminal::input::{Input, MenuPositioning};
use crate::terminal::keys_settings::KeysSettings;
use crate::terminal::ligature_settings::should_use_ligature_rendering;
#[cfg(feature = "local_tty")]
use crate::terminal::local_tty::docker_sandbox::resolve_sbx_path_from_user_shell;
use crate::terminal::model::blockgrid::BlockGrid;
#[cfg(feature = "local_fs")]
use crate::terminal::model::session::Session;
use crate::terminal::model::session::SessionId;
use crate::terminal::model::terminal_model::ConversationTranscriptViewerStatus;
use crate::terminal::resizable_data::{
    ModalSizes, ModalType, ResizableData, DEFAULT_LEFT_PANEL_WIDTH, DEFAULT_RIGHT_PANEL_WIDTH,
};
use crate::terminal::safe_mode_settings::SafeModeSettings;
use crate::terminal::session_settings::{
    NewSessionSource, NotificationsMode, NotificationsSettings, SessionSettings,
    SessionSettingsChangedEvent, WorkingDirectoryMode,
};
use crate::terminal::settings::{SpacingMode, TerminalSettings};
use crate::terminal::shell::ShellType;
use crate::terminal::view::ssh_file_upload::FileUploadId;
use crate::terminal::view::{
    AgentOnboardingVersion, LeftPanelTargetView,
    OnboardingIntention, OnboardingVersion, SyncEvent, SyncInputType, TerminalAction,
    NOTIFICATIONS_TROUBLESHOOT_URL,
};
use crate::terminal::warpify::settings::WarpifySettings;
use crate::terminal::{self, BlockListSettings, SizeInfo, TerminalModel, TerminalView};
use crate::terminal::model::block::SerializedBlockListItem;
use crate::themes::theme::{AnsiColorIdentifier, RespectSystemTheme, ThemeKind};
use crate::themes::theme_chooser::{ThemeChooser, ThemeChooserEvent, ThemeChooserMode};
use crate::themes::theme_creator_modal::{ThemeCreatorModal, ThemeCreatorModalEvent};
use crate::themes::theme_deletion_modal::{ThemeDeletionModal, ThemeDeletionModalEvent};
use crate::tips::{TipsEvent, TipsView};
use crate::ui_components::avatar::{Avatar, AvatarContent, StatusElementTypes};
use crate::ui_components::buttons::{combo_inner_button, icon_button_with_color};
use crate::ui_components::red_notification_dot::RedNotificationDot;
use crate::ui_components::window_focus_dimming::WindowFocusDimming;
use crate::ui_components::{blended_colors, icons};
use crate::undo_close::UndoCloseStack;
#[cfg(target_family = "wasm")]
use crate::uri::browser_url_handler::{parse_current_url, update_browser_url};
#[cfg(feature = "local_fs")]
use crate::user_config::{
    ensure_default_worktree_config, find_unused_tab_config_path, find_unused_toml_path,
    find_unused_worktree_config_path, materialize_default_worktree_config, sanitize_toml_base_name,
    tab_configs_dir,
};
use crate::user_config::{WarpConfig, WarpConfigUpdateEvent};
use crate::util::bindings::{
    keybinding_name_to_display_string, keybinding_name_to_keystroke, trigger_to_keystroke,
};
#[cfg(feature = "local_fs")]
use crate::util::file::external_editor::settings::OpenConversationPreference;
#[cfg(feature = "local_fs")]
use crate::util::file::external_editor::Editor;
#[cfg(feature = "local_fs")]
use crate::util::file::external_editor::EditorSettings;
use crate::util::links;
use crate::util::openable_file_type::FileTarget;
#[cfg(feature = "local_fs")]
use crate::util::openable_file_type::{resolve_file_target_with_editor_choice, EditorLayout};
use crate::util::traffic_lights::{traffic_light_data, TrafficLightMouseStates, TrafficLightSide};
use crate::util::truncation::truncate_from_end;
#[cfg(target_family = "wasm")]
use crate::view_components::action_button::ActionButton;
use crate::view_components::callout_bubble::{
    render_callout_bubble, CalloutArrowDirection, CalloutArrowPosition, CalloutBubbleConfig,
};
use crate::view_components::{
    AgentToast, AgentToastStack, DismissibleToast, DismissibleToastStack, ToastLink,
};
#[cfg(target_family = "wasm")]
use crate::wasm_nux_dialog::WasmNUXDialog;
use crate::window_settings::{WindowSettings, WindowSettingsChangedEvent, ZoomLevel};
use crate::workspace::action::CommandSearchOptions;
#[cfg(target_os = "macos")]
use crate::workspace::cli_install;
use crate::workspace::cross_window_tab_drag::{
    AttachTarget, CrossWindowTabDrag, DragResult, DropResult, GhostState,
};
use crate::workspace::header_toolbar_editor::{HeaderToolbarEditorEvent, HeaderToolbarEditorModal};
use crate::workspace::header_toolbar_item::HeaderToolbarItemKind;
use crate::workspace::one_time_modal_model::OneTimeModalModel;
use crate::workspace::sync_inputs::SyncedInputState;
use crate::workspace::tab_group::{TabGroup, TabGroupId};
use crate::workspace::tab_settings::TabCloseButtonPosition;
use crate::workspace::toast_stack::{
    ToastStack, ToastStack as WorkspaceToastStack, ToastStackEvent as WorkspaceToastStackEvent,
};
use crate::workspace::view::build_plan_migration_modal::{
    BuildPlanMigrationModal, BuildPlanMigrationModalEvent,
};
use crate::workspace::view::cloud_agent_capacity_modal::{
    CloudAgentCapacityModal, CloudAgentCapacityModalEvent, CloudAgentCapacityModalVariant,
};
use crate::workspace::view::codex_modal::{CodexModal, CodexModalEvent};
use crate::workspace::view::free_tier_limit_hit_modal::{
    FreeTierLimitHitModal, FreeTierLimitHitModalEvent,
};
use crate::workspace::view::global_search::view::GlobalSearchEntryFocus;
use crate::workspace::view::launch_modal::{LaunchModal, LaunchModalEvent, OzLaunchSlide};
use crate::workspace::view::left_panel::{
    LeftPanelAction, LeftPanelEvent, LeftPanelView, ToolPanelView,
};
use crate::workspace::view::openwarp_launch_modal::{
    OpenWarpLaunchModal, OpenWarpLaunchModalEvent,
};
use crate::workspace::view::orchestration_launch_modal::{
    OrchestrationLaunchModal, OrchestrationLaunchModalEvent,
};
use crate::workspace::ForkedConversationDestination;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::AdminEnablementSetting;
use crate::{
    autoupdate, report_if_error, send_telemetry_from_ctx, settings,
    GlobalResourceHandles, TelemetryEvent,
};

/// The padding that should be applied to the workspace as a whole.
pub const WORKSPACE_PADDING: f32 = 1.0;

/// The minimum font size at which terminal text will be rendered.
const MIN_FONT_SIZE: f32 = 5.0;

/// The maximum font size at which terminal text will be rendered.
const MAX_FONT_SIZE: f32 = 25.0;

/// The increment for increasing/decreasing the font size.
const FONT_SIZE_INCREMENT: f32 = 1.0;

pub const TAB_BAR_HEIGHT: f32 = 34.;
/// Height for all panel headers (tab bar, warp drive, resource center, theme chooser, etc.).
/// This ensures consistent header heights across all UI panels.
pub const PANEL_HEADER_HEIGHT: f32 = TAB_BAR_HEIGHT;
/// The hover area height for states where the tab bar is revealed on hover.
const TAB_BAR_HOVER_HEIGHT: f32 = 12.;
const TAB_BAR_PADDING_LEFT: f32 = 4.;
const TAB_BAR_PADDING_RIGHT: f32 = 8.;
const TITLE_BAR_SEARCH_BAR_MAX_WIDTH: f32 = 320.;
const TITLE_BAR_SEARCH_BAR_SLOT_PADDING: f32 = 8.;

// The total height taken up by the tab bar, including its bottom border.
pub const TOTAL_TAB_BAR_HEIGHT: f32 = TAB_BAR_HEIGHT + TAB_BAR_BORDER_HEIGHT;

const TAB_BAR_ICON_PADDING: f32 = 4.;

const TAB_BAR_PILL_WIDTH: f32 = 100.;
const PILL_FONT_SIZE: f32 = 12.;
// We use the word "Warp" in the Update Ready button to make it obvious that the terminal is Warp.
// This can lead to free advertising when users screen-share Warp when an update is available.
const UPDATE_READY_TEXT: &str = "Update Warp";

const TAB_BAR_OVERFLOW_MENU_WIDTH: f32 = 300.;

#[cfg(not(target_family = "wasm"))]
const RESOURCE_CENTER_WIDTH: f32 = 361.;

// Ratio of terminal : theme chooser when theme chooser is active
const THEME_CHOOSER_RATIO: f32 = 3.5;

/// Save position for the tab bar.
pub(crate) const TAB_BAR_POSITION_ID: &str = "workspace_view:tab_bar";

/// Save position for the vertical tabs panel.
/// HOA onboarding callouts anchor relative to this position, so whichever code
/// path renders the vertical tabs panel must wrap it in a `SavePosition` with
/// this id.
pub(crate) const VERTICAL_TABS_PANEL_POSITION_ID: &str = "workspace_view:vertical_tabs_panel";

/// The main content area in a workspace. This is directly below the tab bar.
const TAB_CONTENT_POSITION_ID: &str = "workspace_view:tab_content";

const WELCOME_TIPS_POSITION_ID: &str = "welcome_tips_pill";
const ELLIPSE_SVG_PATH: &str = "bundled/svg/ellipse.svg";

const AI_ASSISTANT_BUTTON_ID: &str = "workspace_view:ai_assistant_button";

const VERSION_DEPRECATION_BANNER_TEXT: &str = "Your app is out of date and some features may not work as expected. Please update immediately.";

const VERSION_DEPRECATION_WITHOUT_PERMISSIONS_BANNER_TEXT: &str = "Some Warp features may not work as expected without updating immediately, but Warp is unable to perform the update.";

const ASK_AI_ASSISTANT_KEYBINDING_NAME: &str = "workspace:toggle_ai_assistant";
const TOGGLE_RESOURCE_CENTER_KEYBINDING_NAME: &str = "workspace:toggle_resource_center";

/// Shared position ID for the new-session sidecar overlay. Used for both the
/// `SavePosition` wrapper and the safe-zone rect lookup.
const NEW_SESSION_SIDECAR_POSITION_ID: &str = "new_session_sidecar";
const NEW_SESSION_SIDECAR_WIDTH: f32 = 300.;

/// Shared position ID for the move-to-group sidecar overlay, used by both the
/// `SavePosition` wrapper and the safe-zone rect lookup on the parent
/// right-click menu.
const MOVE_TO_GROUP_SIDECAR_POSITION_ID: &str = "move_to_group_sidecar";
const MOVE_TO_GROUP_SIDECAR_WIDTH: f32 = 200.;
const NEW_SESSION_SIDECAR_SEARCH_BOX_HEIGHT: f32 = 32.;
const NEW_SESSION_SIDECAR_SEARCH_BOX_HORIZONTAL_PADDING: f32 = 12.;
const NEW_SESSION_SIDECAR_SEARCH_BOX_VERTICAL_PADDING: f32 = 6.;
const NEW_SESSION_SIDECAR_FOOTER_HORIZONTAL_PADDING: f32 = 16.;
const NEW_SESSION_SIDECAR_FOOTER_VERTICAL_PADDING: f32 = 8.;
const SESSION_CONFIG_TAB_CONFIG_CHIP_TEXT: &str = "Access your tab configs here.";
const SESSION_CONFIG_TAB_CONFIG_CHIP_WIDTH: f32 = 206.;
const SHOW_SETTINGS_KEYBINDING_NAME: &str = "workspace:show_settings";
pub const TOGGLE_COMMAND_PALETTE_KEYBINDING_NAME: &str = "workspace:toggle_command_palette";

const USER_AVATAR_BUTTON_POSITION_ID: &str = "workspace:user_avatar_button";
const NOTIFICATIONS_MAILBOX_POSITION_ID: &str = "workspace:notifications_mailbox";
pub(crate) const JUMP_TO_LATEST_TOAST_BINDING_NAME: &str = "workspace:jump_to_latest_toast";
pub(crate) const TOGGLE_NOTIFICATION_MAILBOX_BINDING_NAME: &str =
    "workspace:toggle_notification_mailbox";

// these won't have to be public after we deprecate the code mode v1 project explorer which is defined in terminal
pub(crate) const TOGGLE_PROJECT_EXPLORER_BINDING_NAME: &str = "workspace:toggle_project_explorer";
pub(crate) const TOGGLE_WARP_DRIVE_BINDING_NAME: &str = "workspace:toggle_warp_drive";
pub(crate) const TOGGLE_RIGHT_PANEL_BINDING_NAME: &str = "workspace:toggle_right_panel";
pub(crate) const TOGGLE_VERTICAL_TABS_PANEL_BINDING_NAME: &str =
    "workspace:toggle_vertical_tabs_panel";
pub(crate) const OPEN_GLOBAL_SEARCH_BINDING_NAME: &str = "workspace:open_global_search";
pub(crate) const TOGGLE_CONVERSATION_LIST_VIEW_BINDING_NAME: &str =
    "workspace:toggle_conversation_list_view";
pub(crate) const NEW_TAB_BINDING_NAME: &str = "workspace:new_tab";
pub(crate) const NEW_TERMINAL_TAB_BINDING_NAME: &str = "workspace:new_terminal_tab";
pub(crate) const NEW_AGENT_TAB_BINDING_NAME: &str = "workspace:new_agent_tab";
pub(crate) const NEW_AMBIENT_AGENT_TAB_BINDING_NAME: &str = "workspace:new_ambient_agent_tab";
pub(crate) const TOGGLE_TAB_CONFIGS_MENU_BINDING_NAME: &str = "workspace:toggle_tab_configs_menu";

// Editable left panel toolbelt keybindings.
pub(crate) const LEFT_PANEL_PROJECT_EXPLORER_BINDING_NAME: &str =
    "workspace:left_panel_project_explorer";
pub(crate) const LEFT_PANEL_GLOBAL_SEARCH_BINDING_NAME: &str = "workspace:left_panel_global_search";
pub(crate) const LEFT_PANEL_WARP_DRIVE_BINDING_NAME: &str = "workspace:left_panel_warp_drive";
pub(crate) const LEFT_PANEL_AGENT_CONVERSATIONS_BINDING_NAME: &str =
    "workspace:left_panel_agent_conversations";

const KEYBINDINGS_TO_CACHE: [&str; 4] = [
    ASK_AI_ASSISTANT_KEYBINDING_NAME,
    TOGGLE_RESOURCE_CENTER_KEYBINDING_NAME,
    SHOW_SETTINGS_KEYBINDING_NAME,
    TOGGLE_COMMAND_PALETTE_KEYBINDING_NAME,
];

const WORKFLOW_AND_ENV_VAR_SPLIT_RATIO: f32 = 0.56;
const NOTEBOOK_SMART_SPLIT_RATIO: f32 = 0.42;

#[cfg(target_family = "wasm")]
const MOBILE_OVERLAY_PANEL_WIDTH_RATIO: f32 = 0.9;
#[cfg(target_family = "wasm")]
const MOBILE_OVERLAY_SCRIM_ALPHA: u8 = 128;

pub const NEW_TAB_BUTTON_POSITION_ID: &str = "new_tab_button";
pub const NEW_SESSION_MENU_BUTTON_POSITION_ID: &str = "new_session_menu_button";

// The max length of the title of a fork toast (after which we truncate it).
const MAX_FORK_TOAST_TITLE_LENGTH: usize = 100;

// The max length of the window title (matching conversation title truncation).
const MAX_WINDOW_TITLE_LENGTH: usize = 80;

#[cfg(all(feature = "local_fs", not(target_family = "wasm")))]
const AUTO_CLOUD_HANDOFF_PROMPT: &str =
    "Continue this local Warp Agent task in the cloud from the current conversation state.";

/// The default display name used for the user if they have no associated display name.
pub const DEFAULT_USER_DISPLAY_NAME: &str = "User";

lazy_static! {
    static ref OPENING_WARP_DRIVE_ON_START_UP: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref PANEL_CORNER_RADIUS: CornerRadius = CornerRadius::with_all(Radius::Pixels(8.));
    static ref PANEL_HEADER_CORNER_RADIUS: CornerRadius =
        CornerRadius::with_top(Radius::Pixels(8.));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabConfigsMenuOpenSource {
    KeyboardShortcut,
    Pointer,
}

/// This enumerates the different kinds of banners we show to the user.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceBanner {
    /// to display the banner when we are in AutoupdateStage::UpdateReady and the
    /// user's current version has been deprecated
    VersionDeprecated,
    /// to display the AutoupdateStage::UnableToUpdateToNewVersion
    UnableToUpdateToNewVersion,
    /// to display the AutoupdateStage::UnableToLaunchNewVersion
    UnableToLaunchNewVersion,
    /// to display when the user needs to reauthenticate
    Reauth,
    // to display an anonymous user has X days left to sign in
    AnonymousUserAuth,
    /// to display when recovering from a crash that may have been due to use
    /// of Wayland
    #[cfg(target_os = "linux")]
    WaylandCrashRecovery,
    /// to display when settings.toml has errors (parse failure or invalid values)
    InvalidSettings,
}

impl WorkspaceBanner {
    /// We want some banners to have a close button and not others, e.g. if they are running a very
    /// outdated version and we want to nag them to update, AutoupdateBanner::VersionDeprecated should
    /// not be dismissible
    fn is_dismissible(&self) -> bool {
        match self {
            Self::UnableToUpdateToNewVersion => true,
            Self::UnableToLaunchNewVersion => true,
            Self::VersionDeprecated => false,
            Self::AnonymousUserAuth => false,
            Self::Reauth => true,
            #[cfg(target_os = "linux")]
            Self::WaylandCrashRecovery => true,
            Self::InvalidSettings => true,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum SessionCycleDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanePanelDirection {
    Prev,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusRegion {
    LeftPanel,
    PaneGroup,
    RightPanel,
    Other,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PanelPosition {
    Left,
    Right,
}

pub struct TabPaneGroupIdentifiers {
    pub tab_idx: usize,
    pub pane_group_id: EntityId,
    pub terminal_ids: Vec<EntityId>,
}

/// Categorization of how the tab bar should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ShowTabBar {
    /// Show the tab bar stacked on top of the pane group area.
    #[default]
    Stacked,
    /// Hide the tab bar.
    Hidden,
}

impl ShowTabBar {
    fn has_tab_bar(self) -> bool {
        matches!(self, ShowTabBar::Stacked)
    }
}

/// The type of content being displayed when the simplified WASM tab bar is shown.
/// Used to determine which elements to render (e.g., icon, info button).
#[cfg(target_family = "wasm")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimplifiedWasmTabBarContent {
    /// Viewing a Warp Drive object (notebook, workflow, env vars, AI facts, MCP servers)
    WarpDriveObject,
    /// Participating in a shared session (viewer or writer).
    SharedSession,
    /// Viewing a conversation transcript.
    ConversationTranscript,
}

type RemoteUploadId = (TerminalPaneId, FileUploadId);
type WorkspaceMenuHandles = (
    ViewHandle<Menu<WorkspaceAction>>,
    ViewHandle<Menu<WorkspaceAction>>,
    ViewHandle<Menu<NewSessionSidecarSelection>>,
    ViewHandle<Menu<WorkspaceAction>>,
);

#[derive(Clone, Debug, PartialEq, Eq)]
enum NewSessionSidecarSelection {
    OpenWorktreeRepo { repo_path: String },
}

#[derive(Debug, Default)]
struct FileUploadSessions {
    /// Maps a local session pane handling a file upload
    /// to the remote session pane through which the upload was initiated.
    local_to_remote_map: HashMap<TerminalPaneId, TerminalPaneId>,
    /// Maps a local pane to the ID of the file upload it is responsible for.
    local_to_upload_id_map: HashMap<TerminalPaneId, RemoteUploadId>,
    upload_id_to_local_map: HashMap<RemoteUploadId, TerminalPaneId>,
}

/// Controls the color palette used for a workspace banner.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BannerSeverity {
    /// Warning banners use an ansi-blended yellow background.
    Warning,
    /// Error banners use an ansi-blended red background.
    Error,
}

/// Visual style for an individual banner action button.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BannerButtonVariant {
    /// No fill, no border, just text (and optional icon). Used for the primary
    /// action in the Figma design (e.g. "Fix with Oz").
    Naked,
    /// Border-only, no fill (e.g. "Open file").
    Outlined,
}

struct WorkspaceBannerButtonDetails {
    text: String,
    action: WorkspaceAction,
    variant: BannerButtonVariant,
    /// Optional leading icon shown before the label.
    icon: Option<Icon>,
    /// If set, renders an adjacent "More info" pill that dispatches this action.
    more_info_button_action: Option<WorkspaceAction>,
}

struct WorkspaceBannerFields {
    banner_type: WorkspaceBanner,
    severity: BannerSeverity,
    /// Optional bold heading rendered inline before the description.
    heading: Option<String>,
    /// Main description text (regular weight).
    description: String,
    secondary_button: Option<WorkspaceBannerButtonDetails>,
    button: Option<WorkspaceBannerButtonDetails>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefaultSessionModeBehavior {
    /// Respect the user's default-session-mode setting and auto-enter agent view when applicable.
    Apply,
    /// Skip default-session-mode auto-entry because the caller is explicitly specifying the mode for the new session.
    Ignore,
}


/// Groups a modal view handle with the ID of the tab that was created to host
/// it, so the custom tab title can be cleared on close regardless of which tab
/// is active at that point.
struct ModalWithTab<V> {
    view: ViewHandle<V>,
    /// Set when the modal opens a new tab; consumed (taken) when the modal
    /// closes so we can clear the custom tab title.
    tab_pane_group_id: Option<EntityId>,
}
/// Context saved when the session config modal triggers `open_tab_config` and
/// the tab config has params (worktree). The params modal opens asynchronously,
/// so we store what we need to finish the tab replacement when it completes.
struct PendingSessionConfigReplacement {
    old_pane_group_id: EntityId,
}
enum PendingSessionConfigTabConfigChipTutorial {
    WhenBootstrapped {
        has_project: bool,
        intention: OnboardingIntention,
    },
    AfterSetupCommands {
        intention: OnboardingIntention,
    },
}

/// Snapshot of a tab used to move it between workspaces or into a new window.
/// Built by `Workspace::tab_transfer_info_at_index` and consumed by
/// `insert_transferred_tab_at_index`. Captures the pane group handle, visual
/// metadata, panel-open state, and `DraggableState` so an in-progress drag
/// animation continues seamlessly after a handoff.
pub struct TransferredTab {
    pub pane_group: ViewHandle<PaneGroup>,
    pub color: Option<AnsiColorIdentifier>,
    pub custom_title: Option<String>,
    pub left_panel_open: bool,
    pub vertical_tabs_panel_open: bool,
    pub right_panel_open: bool,
    pub is_right_panel_maximized: bool,
    pub draggable_state: DraggableState,
}

pub struct Workspace {
    window_id: WindowId,
    pub(crate) tabs: Vec<TabData>,
    active_tab_index: usize,
    /// Tracks tab activation order (most-recently-used first).
    /// Each entry is the `pane_group.id()` of the corresponding tab.
    tab_mru_order: Vec<EntityId>,
    pub(crate) hovered_tab_index: Option<TabBarHoverIndex>,
    tab_bar_hover_state: MouseStateHandle,
    tab_fixed_width: Option<f32>,
    traffic_light_mouse_states: TrafficLightMouseStates,
    /// Tab groups in this workspace, keyed by id.
    pub(crate) tab_groups: HashMap<TabGroupId, TabGroup>,
    tab_rename_editor: ViewHandle<EditorView>,
    pane_rename_editor: ViewHandle<EditorView>,
    tab_group_rename_editor: ViewHandle<EditorView>,
    vertical_tabs_search_input: ViewHandle<EditorView>,
    tips_completed: ModelHandle<TipsCompleted>,
    user_default_shell_unsupported_banner_model_handle: ModelHandle<BannerState>,
    server_api: Arc<ServerApi>,
    auth_state: Arc<AuthState>,
    server_time: Option<Arc<ServerTime>>,
    tab_bar_overflow_menu: ViewHandle<Menu<WorkspaceAction>>,
    show_tab_bar_overflow_menu: bool,
    tab_right_click_menu: ViewHandle<Menu<WorkspaceAction>>,
    show_tab_right_click_menu: Option<(usize, TabContextMenuAnchor)>,
    /// Open tab group more-options menu; reuses the `tab_right_click_menu` view.
    show_tab_group_right_click_menu: Option<(TabGroupId, TabContextMenuAnchor)>,
    // TODO(CORE-2300): this used to be add_tab_dropdown_menu.
    // Because we are rolling out the change behind a feature flag,
    // keep this comment here until the feature flag is removed.
    // Otherwise people might be confused as to why there is a right click
    // menu in the "new_session_dropdown_menu"
    // Same applies to "show_new_session_dropdown_menu"
    new_session_dropdown_menu: ViewHandle<Menu<WorkspaceAction>>,
    /// Anchor used to position the new-session dropdown when it's open. The
    /// variant determines whether the menu sits below the `+` add-tab button
    /// or floats at the pointer position (right-click on the panel chrome).
    show_new_session_dropdown_menu: Option<NewSessionMenuAnchor>,
    changelog_model: ModelHandle<ChangelogModel>,
    palette: ViewHandle<CommandPalette>,
    ctrl_tab_palette: ViewHandle<CommandPalette>,
    mouse_states: WorkspaceMouseStates,
    settings_pane: ViewHandle<SettingsView>,
    theme_chooser_view: ViewHandle<ThemeChooser>,
    previous_theme: Option<ThemeKind>,
    pub(crate) current_workspace_state: WorkspaceState,
    previous_workspace_state: Option<WorkspaceState>,
    welcome_tips_view_state: WelcomeTipsViewState,
    welcome_tips_view: ViewHandle<TipsView>,
    model_event_sender: Option<mpsc::SyncSender<ModelEvent>>,
    launch_config_save_modal: ModalViewState<LaunchConfigSaveModal>,
    tab_config_params_modal: ModalViewState<Modal<TabConfigParamsModal>>,
    session_config_modal: ModalViewState<Modal<SessionConfigModal>>,
    pending_session_config_replacement: Option<PendingSessionConfigReplacement>,
    /// When set, the guided onboarding tutorial will start after the session
    /// config modal is closed (submitted or dismissed).
    pending_onboarding_intention: Option<OnboardingIntention>,
    pending_session_config_tab_config_chip: bool,
    show_session_config_tab_config_chip: bool,
    pending_session_config_tab_config_chip_tutorial:
        Option<PendingSessionConfigTabConfigChipTutorial>,
    new_worktree_modal: ModalViewState<Modal<NewWorktreeModal>>,
    close_session_confirmation_dialog: ViewHandle<CloseSessionConfirmationDialog>,
    resource_center_view: ViewHandle<ResourceCenterView>,
    autoupdate_unable_to_update_banner_dismissed: bool,
    autoupdate_unable_to_launch_new_version: bool,
    reauth_banner_dismissed: bool,
    settings_file_error: Option<crate::settings::SettingsFileError>,
    settings_error_banner_dismissed: bool,
    should_show_ai_assistant_warm_welcome: bool,
    ai_assistant_close_warm_welcome_mouse_state_handle: MouseStateHandle,
    auth_override_warning_modal: ViewHandle<AuthOverrideWarningModal>,
    require_login_modal: ViewHandle<AuthView>,
    prompt_editor_modal: ViewHandle<PromptEditorModal>,
    header_toolbar_editor_modal: ViewHandle<HeaderToolbarEditorModal>,
    header_toolbar_context_menu: ViewHandle<Menu<WorkspaceAction>>,
    show_header_toolbar_context_menu: Option<Vector2F>,
    theme_creator_modal: ViewHandle<ThemeCreatorModal>,
    theme_deletion_modal: ViewHandle<ThemeDeletionModal>,
    oz_launch_modal: ModalWithTab<LaunchModal<OzLaunchSlide>>,
    openwarp_launch_modal: ViewHandle<OpenWarpLaunchModal>,
    orchestration_launch_modal: ViewHandle<OrchestrationLaunchModal>,
    build_plan_migration_modal: ViewHandle<BuildPlanMigrationModal>,
    codex_modal: ViewHandle<CodexModal>,
    cloud_agent_capacity_modal: ViewHandle<CloudAgentCapacityModal>,
    free_tier_limit_hit_modal: ViewHandle<FreeTierLimitHitModal>,
    free_tier_limit_check_triggered: bool,
    toast_stack: ViewHandle<DismissibleToastStack<WorkspaceAction>>,
    agent_toast_stack: ViewHandle<AgentToastStack>,
    update_toast_stack: ViewHandle<DismissibleToastStack<WorkspaceAction>>,
    /// We need to render some dynamic keybindings for our tooltips. These cannot be looked up in the
    /// render method, so look them up when the view is constructed and cache them here. Note that they
    /// need to be kept in sync as the keybindings change.
    cached_keybindings: HashMap<String, Option<String>>,
    is_user_menu_open: bool,
    tab_bar_pinned_by_popup: bool,
    user_menu: ViewHandle<Menu<WorkspaceAction>>,
    native_modal: ViewHandle<NativeModal>,
    shown_staging_banner_count: u32,

    // When user's open WEB for the first time, we ask them to select a preference of
    // always opening in web or opening in native app.
    #[cfg(target_family = "wasm")]
    show_wasm_nux_dialog: bool,
    #[cfg(target_family = "wasm")]
    wasm_nux_dialog: ViewHandle<WasmNUXDialog>,
    #[cfg(target_family = "wasm")]
    open_in_warp_button: ViewHandle<ActionButton>,
    #[cfg(target_family = "wasm")]
    view_cloud_runs_button: ViewHandle<ActionButton>,
    #[cfg(target_family = "wasm")]
    transcript_info_button: ViewHandle<ActionButton>,
    #[cfg(target_family = "wasm")]
    transcript_details_panel: ViewHandle<ConversationDetailsPanel>,

    file_upload_sessions: FileUploadSessions,
    left_panel_open: bool,
    vertical_tabs_panel_open: bool,
    vertical_tabs_panel: VerticalTabsPanelState,
    left_panel_view: ViewHandle<LeftPanelView>,
    left_panel_views: Vec<ToolPanelView>,
    working_directories_model: ModelHandle<pane_group::WorkingDirectoriesModel>,
    lightbox_view: Option<ViewHandle<LightboxView>>,
    hoa_onboarding_flow: Option<ViewHandle<HoaOnboardingFlow>>,
    /// Pinned position for the vertical tabs callout so it doesn't move when
    /// the user toggles between vertical and horizontal tabs.
    hoa_vtabs_callout_pinned_position: Option<Vector2F>,
    /// When true, this workspace was created to receive a transferred PaneGroup.
    /// The placeholder tab will be replaced when adopt_transferred_pane_group is called.
    pending_pane_group_transfer: bool,
    /// When true, `on_window_closed` skips detaching panes, so pane groups
    /// transferred to another window aren't torn down when this window closes
    /// via `TerminationMode::ContentTransferred`.
    suppress_detach_panes_on_window_close: bool,
    /// True while this workspace is acting as the temporary preview window
    /// for a multi-tab cross-window drag. Reduces chrome (e.g. hides traffic
    /// lights). Cleared when the preview is promoted or hands off its tab.
    is_tab_drag_preview: bool,
    /// Sidecar menu for submenu-parent items (Terminal, New worktree config) in the
    /// new-session dropdown. Shown as a positioned overlay next to the hovered
    /// parent item, following the model picker sidecar pattern.
    new_session_sidecar_menu: ViewHandle<Menu<NewSessionSidecarSelection>>,
    show_new_session_sidecar: bool,
    worktree_sidecar_active: bool,
    worktree_sidecar_search_editor: ViewHandle<EditorView>,
    worktree_sidecar_search_query: String,
    new_session_sidecar_add_repo_mouse_state: MouseStateHandle,
    /// Sidecar menu rendered next to the tab more options menu when the user
    /// hovers "Move to group". Populated with one item per other available
    /// tab group; each selection dispatches `WorkspaceAction::MoveTabToGroup`.
    move_to_group_sidecar_menu: ViewHandle<Menu<WorkspaceAction>>,
    show_move_to_group_sidecar: bool,
    tab_config_action_sidecar_item: Option<SidecarItemKind>,
    tab_config_action_sidecar_mouse_states: crate::tab_configs::action_sidecar::SidecarMouseStates,
    remove_tab_config_confirmation_dialog: ViewHandle<RemoveTabConfigConfirmationDialog>,
}

impl Workspace {
    pub fn is_tab_drag_preview(&self) -> bool {
        self.is_tab_drag_preview
    }

    pub(crate) fn set_is_tab_drag_preview(&mut self, value: bool) {
        self.is_tab_drag_preview = value;
    }

    pub(crate) fn set_suppress_detach_panes_on_window_close(&mut self, value: bool) {
        self.suppress_detach_panes_on_window_close = value;
    }
    fn tab_rename_editor_font_size(ctx: &AppContext, appearance: &Appearance) -> f32 {
        if FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(ctx).use_vertical_tabs {
            match *TabSettings::as_ref(ctx)
                .vertical_tabs_display_granularity
                .value()
            {
                VerticalTabsDisplayGranularity::Panes => 10.,
                VerticalTabsDisplayGranularity::Tabs => 12.,
            }
        } else {
            appearance.ui_font_size()
        }
    }

    /// Clears the worktree sidecar state and hides the sidecar.
    fn clear_worktree_sidecar_state(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_new_session_sidecar = false;
        self.worktree_sidecar_active = false;
        self.worktree_sidecar_search_query.clear();
        self.worktree_sidecar_search_editor
            .update(ctx, |editor, ctx| {
                editor.clear_buffer(ctx);
            });
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            menu.clear_pinned_header_builder();
            menu.clear_pinned_footer_builder();
            menu.set_content_padding_overrides(None, None);
            menu.reset_selection(view_ctx);
        });
    }

    fn close_new_session_dropdown_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_new_session_dropdown_menu = None;
        self.tab_config_action_sidecar_item = None;
        self.clear_worktree_sidecar_state(ctx);
        self.new_session_dropdown_menu.update(ctx, |menu, _| {
            menu.set_safe_zone_target(None);
            menu.set_submenu_being_shown_for_item_index(None);
        });
        ctx.notify();
    }

    fn select_first_worktree_sidecar_repo(&mut self, ctx: &mut ViewContext<Self>) {
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            if menu.items_len() > 1 {
                menu.set_selected_by_index(1, view_ctx);
            } else {
                menu.reset_selection(view_ctx);
            }
        });
    }

    fn reset_worktree_sidecar_repo_selection(&mut self, ctx: &mut ViewContext<Self>) {
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            menu.reset_selection(view_ctx);
        });
    }

    fn navigate_worktree_sidecar_selection(
        &mut self,
        select_next: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            let items_len = menu.items_len();
            if items_len <= 1 {
                return;
            }

            match menu.selected_index() {
                Some(_) if select_next => menu.select_next(view_ctx),
                Some(_) => menu.select_previous(view_ctx),
                None if select_next => menu.set_selected_by_index(1, view_ctx),
                None => menu.set_selected_by_index(items_len.saturating_sub(1), view_ctx),
            }
        });
    }

    fn confirm_worktree_sidecar_selection(&mut self, ctx: &mut ViewContext<Self>) {
        let selected_selection = self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            if menu.items_len() <= 1 {
                return None;
            }

            if menu.selected_index().is_none() {
                menu.set_selected_by_index(1, view_ctx);
            }

            menu.selected_item().and_then(|item| match item {
                MenuItem::Item(fields) => fields.on_select_action().cloned(),
                _ => None,
            })
        });

        if let Some(selection) = selected_selection {
            self.execute_new_session_sidecar_selection(selection, ctx);
            self.close_new_session_dropdown_menu(ctx);
        }
    }

    fn sync_new_session_sidecar_selection_to_hover(&mut self, ctx: &mut ViewContext<Self>) {
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            let Some(hovered_index) = menu.hovered_index() else {
                return;
            };
            let hovered_item_has_action = menu
                .items()
                .get(hovered_index)
                .and_then(MenuItem::item_on_select_action)
                .is_some();

            if hovered_item_has_action && menu.selected_index() != Some(hovered_index) {
                menu.set_selected_by_index(hovered_index, view_ctx);
            }
        });
    }

    fn build_worktree_sidecar_search_input(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let mut editor = EditorView::single_line(
                SingleLineEditorOptions {
                    text: TextOptions::ui_text(Some(appearance.ui_font_size()), appearance),
                    select_all_on_focus: true,
                    clear_selections_on_blur: true,
                    propagate_and_no_op_vertical_navigation_keys:
                        PropagateAndNoOpNavigationKeys::Always,
                    ..Default::default()
                },
                ctx,
            );
            editor.set_placeholder_text("Search repos", ctx);
            editor
        });
        ctx.subscribe_to_view(&editor, |me, editor_view, event, ctx| match event {
            EditorEvent::Edited(_) => {
                me.worktree_sidecar_search_query = editor_view.as_ref(ctx).buffer_text(ctx);
                me.refresh_worktree_sidecar_if_active(ctx);
                ctx.notify();
            }
            EditorEvent::Escape => {
                me.close_new_session_dropdown_menu(ctx);
            }
            EditorEvent::Navigate(NavigationKey::Up) => {
                me.navigate_worktree_sidecar_selection(false, ctx);
            }
            EditorEvent::Navigate(NavigationKey::Down) => {
                me.navigate_worktree_sidecar_selection(true, ctx);
            }
            EditorEvent::Enter => {
                me.confirm_worktree_sidecar_selection(ctx);
            }
            _ => {}
        });
        editor
    }

    fn vertical_tabs_search_input(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let options = SingleLineEditorOptions {
                text: TextOptions::ui_text(Some(12.), appearance),
                ..Default::default()
            };
            EditorView::single_line(options, ctx)
        });
        editor.update(ctx, |editor, ctx| {
            editor.set_placeholder_text("Search tabs...", ctx);
        });
        ctx.subscribe_to_view(&editor, |me, editor_view, event, ctx| match event {
            EditorEvent::Edited(_) => {
                me.vertical_tabs_panel.search_query = editor_view.as_ref(ctx).buffer_text(ctx);
                ctx.notify();
            }
            EditorEvent::Escape => {
                me.vertical_tabs_panel.search_query.clear();
                me.focus_active_tab(ctx);
            }
            _ => {}
        });
        editor
    }
    fn tab_rename_editor(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = {
            ctx.add_typed_action_view(|ctx| {
                let appearance = Appearance::as_ref(ctx);
                let options = SingleLineEditorOptions {
                    text: TextOptions::ui_text(
                        Some(Self::tab_rename_editor_font_size(ctx, appearance)),
                        appearance,
                    ),
                    ..Default::default()
                };
                EditorView::single_line(options, ctx)
            })
        };
        ctx.subscribe_to_view(&editor, move |me, _, event, ctx| {
            me.handle_tab_rename_editor_event(event, ctx);
        });
        editor
    }

    fn pane_rename_editor(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let options = SingleLineEditorOptions {
                text: TextOptions::ui_text(Some(12.), appearance),
                ..Default::default()
            };
            EditorView::single_line(options, ctx)
        });
        ctx.subscribe_to_view(&editor, move |me, _, event, ctx| {
            me.handle_pane_rename_editor_event(event, ctx);
        });
        editor
    }

    fn tab_group_rename_editor(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let options = SingleLineEditorOptions {
                text: TextOptions::ui_text(Some(12.), appearance),
                ..Default::default()
            };
            EditorView::single_line(options, ctx)
        });
        ctx.subscribe_to_view(&editor, move |me, _, event, ctx| {
            me.handle_tab_group_rename_editor_event(event, ctx);
        });
        editor
    }

    pub fn handle_tab_rename_editor_event(
        &mut self,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.current_workspace_state.is_tab_being_renamed() {
            match event {
                EditorEvent::Blurred | EditorEvent::Enter => {
                    self.finish_tab_rename(ctx);
                }
                EditorEvent::Escape => {
                    self.cancel_tab_rename(ctx);
                }
                _ => {}
            }
        }
    }

    pub fn handle_pane_rename_editor_event(
        &mut self,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.current_workspace_state.is_any_pane_being_renamed() {
            match event {
                EditorEvent::Blurred | EditorEvent::Enter => {
                    self.finish_pane_rename(ctx);
                }
                EditorEvent::Escape => {
                    self.cancel_pane_rename(ctx);
                }
                _ => {}
            }
        }
    }

    pub fn handle_tab_group_rename_editor_event(
        &mut self,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if self
            .current_workspace_state
            .is_any_tab_group_being_renamed()
        {
            match event {
                EditorEvent::Blurred | EditorEvent::Enter => {
                    self.finish_tab_group_rename(ctx);
                }
                EditorEvent::Escape => {
                    self.cancel_tab_group_rename(ctx);
                }
                _ => {}
            }
        }
    }

    fn finish_tab_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(tab_index) = self.current_workspace_state.tab_being_renamed() {
            self.current_workspace_state.clear_tab_being_renamed();
            let title = self.tab_rename_editor.as_ref(ctx).buffer_text(ctx);
            let tab = &self.tabs[tab_index];
            tab.pane_group.update(ctx, |view, ctx| {
                // Only update the title if it was actually changed. Otherwise, lets assume
                // user's intend was to cancel the operation.
                if view.display_title(ctx) != title {
                    send_telemetry_from_ctx!(
                        TelemetryEvent::TabRenamed(TabRenameEvent::CustomNameSet),
                        ctx
                    );
                    view.set_title(&title, ctx);
                }
            });
            self.clear_tab_name_editor(ctx);
            self.update_window_title(ctx);
            ctx.notify();
        }
    }

    fn finish_pane_rename(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(locator) = self.current_workspace_state.pane_being_renamed() else {
            return;
        };

        self.current_workspace_state.clear_pane_being_renamed();
        let title = self.pane_rename_editor.as_ref(ctx).buffer_text(ctx);
        self.set_custom_pane_name(locator, title, ctx);
        self.clear_pane_name_editor(ctx);
        self.focus_pane(locator, ctx);
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    fn cancel_tab_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if self.current_workspace_state.is_tab_being_renamed() {
            self.current_workspace_state.clear_tab_being_renamed();
            self.clear_tab_name_editor(ctx);
            self.focus_active_tab(ctx);
            ctx.notify();
        }
    }

    fn cancel_pane_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(locator) = self.current_workspace_state.pane_being_renamed() {
            self.current_workspace_state.clear_pane_being_renamed();
            self.clear_pane_name_editor(ctx);
            self.focus_pane(locator, ctx);
            ctx.notify();
        }
    }

    fn finish_tab_group_rename(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(group_id) = self.current_workspace_state.tab_group_being_renamed() else {
            return;
        };
        self.current_workspace_state.clear_tab_group_being_renamed();
        let title = self.tab_group_rename_editor.as_ref(ctx).buffer_text(ctx);
        let trimmed = title.trim();
        // If the user cleared the input, keep the existing name (mirror tab/pane rename behavior).
        if !trimmed.is_empty() {
            if let Some(group) = self.tab_groups.get_mut(&group_id) {
                group.name = Some(trimmed.to_string());
            }
        }
        self.clear_tab_group_name_editor(ctx);
        self.focus_active_tab(ctx);
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    fn cancel_tab_group_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if self
            .current_workspace_state
            .is_any_tab_group_being_renamed()
        {
            self.current_workspace_state.clear_tab_group_being_renamed();
            self.clear_tab_group_name_editor(ctx);
            self.focus_active_tab(ctx);
            ctx.notify();
        }
    }



    fn build_prompt_editor_modal(ctx: &mut ViewContext<Self>) -> ViewHandle<PromptEditorModal> {
        let modal = ctx.add_typed_action_view(PromptEditorModal::new);
        ctx.subscribe_to_view(&modal, |me, _, event, ctx| {
            me.handle_prompt_editor_modal_event(event, ctx);
        });
        modal
    }


    fn build_welcome_tips(
        tips_completed: ModelHandle<TipsCompleted>,
        ctx: &mut ViewContext<Self>,
    ) -> (ViewHandle<TipsView>, WelcomeTipsViewState) {
        let welcome_tips_view = ctx.add_typed_action_view(|ctx| {
            TipsView::new(tips_completed.clone(), WELCOME_TIPS_POSITION_ID.into(), ctx)
        });

        ctx.subscribe_to_view(&welcome_tips_view, move |me, _, event, ctx| {
            me.handle_welcome_tips_event(event, ctx);
        });
        let show_welcome_tips = !tips_completed.as_ref(ctx).skipped_or_completed;
        let welcome_tips_view_state = if show_welcome_tips {
            WelcomeTipsViewState::Available {
                is_popup_open: false,
            }
        } else {
            WelcomeTipsViewState::Unavailable
        };
        (welcome_tips_view, welcome_tips_view_state)
    }


    fn build_resource_center_view(
        ctx: &mut ViewContext<Self>,
        tips_completed: ModelHandle<TipsCompleted>,
        changelog_model_handle: ModelHandle<ChangelogModel>,
    ) -> ViewHandle<ResourceCenterView> {
        let resource_center_view = ctx.add_typed_action_view(|ctx| {
            ResourceCenterView::new(ctx, tips_completed.clone(), changelog_model_handle)
        });

        ctx.subscribe_to_view(&resource_center_view, |me, _, event, ctx| {
            me.handle_resource_center_event(event, ctx);
        });

        resource_center_view
    }

    fn build_settings_views(
        tips_completed: ModelHandle<TipsCompleted>,
        ctx: &mut ViewContext<Self>,
    ) -> (ViewHandle<SettingsView>, ViewHandle<ThemeChooser>) {
        let theme_chooser_view = ctx.add_typed_action_view(|ctx| {
            ThemeChooser::new(ctx, tips_completed)
        });

        ctx.subscribe_to_view(&theme_chooser_view, |me, _, event, ctx| {
            me.handle_theme_chooser_event(event, ctx);
        });

        let settings_pane = ctx.add_typed_action_view(move |ctx| SettingsView::new(None, ctx));
        ctx.subscribe_to_view(&settings_pane, move |me, _, event, ctx| {
            me.handle_settings_pane_event(event, ctx);
        });

        let window_id = ctx.window_id();
        SettingsPaneManager::handle(ctx).update(ctx, |manager, _| {
            manager.register_view(window_id, settings_pane.clone());
        });

        (settings_pane, theme_chooser_view)
    }

    fn build_require_login_modal(ctx: &mut ViewContext<Self>) -> ViewHandle<AuthView> {
        let require_login_modal = ctx.add_typed_action_view(|ctx| {
            AuthView::new(AuthViewVariant::RequireLoginCloseable, ctx)
        });
        ctx.subscribe_to_view(&require_login_modal, move |me, _, event, ctx| {
            me.handle_require_login_modal_event(event, ctx);
        });

        require_login_modal
    }

    fn build_auth_override_warning_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<AuthOverrideWarningModal> {
        let auth_override_warning_modal = ctx.add_typed_action_view(|ctx| {
            AuthOverrideWarningModal::new(ctx, AuthOverrideWarningModalVariant::WorkspaceModal)
        });

        ctx.subscribe_to_view(&auth_override_warning_modal, |me, _, event, ctx| {
            me.handle_auth_override_warning_modal_event(event, ctx);
        });

        auth_override_warning_modal
    }

    fn handle_auth_override_warning_modal_event(
        &mut self,
        event: &AuthOverrideWarningModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            AuthOverrideWarningModalEvent::Close => {
                self.current_workspace_state.is_auth_override_modal_open = false;
                ctx.notify();
            }
            AuthOverrideWarningModalEvent::BulkExport => {
                self.export_all_warp_drive_objects(ctx);
            }
        }
    }


    fn build_theme_creator_modal(ctx: &mut ViewContext<Self>) -> ViewHandle<ThemeCreatorModal> {
        let theme_creator_modal = ctx.add_typed_action_view(ThemeCreatorModal::new);
        ctx.subscribe_to_view(&theme_creator_modal, move |me, _, event, ctx| {
            me.handle_theme_creator_modal_event(event, ctx);
        });

        theme_creator_modal
    }

    fn build_theme_deletion_modal(ctx: &mut ViewContext<Self>) -> ViewHandle<ThemeDeletionModal> {
        let theme_deletion_modal = ctx.add_typed_action_view(ThemeDeletionModal::new);
        ctx.subscribe_to_view(&theme_deletion_modal, move |me, _, event, ctx| {
            me.handle_theme_deletion_modal_event(event, ctx);
        });

        theme_deletion_modal
    }




    fn build_close_session_confirmation_dialog(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<CloseSessionConfirmationDialog> {
        let close_session_confirmation_dialog =
            ctx.add_typed_action_view(|_| CloseSessionConfirmationDialog::new());
        ctx.subscribe_to_view(
            &close_session_confirmation_dialog,
            move |me, _, event, ctx| {
                me.handle_close_session_confirmation_dialog_event(event, ctx);
            },
        );

        close_session_confirmation_dialog
    }



    fn build_native_modal_view(ctx: &mut ViewContext<Self>) -> ViewHandle<NativeModal> {
        let native_modal = ctx.add_typed_action_view(NativeModal::new);
        ctx.subscribe_to_view(&native_modal, move |me, _, event, ctx| {
            me.handle_native_modal_event(event, ctx);
        });
        native_modal
    }

    fn build_tab_bar_overflow_menu(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<Menu<WorkspaceAction>> {
        let tab_bar_overflow_menu = ctx.add_typed_action_view(|_| {
            Menu::new()
                .with_width(TAB_BAR_OVERFLOW_MENU_WIDTH)
                .with_drop_shadow()
        });
        ctx.subscribe_to_view(&tab_bar_overflow_menu, move |me, _, event, ctx| {
            me.handle_tab_bar_overflow_menu_event(event, ctx);
        });
        tab_bar_overflow_menu
    }

    fn build_menus(ctx: &mut ViewContext<Self>) -> WorkspaceMenuHandles {
        // `prevent_interaction_with_other_elements` so that a click outside
        // the menu only dismisses it instead of also firing handlers on
        // whatever element is behind the click (e.g. the vertical tabs
        // panel's right-click handler that opens the new-session dropdown).
        // `with_safe_triangle` lets the move-to-group sidecar suppress hover
        // on intermediate items as the cursor moves toward it.
        let tab_right_click_menu = ctx.add_typed_action_view(|_| {
            Menu::new()
                .with_safe_triangle()
                .prevent_interaction_with_other_elements()
        });
        ctx.subscribe_to_view(&tab_right_click_menu, move |me, _, event, ctx| {
            me.handle_tab_right_click_menu_event(event, ctx);
        });

        // Currently setting the width to 300 px as a middle ground that looks
        // ok when the shells show the path to the executables, and when they
        // don't. Going forward we may want to enhance the menu to allow for a
        // `max_width` and `min_width` instead, so we can allow the menu to
        // grow as needed.
        const NEW_SESSION_MENU_WIDTH: f32 = 300.;
        let new_session_menu = ctx.add_typed_action_view(|ctx| {
            if FeatureFlag::ShellSelector.is_enabled() {
                let theme = Appearance::as_ref(ctx).theme();
                Menu::new()
                    .with_width(NEW_SESSION_MENU_WIDTH)
                    .with_border(Border::all(1.).with_border_color(theme.outline().into()))
                    .with_drop_shadow()
                    .with_safe_triangle()
                    .with_ignore_hover_when_covered()
                    .prevent_interaction_with_other_elements()
            } else {
                Menu::new()
                    .with_safe_triangle()
                    .with_ignore_hover_when_covered()
                    .prevent_interaction_with_other_elements()
            }
        });
        ctx.subscribe_to_view(&new_session_menu, move |me, _, event, ctx| {
            me.handle_new_session_menu_event(event, ctx);
        });

        let new_session_sidecar = ctx.add_typed_action_view(|_ctx| {
            let mut menu = Menu::new()
                .without_item_action_dispatch()
                .with_width(NEW_SESSION_SIDECAR_WIDTH)
                .with_menu_variant(crate::menu::MenuVariant::scrollable());
            menu.set_height(400.);
            menu
        });
        ctx.subscribe_to_view(&new_session_sidecar, move |me, _, event, ctx| {
            me.handle_new_session_sidecar_event(event, ctx);
        });

        // The move-to-group sidecar dispatches `MoveTabToGroup` directly when
        // a group item is clicked.
        let move_to_group_sidecar = ctx.add_typed_action_view(|_ctx| {
            let mut menu = Menu::new()
                .with_width(MOVE_TO_GROUP_SIDECAR_WIDTH)
                .with_drop_shadow()
                .with_menu_variant(crate::menu::MenuVariant::scrollable());
            menu.set_height(300.);
            menu
        });
        ctx.subscribe_to_view(&move_to_group_sidecar, move |me, _, event, ctx| {
            me.handle_move_to_group_sidecar_event(event, ctx);
        });

        (
            tab_right_click_menu,
            new_session_menu,
            new_session_sidecar,
            move_to_group_sidecar,
        )
    }

    fn build_launch_config_save_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ModalViewState<LaunchConfigSaveModal> {
        let launch_config_save_modal = ctx.add_typed_action_view(LaunchConfigSaveModal::new);
        ctx.subscribe_to_view(&launch_config_save_modal, move |me, _, event, ctx| {
            me.handle_launch_config_save_modal_event(event, ctx);
        });

        ModalViewState::new(launch_config_save_modal)
    }

    fn build_tab_config_params_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ModalViewState<Modal<TabConfigParamsModal>> {
        let body = ctx.add_typed_action_view(TabConfigParamsModal::new);
        // Subscribe to body events before moving `body` into the Modal closure.
        ctx.subscribe_to_view(&body, |me, _, event, ctx| {
            me.handle_tab_config_params_modal_body_event(event, ctx);
        });
        let modal = ctx.add_typed_action_view(|ctx| {
            Modal::new(None, body, ctx)
                .with_modal_style(UiComponentStyles {
                    width: Some(460.),
                    height: Some(480.),
                    ..Default::default()
                })
                .with_body_style(UiComponentStyles {
                    padding: Some(Coords::uniform(0.)),
                    height: Some(480.),
                    background: Some(ElementFill::None),
                    ..Default::default()
                })
                .with_dismiss_on_click()
        });
        ctx.subscribe_to_view(&modal, |me, _, event, ctx| {
            me.handle_tab_config_params_modal_event(event, ctx);
        });
        ModalViewState::new(modal)
    }

    fn build_session_config_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ModalViewState<Modal<SessionConfigModal>> {
        let body = ctx.add_typed_action_view(SessionConfigModal::new);
        ctx.subscribe_to_view(&body, |me, _, event, ctx| {
            me.handle_session_config_modal_event(event, ctx);
        });
        let modal = ctx.add_typed_action_view(|ctx| {
            Modal::new(None, body, ctx)
                .close_modal_button_disabled()
                .with_modal_style(UiComponentStyles {
                    width: Some(424.),
                    ..Default::default()
                })
                .with_background_opacity(0)
                .with_body_style(UiComponentStyles {
                    padding: Some(Coords::uniform(0.)),
                    ..Default::default()
                })
                .with_header_style(UiComponentStyles {
                    height: Some(0.),
                    padding: Some(Coords::uniform(0.)),
                    ..Default::default()
                })
        });
        ctx.subscribe_to_view(&modal, |me, _, event, ctx| {
            if matches!(event, ModalEvent::Close) {
                me.close_session_config_modal(ctx);
            }
        });
        ModalViewState::new(modal)
    }

    fn build_new_worktree_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ModalViewState<Modal<NewWorktreeModal>> {
        let body = ctx.add_typed_action_view(NewWorktreeModal::new);
        ctx.subscribe_to_view(&body, |me, _, event, ctx| {
            me.handle_new_worktree_modal_body_event(event, ctx);
        });
        let modal = ctx.add_typed_action_view(|ctx| {
            // We intentionally pass `None` for the title so the Modal renders
            // no built-in header — the body view renders its own header to
            // match the Figma mock exactly (bold title + X close + ESC badge).
            Modal::new(None, body, ctx)
                .with_modal_style(UiComponentStyles {
                    width: Some(460.),
                    height: Some(480.),
                    ..Default::default()
                })
                .with_body_style(UiComponentStyles {
                    padding: Some(Coords {
                        top: 0.,
                        bottom: 0.,
                        left: 0.,
                        right: 0.,
                    }),
                    height: Some(480.),
                    background: Some(ElementFill::None),
                    ..Default::default()
                })
        });
        ctx.subscribe_to_view(&modal, |me, _, event, ctx| {
            me.handle_new_worktree_modal_event(event, ctx);
        });
        ModalViewState::new(modal)
    }

    fn build_remove_tab_config_confirmation_dialog(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<RemoveTabConfigConfirmationDialog> {
        let dialog = ctx.add_typed_action_view(RemoveTabConfigConfirmationDialog::new);
        ctx.subscribe_to_view(&dialog, |me, _, event, ctx| {
            me.handle_remove_tab_config_confirmation_event(event, ctx);
        });
        dialog
    }

    #[cfg(feature = "local_fs")]
    fn handle_remove_tab_config_confirmation_event(
        &mut self,
        event: &RemoveTabConfigConfirmationEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            RemoveTabConfigConfirmationEvent::Confirm { path } => {
                // If the removed config was the default, revert to Terminal.
                let ai_settings = AISettings::as_ref(ctx);
                let is_removed_default = ai_settings.default_session_mode(ctx)
                    == DefaultSessionMode::TabConfig
                    && ai_settings.default_tab_config_path() == path.to_string_lossy();
                if is_removed_default {
                    AISettings::handle(ctx).update(ctx, |settings, ctx| {
                        report_if_error!(settings
                            .default_session_mode_internal
                            .set_value(DefaultSessionMode::Terminal, ctx));
                        report_if_error!(settings
                            .default_tab_config_path
                            .set_value(String::new(), ctx));
                    });
                }
                if let Err(e) = std::fs::remove_file(path) {
                    log::warn!("Failed to remove tab config file: {e:?}");
                    self.toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.add_ephemeral_toast(
                            DismissibleToast::error(format!("Failed to remove tab config: {e}")),
                            ctx,
                        );
                    });
                } else {
                    WarpConfig::handle(ctx).update(ctx, |warp_config, ctx| {
                        warp_config.remove_tab_config_by_path(path, ctx);
                    });
                }
                self.current_workspace_state
                    .is_remove_tab_config_dialog_open = false;
                ctx.notify();
            }
            RemoveTabConfigConfirmationEvent::Cancel => {
                self.current_workspace_state
                    .is_remove_tab_config_dialog_open = false;
                ctx.notify();
            }
        }
    }

    #[cfg(not(feature = "local_fs"))]
    fn handle_remove_tab_config_confirmation_event(
        &mut self,
        _event: &RemoveTabConfigConfirmationEvent,
        _ctx: &mut ViewContext<Self>,
    ) {
        log::error!("Cannot delete a tab config from the web");
    }

    fn handle_session_config_modal_event(
        &mut self,
        event: &SessionConfigModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            SessionConfigModalEvent::Completed(selection) => {
                let pending_intention = self.pending_onboarding_intention.take();
                send_telemetry_from_ctx!(
                    TabConfigsTelemetryEvent::GuidedModalSubmitted {
                        session_type: GuidedModalSessionType::from(&selection.session_type),
                        enable_worktree: selection.enable_worktree,
                        autogenerate_worktree_branch_name: selection
                            .autogenerate_worktree_branch_name,
                    },
                    ctx
                );
                self.close_session_config_modal(ctx);
                let has_worktree = selection.enable_worktree;
                let has_params = {
                    use crate::tab_configs::session_config::build_tab_config;
                    let config = build_tab_config(
                        &selection.session_type,
                        &selection.directory,
                        selection.enable_worktree,
                        selection.autogenerate_worktree_branch_name,
                    );
                    !config.params.is_empty()
                };
                self.handle_session_config_completed(selection, ctx);

                if let Some(intention) = pending_intention {
                    if has_worktree && has_params {
                        // Worktree with params modal: the tab hasn't been
                        // created yet. Keep the intention so the params modal
                        // handler can queue the tutorial after it closes.
                        self.pending_onboarding_intention = Some(intention);
                    } else if has_worktree {
                        self.queue_onboarding_tutorial_after_session_config_tab_config_chip(
                            PendingSessionConfigTabConfigChipTutorial::AfterSetupCommands {
                                intention,
                            },
                            ctx,
                        );
                    } else {
                        // No worktree: tab is ready. Start the tutorial after
                        // the tab-config chip is dismissed.
                        // TODO(roland): We do have a directory in this case so we could consider passing has_project = true
                        // which has an optional /init flow. But the behavior of /init needs to be revisited:
                        // 1. Sends /init as a query which differs in behavior from /init slash command
                        // 2. Sends /init even if not in a git repo - unclear if this should happen (depends on desired behavior from 1)
                        // 3. With no free AI, /init will not work.
                        self.queue_onboarding_tutorial_after_session_config_tab_config_chip(
                            PendingSessionConfigTabConfigChipTutorial::WhenBootstrapped {
                                has_project: false,
                                intention,
                            },
                            ctx,
                        );
                    }
                }

                // Show the chip only when no params modal followed.
                if !self.current_workspace_state.is_tab_config_params_modal_open {
                    self.promote_session_config_tab_config_chip(ctx);
                }
            }
            SessionConfigModalEvent::Dismissed => {
                let pending_intention = self.pending_onboarding_intention.take();

                // No tab config was created, so don't show the chip.
                self.pending_session_config_tab_config_chip = false;
                self.close_session_config_modal(ctx);

                // Start the onboarding tutorial without project context.
                if let Some(intention) = pending_intention {
                    self.dispatch_tutorial_when_bootstrapped(false, intention, ctx);
                }
            }
        }
    }

    #[cfg(feature = "local_fs")]
    fn handle_session_config_completed(
        &mut self,
        selection: &crate::tab_configs::session_config::SessionConfigSelection,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::tab_configs::session_config::{build_tab_config, write_tab_config};

        // Build a TabConfig.
        let config = build_tab_config(
            &selection.session_type,
            &selection.directory,
            selection.enable_worktree,
            selection.autogenerate_worktree_branch_name,
        );

        let old_pane_group_id = self.active_tab_pane_group().id();
        let has_params = !config.params.is_empty();

        // Save and open the tab config. The user's `default_session_mode`
        // is intentionally left untouched: creating a tab config should not
        // change the global default for new tabs.
        // Agent view entry for Oz is handled by PaneMode::Agent in the tab config,
        // so no manual enter_agent_view call is needed.
        let dir = crate::user_config::tab_configs_dir();
        if let Err(e) = write_tab_config(&config, &dir, "startup_config") {
            log::warn!("Failed to write startup tab config: {e:?}");
        }

        if has_params {
            // When the config has params (worktree), open_tab_config shows the
            // params modal instead of creating the tab immediately.
            // Store the replacement context so we can finish when the modal completes.
            self.pending_session_config_replacement =
                Some(PendingSessionConfigReplacement { old_pane_group_id });
            self.open_tab_config(config, ctx);
        } else {
            let worktree_branch_name = self.maybe_generate_worktree_name(&config);
            let param_values = config.default_param_values();
            self.open_tab_config_with_params(
                config,
                param_values,
                worktree_branch_name.as_deref(),
                ctx,
            );
            self.remove_tab_by_pane_group_id(old_pane_group_id, ctx);
        }
    }

    #[cfg(not(feature = "local_fs"))]
    fn handle_session_config_completed(
        &mut self,
        _selection: &crate::tab_configs::session_config::SessionConfigSelection,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    /// Opens the vertical tabs panel if the setting was enabled.
    /// Called from the onboarding flow before the session config modal is shown.
    pub(crate) fn open_vertical_tabs_panel_if_enabled(&mut self, ctx: &mut ViewContext<Self>) {
        if FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(ctx).use_vertical_tabs {
            self.vertical_tabs_panel_open = true;
            self.sync_window_button_visibility(ctx);
            ctx.notify();
        }
    }

    fn show_hoa_onboarding_flow(&mut self, ctx: &mut ViewContext<Self>) {
        // Mark as completed immediately so the flow is never shown again,
        // even if the user quits mid-flow.
        mark_hoa_onboarding_completed(ctx);

        // Enable vertical tabs and open the panel so Step 2 has something to anchor to.
        TabSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.use_vertical_tabs.set_value(true, ctx);
        });
        self.vertical_tabs_panel_open = true;
        self.sync_window_button_visibility(ctx);

        // The pinned position is captured lazily on the first step change
        // (when the user advances past the welcome banner). At that point the
        // vertical tabs panel has been rendered for several frames and the save
        // position is accurate.
        self.hoa_vtabs_callout_pinned_position = None;

        let flow = ctx.add_typed_action_view(HoaOnboardingFlow::new);
        ctx.subscribe_to_view(&flow, |me, _, event, ctx| match event {
            HoaOnboardingFlowEvent::StepChanged | HoaOnboardingFlowEvent::TabLayoutToggled => {
                if me.hoa_vtabs_callout_pinned_position.is_none() {
                    me.hoa_vtabs_callout_pinned_position = ctx
                        .element_position_by_id(VERTICAL_TABS_PANEL_POSITION_ID)
                        .map(|rect| vec2f(rect.max_x(), rect.min_y() + 8.));
                }
                if let Some(flow) = &me.hoa_onboarding_flow {
                    ctx.focus(flow);
                }
                ctx.notify();
            }
            _ => me.handle_hoa_onboarding_event(event, ctx),
        });
        self.hoa_onboarding_flow = Some(flow.clone());
        ctx.focus(&flow);
        ctx.notify();
    }

    fn handle_hoa_onboarding_event(
        &mut self,
        event: &HoaOnboardingFlowEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        OneTimeModalModel::handle(ctx).update(ctx, |model, ctx| {
            model.mark_hoa_onboarding_dismissed(ctx);
        });

        match event {
            HoaOnboardingFlowEvent::Completed(Some(selection)) => {
                self.hoa_onboarding_flow = None;
                self.handle_session_config_completed(selection, ctx);
            }
            HoaOnboardingFlowEvent::Completed(None) | HoaOnboardingFlowEvent::Dismissed => {
                self.hoa_onboarding_flow = None;
            }
            HoaOnboardingFlowEvent::StepChanged | HoaOnboardingFlowEvent::TabLayoutToggled => {
                return;
            }
        }

        self.focus_active_tab(ctx);
        ctx.notify();
    }

    pub(crate) fn show_session_config_modal(&mut self, ctx: &mut ViewContext<Self>) {
        // Configure the modal to hide Oz when AI is disabled.
        let show_oz = AISettings::as_ref(ctx).is_any_ai_enabled(ctx);
        self.session_config_modal.view.update(ctx, |modal, ctx| {
            modal.body().update(ctx, |body, ctx| {
                body.configure(show_oz);
                ctx.notify();
            });
        });

        self.session_config_modal.open();
        self.current_workspace_state.is_session_config_modal_open = true;
        self.pending_session_config_tab_config_chip = self.pending_onboarding_intention.is_some();
        self.show_session_config_tab_config_chip = false;
        ctx.focus(&self.session_config_modal.view);
        send_telemetry_from_ctx!(TabConfigsTelemetryEvent::GuidedModalOpened, ctx);
        ctx.notify();
    }

    fn close_session_config_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.session_config_modal.close();
        self.current_workspace_state.is_session_config_modal_open = false;
        // Don't promote pending → show here. The caller is responsible for
        // calling `promote_session_config_tab_config_chip` once all
        // intermediate modals (e.g. params modal) have closed.
        self.focus_active_tab(ctx);
        ctx.notify();
    }

    /// Promotes the pending tab-config chip to visible. This must be called
    /// only after **all** intermediate modals (session config modal, params
    /// modal) are closed. The chip is non-blocking: the user can still
    /// interact with the terminal and must click the chip's close button or
    /// press Escape/Enter to dismiss it.
    fn promote_session_config_tab_config_chip(&mut self, ctx: &mut ViewContext<Self>) {
        if self.pending_session_config_tab_config_chip {
            self.show_session_config_tab_config_chip = true;
            self.pending_session_config_tab_config_chip = false;
            ctx.notify();
        }
    }

    fn should_show_session_config_tab_config_chip(&self) -> bool {
        self.show_session_config_tab_config_chip
            && !self.current_workspace_state.is_session_config_modal_open
            && !self.current_workspace_state.is_tab_config_params_modal_open
    }

    fn queue_onboarding_tutorial_after_session_config_tab_config_chip(
        &mut self,
        pending_tutorial: PendingSessionConfigTabConfigChipTutorial,
        ctx: &mut ViewContext<Self>,
    ) {
        if matches!(
            pending_tutorial,
            PendingSessionConfigTabConfigChipTutorial::AfterSetupCommands { .. }
        ) {
            if let Some(terminal_view) = self.active_session_view(ctx) {
                terminal_view.update(ctx, |view, _| {
                    view.clear_enter_agent_view_after_pending_commands();
                });
            }
        }
        self.pending_session_config_tab_config_chip_tutorial = Some(pending_tutorial);
    }

    fn dismiss_session_config_tab_config_chip(&mut self, ctx: &mut ViewContext<Self>) {
        self.pending_session_config_tab_config_chip = false;
        self.show_session_config_tab_config_chip = false;
        if let Some(pending_tutorial) = self.pending_session_config_tab_config_chip_tutorial.take()
        {
            match pending_tutorial {
                PendingSessionConfigTabConfigChipTutorial::WhenBootstrapped {
                    has_project,
                    intention,
                } => {
                    self.dispatch_tutorial_when_bootstrapped(has_project, intention, ctx);
                }
                PendingSessionConfigTabConfigChipTutorial::AfterSetupCommands { intention } => {
                    self.dispatch_tutorial_after_setup_commands(intention, ctx);
                }
            }
        }
        ctx.notify();
    }

    fn render_session_config_tab_config_chip(
        &self,
        use_vertical: bool,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let close_button = Hoverable::new(
            self.mouse_states
                .session_config_tab_config_chip_close
                .clone(),
            |hover_state| {
                let icon = ConstrainedBox::new(
                    icons::Icon::X
                        .to_warpui_icon(Fill::Solid(PhenomenonStyle::modal_close_button_text()))
                        .finish(),
                )
                .with_width(16.)
                .with_height(16.)
                .finish();

                let mut button = Container::new(icon)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));
                if hover_state.is_hovered() {
                    button =
                        button.with_background_color(PhenomenonStyle::modal_close_button_hover());
                }
                button.finish()
            },
        )
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::DismissSessionConfigTabConfigChip);
        })
        .finish();

        let text = Text::new_inline(
            SESSION_CONFIG_TAB_CONFIG_CHIP_TEXT.to_string(),
            appearance.ui_font_family(),
            12.,
        )
        .with_color(PhenomenonStyle::body_text())
        .with_selectable(false)
        .finish();

        let content = Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(4.)
            .with_child(text)
            .with_child(close_button)
            .finish();
        let chip_content = Container::new(content)
            .with_padding_left(16.)
            .with_padding_right(12.)
            .with_padding_top(12.)
            .with_padding_bottom(12.)
            .finish();

        let (arrow_direction, arrow_position) = if use_vertical {
            (CalloutArrowDirection::Left, CalloutArrowPosition::Center)
        } else {
            (CalloutArrowDirection::Up, CalloutArrowPosition::Center)
        };

        render_callout_bubble(
            chip_content,
            &CalloutBubbleConfig {
                width: SESSION_CONFIG_TAB_CONFIG_CHIP_WIDTH,
                arrow_direction,
                arrow_position,
            },
            appearance,
        )
    }
    /// Subscribe to the [`ServerApiProvider`] model to report status changes.
    fn observe_server_api(ctx: &mut ViewContext<Self>) {
        let server_api_events = ServerApiProvider::handle(ctx);
        ctx.subscribe_to_model(&server_api_events, |me, _, event, ctx| {
            if let AuthEvent::StagingAccessBlocked = event {
                if ChannelState::uses_staging_server() && me.shown_staging_banner_count < 5 {
                    me.shown_staging_banner_count += 1;
                    me.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(
                            "Staging API call failed. Did your IP address change?".to_string(),
                        )
                        .with_object_id("staging_access_blocked_toast".to_string());
                        toast_stack.add_ephemeral_toast(toast, ctx);
                    });
                }
            }
        });
    }

    fn subscribe_to_workspace_toast_stack(
        toast_stack: ViewHandle<DismissibleToastStack<WorkspaceAction>>,
        ctx: &mut ViewContext<Self>,
    ) {
        let workspace_toast_stack = WorkspaceToastStack::handle(ctx);
        ctx.subscribe_to_model(
            &workspace_toast_stack,
            move |_me, _, event, ctx| match event {
                WorkspaceToastStackEvent::AddEphemeralToast { window_id, toast }
                    if *window_id == ctx.window_id() =>
                {
                    toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.add_ephemeral_toast(toast.clone(), ctx)
                    });
                }
                WorkspaceToastStackEvent::AddPersistentToast { window_id, toast }
                    if *window_id == ctx.window_id() =>
                {
                    toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.add_persistent_toast(toast.clone(), ctx)
                    });
                }
                WorkspaceToastStackEvent::RemoveToast {
                    window_id,
                    identifier,
                } if *window_id == ctx.window_id() => {
                    toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.dismiss_older_toasts(identifier, ctx)
                    });
                }
                _ => {}
            },
        );
    }

    /// Subscribes to `WarpConfigUpdateEvent::TabConfigErrors` and shows a persistent
    /// error toast for each tab config file that failed to parse.  Uses `object_id`
    /// keyed by file path so that re-saving the same file auto-dismisses the stale
    /// toast.
    fn subscribe_to_tab_config_errors(
        toast_stack: ViewHandle<DismissibleToastStack<WorkspaceAction>>,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.subscribe_to_model(&WarpConfig::handle(ctx), move |_me, _, event, ctx| {
            match event {
                WarpConfigUpdateEvent::TabConfigs => {
                    // On every tab config reload, dismiss error toasts for
                    // files that now parse successfully.  The model has already
                    // been updated with the current error set before this event
                    // fires, so we just need to clear stale toasts.
                    //
                    // `TabConfigErrors` is only emitted when errors exist, so
                    // when all files are fixed we only get `TabConfigs` — this
                    // branch handles that case by prefix-dismissing all
                    // tab-config-error toasts and letting `TabConfigErrors`
                    // re-add any that still apply.
                    toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.dismiss_toasts_by_prefix("tab_config_error:", ctx);
                    });
                }
                WarpConfigUpdateEvent::TabConfigErrors(errors) => {
                    let home_dir = dirs::home_dir();
                    for error in errors {
                        let object_id = format!("tab_config_error:{}", error.file_path.display());
                        let raw_path = error.file_path.display().to_string();
                        let friendly_path = user_friendly_path(
                            &raw_path,
                            home_dir.as_ref().and_then(|h| h.to_str()),
                        );
                        let message = format!(
                            "Failed to load tab config {friendly_path}: {}",
                            error.error_message
                        );
                        let path = error.file_path.clone();
                        let toast = DismissibleToast::error(message)
                            .with_object_id(object_id.clone())
                            .with_link(
                                ToastLink::new("Open file".to_string()).with_onclick_action(
                                    WorkspaceAction::OpenTabConfigErrorFile {
                                        path,
                                        toast_object_id: object_id,
                                    },
                                ),
                            );
                        toast_stack.update(ctx, |toast_stack, ctx| {
                            toast_stack.add_persistent_toast(toast, ctx);
                        });
                    }
                }
                _ => {}
            }
        });
    }

    /// Subscribes to `WarpConfigUpdateEvent::SettingsErrors` and
    /// `SettingsErrorsCleared` to update the workspace settings-error banner
    /// and mirror the state into the settings pane for its nav-rail footer.
    fn subscribe_to_settings_errors(ctx: &mut ViewContext<Self>) {
        ctx.subscribe_to_model(&WarpConfig::handle(ctx), |me, _, event, ctx| match event {
            WarpConfigUpdateEvent::SettingsErrors(error) => {
                me.settings_file_error = Some(error.clone());
                me.sync_settings_error_state_into_settings_pane(ctx);
                ctx.notify();
            }
            WarpConfigUpdateEvent::SettingsErrorsCleared => {
                me.settings_file_error = None;
                me.sync_settings_error_state_into_settings_pane(ctx);
                ctx.notify();
            }
            _ => {}
        });
    }

    /// Pushes the current settings-file error + banner-dismissal state into
    /// the settings pane so its nav-rail footer ("Open settings file" button
    /// or inline error alert) stays in sync with the workspace banner.
    fn sync_settings_error_state_into_settings_pane(&mut self, ctx: &mut ViewContext<Self>) {
        let error = self.settings_file_error.clone();
        let dismissed = self.settings_error_banner_dismissed;
        self.settings_pane.update(ctx, |view, ctx| {
            view.set_settings_error_state(error, dismissed, ctx);
        });
    }

    pub fn dismiss_older_toasts(&mut self, object_id: &str, ctx: &mut ViewContext<Self>) {
        self.toast_stack.update(ctx, |toast_stack, ctx| {
            toast_stack.dismiss_older_toasts(object_id, ctx);
        });
    }

    fn on_tips_model_changed(
        &mut self,
        _: ModelHandle<TipsCompleted>,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.notify();
    }

    pub fn new(
        global_resource_handles: GlobalResourceHandles,
        server_time: Option<Arc<ServerTime>>,
        workspace_setting: NewWorkspaceSource,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let GlobalResourceHandles {
            model_event_sender,
            tips_completed,
            user_default_shell_unsupported_banner_model_handle,
            settings_file_error,
        } = global_resource_handles.clone();

        let server_api_provider = ServerApiProvider::as_ref(ctx);
        let server_api = server_api_provider.get();
        let ai_client = server_api_provider.get_ai_client();

        // Inserting a (window, ModalSizes) pair to the ResizableData singleton. A restored window
        // reads the sizes from the window snapshot. A new window initializes with all default sizes.
        let resizable_data = ResizableData::handle(ctx);
        let window_id = ctx.window_id();
        let has_horizontal_split = workspace_setting.has_horizontal_split();

        let (left_panel_size, right_panel_size) =
            compute_default_panel_widths(ctx, window_id, has_horizontal_split);
        let new_resizable_modal_sizes = match workspace_setting.clone() {
            NewWorkspaceSource::Restored {
                window_snapshot, ..
            } => ModalSizes::from_restored(&window_snapshot, left_panel_size, right_panel_size),
            _ => ModalSizes::default_with_panel_defaults(left_panel_size, right_panel_size),
        };
        resizable_data.update(ctx, |model, _| {
            model.insert(window_id, new_resizable_modal_sizes)
        });

        terminal::platform::init().expect("Terminal platform initialized");

        let tab_bar_overflow_menu = Self::build_tab_bar_overflow_menu(ctx);
        let (
            tab_right_click_menu,
            new_session_dropdown_menu,
            new_session_sidecar_menu,
            move_to_group_sidecar_menu,
        ) = Self::build_menus(ctx);

        // Subscribe to network changes
        ctx.subscribe_to_model(
            &NetworkStatus::handle(ctx),
            Self::handle_network_status_event,
        );

        let palette =
            ctx.add_typed_action_view(|ctx| CommandPalette::new(NavigationMode::Normal, ctx));
        ctx.subscribe_to_view(&palette, |me, _, event, ctx| {
            me.handle_palette_event(event, ctx);
        });

        let ctrl_tab_palette =
            ctx.add_typed_action_view(|ctx| CommandPalette::new(NavigationMode::CtrlTab, ctx));
        ctx.subscribe_to_view(&ctrl_tab_palette, |me, _, event, ctx| {
            me.handle_palette_event(event, ctx);
        });

        let auth_manager = AuthManager::handle(ctx);
        ctx.subscribe_to_model(&auth_manager, Self::handle_auth_manager_event);

        // Handle theme updates when there is a cloud update to themes while the picker is open.
        ctx.subscribe_to_model(&ThemeSettings::handle(ctx), |me, _, _, ctx| {
            if me.is_theme_chooser_open() {
                me.theme_chooser_view.update(ctx, |view, ctx| {
                    view.handle_theme_change(ctx);
                });
            }
        });

        let bindings_notifier = KeybindingChangedNotifier::handle(ctx);
        ctx.subscribe_to_model(&bindings_notifier, |me, _, event, ctx| {
            me.handle_keybinding_changed(event, ctx);
        });

        let state_handle = WindowManager::handle(ctx);
        ctx.subscribe_to_model(&state_handle, |me, _, event, ctx| {
            me.handle_window_state_change(event, ctx);
        });

        ctx.observe(&RelaunchModel::handle(ctx), |_, _, ctx| {
            ctx.notify();
        });

        let changelog_model = ChangelogModel::handle(ctx);
        ctx.subscribe_to_model(&changelog_model, |me, _, event, ctx| {
            me.handle_changelog_event(event, ctx);
        });

        let (welcome_tips_view, welcome_tips_view_state) =
            Self::build_welcome_tips(tips_completed.clone(), ctx);
        let (settings_pane, theme_chooser_view) =
            Self::build_settings_views(tips_completed.clone(), ctx);

        let resource_center_view =
            Self::build_resource_center_view(ctx, tips_completed.clone(), changelog_model.clone());


        let build_plan_migration_modal = ctx.add_typed_action_view(BuildPlanMigrationModal::new);
        ctx.subscribe_to_view(&build_plan_migration_modal, |me, _, event, ctx| {
            me.handle_build_plan_migration_modal_event(event, ctx);
        });

        let codex_modal = ctx.add_typed_action_view(CodexModal::new);
        ctx.subscribe_to_view(&codex_modal, |me, _, event, ctx| {
            me.handle_codex_modal_event(event, ctx);
        });

        let cloud_agent_capacity_modal =
            ctx.add_typed_action_view(|_| CloudAgentCapacityModal::new());
        ctx.subscribe_to_view(&cloud_agent_capacity_modal, |me, _, event, ctx| {
            me.handle_cloud_agent_capacity_modal_event(event, ctx);
        });

        let free_tier_limit_hit_modal = ctx.add_typed_action_view(FreeTierLimitHitModal::new);
        ctx.subscribe_to_view(&free_tier_limit_hit_modal, |me, _, event, ctx| {
            me.handle_free_tier_limit_modal_event(event, ctx);
        });

        let require_login_modal = Self::build_require_login_modal(ctx);

        let auth_override_warning_modal = Self::build_auth_override_warning_modal(ctx);


        let theme_creator_modal = Self::build_theme_creator_modal(ctx);

        let theme_deletion_modal = Self::build_theme_deletion_modal(ctx);



        let oz_launch_view = ctx.add_typed_action_view(LaunchModal::<OzLaunchSlide>::new);
        ctx.subscribe_to_view(&oz_launch_view, |me, _, event, ctx| {
            me.handle_oz_launch_modal_event(event, ctx);
        });

        let openwarp_launch_view = ctx.add_typed_action_view(OpenWarpLaunchModal::new);
        ctx.subscribe_to_view(&openwarp_launch_view, |me, _, event, ctx| {
            me.handle_openwarp_launch_modal_event(event, ctx);
        });

        let orchestration_launch_view = ctx.add_typed_action_view(OrchestrationLaunchModal::new);
        ctx.subscribe_to_view(&orchestration_launch_view, |me, _, event, ctx| {
            me.handle_orchestration_launch_modal_event(event, ctx);
        });

        let launch_config_save_modal = Self::build_launch_config_save_modal(ctx);

        let tab_config_params_modal = Self::build_tab_config_params_modal(ctx);
        let new_worktree_modal = Self::build_new_worktree_modal(ctx);

        let session_config_modal = Self::build_session_config_modal(ctx);


        let close_session_confirmation_dialog = Self::build_close_session_confirmation_dialog(ctx);

        let working_directories_model =
            ctx.add_model(|_| pane_group::WorkingDirectoriesModel::new());

        let left_panel_views = Self::compute_left_panel_views(ctx);

        let left_panel_view = ctx.add_typed_action_view(|ctx| {
            LeftPanelView::new(
                working_directories_model.clone(),
                left_panel_views.clone(),
                ctx,
            )
        });

        ctx.subscribe_to_view(&left_panel_view, |me, _, event, ctx| {
            me.handle_left_panel_event(event, ctx);
        });

        ctx.observe(&tips_completed, Workspace::on_tips_model_changed);

        let autoupdate_handle = AutoupdateState::handle(ctx);
        ctx.subscribe_to_model(&autoupdate_handle, |_view, _handle, evt, ctx| {
            if let AutoupdateStateEvent::UpdateAvailable = evt {
                ctx.notify();
            }
        });

        ctx.subscribe_to_model(&CLIAgentSessionsModel::handle(ctx), |me, _, event, ctx| {
            me.handle_cli_agent_sessions_event(event, ctx);
        });

        ctx.subscribe_to_model(
            &SessionSettings::handle(ctx),
            Self::handle_session_settings_event,
        );

        ctx.subscribe_to_model(&WindowSettings::handle(ctx), |me, _handle, event, ctx| {
            me.handle_window_settings_changed_event(event, ctx);
        });

        // Show the Warp AI warm welcome iff the user hasn't dismissed it nor interacted with Warp AI before.
        // Also, avoid showing it in integration tests to prevent interaction with other tests.
        let mut should_show_ai_assistant_warm_welcome: bool = !FeatureFlag::AgentMode.is_enabled()
            && AISettings::as_ref(ctx).is_any_ai_enabled(ctx)
            && !matches!(ChannelState::channel(), Channel::Integration)
            && ctx
                .private_user_preferences()
                .read_value(settings::DISMISSED_AI_ASSISTANT_WELCOME_KEY)
                .unwrap_or_default()
                .and_then(|s| serde_json::from_str(&s).ok())
                .map(|dismissed: bool| !dismissed)
                .unwrap_or(true);

        // Don't automatically show the Warp AI welcome during onboarding if the block onboarding flow is being used.
        // This way, we can delay the reveal until the end of the onboarding flow so as not to overwhelm the user.
        if matches!(
            BlockOnboarding::get_group(ctx),
            Some(BlockOnboarding::VariantOne) | Some(BlockOnboarding::VariantTwo)
        ) {
            should_show_ai_assistant_warm_welcome = false;
        }

        let tab_settings_handle = TabSettings::handle(ctx);
        ctx.subscribe_to_model(&tab_settings_handle, |me, _, event, ctx| {
            me.handle_tab_settings_change(event, ctx)
        });

        ctx.subscribe_to_model(&CodeSettings::handle(ctx), |me, _, event, ctx| {
            if matches!(
                event,
                CodeSettingsChangedEvent::ShowProjectExplorer { .. }
                    | CodeSettingsChangedEvent::ShowGlobalSearch { .. }
            ) {
                me.update_left_panel_available_views(ctx);
                ctx.notify();
            }
        });

        let toast_stack =
            ctx.add_typed_action_view(|_| DismissibleToastStack::new(Duration::from_secs(4)));

        let agent_toast_stack =
            ctx.add_typed_action_view(|ctx| AgentToastStack::new(Duration::from_secs(4), ctx));

        let update_toast_stack =
            ctx.add_typed_action_view(|_| DismissibleToastStack::new(Duration::from_secs(4)));

        #[cfg(target_family = "wasm")]
        let wasm_nux_dialog = Self::build_wasm_nux_dialog(ctx);

        #[cfg(target_family = "wasm")]
        let open_in_warp_button = Self::build_open_in_warp_button(ctx);

        #[cfg(target_family = "wasm")]
        let transcript_info_button = Self::build_transcript_info_button(ctx);

        #[cfg(target_family = "wasm")]
        let view_cloud_runs_button = Self::build_view_cloud_runs_button(ctx);

        #[cfg(target_family = "wasm")]
        let transcript_details_panel = Self::build_transcript_details_panel(ctx);

        // Subscribe to task updates so the transcript details panel can refresh when task data arrives
        #[cfg(target_family = "wasm")]
        ctx.subscribe_to_model(
            &AgentConversationsModel::handle(ctx),
            |me, _, event, ctx| match event {
                // Update transcript details if task or conversation data is updated
                AgentConversationsModelEvent::NewTasksReceived
                | AgentConversationsModelEvent::TasksUpdated
                | AgentConversationsModelEvent::ConversationUpdated { .. }
                | AgentConversationsModelEvent::ConversationArtifactsUpdated { .. } => {
                    me.update_transcript_details_panel_data(ctx);
                }
                _ => {}
            },
        );

        let cached_keybindings = KEYBINDINGS_TO_CACHE
            .iter()
            .map(|name| {
                (
                    String::from(*name),
                    keybinding_name_to_display_string(name, ctx),
                )
            })
            .collect();

        let prompt_editor_modal = Self::build_prompt_editor_modal(ctx);


        Self::observe_server_api(ctx);

        Self::subscribe_to_workspace_toast_stack(toast_stack.clone(), ctx);
        Self::subscribe_to_tab_config_errors(toast_stack.clone(), ctx);
        Self::subscribe_to_settings_errors(ctx);
        Self::subscribe_to_shared_session_manager(ctx);

        let user_menu = ctx.add_typed_action_view(|_| {
            Menu::new()
                .with_drop_shadow()
                .prevent_interaction_with_other_elements()
        });
        ctx.subscribe_to_view(&user_menu, |me, _, event, ctx| {
            if let MenuEvent::Close { .. } = event {
                me.is_user_menu_open = false;
                ctx.notify();
            }
        });

        let native_modal = Self::build_native_modal_view(ctx);

        ctx.subscribe_to_model(&AISettings::handle(ctx), |me, _, event, ctx| match event {
            AISettingsChangedEvent::IsAnyAIEnabled { .. }
            | AISettingsChangedEvent::ShowConversationHistory { .. } => {
                me.update_left_panel_available_views(ctx);
                ctx.notify();
            }
            AISettingsChangedEvent::IsActiveAIEnabled { .. }
            | AISettingsChangedEvent::ThinkingDisplayMode { .. }
            | AISettingsChangedEvent::PromptSubmissionMode { .. } => {
                ctx.notify();
            }
            AISettingsChangedEvent::ShowAgentNotifications { .. } => {
                // When agent notifications are turned off, close the mailbox if it's open.
                if !*AISettings::as_ref(ctx).show_agent_notifications {
                    me.current_workspace_state.is_notification_mailbox_open = false;
                }
                ctx.notify();
            }
            _ => (),
        });

        ctx.subscribe_to_model(&OneTimeModalModel::handle(ctx), |me, model, event, ctx| {
            let OneTimeModalEvent::VisibilityChanged { is_open } = event;
            if *is_open {
                // Only trigger modal actions if this is the target window.
                // The model has already determined which window should show the modal.
                let model_ref = model.as_ref(ctx);
                if model_ref.target_window_id() == Some(ctx.window_id()) {
                    if model_ref.is_oz_launch_modal_open() {
                        me.open_tab_and_focus_oz_launch_modal(ctx);
                    } else if model_ref.is_openwarp_launch_modal_open() {
                        me.focus_openwarp_launch_modal(ctx);
                    } else if model_ref.is_orchestration_launch_modal_open() {
                        me.focus_orchestration_launch_modal(ctx);
                    } else if model_ref.is_hoa_onboarding_open() {
                        me.show_hoa_onboarding_flow(ctx);
                    } else if model_ref.is_build_plan_migration_modal_open() {
                        me.focus_build_plan_migration_modal(ctx);
                    }
                }
            }
            ctx.notify();
        });

        let mut ws = Self {
            tabs: Vec::new(),
            active_tab_index: 0,
            tab_mru_order: Vec::new(),
            hovered_tab_index: None,
            tab_bar_hover_state: Default::default(),
            traffic_light_mouse_states: Default::default(),
            tab_groups: HashMap::new(),
            tab_rename_editor: Self::tab_rename_editor(ctx),
            pane_rename_editor: Self::pane_rename_editor(ctx),
            tab_group_rename_editor: Self::tab_group_rename_editor(ctx),
            vertical_tabs_search_input: Self::vertical_tabs_search_input(ctx),
            tips_completed,
            user_default_shell_unsupported_banner_model_handle,
            server_api,
            auth_state: AuthStateProvider::as_ref(ctx).get().clone(),
            server_time,
            tab_bar_overflow_menu,
            show_tab_bar_overflow_menu: false,
            tab_right_click_menu,
            show_tab_right_click_menu: None,
            show_tab_group_right_click_menu: None,
            new_session_dropdown_menu,
            show_new_session_dropdown_menu: None,
            changelog_model,
            welcome_tips_view_state,
            welcome_tips_view,
            palette,
            ctrl_tab_palette,
            mouse_states: Default::default(),
            previous_theme: None,
            settings_pane,
            theme_chooser_view,
            current_workspace_state: Default::default(),
            previous_workspace_state: None,
            model_event_sender,
            launch_config_save_modal,
            tab_config_params_modal,
            session_config_modal,
            pending_session_config_replacement: None,
            pending_onboarding_intention: None,
            pending_session_config_tab_config_chip: false,
            show_session_config_tab_config_chip: false,
            pending_session_config_tab_config_chip_tutorial: None,
            new_worktree_modal,
            close_session_confirmation_dialog,
            resource_center_view,
            autoupdate_unable_to_update_banner_dismissed: false,
            autoupdate_unable_to_launch_new_version: false,
            reauth_banner_dismissed: false,
            settings_file_error,
            settings_error_banner_dismissed: false,
            should_show_ai_assistant_warm_welcome,
            ai_assistant_close_warm_welcome_mouse_state_handle: Default::default(),
            auth_override_warning_modal,
            build_plan_migration_modal,
            require_login_modal,
            theme_creator_modal,
            theme_deletion_modal,
            window_id: ctx.window_id(),
            toast_stack,
            agent_toast_stack,
            update_toast_stack,
            cached_keybindings,
            prompt_editor_modal,
            header_toolbar_editor_modal: Self::build_header_toolbar_editor_modal(ctx),
            header_toolbar_context_menu: Self::build_header_toolbar_context_menu(ctx),
            show_header_toolbar_context_menu: None,
            is_user_menu_open: false,
            tab_bar_pinned_by_popup: false,
            user_menu,
            native_modal,
            file_upload_sessions: Default::default(),
            left_panel_open: false,
            vertical_tabs_panel_open: false,
            vertical_tabs_panel: Default::default(),
            left_panel_view,
            left_panel_views,
            working_directories_model,
            shown_staging_banner_count: 0,

            #[cfg(target_family = "wasm")]
            show_wasm_nux_dialog: WasmNUXDialog::should_display(ctx),
            #[cfg(target_family = "wasm")]
            wasm_nux_dialog,
            #[cfg(target_family = "wasm")]
            open_in_warp_button,
            #[cfg(target_family = "wasm")]
            transcript_info_button,
            #[cfg(target_family = "wasm")]
            view_cloud_runs_button,
            #[cfg(target_family = "wasm")]
            transcript_details_panel,
            tab_fixed_width: None,
            oz_launch_modal: ModalWithTab {
                view: oz_launch_view,
                tab_pane_group_id: None,
            },
            openwarp_launch_modal: openwarp_launch_view,
            orchestration_launch_modal: orchestration_launch_view,
            codex_modal,
            cloud_agent_capacity_modal,
            free_tier_limit_hit_modal,
            free_tier_limit_check_triggered: false,
            lightbox_view: None,
            hoa_onboarding_flow: None,
            hoa_vtabs_callout_pinned_position: None,
            pending_pane_group_transfer: false,
            suppress_detach_panes_on_window_close: false,
            is_tab_drag_preview: false,
            new_session_sidecar_menu,
            show_new_session_sidecar: false,
            worktree_sidecar_active: false,
            worktree_sidecar_search_editor: Self::build_worktree_sidecar_search_input(ctx),
            worktree_sidecar_search_query: String::new(),
            new_session_sidecar_add_repo_mouse_state: Default::default(),
            move_to_group_sidecar_menu,
            show_move_to_group_sidecar: false,
            tab_config_action_sidecar_item: None,
            tab_config_action_sidecar_mouse_states: Default::default(),
            remove_tab_config_confirmation_dialog:
                Self::build_remove_tab_config_confirmation_dialog(ctx),
        };

        ws.configure_new_workspace(workspace_setting, ctx);
        ws.sync_panel_positions_from_config(ctx);
        ws.sync_window_button_visibility(ctx);
        ws.update_titlebar_height(ctx);
        // Seed the settings pane with the initial settings-file error (if
        // any) read from `GlobalResourceHandles`. Subsequent updates are
        // pushed by `subscribe_to_settings_errors` and `dismiss_workspace_banner`.
        ws.sync_settings_error_state_into_settings_pane(ctx);

        let weak_handle = ctx.handle();
        WorkspaceRegistry::handle(ctx).update(ctx, |registry, _| {
            registry.register(window_id, weak_handle);
        });

        ws
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub fn command_palette_view(&self) -> ViewHandle<crate::search::command_palette::View> {
        self.palette.clone()
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub fn ai_fact_view(&self) -> ViewHandle<AIFactView> {
        self.ai_fact_view.clone()
    }

    fn handle_task_status_reset(&mut self, pane_group_id: EntityId, ctx: &mut ViewContext<Self>) {
        // Re-render the workspace so the tab indicator picks up the new state.
        let has_tab = self
            .tabs
            .iter()
            .any(|tab| tab.pane_group.id() == pane_group_id);
        if has_tab {
            ctx.notify();
        }
    }



    fn workspace_contains_terminal_view(
        &self,
        terminal_view_id: EntityId,
        ctx: &AppContext,
    ) -> bool {
        self.tabs.iter().any(|tab| {
            tab.pane_group
                .as_ref(ctx)
                .contains_terminal_view(terminal_view_id, ctx)
        })
    }


    fn handle_cli_agent_sessions_event(
        &mut self,
        event: &CLIAgentSessionsModelEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if matches!(
            event,
            CLIAgentSessionsModelEvent::Started { .. }
                | CLIAgentSessionsModelEvent::StatusChanged { .. }
                | CLIAgentSessionsModelEvent::Ended { .. }
                | CLIAgentSessionsModelEvent::SessionUpdated { .. }
        ) && self.workspace_contains_terminal_view(event.terminal_view_id(), ctx)
        {
            ctx.notify();
        }
    }

    /// Handle session settings changes.
    fn handle_session_settings_event(
        &mut self,
        session_settings: ModelHandle<SessionSettings>,
        event: &SessionSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if let SessionSettingsChangedEvent::HonorPS1 { .. } = event {
            let honor_ps1 = *session_settings.as_ref(ctx).honor_ps1;
            for tab in &self.tabs {
                // Each tab has a pane group.
                tab.pane_group.update(ctx, |pane_group, ctx| {
                    pane_group.send_prompt_change_bindkey_to_all_sessions(honor_ps1, ctx);
                });
            }
        }

        // When Notifications settings change, request system notification permissions if needed.
        if let SessionSettingsChangedEvent::Notifications { .. } = event {
            self.request_notification_permissions_if_needed(ctx);
        }
    }

    /// Handle a change to the tab settings.
    fn handle_tab_settings_change(
        &mut self,
        event: &TabSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            TabSettingsChangedEvent::WorkspaceDecorationVisibility { .. } => {
                self.sync_window_button_visibility(ctx);
                ctx.notify();
            }
            TabSettingsChangedEvent::ShowIndicatorsButton { .. }
            | TabSettingsChangedEvent::NewTabPlacement { .. }
            | TabSettingsChangedEvent::TabCloseButtonPosition { .. }
            | TabSettingsChangedEvent::PreserveActiveTabColor { .. } => {
                self.sync_window_button_visibility(ctx);
                ctx.notify();
            }
            TabSettingsChangedEvent::UseVerticalTabs { .. } => {
                let vertical_tabs_enabled = *TabSettings::as_ref(ctx).use_vertical_tabs;
                // During HOA onboarding, keep the vertical tabs panel open
                // regardless of the setting so the callout stays anchored.
                if self.hoa_onboarding_flow.is_none() {
                    self.vertical_tabs_panel_open = vertical_tabs_enabled;
                }

                if vertical_tabs_enabled {
                    Self::ensure_tabs_panel_in_config(ctx);
                }

                let appearance = Appearance::as_ref(ctx);
                let font_family = appearance.ui_font_family();
                let font_size = Self::tab_rename_editor_font_size(ctx, appearance);
                self.tab_rename_editor.update(ctx, |editor, ctx| {
                    editor.set_font_family(font_family, ctx);
                    editor.set_font_size(font_size, ctx);
                });
                if !vertical_tabs_enabled {
                    self.close_vertical_tabs_settings_popup();
                }
                self.sync_panel_positions_from_config(ctx);
                self.sync_window_button_visibility(ctx);
                ctx.notify();
            }
            TabSettingsChangedEvent::ShowVerticalTabPanelInRestoredWindows { .. } => {
                if FeatureFlag::VerticalTabs.is_enabled()
                    && *TabSettings::as_ref(ctx).use_vertical_tabs
                    && *TabSettings::as_ref(ctx).show_vertical_tab_panel_in_restored_windows
                {
                    self.vertical_tabs_panel_open = true;
                }
                ctx.notify();
            }
            TabSettingsChangedEvent::ShowCodeReviewButton { .. } => {
                ctx.notify();
            }
            TabSettingsChangedEvent::ShowCodeReviewDiffStats { .. } => {
                ctx.notify();
            }
            TabSettingsChangedEvent::DirectoryTabColors { .. } => {
                if FeatureFlag::DirectoryTabColors.is_enabled() {
                    for tab in &mut self.tabs {
                        Self::sync_codebase_tab_color(tab, ctx);
                    }
                }
                ctx.notify();
            }
            TabSettingsChangedEvent::VerticalTabsViewMode { .. }
            | TabSettingsChangedEvent::VerticalTabsTabItemMode { .. }
            | TabSettingsChangedEvent::VerticalTabsPrimaryInfo { .. }
            | TabSettingsChangedEvent::VerticalTabsCompactSubtitle { .. }
            | TabSettingsChangedEvent::UseLatestUserPromptAsConversationTitleInTabNames {
                ..
            }
            | TabSettingsChangedEvent::VerticalTabsShowPrLink { .. }
            | TabSettingsChangedEvent::VerticalTabsShowDiffStats { .. } => {
                ctx.notify();
            }
            TabSettingsChangedEvent::VerticalTabsShowDetailsOnHover { .. } => {
                if !*TabSettings::as_ref(ctx).vertical_tabs_show_details_on_hover {
                    self.vertical_tabs_panel.clear_detail_sidecar();
                }
                ctx.notify();
            }
            TabSettingsChangedEvent::VerticalTabsDisplayGranularity { .. } => {
                let appearance = Appearance::as_ref(ctx);
                let font_size = Self::tab_rename_editor_font_size(ctx, appearance);
                self.tab_rename_editor.update(ctx, |editor, ctx| {
                    editor.set_font_size(font_size, ctx);
                });
                ctx.notify();
            }
            TabSettingsChangedEvent::HeaderToolbarChipSelection { .. } => {
                self.sync_panel_positions_from_config(ctx);
                ctx.notify();
            }
        }
    }

    /// Opens a launch config window into the workspace.
    pub fn open_launch_config_window(
        &mut self,
        window: WindowTemplate,
        ctx: &mut ViewContext<Self>,
    ) {
        let start_index = self.tabs.len();

        window
            .tabs
            .iter()
            .enumerate()
            .for_each(|(tab_index, tab_template)| {
                self.add_tab_with_pane_layout(
                    PanesLayout::Template(tab_template.layout.clone()),
                    Arc::new(HashMap::new()),
                    tab_template.title.clone(),
                    ctx,
                );
                self.tabs[start_index + tab_index].selected_color = tab_template
                    .color
                    .map_or(SelectedTabColor::Unset, SelectedTabColor::Color);
            });

        if !window.tabs.is_empty() {
            // Focus the active tab from the launch config.

            let mut index = start_index + window.active_tab_index.unwrap_or_default();

            if index >= self.tab_count() {
                index = start_index;
            }

            self.activate_tab_internal(index, ctx);
        }
    }

    fn configure_new_workspace(
        &mut self,
        workspace_setting: NewWorkspaceSource,
        ctx: &mut ViewContext<Self>,
    ) {
        self.vertical_tabs_panel_open =
            Self::initial_vertical_tabs_panel_open(&workspace_setting, ctx);
        match workspace_setting {
            NewWorkspaceSource::Empty {
                previous_active_window,
                shell,
            } => {
                self.configure_empty_workspace(previous_active_window, shell, ctx);
            }
            NewWorkspaceSource::Restored {
                window_snapshot,
                block_lists,
            } => {
                let active_tab_index = window_snapshot.active_tab_index;
                let restored_left_panel_open = window_snapshot.left_panel_open;

                window_snapshot
                    .tabs
                    .iter()
                    .enumerate()
                    .for_each(|(tab_index, saved_tab)| {
                        let custom_title = saved_tab.custom_title.clone();
                        self.add_tab_with_pane_layout(
                            PanesLayout::Snapshot(Box::new(saved_tab.root.clone())),
                            block_lists.clone(),
                            custom_title,
                            ctx,
                        );
                        self.tabs[tab_index].default_directory_color =
                            saved_tab.default_directory_color;
                        self.tabs[tab_index].selected_color = saved_tab.selected_color;

                        let pane_group = self.tabs[tab_index].pane_group.clone();

                        if let Some(left_panel_snapshot) = &saved_tab.left_panel {
                            self.restore_left_panel_for_tab(&pane_group, left_panel_snapshot, ctx);
                        }

                        if let Some(right_panel_snapshot) = &saved_tab.right_panel {
                            self.restore_right_panel_for_tab(
                                &pane_group,
                                right_panel_snapshot,
                                ctx,
                            );
                        }
                    });

                if self.tab_count() == 0 {
                    if self.should_trigger_get_started_onboarding(ctx) {
                        self.trigger_get_started_onboarding(ctx);
                        return;
                    }
                    // If we still haven't created any tabs after attempting to restore, create a new tab
                    // with sensible defaults.
                    self.add_new_session_tab_with_default_mode(
                        NewSessionSource::Window,
                        None,  /* previous_active_window */
                        None,  /* chosen_shell */
                        false, /* hide_homepage */
                        ctx,
                    );
                } else if self.left_panel_visibility_across_tabs_enabled(ctx) {
                    self.left_panel_open = restored_left_panel_open;
                }

                self.activate_tab_internal(active_tab_index, ctx);
                self.check_and_trigger_onboarding(ctx);
            }
            NewWorkspaceSource::FromTemplate { window_template } => {
                self.open_launch_config_window(window_template, ctx);
                self.check_and_trigger_onboarding(ctx);
            }
            NewWorkspaceSource::Session { options } => {
                self.add_tab_with_pane_layout(
                    PanesLayout::SingleTerminal(options),
                    Arc::new(HashMap::new()),
                    None,
                    ctx,
                );
                self.check_and_trigger_onboarding(ctx);
            }
            NewWorkspaceSource::SharedSessionAsViewer { session_id } => {
                self.add_tab_for_joining_shared_session(session_id, ctx);
            }
            NewWorkspaceSource::FromCloudConversationId { conversation_id } => {
                self.open_cloud_conversation_from_server_token(conversation_id, ctx);
            }
            NewWorkspaceSource::AgentSession {
                options,
                initial_query,
            } => {
                self.add_tab_with_pane_layout(
                    PanesLayout::SingleTerminal(options),
                    Arc::new(HashMap::new()),
                    None,
                    ctx,
                );
                // Enter agent mode with the environment creation query.
                self.active_tab_pane_group().update(ctx, |pane_group, ctx| {
                    pane_group.start_agent_mode_in_new_pane(initial_query.as_deref(), None, ctx);
                });
                self.check_and_trigger_onboarding(ctx);
            }
            NewWorkspaceSource::AmbientAgent => {
                self.add_tab_with_pane_layout(
                    PanesLayout::AmbientAgent,
                    Arc::new(HashMap::new()),
                    None,
                    ctx,
                );
                self.check_and_trigger_onboarding(ctx);
            }
            NewWorkspaceSource::NotebookFromFilePath { file_path } => {
                self.add_tab_for_file_notebook(file_path, ctx);
            }
            NewWorkspaceSource::NotebookById { id, settings } => {
                self.add_tab_for_cloud_notebook(id, &settings, ctx);
            }
            NewWorkspaceSource::WorkflowById { id, settings } => {
                self.open_workflow_from_intent(id, &settings, ctx);
            }
            #[cfg(feature = "local_fs")]
            NewWorkspaceSource::TransferredTab {
                tab_color,
                custom_title,
                left_panel_open,
                right_panel_open,
                is_right_panel_maximized,
                is_tab_drag_preview,
                ..
            } => {
                self.set_is_tab_drag_preview(is_tab_drag_preview);
                self.add_tab_with_pane_layout(
                    Default::default(),
                    Arc::new(HashMap::new()),
                    custom_title,
                    ctx,
                );
                if let (Some(color), Some(tab)) = (tab_color, self.tabs.last_mut()) {
                    tab.selected_color = SelectedTabColor::Color(color);
                }
                if self.left_panel_visibility_across_tabs_enabled(ctx) {
                    self.left_panel_open = left_panel_open;
                }
                let _ = (right_panel_open, is_right_panel_maximized);
                self.pending_pane_group_transfer = true;
            }
            #[cfg(not(feature = "local_fs"))]
            NewWorkspaceSource::TransferredTab {
                tab_color,
                custom_title,
                left_panel_open,
                is_tab_drag_preview,
                ..
            } => {
                self.set_is_tab_drag_preview(is_tab_drag_preview);
                self.add_tab_with_pane_layout(
                    Default::default(),
                    Arc::new(HashMap::new()),
                    custom_title,
                    ctx,
                );
                if let (Some(color), Some(tab)) = (tab_color, self.tabs.last_mut()) {
                    tab.selected_color = SelectedTabColor::Color(color);
                }
                if self.left_panel_visibility_across_tabs_enabled(ctx) {
                    self.left_panel_open = left_panel_open;
                }
                self.pending_pane_group_transfer = true;
            }
        };

        debug_assert!(
            self.tab_count() > 0,
            "Workspace should have at least one tab upon configuration"
        );

        if self.left_panel_visibility_across_tabs_enabled(ctx) {
            self.reconcile_left_panel_open_for_active_tab(ctx);
        }

        let active_pane_group = self.active_tab_pane_group().clone();
        let working_directories_model = self.working_directories_model.clone();
        self.left_panel_view.update(ctx, |left_panel, ctx| {
            left_panel.set_active_pane_group(active_pane_group, &working_directories_model, ctx);
        });
    }

    fn initial_vertical_tabs_panel_open(
        workspace_setting: &NewWorkspaceSource,
        ctx: &AppContext,
    ) -> bool {
        let should_default_open =
            FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(ctx).use_vertical_tabs;

        match workspace_setting {
            NewWorkspaceSource::Restored {
                window_snapshot, ..
            } => {
                if !should_default_open {
                    // Stale "panel open" snapshot would leave a click-eating dismiss underlay (#9505).
                    false
                } else if *TabSettings::as_ref(ctx).show_vertical_tab_panel_in_restored_windows {
                    true
                } else {
                    window_snapshot.vertical_tabs_panel_open
                }
            }
            NewWorkspaceSource::TransferredTab {
                vertical_tabs_panel_open,
                ..
            } => *vertical_tabs_panel_open,
            NewWorkspaceSource::Empty { .. }
            | NewWorkspaceSource::FromTemplate { .. }
            | NewWorkspaceSource::Session { .. }
            | NewWorkspaceSource::AgentSession { .. }
            | NewWorkspaceSource::AmbientAgent
            | NewWorkspaceSource::NotebookFromFilePath { .. } => should_default_open,
            #[cfg(not(target_family = "wasm"))]
            NewWorkspaceSource::SharedSessionAsViewer { .. }
            | NewWorkspaceSource::FromCloudConversationId { .. }
            | NewWorkspaceSource::NotebookById { .. }
            | NewWorkspaceSource::WorkflowById { .. } => should_default_open,
            #[cfg(target_family = "wasm")]
            NewWorkspaceSource::SharedSessionAsViewer { .. }
            | NewWorkspaceSource::FromCloudConversationId { .. }
            | NewWorkspaceSource::NotebookById { .. }
            | NewWorkspaceSource::WorkflowById { .. } => {
                // Web opens these as single-purpose views without exposed multi-tab UI, so keep
                // the tabs panel closed even though native windows still expose workspace chrome.
                false
            }
        }
    }

    fn restore_left_panel_for_tab(
        &mut self,
        pane_group: &ViewHandle<PaneGroup>,
        left_panel_snapshot: &LeftPanelSnapshot,
        ctx: &mut ViewContext<Self>,
    ) {
        pane_group.update(ctx, |pg, ctx| {
            pg.set_left_panel_open(true, ctx);
        });

        let resizable = ResizableData::handle(ctx);
        if let Some(modal_sizes) = resizable.as_ref(ctx).get_all_handles(self.window_id) {
            if let Ok(mut handle) = modal_sizes.left_panel_width.lock() {
                handle.set_size(left_panel_snapshot.width as f32);
            }
        }

        self.left_panel_view.update(ctx, |lp, ctx| {
            // Restore which panel tab was active
            let active_view = match left_panel_snapshot.left_panel_displayed_tab {
                LeftPanelDisplayedTab::FileTree => ToolPanelView::ProjectExplorer,
                LeftPanelDisplayedTab::GlobalSearch => ToolPanelView::GlobalSearch {
                    entry_focus: GlobalSearchEntryFocus::Results,
                },
                LeftPanelDisplayedTab::WarpDrive => ToolPanelView::WarpDrive,
                LeftPanelDisplayedTab::ConversationListView => ToolPanelView::ConversationListView,
            };
            lp.restore_active_view_from_snapshot(active_view, ctx);
            lp.set_active_pane_group(pane_group.clone(), &self.working_directories_model, ctx);
        });

        ctx.notify();
    }

    fn restore_right_panel_for_tab(
        &mut self,
        pane_group: &ViewHandle<PaneGroup>,
        right_panel_snapshot: &RightPanelSnapshot,
        ctx: &mut ViewContext<Self>,
    ) {
        pane_group.update(ctx, |pg, _| {
            pg.right_panel_open = true;
            pg.is_right_panel_maximized = right_panel_snapshot.is_maximized;
        });

        let resizable = ResizableData::handle(ctx);
        if let Some(modal_sizes) = resizable.as_ref(ctx).get_all_handles(self.window_id) {
            if let Ok(mut handle) = modal_sizes.right_panel_width.lock() {
                handle.set_size(right_panel_snapshot.width as f32);
            }
        }

        let _ = pane_group;
        ctx.notify();
    }

    // Configure an empty workspace. The behavior here is platform-specific.
    fn configure_empty_workspace(
        &mut self,
        previous_active_window: Option<WindowId>,
        shell: Option<AvailableShell>,
        ctx: &mut ViewContext<Self>,
    ) {
        // Warp Home / Warp Drive were removed; only the session-creation path remains.
        if !ContextFlag::CreateNewSession.is_enabled() {
            if self.should_trigger_get_started_onboarding(ctx) {
                self.trigger_get_started_onboarding(ctx);
            } else if FeatureFlag::WelcomeTab.is_enabled() {
                self.add_welcome_tab(ctx);
            } else {
                self.add_new_session_tab_with_default_mode(
                    NewSessionSource::Window,
                    previous_active_window,
                    shell,
                    false, /* hide_homepage */
                    ctx,
                );
                self.check_and_trigger_onboarding(ctx);
            }
        }
    }

    pub fn add_tab_for_joining_shared_session(
        &mut self,
        session_id: SharedSessionId,
        ctx: &mut ViewContext<Self>,
    ) {
        let new_pane_group = ctx.add_typed_action_view(|ctx| {
            PaneGroup::new_for_shared_session_viewer(
                session_id,
                self.tips_completed.clone(),
                self.user_default_shell_unsupported_banner_model_handle
                    .clone(),
                self.server_api.clone(),
                self.model_event_sender.clone(),
                ctx,
            )
        });

        ctx.subscribe_to_view(&new_pane_group, move |me, pane_group, event, ctx| {
            me.handle_file_tree_event(pane_group, event, ctx)
        });

        self.tabs.push(TabData::new(new_pane_group));
        self.activate_tab_internal(self.tab_count() - 1, ctx);
    }









    pub fn is_conversation_transcript_viewer_focused(&self, app: &AppContext) -> bool {
        self.active_tab_pane_group()
            .as_ref(app)
            .active_session_view(app)
            .is_some_and(|view| {
                view.as_ref(app)
                    .model
                    .lock()
                    .is_conversation_transcript_viewer()
            })
    }

    /// Returns the type of simplified WASM tab bar content to display, if any.
    /// Used to determine whether to show the simplified tab bar layout on WASM.
    #[cfg(target_family = "wasm")]
    fn get_simplified_wasm_tab_bar_content(
        &self,
        ctx: &AppContext,
    ) -> Option<SimplifiedWasmTabBarContent> {
        let pane_group = self.active_tab_pane_group().as_ref(ctx);

        // Check if focused pane is a terminal with special state
        if let Some(terminal_view) = pane_group.focused_session_view(ctx) {
            let model = terminal_view.as_ref(ctx).model.lock();

            // Conversation transcript viewer takes priority
            if model.is_conversation_transcript_viewer() {
                return Some(SimplifiedWasmTabBarContent::ConversationTranscript {
                    task_id: model.ambient_agent_task_id(),
                });
            }

            // Check for shared session (viewer or writer)
            if model.shared_session_status().is_sharer_or_viewer() {
                return Some(SimplifiedWasmTabBarContent::SharedSession {
                    task_id: model.ambient_agent_task_id(),
                });
            }
        }

        // Check if focused pane is a Warp Drive object
        let focused_pane_id = pane_group.focused_pane_id(ctx);
        if focused_pane_id.is_warp_drive_object_pane() {
            return Some(SimplifiedWasmTabBarContent::WarpDriveObject);
        }

        None
    }

    pub fn check_for_changelog(
        &self,
        request_type: ChangelogRequestType,
        ctx: &mut ViewContext<Self>,
    ) {
        self.changelog_model.update(ctx, |changelog_model, ctx| {
            changelog_model.check_for_changelog(request_type, ctx);
            ctx.notify();
        });
    }

    fn dismiss_ai_assistant_warm_welcome(&mut self, ctx: &mut ViewContext<Self>) {
        self.should_show_ai_assistant_warm_welcome = false;
        let _ = ctx.private_user_preferences().write_value(
            settings::DISMISSED_AI_ASSISTANT_WELCOME_KEY,
            true.to_string(),
        );
        ctx.notify();
    }



    fn toggle_ai_assistant_panel(&mut self, ctx: &mut ViewContext<Self>) {
        // Now that the user has interacted with the panel, we can close
        // the dialogue and mark it as dismissed.
        if self.should_show_ai_assistant_warm_welcome {
            self.dismiss_ai_assistant_warm_welcome(ctx);
        }

        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::WarpAI),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        // The panel is already open and no models are open, so just refocus the panel.
        // If there is a modal open, it would sit above the Warp AI panel and we would end up
        // focusing the Warp AI panel _behind_ the floating modal. Instead, we opt for the normal
        // toggle behavior which will close the current modal view and then toggle Warp AI.
        if self.current_workspace_state.is_ai_assistant_panel_open
            && !self.ai_assistant_panel.is_self_or_child_focused(ctx)
            && !self.current_workspace_state.is_any_modal_open(ctx)
        {
            ctx.focus(&self.ai_assistant_panel);
            return;
        }

        // Otherwise, open / close the panel accordingly.
        self.current_workspace_state.is_ai_assistant_panel_open =
            !self.current_workspace_state.is_ai_assistant_panel_open;

        // Close any other modals that could be floating on top of the Warp AI panel.
        self.current_workspace_state.close_all_modals();

        if self.current_workspace_state.is_ai_assistant_panel_open {
            // Close the resource center panel if we open the AI Assistant panel.
            self.current_workspace_state.is_resource_center_open = false;
            ctx.focus(&self.ai_assistant_panel);
        } else {
            self.focus_active_tab(ctx);
        }
        ctx.notify();
    }

    /// Sets focused to the index of either the selected object or the first item in WD
    fn reset_focused_index_in_warp_drive(
        &mut self,
        should_scroll: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.focus(&self.left_panel_view);

        self.update_warp_drive_view(ctx, |drive_panel, ctx| {
            drive_panel.reset_focused_index_in_warp_drive(should_scroll, ctx);
        });
    }

    pub fn has_warp_drive_initialized_sections(
        &self,
        app: &AppContext,
    ) -> impl Future<Output = ()> {
        self.left_panel_view
            .as_ref(app)
            .warp_drive_view()
            .as_ref(app)
            .has_warp_drive_initialized_sections(app)
    }

    /// Check if Warp Drive view is focused within.
    /// Routes to the appropriate Warp Drive panel.
    fn is_warp_drive_view_focused(&self, ctx: &mut ViewContext<Self>) -> bool {
        let app = ctx;
        self.left_panel_view.is_self_or_child_focused(app)
    }

    fn current_focus_region(&self, ctx: &mut ViewContext<Self>) -> FocusRegion {
        let app = ctx;
        if self.active_tab_pane_group().is_self_or_child_focused(app) {
            return FocusRegion::PaneGroup;
        }

        if self.left_panel_view.is_self_or_child_focused(app) {
            return FocusRegion::LeftPanel;
        }

        if self.ai_assistant_panel.is_self_or_child_focused(app)
            || self.resource_center_view.is_self_or_child_focused(app)
        {
            return FocusRegion::RightPanel;
        }

        FocusRegion::Other
    }

    fn has_left_region(&self, app: &AppContext) -> bool {
        self.active_tab_pane_group().as_ref(app).left_panel_open
    }

    fn has_right_region(&self, app: &AppContext) -> bool {
        let group = self.active_tab_pane_group().as_ref(app);
        group.right_panel_open || self.current_workspace_state.is_right_panel_open()
    }

    fn focus_next_pane_in_group(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| pane_group.try_navigate_next(ctx))
    }

    fn focus_prev_pane_in_group(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| pane_group.try_navigate_prev(ctx))
    }

    fn focus_first_visible_pane_in_group(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| pane_group.focus_first_pane(ctx))
    }

    fn focus_last_visible_pane_in_group(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| pane_group.focus_last_pane(ctx))
    }

    fn focus_left_region_entry(&mut self, ctx: &mut ViewContext<Self>) {
        if self.has_left_region(ctx) {
            self.left_panel_view.update(ctx, |left_panel, ctx| {
                left_panel.focus_active_view_on_entry(ctx);
            });
        }
    }

    fn focus_right_region_entry(&mut self, ctx: &mut ViewContext<Self>) {
        if self.current_workspace_state.is_ai_assistant_panel_open {
            ctx.focus(&self.ai_assistant_panel);
        } else if self.current_workspace_state.is_resource_center_open {
            ctx.focus(&self.resource_center_view);
        }
    }

    fn navigate_pane_or_panel(
        &mut self,
        direction: PanePanelDirection,
        ctx: &mut ViewContext<Self>,
    ) {
        let current_region = self.current_focus_region(ctx);
        let has_left_panel = self.has_left_region(ctx);
        let has_right_panel = self.has_right_region(ctx);

        let target_region = self.compute_target_focus_region(
            current_region,
            direction,
            has_left_panel,
            has_right_panel,
            ctx,
        );

        self.set_pane_dimming_for_region(target_region, ctx);

        ctx.notify();
    }

    fn compute_target_focus_region(
        &mut self,
        region: FocusRegion,
        direction: PanePanelDirection,
        has_left_panel: bool,
        has_right_panel: bool,
        ctx: &mut ViewContext<Self>,
    ) -> FocusRegion {
        match (region, direction) {
            // NEXT: Left panel to first pane
            (FocusRegion::LeftPanel, PanePanelDirection::Next) => {
                // Always attempt to focus the first pane in the group and ensure the pane group
                // regains application focus.
                self.focus_first_visible_pane_in_group(ctx);
                self.focus_active_tab(ctx);
                FocusRegion::PaneGroup
            }
            // NEXT: Right panel to left panel if open, else first pane
            (FocusRegion::RightPanel, PanePanelDirection::Next) => {
                if has_left_panel {
                    self.focus_left_region_entry(ctx);
                    FocusRegion::LeftPanel
                } else {
                    self.focus_first_visible_pane_in_group(ctx);
                    FocusRegion::PaneGroup
                }
            }
            // NEXT: Pane group to next pane, or at end to right panel, left panel, first pane
            // Included Other here for cases like the command palette action "Activate next Pane"
            (FocusRegion::PaneGroup, PanePanelDirection::Next)
            | (FocusRegion::Other, PanePanelDirection::Next) => {
                let moved = self.focus_next_pane_in_group(ctx);
                if moved {
                    FocusRegion::PaneGroup
                } else if has_right_panel {
                    self.focus_right_region_entry(ctx);
                    FocusRegion::RightPanel
                } else if has_left_panel {
                    self.focus_left_region_entry(ctx);
                    FocusRegion::LeftPanel
                } else {
                    // No panels, wrap within panes.
                    self.focus_first_visible_pane_in_group(ctx);
                    FocusRegion::PaneGroup
                }
            }

            // PREV: Right panel to last pane
            (FocusRegion::RightPanel, PanePanelDirection::Prev) => {
                // Always attempt to focus the last pane in the group and ensure the pane group
                // regains application focus.
                self.focus_last_visible_pane_in_group(ctx);
                self.focus_active_tab(ctx);
                FocusRegion::PaneGroup
            }
            // PREV: Left panel to right panel if open, else last pane
            (FocusRegion::LeftPanel, PanePanelDirection::Prev) => {
                if has_right_panel {
                    self.focus_right_region_entry(ctx);
                    FocusRegion::RightPanel
                } else {
                    self.focus_last_visible_pane_in_group(ctx);
                    FocusRegion::PaneGroup
                }
            }
            // PREV: Pane group to prev pane, or at beginning to left panel to right panel to last pane
            // Included Other here for cases like the command palette action "Activate next Pane"
            (FocusRegion::PaneGroup, PanePanelDirection::Prev)
            | (FocusRegion::Other, PanePanelDirection::Prev) => {
                let did_move = self.focus_prev_pane_in_group(ctx);
                if did_move {
                    FocusRegion::PaneGroup
                } else if has_left_panel {
                    self.focus_left_region_entry(ctx);
                    FocusRegion::LeftPanel
                } else if has_right_panel {
                    self.focus_right_region_entry(ctx);
                    FocusRegion::RightPanel
                } else {
                    // No panels, wrap within panes.
                    self.focus_last_visible_pane_in_group(ctx);
                    FocusRegion::PaneGroup
                }
            }
        }
    }

    fn update_pane_dimming_for_current_focus_region(&mut self, ctx: &mut ViewContext<Self>) {
        let current_region = self.current_focus_region(ctx);
        self.set_pane_dimming_for_region(current_region, ctx);
    }

    fn set_pane_dimming_for_region(&mut self, region: FocusRegion, ctx: &mut ViewContext<Self>) {
        let dim_even_if_focused =
            matches!(region, FocusRegion::LeftPanel | FocusRegion::RightPanel);
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| {
            pane_group.set_dim_even_if_focused_for_all_panes(dim_even_if_focused, ctx);
        });
    }

    /// This function shifts focus to the panel on the left.
    /// The current focusable panels are: Warp Drive, theme chooser, AI, and resource center (keyboard shortcuts page only)
    fn focus_left_panel(&mut self, ctx: &mut ViewContext<Self>) {
        // Starts from terminal
        if self.active_tab_pane_group().is_self_or_child_focused(ctx) {
            if self.current_workspace_state.is_warp_drive_open {
                self.reset_focused_index_in_warp_drive(true, ctx);
            } else if self.is_theme_chooser_open() {
                ctx.focus(&self.theme_chooser_view);
            } else if self.current_workspace_state.is_ai_assistant_panel_open {
                ctx.focus(&self.ai_assistant_panel);
            } else if self.current_workspace_state.is_resource_center_open {
                ctx.focus(&self.resource_center_view);
            }
        }
        // Starts from a right panel: AI panel, resource center (keyboard shortcuts page only)
        else if self.ai_assistant_panel.is_self_or_child_focused(ctx)
            || self.resource_center_view.is_self_or_child_focused(ctx)
        {
            self.focus_active_tab(ctx);
        }
        // Starts from a left panel: Warp Drive
        else if self.is_warp_drive_view_focused(ctx) {
            if self.current_workspace_state.is_right_panel_open() {
                self.set_selected_object(None, ctx);
                if self.current_workspace_state.is_ai_assistant_panel_open {
                    ctx.focus(&self.ai_assistant_panel);
                } else if self.current_workspace_state.is_resource_center_open {
                    ctx.focus(&self.resource_center_view);
                }
            } else {
                self.focus_active_tab(ctx);
            }
        }
        // Starts from a left panel: theme chooser
        else if self.theme_chooser_view.is_self_or_child_focused(ctx) {
            if self.current_workspace_state.is_right_panel_open() {
                if self.current_workspace_state.is_ai_assistant_panel_open {
                    ctx.focus(&self.ai_assistant_panel);
                } else if self.current_workspace_state.is_resource_center_open {
                    ctx.focus(&self.resource_center_view);
                }
            } else {
                self.focus_active_tab(ctx);
            }
        }

        self.update_pane_dimming_for_current_focus_region(ctx);

        ctx.notify();
    }

    /// This function shifts focus to the panel on the right.
    fn focus_right_panel(&mut self, ctx: &mut ViewContext<Self>) {
        // Starts from terminal
        if self.active_tab_pane_group().is_self_or_child_focused(ctx) {
            if self.current_workspace_state.is_ai_assistant_panel_open {
                ctx.focus(&self.ai_assistant_panel);
            } else if self.current_workspace_state.is_resource_center_open {
                ctx.focus(&self.resource_center_view);
            } else if self.current_workspace_state.is_warp_drive_open {
                self.reset_focused_index_in_warp_drive(true, ctx);
            } else if self.is_theme_chooser_open() {
                ctx.focus(&self.theme_chooser_view);
            }
        }
        // Starts from a left panel: Warp Drive, theme chooser
        else if self.is_warp_drive_view_focused(ctx)
            || self.theme_chooser_view.is_self_or_child_focused(ctx)
        {
            self.focus_active_tab(ctx);
        }
        // Starts from a right panel: AI panel, resource center (keyboard shortcuts page only)
        else if self.ai_assistant_panel.is_self_or_child_focused(ctx)
            || self.resource_center_view.is_self_or_child_focused(ctx)
        {
            if self.current_workspace_state.is_left_panel_open() {
                if self.current_workspace_state.is_warp_drive_open {
                    self.reset_focused_index_in_warp_drive(true, ctx);
                } else if self.is_theme_chooser_open() {
                    ctx.focus(&self.theme_chooser_view);
                }
            } else {
                self.focus_active_tab(ctx);
            }
        }

        self.update_pane_dimming_for_current_focus_region(ctx);

        ctx.notify();
    }

    pub fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    pub fn is_overflow_menu_showing(&self) -> bool {
        self.show_tab_bar_overflow_menu
    }

    pub fn is_resource_center_showing(&self) -> bool {
        self.current_workspace_state.is_resource_center_open
    }

    #[cfg(feature = "integration_tests")]
    pub fn is_command_search_open(&self) -> bool {
        self.current_workspace_state.is_command_search_open
    }

    /// Retrieves the Pane Group view for the passed tab index.
    pub fn get_pane_group_view(&self, index: usize) -> Option<&ViewHandle<PaneGroup>> {
        self.tabs.get(index).map(|s| &s.pane_group)
    }

    /// Retrieves the Pane Group view for the passed tab index. Unlike the other
    /// method, this does not check for out of bounds.
    pub fn get_pane_group_view_unchecked(&self, index: usize) -> &ViewHandle<PaneGroup> {
        &self.tabs[index].pane_group
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[cfg(test)]
    pub fn tab_mru_order(&self) -> &[EntityId] {
        &self.tab_mru_order
    }

    pub(crate) fn activate_tab_by_pane_group_id(
        &mut self,
        pane_group_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.pane_group.id() == pane_group_id)
        {
            self.activate_tab(index, ctx);
        }
    }

    fn tab_navigation_data(&self, window_id: WindowId, ctx: &AppContext) -> Vec<TabNavigationData> {
        self.tab_mru_order
            .iter()
            .filter_map(|&pane_group_id| {
                let (tab_index, tab) = self
                    .tabs
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.pane_group.id() == pane_group_id)?;
                let title = tab.pane_group.as_ref(ctx).display_title(ctx);
                let subtitle = tab
                    .pane_group
                    .as_ref(ctx)
                    .active_session_path(ctx)
                    .map(|p| {
                        if let Some(home) = dirs::home_dir() {
                            if let Ok(stripped) = p.strip_prefix(&home) {
                                return format!("~/{}", stripped.display());
                            }
                        }
                        p.display().to_string()
                    });
                Some(TabNavigationData {
                    pane_group_id,
                    title,
                    subtitle,
                    window_id,
                    tab_index: tab_index + 1,
                })
            })
            .collect()
    }

    pub fn tab_views(&self) -> impl Iterator<Item = &ViewHandle<PaneGroup>> {
        self.tabs.iter().map(|s| &s.pane_group)
    }

    /// Get the tab color for a given tab index.
    pub fn get_tab_color(&self, index: usize) -> Option<AnsiColorIdentifier> {
        self.tabs.get(index).and_then(|tab| tab.color())
    }


    /// Gets all sessions in the current workspace.
    pub fn workspace_sessions<'a>(
        &'a self,
        window_id: WindowId,
        app: &'a AppContext,
    ) -> impl Iterator<Item = SessionNavigationData> + 'a {
        self.tabs.iter().flat_map(move |tab| {
            // Each tab has a pane group
            let pane_group_id = tab.pane_group.id();
            let view = tab.pane_group.as_ref(app);

            view.pane_sessions(pane_group_id, window_id, app)
        })
    }

    pub fn set_server_time(&mut self, server_time: Arc<ServerTime>) {
        self.server_time = Some(server_time);
    }

    /// Returns the PaneGroup view handle for the currently active tab.
    pub fn active_tab_pane_group(&self) -> &ViewHandle<PaneGroup> {
        self.get_pane_group_view(self.active_tab_index)
            .expect("Active tab index entry should exist")
    }

    /// Attempts to get selected text from the focused pane.
    /// Returns None if there is no selection, multiple selections, or an empty selection.
    /// Supports code, notebook, AI document, and terminal panes.
    fn get_selected_text_from_focused_view(&self, ctx: &AppContext) -> Option<String> {
        self.active_tab_pane_group()
            .as_ref(ctx)
            .selected_text_from_focused_pane(ctx)
    }

    /// This is meant to be dispatched directly by actions.
    pub fn activate_tab(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        self.activate_tab_internal(index, ctx);
        ctx.notify();
    }

    /// This function is meant to be used by other actions to perform the logic to update the
    /// view's state. It's not meant to be invoked directly by an action.
    pub fn activate_tab_internal(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if index < self.tab_count() {
            // If the command palette is open when the tab is switched using a keybinding,
            // we want to close the palette so that we don't get into a state where the palette
            // is open but doesn't have focus.
            if self.is_palette_open() {
                self.close_palette(false, None, ctx);
            }

            // If the agent management view is open, we want to close it when we activate a new tab.
            if FeatureFlag::AgentManagementView.is_enabled() {
                self.set_is_agent_management_view_open(false, ctx);
            }

            self.set_active_tab_index(index, ctx);
            self.focus_active_tab(ctx);
            self.update_window_title(ctx);
        }
    }

    fn left_panel_visibility_across_tabs_enabled(&self, ctx: &AppContext) -> bool {
        *WindowSettings::as_ref(ctx)
            .left_panel_visibility_across_tabs
            .value()
    }

    /// Reconciles the active tab's tools panel open/closed state to match the window-scoped desired state
    /// (syncing left panel open/closed state across tabs).
    fn reconcile_left_panel_open_for_active_tab(&mut self, ctx: &mut ViewContext<Self>) {
        let pane_group = self.active_tab_pane_group().clone();
        let pane_group_supports_tools_panel = pane_group.read(ctx, |pane_group, _| {
            Self::should_enable_file_tree_and_global_search_for_pane_group(pane_group)
        });

        if !pane_group_supports_tools_panel {
            return;
        }

        let desired_open = self.left_panel_open;
        pane_group.update(ctx, |pane_group, ctx| {
            pane_group.set_left_panel_open(desired_open, ctx);
        });
    }


    /// The agent-views/notifications models were removed; this is now a no-op.
    fn notify_terminal_focus_change(
        &self,
        _focused_terminal_view_id: Option<EntityId>,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    /// Change the active tab index. This must be used instead of setting `self.active_tab_index`
    /// directly, as it updates related state.
    pub(crate) fn set_active_tab_index(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let index = if index >= self.tab_count() {
            log::warn!(
                "Attempted to set active tab index {index} but only {} tabs exist, clamping",
                self.tab_count()
            );
            self.tab_count().saturating_sub(1)
        } else {
            index
        };

        self.active_tab_index = index;

        if let Some(tab) = self.tabs.get(index) {
            let pane_group_id = tab.pane_group.id();
            self.tab_mru_order.retain(|id| *id != pane_group_id);
            self.tab_mru_order.insert(0, pane_group_id);
        }
        if self.vertical_tabs_panel_open
            && FeatureFlag::VerticalTabs.is_enabled()
            && *TabSettings::as_ref(ctx).use_vertical_tabs
        {
            self.vertical_tabs_panel.scroll_to_tab(index);
        }

        if self.left_panel_visibility_across_tabs_enabled(ctx) {
            self.reconcile_left_panel_open_for_active_tab(ctx);
        }

        let left_active_pane_group = self.active_tab_pane_group().clone();
        let right_active_pane_group = self.active_tab_pane_group().clone();
        let working_directories_model = self.working_directories_model.clone();

        self.left_panel_view.update(ctx, |left_panel, ctx| {
            left_panel.set_active_pane_group(
                left_active_pane_group,
                &working_directories_model,
                ctx,
            );
        });
        let _ = (right_active_pane_group, &working_directories_model);

        let pane_group = self.active_tab_pane_group();
        let focused_terminal_view_id = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .terminal_view_from_pane_id(pane_group.as_ref(ctx).focused_pane_id(ctx), ctx)
            .map(|tv| tv.id());
        self.notify_terminal_focus_change(focused_terminal_view_id, ctx);

        self.update_active_session(ctx);
    }

    fn update_window_title(&self, ctx: &mut ViewContext<Self>) {
        let Some(tab) = self.tabs.get(self.active_tab_index) else {
            log::warn!(
                "Tried to update window title but active tab index ({}) was out of range 0..{}",
                self.active_tab_index,
                self.tabs.len()
            );
            return;
        };
        let tab_title = tab.pane_group.as_ref(ctx).display_title(ctx);

        let window_title = truncate_from_end(&tab_title, MAX_WINDOW_TITLE_LENGTH);

        let window_id = ctx.window_id();
        ctx.windows().set_window_title(window_id, &window_title);
    }

    fn rename_tab_internal(&mut self, index: usize, title: &str, ctx: &mut ViewContext<Self>) {
        // Focusing on the clicked tab
        if index >= self.tab_count() {
            return;
        }

        // If the agent management view is open, we want to close it when we change focus to rename a tab.
        // This function doesn't call `activate_tab_internal`, which is why we need the extra check here.
        if FeatureFlag::AgentManagementView.is_enabled() {
            self.set_is_agent_management_view_open(false, ctx);
        }

        self.set_active_tab_index(index, ctx);

        self.current_workspace_state.set_tab_being_renamed(index);

        // Clear the tab name editor to handle the case when another tab is already being renamed
        self.clear_tab_name_editor(ctx);
        let font_size = Self::tab_rename_editor_font_size(ctx, Appearance::as_ref(ctx));

        self.tab_rename_editor.update(ctx, move |editor, ctx| {
            editor.set_font_size(font_size, ctx);
            editor.insert_selected_text(title, ctx);
        });

        ctx.focus(&self.tab_rename_editor);
        ctx.notify();
    }

    pub fn rename_tab(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let tab = &self.tabs[index];
        let title = tab.pane_group.as_ref(ctx).display_title(ctx);

        self.rename_tab_internal(index, &title, ctx);
        send_telemetry_from_ctx!(
            TelemetryEvent::TabRenamed(TabRenameEvent::OpenedEditor),
            ctx
        );
    }

    fn set_active_tab_name(&mut self, title: &str, ctx: &mut ViewContext<Self>) {
        let Some(pane_group) = self
            .tabs
            .get(self.active_tab_index)
            .map(|tab| tab.pane_group.clone())
        else {
            log::warn!(
                "Tried to set active tab name but active tab index ({}) was out of range 0..{}",
                self.active_tab_index,
                self.tabs.len()
            );
            return;
        };

        if self.current_workspace_state.is_tab_being_renamed() {
            self.current_workspace_state.clear_tab_being_renamed();
            self.clear_tab_name_editor(ctx);
        }

        let title = title.trim();
        if title.is_empty() {
            ctx.notify();
            return;
        }
        pane_group.update(ctx, |pane_group, ctx| {
            if pane_group.display_title(ctx) != title {
                pane_group.set_title(title, ctx);
                send_telemetry_from_ctx!(
                    TelemetryEvent::TabRenamed(TabRenameEvent::CustomNameSet),
                    ctx
                );
            }
        });
        ctx.notify();
    }

    /// Programmatically sets the manual color override for a tab.
    ///
    /// - `Color(_)` applies that color.
    /// - `Cleared` explicitly clears the color (also suppresses any directory default).
    /// - `Unset` removes the manual override, letting the directory default apply.
    pub fn set_tab_color(
        &mut self,
        index: usize,
        color: SelectedTabColor,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.tabs.get(index).is_none() {
            log::warn!(
                "Not setting tab color: index was {index} but len is {}",
                self.tabs.len()
            );
            return;
        }
        if self.tabs[index].selected_color == color {
            return;
        }
        self.tabs[index].selected_color = color;
        send_telemetry_from_ctx!(
            TelemetryEvent::TabOperations {
                action: if matches!(color, SelectedTabColor::Color(_)) {
                    TabTelemetryAction::SetColor
                } else {
                    TabTelemetryAction::ResetColor
                },
            },
            ctx
        );
        ctx.notify();
    }

    pub fn toggle_tab_color(
        &mut self,
        index: usize,
        color: AnsiColorIdentifier,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.tabs.get(index).is_none() {
            log::warn!(
                "Not toggling tab color: index was {index} but len is {}",
                self.tabs.len()
            );
            return;
        }
        let next = if self.tabs[index].color() == Some(color) {
            if FeatureFlag::DirectoryTabColors.is_enabled() {
                SelectedTabColor::Cleared
            } else {
                SelectedTabColor::Unset
            }
        } else {
            SelectedTabColor::Color(color)
        };
        self.set_tab_color(index, next, ctx);
    }

    /// Syncs the tab color for the given tab based on the active terminal's CWD.
    /// If the CWD is within a directory that has a configured color, applies it.
    /// If the CWD moves outside all configured directories, the directory color is cleared.
    fn sync_codebase_tab_color(tab: &mut TabData, ctx: &mut ViewContext<Self>) {
        let cwd = tab
            .pane_group
            .as_ref(ctx)
            .active_session_view(ctx)
            .and_then(|tv| tv.as_ref(ctx).pwd_if_local(ctx));

        let Some(cwd) = cwd else {
            return;
        };

        let cwd_path = Path::new(&cwd);
        let color = TabSettings::as_ref(ctx)
            .directory_tab_colors
            .value()
            .color_for_directory(cwd_path)
            .and_then(|c| c.ansi_color());

        tab.default_directory_color = color;
        ctx.notify();
    }

    fn clear_tab_name_editor(&mut self, ctx: &mut ViewContext<Self>) {
        self.tab_rename_editor.update(ctx, move |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
        });
    }

    fn clear_pane_name_editor(&mut self, ctx: &mut ViewContext<Self>) {
        self.pane_rename_editor.update(ctx, move |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
        });
    }

    pub fn clear_tab_name(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let tab = &self.tabs[index];
        tab.pane_group.update(ctx, |view, ctx| {
            view.clear_title(ctx);
        });
        send_telemetry_from_ctx!(
            TelemetryEvent::TabRenamed(TabRenameEvent::CustomNameCleared),
            ctx
        );
        self.update_window_title(ctx);
        ctx.notify();
    }

    fn set_custom_pane_name(
        &mut self,
        locator: PaneViewLocator,
        title: String,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(pane_group_view) = self.get_pane_group_view_with_id(locator.pane_group_id) else {
            log::warn!("Tried to rename pane in a missing pane group");
            return;
        };
        pane_group_view.update(ctx, |pane_group, ctx| {
            let Some(pane) = pane_group.pane_by_id(locator.pane_id) else {
                log::warn!("Tried to rename a missing pane");
                return;
            };
            pane.pane_configuration().update(ctx, |configuration, ctx| {
                configuration.set_custom_vertical_tabs_title(title, ctx);
            });
            ctx.emit(pane_group::Event::AppStateChanged);
        });
    }

    pub fn clear_pane_name(&mut self, locator: PaneViewLocator, ctx: &mut ViewContext<Self>) {
        let Some(pane_group_view) = self.get_pane_group_view_with_id(locator.pane_group_id) else {
            log::warn!("Tried to clear pane name in a missing pane group");
            return;
        };
        pane_group_view.update(ctx, |pane_group, ctx| {
            let Some(pane) = pane_group.pane_by_id(locator.pane_id) else {
                log::warn!("Tried to clear a missing pane name");
                return;
            };
            pane.pane_configuration().update(ctx, |configuration, ctx| {
                configuration.clear_custom_vertical_tabs_title(ctx);
            });
            ctx.emit(pane_group::Event::AppStateChanged);
        });
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    pub fn rename_pane(&mut self, locator: PaneViewLocator, ctx: &mut ViewContext<Self>) {
        let Some((index, tab)) = self
            .tabs
            .iter()
            .enumerate()
            .find(|(_, tab_data)| tab_data.pane_group.id() == locator.pane_group_id)
        else {
            log::warn!("Tried to rename pane in a missing tab");
            return;
        };

        let Some(title) = tab
            .pane_group
            .as_ref(ctx)
            .pane_by_id(locator.pane_id)
            .map(|pane| {
                let configuration = pane.pane_configuration();
                let configuration = configuration.as_ref(ctx);
                configuration
                    .custom_vertical_tabs_title()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        let title = configuration.title().trim();
                        if title.is_empty() {
                            "Untitled pane".to_string()
                        } else {
                            title.to_string()
                        }
                    })
            })
        else {
            log::warn!("Tried to rename a missing pane");
            return;
        };

        tab.pane_group.update(ctx, |pane_group, ctx| {
            pane_group.focus_pane_by_id(locator.pane_id, ctx);
        });
        self.set_active_tab_index(index, ctx);
        self.current_workspace_state.set_pane_being_renamed(locator);
        self.clear_pane_name_editor(ctx);
        self.pane_rename_editor.update(ctx, move |editor, ctx| {
            editor.insert_selected_text(&title, ctx);
        });
        ctx.focus(&self.pane_rename_editor);
        ctx.notify();
    }

    pub fn list_tab_pane_groups(&self, app: &AppContext) -> Vec<TabPaneGroupIdentifiers> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(tab_idx, tab)| {
                let pane_group_id = tab.pane_group.id();
                let pane_group = tab.pane_group.as_ref(app);

                let pane_ids = pane_group.terminal_pane_ids();
                let terminal_ids = pane_ids
                    .into_iter()
                    .filter_map(|pane_id| {
                        let terminal_view = pane_group.terminal_view_from_pane_id(pane_id, app)?;
                        Some(terminal_view.id())
                    })
                    .collect::<Vec<_>>();

                TabPaneGroupIdentifiers {
                    tab_idx,
                    pane_group_id,
                    terminal_ids,
                }
            })
            .collect::<Vec<_>>()
    }

    pub(crate) fn terminal_view(
        &self,
        terminal_view_id: EntityId,
        app: &AppContext,
    ) -> Option<ViewHandle<TerminalView>> {
        self.tabs.iter().find_map(|tab| {
            tab.pane_group
                .as_ref(app)
                .terminal_views(app)
                .into_iter()
                .find(|terminal_view| terminal_view.id() == terminal_view_id)
        })
    }

    /// Focuses the given pane, revealing it first if it is hidden behind a
    /// temporary swap.
    pub fn focus_pane(&mut self, pane_view_locator: PaneViewLocator, ctx: &mut ViewContext<Self>) {
        if let Some((index, tab)) = self
            .tabs
            .iter()
            .enumerate()
            .find(|(_, tab_data)| tab_data.pane_group.id() == pane_view_locator.pane_group_id)
        {
            // Update the pane group to focus the active pane,
            // and then focus the pane group (tab). The order is important
            // because if we otherwise focus the tab first and another pane
            // was focused in the mean time, that pane will be the one that will
            // remain focused (as opposed to the pane with pane_id) since its
            // input would remain focused.
            tab.pane_group.update(ctx, |view, ctx| {
                view.reveal_and_focus_pane(pane_view_locator.pane_id, ctx);
            });
            self.activate_tab_internal(index, ctx);
            ctx.notify();
        }
    }

    /// Searches this workspace's tabs for the given terminal view and focuses it.
    /// Returns true if the terminal view was found and focused.
    fn focus_terminal_view_locally(
        &mut self,
        terminal_view_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        for tab in self.tabs.iter() {
            let pane_group_handle = &tab.pane_group;
            let pane_group = pane_group_handle.as_ref(ctx);
            if let Some(pane_id) = pane_group.find_pane_id_for_terminal_view(terminal_view_id, ctx)
            {
                self.focus_pane(
                    PaneViewLocator {
                        pane_group_id: pane_group_handle.id(),
                        pane_id,
                    },
                    ctx,
                );
                return true;
            }
        }
        false
    }

    /// Searches other windows for the given terminal view and focuses it there.
    /// (Uses the same cross-window dispatch pattern as open_notebook/open_workflow.)
    fn focus_terminal_view_in_other_window(
        &self,
        terminal_view_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) {
        let current_window = ctx.window_id();
        let result = WorkspaceRegistry::as_ref(ctx)
            .all_workspaces(ctx)
            .iter()
            .filter(|(win_id, _)| *win_id != current_window)
            .find_map(|(win_id, workspace)| {
                workspace.as_ref(ctx).tab_views().find_map(|pane_group| {
                    let pane_id = pane_group
                        .as_ref(ctx)
                        .find_pane_id_for_terminal_view(terminal_view_id, ctx)?;
                    Some((
                        *win_id,
                        PaneViewLocator {
                            pane_group_id: pane_group.id(),
                            pane_id,
                        },
                    ))
                })
            });

        if let Some((window_id, locator)) = result {
            ctx.windows().show_window_and_focus_app(window_id);
            if let Some(root_view_id) = ctx.root_view_id(window_id) {
                ctx.dispatch_action_for_view(
                    window_id,
                    root_view_id,
                    "root_view:handle_pane_navigation_event",
                    &locator,
                );
            }
        }
    }

    /// Shows the notification error in the specific pane.
    pub fn show_notification_error(
        &mut self,
        notification_error: NotificationSendError,
        pane_group_id: EntityId,
        pane_id: PaneId,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(tab) = self
            .tabs
            .iter()
            .find(|tab_data| tab_data.pane_group.id() == pane_group_id)
        {
            tab.pane_group.update(ctx, |view, ctx| {
                view.show_notification_error(notification_error, pane_id, ctx);
            });

            ctx.notify();
        }
    }


    fn handle_prompt_editor_modal_event(
        &mut self,
        event: &PromptEditorModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            PromptEditorModalEvent::Close => {
                self.current_workspace_state.is_prompt_editor_open = false;
                self.focus_active_tab(ctx);
                ctx.notify();
            }
        }
    }


    fn build_header_toolbar_editor_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<HeaderToolbarEditorModal> {
        let modal = ctx.add_typed_action_view(HeaderToolbarEditorModal::new);
        ctx.subscribe_to_view(&modal, |me, _, event, ctx| {
            me.handle_header_toolbar_editor_modal_event(event, ctx);
        });
        modal
    }

    fn handle_header_toolbar_editor_modal_event(
        &mut self,
        event: &HeaderToolbarEditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            HeaderToolbarEditorEvent::Close => {
                self.current_workspace_state.is_header_toolbar_editor_open = false;
                self.focus_active_tab(ctx);
                ctx.notify();
            }
        }
    }

    fn ensure_tabs_panel_in_config(ctx: &mut ViewContext<Self>) {
        let config = TabSettings::as_ref(ctx)
            .header_toolbar_chip_selection
            .clone();
        let left = config.left_items();
        let right = config.right_items();
        let already_present = left.contains(&HeaderToolbarItemKind::TabsPanel)
            || right.contains(&HeaderToolbarItemKind::TabsPanel);
        if already_present {
            return;
        }

        let mut new_left = left;
        new_left.insert(0, HeaderToolbarItemKind::TabsPanel);
        let selection = HeaderToolbarChipSelection::Custom {
            left: new_left,
            right,
        };
        TabSettings::handle(ctx).update(ctx, |settings, ctx| {
            report_if_error!(settings
                .header_toolbar_chip_selection
                .set_value(selection, ctx));
        });
    }

    fn sync_panel_positions_from_config(&mut self, ctx: &mut ViewContext<Self>) {
        let config = TabSettings::as_ref(ctx)
            .header_toolbar_chip_selection
            .clone();
        let left_items = config.left_items();
        let tools_position = if left_items.contains(&HeaderToolbarItemKind::ToolsPanel) {
            PanelPosition::Left
        } else {
            PanelPosition::Right
        };
        let code_review_position = if left_items.contains(&HeaderToolbarItemKind::CodeReview) {
            PanelPosition::Left
        } else {
            PanelPosition::Right
        };
        self.left_panel_view.update(ctx, |view, ctx| {
            view.set_panel_position(tools_position, ctx);
        });
        let _ = code_review_position;
    }

    fn build_header_toolbar_context_menu(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<Menu<WorkspaceAction>> {
        let menu = ctx.add_typed_action_view(|_| Menu::new().with_drop_shadow());
        ctx.subscribe_to_view(&menu, |me, _, event, ctx| {
            if let MenuEvent::Close { .. } = event {
                me.show_header_toolbar_context_menu = None;
                ctx.notify();
            }
        });
        menu
    }

    fn show_header_toolbar_context_menu(
        &mut self,
        position: Vector2F,
        ctx: &mut ViewContext<Self>,
    ) {
        if !FeatureFlag::ConfigurableToolbar.is_enabled() {
            return;
        }
        let items = vec![MenuItemFields::new("Re-arrange toolbar items")
            .with_on_select_action(WorkspaceAction::OpenHeaderToolbarEditor)
            .into_item()];
        self.header_toolbar_context_menu
            .update(ctx, |menu, ctx| menu.set_items(items, ctx));
        self.show_header_toolbar_context_menu = Some(position);
        ctx.focus(&self.header_toolbar_context_menu);
        ctx.notify();
    }

    fn open_header_toolbar_editor(&mut self, ctx: &mut ViewContext<Self>) {
        if !FeatureFlag::ConfigurableToolbar.is_enabled() {
            return;
        }
        self.header_toolbar_editor_modal
            .update(ctx, |modal, ctx| modal.open(ctx));
        self.close_all_overlays(ctx);
        self.current_workspace_state.is_header_toolbar_editor_open = true;
        ctx.focus(&self.header_toolbar_editor_modal);
    }



    #[cfg(feature = "local_fs")]
    fn get_active_session(&self, ctx: &mut ViewContext<Self>) -> Option<Arc<Session>> {
        let pane_group = self.active_tab_pane_group();
        pane_group
            .as_ref(ctx)
            .active_session_id(ctx)
            .and_then(|session_id| {
                pane_group
                    .as_ref(ctx)
                    .terminal_view_from_pane_id(session_id, ctx)
            })
            .and_then(|tv| {
                let tv_ref = tv.as_ref(ctx);
                let session_id = tv_ref.active_block_session_id()?;
                tv_ref.sessions_model().as_ref(ctx).get(session_id)
            })
    }

    #[cfg(not(feature = "local_fs"))]
    pub fn open_file_with_target(
        &mut self,
        _path: PathBuf,
        _target: FileTarget,
        _line_col: Option<LineAndColumnArg>,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    #[cfg(feature = "local_fs")]
    pub fn open_file_with_target(
        &mut self,
        path: PathBuf,
        target: FileTarget,
        line_col: Option<LineAndColumnArg>,
        ctx: &mut ViewContext<Self>,
    ) {
        // Handle directories for CodeEditor(NewTab) target by opening a new terminal tab
        if path.is_dir() && matches!(target, FileTarget::CodeEditor(EditorLayout::NewTab)) {
            self.add_tab_with_pane_layout(
                PanesLayout::SingleTerminal(Box::new(NewTerminalOptions {
                    initial_directory: Some(path.clone()),
                    hide_homepage: true,
                    ..Default::default()
                })),
                Arc::new(HashMap::new()),
                None,
                ctx,
            );
            return;
        }

        match target {
            FileTarget::MarkdownViewer(_layout) => {
                // The built-in notebook/markdown viewer was removed; open externally.
                crate::util::file::open_file_path_with_editor(line_col, path.clone(), None, ctx);
            }
            FileTarget::EnvEditor => {
                let editor_value: Option<String> = self
                    .get_active_session(ctx)
                    .and_then(|session| session.editor().map(|s| s.to_string()));

                if let Some(ref editor_env) = editor_value {
                    if let Ok(editor) = Editor::try_from(editor_env.as_str()) {
                        crate::util::file::open_file_path_with_editor(
                            line_col,
                            path.clone(),
                            Some(editor),
                            ctx,
                        );
                        return;
                    }

                    // If we have an editor string but it's not a known Editor, we try to run it in a new pane
                    let new_pane_id =
                        self.active_tab_pane_group().update(ctx, |pane_group, ctx| {
                            pane_group.add_terminal_pane(
                                Direction::Right,
                                None, /*chosen_shell*/
                                ctx,
                            )
                        });

                    if let Some(terminal_view_handle) = self
                        .active_tab_pane_group()
                        .as_ref(ctx)
                        .terminal_view_from_pane_id(new_pane_id, ctx)
                    {
                        let editor_ref = Some(editor_env.as_str());
                        let path_clone = path.clone();
                        terminal_view_handle.update(ctx, |terminal, ctx| {
                            let editor_command =
                                crate::util::file::external_editor::generate_editor_command(
                                    &path_clone,
                                    line_col,
                                    editor_ref,
                                );
                            terminal.set_pending_command(&editor_command, ctx);
                        });
                        return;
                    } else {
                        log::error!(
                            "Could not get terminal view handle for new pane when attempting to open file with $EDITOR."
                        );
                    }
                }

                crate::util::file::open_file_path_in_external_editor(line_col, path.clone(), ctx);
            }
            FileTarget::CodeEditor(_layout) => {
                // The built-in code editor was removed; fall back to the system default editor.
                crate::util::file::open_file_path_with_editor(line_col, path.clone(), None, ctx);
            }
            FileTarget::ExternalEditor(editor) => {
                crate::util::file::open_file_path_with_editor(
                    line_col,
                    path.clone(),
                    Some(editor),
                    ctx,
                );
            }
            FileTarget::SystemDefault => {
                crate::util::file::open_file_path_with_editor(line_col, path.clone(), None, ctx);
            }
            FileTarget::SystemGeneric => {
                ctx.open_file_path(&path);
            }
        }
    }

    fn handle_left_panel_event(&mut self, event: &LeftPanelEvent, ctx: &mut ViewContext<Self>) {
        match event {
            LeftPanelEvent::FileTree(pane_group_event) => {
                let pane_group = self.active_tab_pane_group().clone();
                self.handle_file_tree_event(pane_group, pane_group_event, ctx);
            }
            LeftPanelEvent::OpenFileWithTarget {
                location,
                target,
                line_col,
            } => {
                if let LocalOrRemotePath::Local(path) = location {
                    self.open_file_with_target(path.clone(), target.clone(), *line_col, ctx);
                }
            }
        }
    }


    fn join_slack(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_url(links::SLACK_URL);
    }

    fn view_user_docs(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_url(links::USER_DOCS_URL);
    }

    fn view_latest_changelog(&mut self, ctx: &mut ViewContext<Self>) {
        self.update_toast_stack.update(ctx, |stack, ctx| {
            stack.clear_toasts(ctx);
        });
        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::Changelog),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });
        self.check_for_changelog(ChangelogRequestType::UserAction, ctx);
    }

    fn view_privacy_policy(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_url(links::PRIVACY_POLICY_URL);
    }

    fn send_feedback(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_url(&links::feedback_form_url());
    }

    #[cfg(not(target_family = "wasm"))]
    fn view_logs(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async { tokio::task::spawn_blocking(rift_logging::create_log_bundle_zip).await },
            |me, result, ctx| match result {
                Ok(Ok(path)) => {
                    ctx.open_file_path_in_explorer(&path);
                }
                Ok(Err(err)) => {
                    let error_message = format!("Failed to create log bundle: {err}");
                    log::error!("{error_message}");
                    me.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(error_message);
                        toast_stack.add_persistent_toast(toast, ctx);
                    });
                }
                Err(err) => {
                    let error_message = format!("Failed to create log bundle: {err}");
                    log::error!("{error_message}");
                    me.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(error_message);
                        toast_stack.add_persistent_toast(toast, ctx);
                    });
                }
            },
        );
    }

    fn copy_version(&mut self, version: &str, ctx: &mut ViewContext<Self>) {
        ctx.clipboard()
            .write(ClipboardContent::plain_text(version.to_string()));
    }


    /// Builds the unified new-session menu items
    /// tab bar chevron and the vertical tab bar `+` button.
    fn unified_new_session_menu_items(
        &self,
        ctx: &mut ViewContext<Self>,
    ) -> Vec<MenuItem<WorkspaceAction>> {
        let mut menu_items = vec![];

        let is_any_ai_enabled = AISettings::as_ref(ctx).is_any_ai_enabled(ctx);
        let ai_settings = AISettings::as_ref(ctx);
        let effective_default = ai_settings.default_session_mode(ctx);
        let default_tab_config_path = ai_settings.default_tab_config_path().to_string();
        let shortcut_label = keybinding_name_to_display_string(NEW_TAB_BINDING_NAME, ctx);
        let reopen_closed_session_shortcut_label =
            keybinding_name_to_display_string("app:reopen_closed_session", ctx);

        // 1. Agent (if AI enabled)
        if is_any_ai_enabled {
            let mut agent_item = MenuItemFields::new("Agent")
                .with_on_select_action(WorkspaceAction::AddAgentTab)
                .with_icon(icons::Icon::LayoutAlt01);
            if effective_default == DefaultSessionMode::Agent {
                agent_item = agent_item.with_key_shortcut_label(shortcut_label.clone());
            }
            menu_items.push(agent_item.into_item());
        }

        // 2. Terminal (+ individual shells on Windows)
        {
            // On Windows, list the default terminal and each available shell as
            // individual top-level items (no submenu) so each gets a sidecar.
            #[cfg(target_os = "windows")]
            {
                let is_terminal_default = effective_default == DefaultSessionMode::Terminal;
                let mut terminal_item = MenuItemFields::new("Terminal")
                    .with_on_select_action(WorkspaceAction::AddTerminalTab {
                        hide_homepage: false,
                    })
                    .with_icon(icons::Icon::LayoutAlt01);
                if is_terminal_default {
                    terminal_item = terminal_item.with_key_shortcut_label(shortcut_label.clone());
                }
                menu_items.push(terminal_item.into_item());

                #[cfg(feature = "local_tty")]
                if FeatureFlag::ShellSelector.is_enabled() {
                    AvailableShells::handle(ctx).read(ctx, |model, _| {
                        for shell in model.get_available_shells() {
                            let shell_name = model.display_name_for_shell(shell);
                            let icon = shell
                                .get_valid_shell_path_and_type()
                                .and_then(|shell_launch_data| {
                                    ShellIndicatorType::try_from(&shell_launch_data).ok()
                                })
                                .map(|shell_indicator_type| shell_indicator_type.to_icon())
                                .unwrap_or(icons::Icon::Terminal);
                            let item = MenuItemFields::new(shell_name)
                                .with_on_select_action(WorkspaceAction::AddTabWithShell {
                                    shell: shell.clone(),
                                    source: AddTabWithShellSource::ShellSelectorMenu,
                                })
                                .with_icon(icon);
                            menu_items.push(item.into_item());
                        }
                    });
                }
            }

            // On other platforms, Terminal is a regular item.
            #[cfg(not(target_os = "windows"))]
            {
                let mut terminal_item = MenuItemFields::new("Terminal")
                    .with_on_select_action(WorkspaceAction::AddTerminalTab {
                        hide_homepage: false,
                    })
                    .with_icon(icons::Icon::LayoutAlt01);
                if effective_default == DefaultSessionMode::Terminal {
                    terminal_item = terminal_item.with_key_shortcut_label(shortcut_label.clone());
                }
                menu_items.push(terminal_item.into_item());
            }
        }

        // 3. Cloud Agent (if flags enabled)
        if is_any_ai_enabled
            && FeatureFlag::AgentView.is_enabled()
            && FeatureFlag::CloudMode.is_enabled()
        {
            let mut cloud_item = MenuItemFields::new("Cloud Agent")
                .with_on_select_action(WorkspaceAction::AddAmbientAgentTab)
                .with_icon(icons::Icon::LayoutAlt01);
            if effective_default == DefaultSessionMode::CloudAgent {
                cloud_item = cloud_item.with_key_shortcut_label(shortcut_label.clone());
            }
            menu_items.push(cloud_item.into_item());
        }

        // 3b. Local Docker Sandbox
        if FeatureFlag::LocalDockerSandbox.is_enabled() {
            let mut docker_item = MenuItemFields::new("Local Docker Sandbox")
                .with_on_select_action(WorkspaceAction::AddDockerSandboxTab)
                .with_icon(icons::Icon::Docker);
            if effective_default == DefaultSessionMode::DockerSandbox {
                docker_item = docker_item.with_key_shortcut_label(shortcut_label.clone());
            }
            menu_items.push(docker_item.into_item());
        }

        // 4. User tab configs
        if FeatureFlag::TabConfigs.is_enabled() {
            let tab_configs = WarpConfig::as_ref(ctx).tab_configs().to_vec();

            // Count occurrences of each config name so we can disambiguate
            // duplicates in the menu (e.g. "My Tab Config", "My Tab Config (1)").
            let mut name_totals: HashMap<String, usize> = HashMap::new();
            for config in &tab_configs {
                *name_totals.entry(config.name.clone()).or_default() += 1;
            }
            let mut name_seen: HashMap<String, usize> = HashMap::new();

            for tab_config in tab_configs {
                let is_worktree = tab_config.is_worktree();
                let icon = if is_worktree {
                    icons::Icon::Dataflow02
                } else {
                    icons::Icon::LayoutAlt01
                };
                let is_default_config = effective_default == DefaultSessionMode::TabConfig
                    && tab_config
                        .source_path
                        .as_ref()
                        .is_some_and(|p| p.to_string_lossy() == default_tab_config_path);

                let display_name = if name_totals.get(&tab_config.name).copied().unwrap_or(0) > 1 {
                    let seen = name_seen.entry(tab_config.name.clone()).or_default();
                    *seen += 1;
                    if *seen == 1 {
                        tab_config.name.clone()
                    } else {
                        format!("{} ({})", tab_config.name, *seen - 1)
                    }
                } else {
                    tab_config.name.clone()
                };

                let mut item = MenuItemFields::new(display_name)
                    .with_on_select_action(WorkspaceAction::SelectTabConfig(tab_config))
                    .with_icon(icon);
                if is_default_config {
                    item = item.with_key_shortcut_label(shortcut_label.clone());
                }
                menu_items.push(item.into_item());
            }
        }

        // 5. Separator + worktree config entry + new tab config
        if FeatureFlag::TabConfigs.is_enabled() {
            menu_items.push(MenuItem::Separator);
            menu_items.push(
                MenuItemFields::new_submenu("New worktree config")
                    .with_icon(icons::Icon::Dataflow02)
                    .into_item(),
            );

            // 6. New tab config — V0: opens the TOML template.
            menu_items.push(
                MenuItemFields::new("New tab config")
                    .with_on_select_action(WorkspaceAction::SelectNewSessionMenuItem(
                        NewSessionMenuItem::CreateNewTabConfig,
                    ))
                    .with_icon(icons::Icon::Plus)
                    .into_item(),
            );
        }

        // 7. Separator + New tab group entry. Gated on the Grouped Tabs flag.
        // TODO(johnturcoo) add group actions.
        if FeatureFlag::GroupedTabs.is_enabled() {
            menu_items.push(MenuItem::Separator);
            menu_items.push(
                MenuItemFields::new("New tab group")
                    .with_on_select_action(WorkspaceAction::SelectNewSessionMenuItem(
                        NewSessionMenuItem::CreateNewTabGroup,
                    ))
                    .with_icon(icons::Icon::LayersThree01)
                    .into_item(),
            );
        }

        menu_items.push(MenuItem::Separator);
        menu_items.push(
            MenuItemFields::new("Reopen closed session")
                .with_on_select_action(WorkspaceAction::ReopenClosedSession)
                .with_key_shortcut_label(reopen_closed_session_shortcut_label)
                .with_disabled(UndoCloseStack::handle(ctx).as_ref(ctx).is_empty())
                .into_item(),
        );

        menu_items
    }

    fn open_tab_configs_menu(
        &mut self,
        anchor: NewSessionMenuAnchor,
        open_source: TabConfigsMenuOpenSource,
        ctx: &mut ViewContext<Self>,
    ) {
        let menu_items = self.unified_new_session_menu_items(ctx);
        ctx.update_view(&self.new_session_dropdown_menu, |context_menu, view_ctx| {
            // Match the Figma mock width (OptionMenuItem component is 268px).
            context_menu.set_width(268.);
            context_menu.set_items(menu_items, view_ctx);
            match open_source {
                TabConfigsMenuOpenSource::KeyboardShortcut => {
                    context_menu.set_selected_by_index(0, view_ctx);
                }
                TabConfigsMenuOpenSource::Pointer => {
                    context_menu.reset_selection(view_ctx);
                }
            }
        });
        self.show_new_session_dropdown_menu = Some(anchor);
        ctx.focus(&self.new_session_dropdown_menu);
        ctx.notify();
    }

    pub fn open_new_session_dropdown_menu(
        &mut self,
        anchor: NewSessionMenuAnchor,
        ctx: &mut ViewContext<Self>,
    ) {
        self.open_tab_configs_menu(anchor, TabConfigsMenuOpenSource::Pointer, ctx);
    }

    fn toggle_tab_configs_menu(&mut self, ctx: &mut ViewContext<Self>) {
        let use_vertical_tabs =
            FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(ctx).use_vertical_tabs;
        if self.show_new_session_dropdown_menu.is_some() {
            self.close_new_session_dropdown_menu(ctx);
            return;
        }

        if use_vertical_tabs {
            if !self.vertical_tabs_panel_open {
                self.vertical_tabs_panel_open = true;
                self.sync_window_button_visibility(ctx);
            }
            self.open_tab_configs_menu(
                NewSessionMenuAnchor::AddTabButton(Vector2F::zero()),
                TabConfigsMenuOpenSource::KeyboardShortcut,
                ctx,
            );
            return;
        }

        let position = ctx
            .element_position_by_id_at_last_frame(self.window_id, NEW_TAB_BUTTON_POSITION_ID)
            .map(|position| position.lower_left())
            .unwrap_or_else(Vector2F::zero);
        self.open_tab_configs_menu(
            NewSessionMenuAnchor::AddTabButton(position),
            TabConfigsMenuOpenSource::KeyboardShortcut,
            ctx,
        );
    }

    pub fn toggle_new_session_dropdown_menu(
        &mut self,
        anchor: NewSessionMenuAnchor,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.show_new_session_dropdown_menu.is_some() {
            self.close_new_session_dropdown_menu(ctx);
            return;
        }

        self.open_tab_configs_menu(anchor, TabConfigsMenuOpenSource::Pointer, ctx);
    }

    fn open_launch_config_from_menu(
        &mut self,
        new_session_menu_item: NewSessionMenuItem,
        ctx: &mut ViewContext<Self>,
    ) {
        match new_session_menu_item {
            NewSessionMenuItem::OpenLaunchConfig(launch_config) => ctx.dispatch_global_action(
                "root_view:open_launch_config",
                OpenLaunchConfigArg {
                    launch_config,
                    ui_location: LaunchConfigUiLocation::TabMenu,
                    open_in_active_window: false,
                },
            ),
            NewSessionMenuItem::OpenLaunchConfigDocs => {
                ctx.open_url("https://docs.warp.dev/terminal/sessions/launch-configurations")
            }
            #[cfg(feature = "local_fs")]
            NewSessionMenuItem::CreateNewTabConfig => {
                self.create_and_open_new_tab_config(ctx);
            }
            #[cfg(not(feature = "local_fs"))]
            NewSessionMenuItem::CreateNewTabConfig => {}
            NewSessionMenuItem::CreateNewTabGroup => {
                if FeatureFlag::GroupedTabs.is_enabled() {
                    self.create_new_tab_group(ctx);
                }
            }
        }
    }

    /// Opens a tab config after the user has filled in (or confirmed) param values.
    fn open_tab_config_with_params(
        &mut self,
        tab_config: crate::tab_configs::TabConfig,
        param_values: HashMap<String, String>,
        worktree_branch_name: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        let tab_color = tab_config.color;
        let (rendered_title, pane_template) =
            crate::tab_configs::render_tab_config(&tab_config, &param_values, worktree_branch_name);
        self.add_tab_with_pane_layout(
            PanesLayout::Template(pane_template),
            Arc::new(HashMap::new()),
            rendered_title,
            ctx,
        );
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            // Apply tab color if specified, matching the launch config pattern.
            if let Some(color) = tab_color {
                tab.selected_color = SelectedTabColor::Color(color);
            }
        }
    }

    /// Opens a tab config, showing the param-fill modal when the config has parameters,
    /// or opening the tab directly when there are no parameters.
    pub(crate) fn open_tab_config(
        &mut self,
        tab_config: crate::tab_configs::TabConfig,
        ctx: &mut ViewContext<Self>,
    ) {
        if tab_config.params.is_empty() {
            let is_worktree_config = tab_config.is_worktree();
            let worktree_branch_name = self.maybe_generate_worktree_name(&tab_config);
            let param_values = tab_config.default_param_values();
            self.open_tab_config_with_params(
                tab_config,
                param_values,
                worktree_branch_name.as_deref(),
                ctx,
            );
            send_telemetry_from_ctx!(
                TabConfigsTelemetryEvent::ExistingConfigOpened {
                    open_mode: ExistingTabConfigOpenMode::Direct,
                    is_worktree_config,
                },
                ctx
            );
        } else {
            // Pass the active terminal's cwd to seed the branch picker's git lookup.
            let cwd = self
                .active_session_view(ctx)
                .and_then(|view| view.as_ref(ctx).pwd())
                .map(PathBuf::from);

            let modal_title = format!("Open: {}", tab_config.name);
            self.tab_config_params_modal.view.update(ctx, |modal, ctx| {
                modal.body().update(ctx, |body, ctx| {
                    body.set_title(modal_title);
                    body.on_open(tab_config, cwd, ctx);
                });
            });
            self.tab_config_params_modal.open();
            self.current_workspace_state.is_tab_config_params_modal_open = true;
            ctx.notify();
        }
    }

    /// Writes the default tab config template to an unused path in `~/.rift/tab_configs/`
    /// and opens it respecting the user's configured editor setting.
    #[cfg(feature = "local_fs")]
    fn create_and_open_new_tab_config(&mut self, ctx: &mut ViewContext<Self>) {
        let dir = tab_configs_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("Failed to create tab_configs dir: {e:?}");
            return;
        }
        let path = find_unused_tab_config_path(&dir);
        const TEMPLATE: &str =
            include_str!("../../resources/tab_configs/new_tab_config_template.toml");
        if let Err(e) = std::fs::write(&path, TEMPLATE) {
            log::warn!("Failed to write new tab config template: {e:?}");
            return;
        }
        let settings = EditorSettings::as_ref(ctx);
        let target = resolve_file_target_with_editor_choice(
            &path,
            *settings.open_code_panels_file_editor,
            *settings.prefer_markdown_viewer,
            *settings.open_file_layout,
            None,
        );
        send_telemetry_from_ctx!(TabConfigsTelemetryEvent::MenuCreateNewTabConfigClicked, ctx);
        self.open_file_with_target(
            path.clone(),
            target,
            None,
            ctx,
        );
    }

    /// Snapshots the given tab's pane layout and writes it as a new tab config
    /// TOML to `~/.rift/tab_configs/`, then opens the file in the user's editor.
    #[cfg(feature = "local_fs")]
    fn save_current_tab_as_new_config(&mut self, tab_index: usize, ctx: &mut ViewContext<Self>) {
        use crate::tab_configs::session_config::{tab_config_from_pane_snapshot, write_tab_config};

        let tab = &self.tabs[tab_index];
        let snapshot = tab.pane_group.as_ref(ctx).snapshot(ctx);
        let custom_title = tab.pane_group.as_ref(ctx).custom_title(ctx);
        let color = tab.color();
        let config = tab_config_from_pane_snapshot(&snapshot, custom_title, color);

        let dir = tab_configs_dir();
        match write_tab_config(&config, &dir, "my_tab_config") {
            Ok(path) => {
                let settings = EditorSettings::as_ref(ctx);
                let target = resolve_file_target_with_editor_choice(
                    &path,
                    *settings.open_code_panels_file_editor,
                    *settings.prefer_markdown_viewer,
                    *settings.open_file_layout,
                    None,
                );
                self.open_file_with_target(path.clone(), target, None, ctx);
            }
            Err(e) => log::warn!("Failed to save tab config: {e:?}"),
        }
    }

    #[cfg(not(feature = "local_fs"))]
    fn save_current_tab_as_new_config(&mut self, _tab_index: usize, _ctx: &mut ViewContext<Self>) {}

    /// Creates a new tab group containing a single new tab.
    fn create_new_tab_group(&mut self, ctx: &mut ViewContext<Self>) {
        let group = TabGroup::new();
        let group_id = group.id;
        self.tab_groups.insert(group_id, group);
        self.add_new_session_tab_with_default_mode(
            NewSessionSource::Tab,
            Some(ctx.window_id()),
            None,
            false,
            ctx,
        );
        let new_tab_index = self.active_tab_index;
        if let Some(tab) = self.tabs.get_mut(new_tab_index) {
            tab.group_id = Some(group_id);
        }

        // New tab groups always land at the top of the tab list.
        if new_tab_index != 0 {
            let tab = self.tabs.remove(new_tab_index);
            self.tabs.insert(0, tab);
            self.active_tab_index = 0;
        }

        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();

        ctx.dispatch_typed_action_deferred(WorkspaceAction::RenameTabGroup(group_id));
    }

    /// Closes every tab in the given group and removes the group.
    pub fn close_tab_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>) {
        let indices: Vec<usize> = group_member_indices(&self.tabs, group_id).collect();
        if indices.is_empty() {
            self.tab_groups.remove(&group_id);
            ctx.notify();
            return;
        }
        let first_index = indices[0];
        let closed = self.close_tabs(
            indices.into_iter(),
            OpenDialogSource::CloseOtherTabs {
                tab_index: first_index,
            },
            false,
            true,
            ctx,
        );
        if closed {
            self.tab_groups.remove(&group_id);
            ctx.notify();
        }
    }

    /// Toggles the collapsed state of the given tab group.
    pub fn toggle_tab_group_collapsed(
        &mut self,
        group_id: TabGroupId,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(group) = self.tab_groups.get_mut(&group_id) {
            group.collapsed = !group.collapsed;
            ctx.notify();
        }
    }

    /// Opens the inline rename editor over the given group's header.
    pub fn rename_tab_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>) {
        let Some(group) = self.tab_groups.get(&group_id) else {
            return;
        };
        // Seed the editor with the existing name, or the "New Group" default
        // label when the group is unnamed. `insert_selected_text` selects the
        // seeded text so the user can type to replace it instantly.
        let seed_text = group
            .name
            .clone()
            .unwrap_or_else(|| "New Group".to_string());

        self.current_workspace_state
            .set_tab_group_being_renamed(group_id);
        self.clear_tab_group_name_editor(ctx);
        self.tab_group_rename_editor
            .update(ctx, move |editor, ctx| {
                editor.insert_selected_text(&seed_text, ctx);
            });
        ctx.focus(&self.tab_group_rename_editor);
        ctx.notify();
    }

    fn clear_tab_group_name_editor(&mut self, ctx: &mut ViewContext<Self>) {
        self.tab_group_rename_editor
            .update(ctx, move |editor, ctx| {
                editor.clear_buffer_and_reset_undo_stack(ctx);
            });
    }

    /// Creates a new group containing the tab and moves it to the top of
    /// the tab list.
    fn new_tab_group_from_tab(&mut self, tab_index: usize, ctx: &mut ViewContext<Self>) {
        if !FeatureFlag::GroupedTabs.is_enabled() {
            return;
        }
        let Some(tab) = self.tabs.get(tab_index) else {
            log::debug!("new_tab_group_from_tab: tab_index {tab_index} out of bounds");
            return;
        };
        let previous_group_id = tab.group_id;

        let group = TabGroup::new();
        let group_id = group.id;
        self.tab_groups.insert(group_id, group);

        self.tabs[tab_index].group_id = Some(group_id);
        self.move_tab_to_index(tab_index, 0, ctx);
        self.set_active_tab_index(0, ctx);

        if let Some(prev_group_id) = previous_group_id {
            self.prune_empty_tab_group(prev_group_id, ctx);
        }

        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();

        ctx.dispatch_typed_action_deferred(WorkspaceAction::RenameTabGroup(group_id));
    }

    /// Moves the tab into `group_id`, appending it to the end of the
    /// group's contiguous run.
    fn move_tab_to_group(
        &mut self,
        tab_index: usize,
        group_id: TabGroupId,
        ctx: &mut ViewContext<Self>,
    ) {
        if !FeatureFlag::GroupedTabs.is_enabled() {
            return;
        }
        let Some(tab) = self.tabs.get(tab_index) else {
            log::debug!("move_tab_to_group: tab_index {tab_index} out of bounds");
            return;
        };
        // No-op when the tab already belongs to the target group.
        if tab.group_id == Some(group_id) {
            return;
        }
        let previous_group_id = tab.group_id;

        let target_index = group_member_indices(&self.tabs, group_id)
            .last()
            .map(|i| i + 1)
            .unwrap_or(self.tabs.len());
        self.tabs[tab_index].group_id = Some(group_id);
        self.move_tab_to_index(tab_index, target_index, ctx);

        if let Some(prev) = previous_group_id {
            self.prune_empty_tab_group(prev, ctx);
        }

        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    /// Removes the tab from its current group and repositions it just past
    /// the group's last remaining member.
    fn remove_tab_from_group(&mut self, tab_index: usize, ctx: &mut ViewContext<Self>) {
        if !FeatureFlag::GroupedTabs.is_enabled() {
            return;
        }
        let Some(tab) = self.tabs.get(tab_index) else {
            return;
        };
        let Some(previous_group_id) = tab.group_id else {
            return;
        };

        self.tabs[tab_index].group_id = None;

        if let Some(last) = group_member_indices(&self.tabs, previous_group_id).last() {
            self.move_tab_to_index(tab_index, last + 1, ctx);
        }

        self.prune_empty_tab_group(previous_group_id, ctx);

        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    fn ungroup_tabs(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>) {
        if !FeatureFlag::GroupedTabs.is_enabled() || !self.tab_groups.contains_key(&group_id) {
            return;
        }
        for tab in &mut self.tabs {
            if tab.group_id == Some(group_id) {
                tab.group_id = None;
            }
        }
        self.tab_groups.remove(&group_id);
        ctx.notify();
    }

    /// An active member reuses the normal new-tab inheritance + placement;
    /// otherwise the new tab is appended to the end of the group's run.
    fn new_tab_in_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>) {
        if !FeatureFlag::GroupedTabs.is_enabled() || !self.tab_groups.contains_key(&group_id) {
            return;
        }
        let active_is_member = self
            .tabs
            .get(self.active_tab_index)
            .is_some_and(|tab| tab.group_id == Some(group_id));

        self.add_new_session_tab_with_default_mode(
            NewSessionSource::Tab,
            Some(ctx.window_id()),
            None,
            false,
            ctx,
        );

        // If the active tab is a member of the group, the new tab inherits this group on creation.
        // Otherwise we must manually update it here, and place this new tab at the end of the group.
        if !active_is_member {
            let new_idx = self.active_tab_index;
            // Resolve the destination from the group's existing members before
            // adding the new tab to the group.
            let target_index = group_member_indices(&self.tabs, group_id)
                .last()
                .map(|last| last + 1)
                .unwrap_or(self.tabs.len());
            if let Some(tab) = self.tabs.get_mut(new_idx) {
                tab.group_id = Some(group_id);
            }
            self.move_tab_to_index(new_idx, target_index, ctx);
        }
        ctx.notify();
    }

    /// Moves the whole group up or down by one "slot", where a slot is the
    /// immediate neighbor in that direction — either a single ungrouped tab or
    /// an entire adjacent group.
    ///
    /// We find the neighbor by looking at the tab directly above the group's
    /// first member or directly below its last member. When that neighbor tab
    /// is itself grouped, we expand to that group's full index range (via
    /// `group_member_index_range`) so the whole neighbor group is hopped over
    /// as a unit, instead of landing the group in the middle of it.
    fn move_tab_group(
        &mut self,
        group_id: TabGroupId,
        direction: TabMovement,
        ctx: &mut ViewContext<Self>,
    ) {
        if !FeatureFlag::GroupedTabs.is_enabled() {
            return;
        }
        let Some((first, last)) = group_member_index_range(&self.tabs, group_id) else {
            return;
        };
        match direction {
            TabMovement::Left => {
                if first == 0 {
                    return;
                }
                // The upward neighbor is the tab directly above the group.
                let above_index = first - 1;
                // If that neighbor is itself grouped, land above its whole
                // group (its first member); otherwise land at the lone tab.
                let target = match self.tabs[above_index].group_id {
                    Some(other_gid) => group_member_index_range(&self.tabs, other_gid)
                        .map(|(f, _)| f)
                        .unwrap_or(above_index),
                    None => above_index,
                };
                self.move_group_block(group_id, target, ctx);
            }
            TabMovement::Right => {
                if last + 1 >= self.tabs.len() {
                    return;
                }
                // The downward neighbor is the tab directly below the group.
                let below_index = last + 1;
                // If that neighbor is itself grouped, expand to its whole
                // group's last member; otherwise it's just the lone tab.
                let below_block_last = match self.tabs[below_index].group_id {
                    Some(other_gid) => group_member_index_range(&self.tabs, other_gid)
                        .map(|(_, l)| l)
                        .unwrap_or(below_index),
                    None => below_index,
                };
                // Land in the slot just past that neighbor.
                let target = below_block_last + 1;
                self.move_group_block(group_id, target, ctx);
            }
        }
    }

    fn close_tabs_outside_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>) {
        let Some((first, _last)) = group_member_index_range(&self.tabs, group_id) else {
            return;
        };
        let indices: Vec<usize> = (0..self.tabs.len())
            .filter(|i| self.tabs[*i].group_id != Some(group_id))
            .collect();
        if indices.is_empty() {
            return;
        }
        self.close_tabs(
            indices.into_iter(),
            OpenDialogSource::CloseOtherTabs { tab_index: first },
            false,
            true,
            ctx,
        );
    }

    fn close_tabs_above_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>) {
        let Some((first, _last)) = group_member_index_range(&self.tabs, group_id) else {
            return;
        };
        if first == 0 {
            return;
        }
        self.close_tabs_direction(first, TabMovement::Left, false, ctx);
    }

    fn close_tabs_below_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>) {
        let Some((_first, last)) = group_member_index_range(&self.tabs, group_id) else {
            return;
        };
        if last + 1 >= self.tabs.len() {
            return;
        }
        self.close_tabs_direction(last, TabMovement::Right, false, ctx);
    }

    /// Moves the contiguous run of tabs belonging to `group_id` so its first
    /// member ends up at `target`, an index into the current tab list. Uses
    /// `Vec::drain` + `Vec::splice` to preserve member order, and re-derives
    /// the active tab index across the move.
    fn move_group_block(
        &mut self,
        group_id: TabGroupId,
        target: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some((first, last)) = group_member_index_range(&self.tabs, group_id) else {
            return;
        };
        // `target` indexes the current list. Draining the group removes
        // `block_size` tabs ahead of any later index, so shift a past-the-group
        // target left to land in the right spot after reinsertion.
        let block_size = last - first + 1;
        let insert_at = if target > last {
            target - block_size
        } else {
            target
        };
        if insert_at == first {
            return;
        }

        let active_pane_group_id = self
            .tabs
            .get(self.active_tab_index)
            .map(|tab| tab.pane_group.id());

        let drained: Vec<TabData> = self.tabs.drain(first..=last).collect();
        self.tabs.splice(insert_at..insert_at, drained);

        if let Some(active_id) = active_pane_group_id {
            if let Some(new_idx) = self
                .tabs
                .iter()
                .position(|tab| tab.pane_group.id() == active_id)
            {
                self.active_tab_index = new_idx;
            }
        }

        ctx.notify();
    }

    /// Flips `tab_index`'s group membership. Callers are responsible for
    /// positioning the tab so groups remain contiguous; this method only
    /// mutates `group_id` and prunes the old group when empty.
    pub fn assign_tab_to_group(
        &mut self,
        tab_index: usize,
        group_id: Option<TabGroupId>,
        ctx: &mut ViewContext<Self>,
    ) {
        if tab_index >= self.tabs.len() {
            log::warn!(
                "Tried to assign tab {tab_index} to a group but only {} tabs exist",
                self.tabs.len()
            );
            return;
        }
        if let Some(gid) = group_id {
            if !self.tab_groups.contains_key(&gid) {
                log::warn!("Tried to assign tab {tab_index} to unknown group {gid:?}");
                return;
            }
        }

        if self.tabs[tab_index].group_id == group_id {
            return;
        }

        let previous_group_id = self.tabs[tab_index].group_id;
        self.tabs[tab_index].group_id = group_id;

        if let Some(previous_group_id) = previous_group_id {
            self.prune_empty_tab_group(previous_group_id, ctx);
        }

        ctx.notify();
    }

    /// Removes a tab group from the workspace if no tabs reference it.
    fn prune_empty_tab_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>) {
        let has_members = group_member_indices(&self.tabs, group_id).next().is_some();
        if !has_members {
            self.tab_groups.remove(&group_id);
            ctx.notify();
        }
    }

    /// Moves the tab at `from` to position `to` (`Vec::insert` semantics).
    /// The active-tab tracker follows the moved tab.
    fn move_tab_to_index(&mut self, from: usize, to: usize, ctx: &mut ViewContext<Self>) {
        if from >= self.tabs.len() {
            log::debug!(
                "move_tab_to_index: from {from} out of bounds (len {})",
                self.tabs.len()
            );
            return;
        }
        let adjusted_to = if to > from { to - 1 } else { to };
        let adjusted_to = adjusted_to.min(self.tabs.len().saturating_sub(1));
        // No-op when the requested destination resolves to the tab's current slot.
        if from == adjusted_to {
            return;
        }

        let active_pane_group_id = self
            .tabs
            .get(self.active_tab_index)
            .map(|tab| tab.pane_group.id());

        let tab = self.tabs.remove(from);
        self.tabs.insert(adjusted_to, tab);

        if let Some(pane_group_id) = active_pane_group_id {
            if let Some(new_active) = self
                .tabs
                .iter()
                .position(|t| t.pane_group.id() == pane_group_id)
            {
                self.active_tab_index = new_active;
            }
        }
        ctx.notify();
    }

    /// Moves the tab at `from_index` so it ends up at `to_index`, sliding the
    /// tabs in between over to fill the gap. This is a `Vec::remove` +
    /// `Vec::insert` (not a `swap`), so it works for arbitrary distances, such
    /// as hopping a tab over a whole group, not just adjacent slots. `to_index`
    /// is an index into the current list and is exactly where the moved tab
    /// ends up.
    ///
    /// `active_tab_index` stores a position, not a reference to a tab, so after
    /// reordering we recompute it to keep the same tab visually active.
    /// Removing `from` then inserting at `to` slides every tab strictly between
    /// the two endpoints by one slot toward `from`; tabs outside that span keep
    /// their index.
    fn hop_tab_to_index(
        &mut self,
        from_index: usize,
        to_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        if from_index == to_index || from_index >= self.tabs.len() || to_index >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(from_index);
        self.tabs.insert(to_index, tab);

        let old_active = self.active_tab_index;
        self.active_tab_index = if old_active == from_index {
            // The active tab is the one we just moved; it follows to `to_index`.
            to_index
        } else if from_index < to_index {
            // Forward move: tabs in `(from_index, to_index]` slid left by one.
            if old_active > from_index && old_active <= to_index {
                old_active - 1
            } else {
                old_active
            }
        } else if old_active >= to_index && old_active < from_index {
            // Backward move: tabs in `[to_index, from_index)` slid right by one.
            old_active + 1
        } else {
            // Active tab is outside the affected span; its index is unchanged.
            old_active
        };

        ctx.notify();
    }

    pub fn toggle_tab_right_click_menu(
        &mut self,
        tab_index: usize,
        anchor: TabContextMenuAnchor,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.show_tab_right_click_menu.is_some() {
            self.show_tab_right_click_menu = None;
            self.hide_move_to_group_sidecar(ctx);
            ctx.notify();
            return;
        }

        let menu_items = {
            let tab = &self.tabs[tab_index];
            tab.menu_items(tab_index, self.tabs.len(), &self.tab_groups, ctx)
        };
        ctx.update_view(&self.tab_right_click_menu, |context_menu, view_ctx| {
            context_menu.set_items(menu_items, view_ctx);
        });
        self.show_tab_group_right_click_menu = None;
        self.show_tab_right_click_menu = Some((tab_index, anchor));
        ctx.focus(&self.tab_right_click_menu);
        ctx.notify();
    }

    pub fn toggle_tab_group_right_click_menu(
        &mut self,
        group_id: TabGroupId,
        anchor: TabContextMenuAnchor,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.show_tab_group_right_click_menu.is_some() {
            self.show_tab_group_right_click_menu = None;
            ctx.notify();
            return;
        }
        if !self.tab_groups.contains_key(&group_id) {
            return;
        }

        let menu_items = self.tab_group_menu_items(group_id);
        ctx.update_view(&self.tab_right_click_menu, |context_menu, view_ctx| {
            context_menu.set_items(menu_items, view_ctx);
        });
        self.show_tab_right_click_menu = None;
        self.hide_move_to_group_sidecar(ctx);
        self.show_tab_group_right_click_menu = Some((group_id, anchor));
        ctx.focus(&self.tab_right_click_menu);
        ctx.notify();
    }

    pub fn toggle_vertical_tabs_pane_context_menu(
        &mut self,
        tab_index: usize,
        target: VerticalTabsPaneContextMenuTarget,
        position: Vector2F,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.show_tab_right_click_menu.is_some() {
            self.show_tab_right_click_menu = None;
            ctx.notify();
            return;
        }

        let Some(tab) = self.tabs.get(tab_index) else {
            log::warn!("Tried to open pane context menu for a missing tab");
            return;
        };
        let pane = target.locator();
        if tab.pane_group.id() != pane.pane_group_id {
            log::warn!("Tried to open pane context menu for a pane in another tab");
            return;
        }

        let pane_name_target = match target {
            VerticalTabsPaneContextMenuTarget::ClickedPane(locator) => PaneNameMenuTarget {
                locator,
                rename_label: "Rename pane",
                reset_label: "Reset pane name",
            },
            VerticalTabsPaneContextMenuTarget::ActivePane(locator) => PaneNameMenuTarget {
                locator,
                rename_label: "Rename active pane",
                reset_label: "Reset active pane name",
            },
        };
        let menu_items = tab.menu_items_with_pane_name_target(
            tab_index,
            self.tabs.len(),
            &self.tab_groups,
            Some(pane_name_target),
            ctx,
        );

        ctx.update_view(&self.tab_right_click_menu, |context_menu, view_ctx| {
            context_menu.set_items(menu_items, view_ctx);
        });
        self.show_tab_right_click_menu = Some((tab_index, TabContextMenuAnchor::Pointer(position)));
        ctx.focus(&self.tab_right_click_menu);
        ctx.notify();
    }

    /// The tab bar overflow menu is the context menu that appears when
    /// a user clicks "Update Warp" in the top right of the tab bar.
    pub fn toggle_tab_bar_overflow_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if self.show_tab_bar_overflow_menu {
            self.close_tab_bar_overflow_menu(ctx);
            return;
        }

        let mut menu_items = vec![];
        if FeatureFlag::Autoupdate.is_enabled() && ChannelState::show_autoupdate_menu_items() {
            if let Some(version) = ChannelState::app_version() {
                menu_items.push(
                    MenuItemFields::new(format!("Current version is {version}"))
                        .with_disabled(true)
                        .into_item(),
                );
                match autoupdate::get_update_state(ctx) {
                    AutoupdateStage::UpdateReady { new_version, .. }
                    | AutoupdateStage::UpdatedPendingRestart { new_version } => menu_items.push(
                        MenuItemFields::new(format!("Install update ({})", new_version.version))
                            .with_on_select_action(WorkspaceAction::ApplyUpdate)
                            .into_item(),
                    ),
                    AutoupdateStage::Updating { new_version, .. } => menu_items.push(
                        MenuItemFields::new(format!("Updating to ({})", new_version.version))
                            .with_disabled(true)
                            .into_item(),
                    ),
                    AutoupdateStage::UnableToUpdateToNewVersion { .. } => menu_items.push(
                        MenuItemFields::new("Update Warp manually")
                            .with_on_select_action(WorkspaceAction::DownloadNewVersion)
                            .into_item(),
                    ),
                    AutoupdateStage::NoUpdateAvailable
                    | AutoupdateStage::CheckingForUpdate
                    | AutoupdateStage::DownloadingUpdate
                    | AutoupdateStage::UnableToLaunchNewVersion { .. } => {}
                }
            }
        }

        ctx.update_view(&self.tab_bar_overflow_menu, |context_menu, view_ctx| {
            context_menu.set_items(menu_items, view_ctx);
        });
        self.show_tab_bar_overflow_menu = true;
        ctx.focus(&self.tab_bar_overflow_menu);
        ctx.notify();
    }

    fn read_from_active_terminal_view<T>(
        &self,
        ctx: &AppContext,
        accessor: impl FnOnce(&TerminalView) -> T,
    ) -> Option<T> {
        self.get_pane_group_view(self.active_tab_index)
            .and_then(|view| {
                view.read(ctx, |pane_group, ctx| {
                    pane_group
                        .active_session_view(ctx)
                        .map(|terminal_view_handle| {
                            terminal_view_handle.read(ctx, |terminal, _| accessor(terminal))
                        })
                })
            })
    }

    pub fn active_terminal_id(&self, app: &AppContext) -> Option<EntityId> {
        self.read_from_active_terminal_view(app, |terminal| terminal.id())
    }

    /// Retrieves the entity id of the active current active input. This is needed
    /// by the Welcome Tip View in order to know where to dispatch the actions
    /// directly from the tip menu.
    fn active_input_id(&self, app: &AppContext) -> Option<EntityId> {
        self.read_from_active_terminal_view(app, |terminal| terminal.input().id())
    }

    /// Gets the ID of the active terminal session, if any.
    pub fn active_session_id(&self, ctx: &ViewContext<Self>) -> Option<SessionId> {
        self.get_pane_group_view(self.active_tab_index)
            .and_then(|view| {
                view.read(ctx, |pane_group, ctx| {
                    pane_group
                        .active_session_view(ctx)
                        .and_then(|terminal_view_handle| {
                            terminal_view_handle
                                .read(ctx, |terminal, _| terminal.active_block_session_id())
                        })
                })
            })
    }


    fn check_and_trigger_telemetry_banner_for_existing_users(
        &mut self,
        ctx: &mut ViewContext<Self>,
    ) {
        if FeatureFlag::GlobalAIAnalyticsBanner.is_enabled()
            && PrivacySettings::as_ref(ctx).is_telemetry_enabled
        {
            if let Some(terminal_view_handle) = self.active_session_view(ctx) {
                terminal_view_handle.update(ctx, |terminal_view, ctx| {
                    terminal_view.insert_telemetry_banner(true, ctx);
                });
            }
        }
    }

    fn should_trigger_get_started_onboarding(&self, ctx: &mut ViewContext<Self>) -> bool {
        // Onboarding requires a real user to interact with it; suppress when
        // running in a headless mode like the SDK/CLI.
        if !AppExecutionMode::as_ref(ctx).can_show_onboarding() {
            return false;
        }

        if !FeatureFlag::GetStartedTab.is_enabled() {
            return false;
        }

        if self.auth_state.is_onboarded().unwrap_or_default() {
            return false;
        }

        if self.auth_state.is_anonymous_or_logged_out() {
            return false;
        }

        // If AgentOnboarding is enabled and the user is NOT in the control group for the
        // AgentOnboarding experiment, don't show Get Started onboarding.
        if self.should_show_agent_onboarding(ctx) {
            return false;
        }

        true
    }

    fn trigger_get_started_onboarding(&mut self, ctx: &mut ViewContext<Self>) {
        self.add_get_started_tab(ctx);
        // After onboarding is triggered, mark the user as onboarded
        AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
            auth_manager.set_user_onboarded(ctx);
        });
    }

    /// If the user is new and therefore has not seen the in app onboarding,
    /// triggers the welcome block to be shown after bootstrapping is completed.
    fn check_and_trigger_onboarding(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        // Onboarding requires a real user to interact with it; suppress when
        // running in a headless mode like the SDK/CLI.
        if !AppExecutionMode::as_ref(ctx).can_show_onboarding() {
            return false;
        }

        if !self.auth_state.is_onboarded().unwrap_or_default() {
            if self.should_show_agent_onboarding(ctx) {
                // If the user is anonymous, we shouldn't trigger agent onboarding.
                // It will not display anyway, and we don't want to mark the user as onboarded.
                if self.auth_state.is_anonymous_or_logged_out() {
                    return false;
                }
                self.trigger_agent_onboarding(ctx);
            } else {
                self.trigger_legacy_onboarding(ctx);
            }

            // Add telemetry banner for new users BEFORE the agentic onboarding blocks.
            if let Some(terminal_view_handle) = self.active_session_view(ctx) {
                terminal_view_handle.update(ctx, |terminal_view, ctx| {
                    terminal_view.insert_telemetry_banner(false, ctx);
                });
            }

            // After onboarding is triggered, mark the user as onboarded
            AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                auth_manager.set_user_onboarded(ctx);
            });

            return true;
        }

        false
    }

    fn trigger_legacy_onboarding(&self, ctx: &mut ViewContext<Self>) {
        self.dispatch_onboarding(
            TerminalAction::OnboardingFlow(OnboardingVersion::Legacy),
            ctx,
        );
    }

    fn trigger_agent_onboarding(&self, ctx: &mut ViewContext<Self>) {
        log::error!(
            "Triggering agent onboarding callout flow but not during initial login. This should not normally happen."
        );
        let version = if FeatureFlag::AgentView.is_enabled() {
            AgentOnboardingVersion::AgentModality {
                has_project: false,
                intention: OnboardingIntention::AgentDrivenDevelopment,
            }
        } else {
            AgentOnboardingVersion::UniversalInput { has_project: false }
        };
        self.dispatch_onboarding(
            TerminalAction::OnboardingFlow(OnboardingVersion::Agent(version)),
            ctx,
        );
    }

    fn dispatch_onboarding(&self, action: TerminalAction, ctx: &mut ViewContext<Self>) {
        if let Some(pane_group_handle) = self.get_pane_group_view(self.active_tab_index) {
            pane_group_handle.update(ctx, |pane_group, ctx| {
                if let Some(terminal_view_handle) = pane_group.active_session_view(ctx) {
                    let window_id = ctx.window_id();
                    ctx.dispatch_typed_action_for_view(
                        window_id,
                        terminal_view_handle.id(),
                        &action,
                    );
                }
            });
        }
    }









    fn open_settings_pane(
        &mut self,
        page: Option<SettingsSection>,
        search_query: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        // Ensure there is only one settings pane per window
        let settings_pane_manager = SettingsPaneManager::handle(ctx);
        if let Some(locator) = settings_pane_manager.as_ref(ctx).find_pane(ctx.window_id()) {
            // Update to new page if specified
            if let Some(page) = page {
                self.settings_pane.update(ctx, |settings_pane, ctx| {
                    settings_pane.set_and_refresh_current_page(page, ctx);
                    if let Some(search_query) = search_query {
                        settings_pane.set_search_query(search_query, ctx);
                    }
                });
            }
            // Navigate to and focus existing pane
            self.focus_pane(locator, ctx);
            return;
        }

        let ps1_grid_info = self.active_session_ps1_grid_info(ctx);
        // Open new tab and update current page
        self.settings_pane.update(ctx, move |settings_pane, ctx| {
            // TODO: This check shouldn't be necessary, but `active_session_ps1_grid_info` returns
            // None when the active tab has no running terminal sessions, e.g. if it contains only
            // notebooks/workflow panes.
            if ps1_grid_info.is_some() {
                settings_pane.set_ps1_info(ps1_grid_info, ctx);
            }
        });

        let panes_layout = PanesLayout::Snapshot(Box::new(PaneNodeSnapshot::Leaf(LeafSnapshot {
            is_focused: true,
            custom_vertical_tabs_title: None,
            contents: LeafContents::Settings(SettingsPaneSnapshot::Local {
                current_page: page.unwrap_or_default(),
                search_query: search_query.map(|s| s.to_owned()),
            }),
        })));
        self.add_tab_with_pane_layout(
            panes_layout,
            Arc::new(HashMap::new()),
            Some("Settings".to_owned()),
            ctx,
        );
    }

    fn attach_path_as_context(&mut self, path: PathBuf, ctx: &mut ViewContext<Self>) {
        let Some(view) = self.active_session_view(ctx) else {
            log::warn!("No active terminal view session when trying to attach path as context");
            return;
        };

        view.update(ctx, |terminal_view, ctx| {
            terminal_view.attach_path_as_context(&path, ctx);
        });
    }

    fn cd_to_directory(&mut self, path: PathBuf, ctx: &mut ViewContext<Self>) {
        let Some(input_handle) = self.get_active_input_view_handle(ctx) else {
            log::warn!("No active input view when trying to cd to directory");
            return;
        };

        let Some(path_str) = path.to_str() else {
            log::warn!("Could not convert path to string for cd command");
            return;
        };

        let cd_command = format!("cd {}", shell_words::quote(path_str));
        input_handle.update(ctx, |input_view, ctx| {
            input_view.replace_buffer_content(&cd_command, ctx);
        });
    }

    fn open_directory_in_new_tab(&mut self, path: PathBuf, ctx: &mut ViewContext<Self>) {
        let options = NewTerminalOptions::default().with_initial_directory(path);
        self.add_tab_with_pane_layout(
            PanesLayout::SingleTerminal(Box::new(options)),
            Arc::new(HashMap::new()),
            None,
            ctx,
        );
    }

    #[cfg(feature = "local_fs")]





    pub(super) fn active_session_view(
        &self,
        ctx: &mut ViewContext<Self>,
    ) -> Option<ViewHandle<TerminalView>> {
        self.active_tab_pane_group()
            .read(ctx, |pane_group, ctx| pane_group.active_session_view(ctx))
    }

    pub fn toggle_welcome_tips_visiblity(&mut self, ctx: &mut ViewContext<Self>) {
        self.welcome_tips_view_state.toggle_popup();
        if self.welcome_tips_view_state.is_popup_open() {
            let input_id = self.active_input_id(ctx);
            self.welcome_tips_view.update(ctx, |tips_view, ctx| {
                tips_view.set_action_target(ctx.window_id(), input_id, ctx)
            });

            send_telemetry_from_ctx!(TelemetryEvent::OpenWelcomeTips, ctx);
        }
        ctx.focus(&self.welcome_tips_view);
        ctx.notify();
    }

    pub fn close_tab_bar_overflow_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_tab_bar_overflow_menu = false;
        ctx.notify();
    }

    /// Find an active session and pre-fill the input editor the Warp executable with the
    /// [`rift_cli::Command::DumpDebugInfo`] subcommand.
    fn dump_debug_info(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(exec) = std::env::current_exe()
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
        {
            let command = format!("{exec} {}", rift_cli::dump_debug_info_flag());
            // Get the active session for this tab if it exists.
            let mut active_session_handle = self
                .active_tab_pane_group()
                .read(ctx, |pane_group_view, ctx| {
                    pane_group_view.active_session_view(ctx)
                });
            // A tab may not have any active session, say if it only contains notebook(s). If
            // that's the case, create a new tab.
            if active_session_handle.is_none() {
                self.add_new_session_tab_with_default_mode(
                    NewSessionSource::Tab,
                    None,
                    None,
                    false,
                    ctx,
                );
            }
            active_session_handle = self
                .active_tab_pane_group()
                .read(ctx, |pane_group_view, ctx| {
                    pane_group_view.active_session_view(ctx)
                });
            if let Some(terminal_view_handle) = active_session_handle {
                terminal_view_handle.update(ctx, |terminal_view, ctx| {
                    terminal_view.set_pending_command(&command, ctx);
                });
            }
        }
    }

    /// Install the Warp CLI by creating a symlink in /usr/local/bin
    #[cfg(target_os = "macos")]
    fn install_cli(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(async { cli_install::install_cli() }, |view, result, ctx| {
            match result {
                Ok(_) => {
                    let command_name = ChannelState::channel().cli_command_name();
                    let message = format!("Successfully installed the Oz CLI! You can now run '{command_name}' from the command line.");
                    view.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::success(message.to_string())
                            .with_link(
                                ToastLink::new("Learn more".to_string()).with_href(
                                    "https://docs.warp.dev/reference/cli".to_string(),
                                ),
                            );
                        toast_stack.add_ephemeral_toast(toast, ctx);
                    });
                }
                Err(error) => {
                    let error_message = format!("Failed to install Oz command: {error}");
                    log::error!("{error_message}");
                    view.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(error_message);
                        toast_stack.add_persistent_toast(toast, ctx);
                    });
                }
            }
        });
    }

    /// Uninstall the Warp CLI by removing the symlink from /usr/local/bin
    #[cfg(target_os = "macos")]
    fn uninstall_cli(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async { cli_install::uninstall_cli() },
            |view, result, ctx| match result {
                Ok(_) => {
                    let message = "Successfully uninstalled the Oz command.";
                    view.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::success(message.to_string());
                        toast_stack.add_ephemeral_toast(toast, ctx);
                    });
                }
                Err(error) => {
                    let error_message = format!("Failed to uninstall Oz command: {error}");
                    log::error!("{error_message}");
                    view.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(error_message);
                        toast_stack.add_persistent_toast(toast, ctx);
                    });
                }
            },
        );
    }


    fn toggle_recording_mode(&self, ctx: &mut ViewContext<Self>) {
        DebugSettings::handle(ctx).update(ctx, |debug_settings, settings_ctx| {
            report_if_error!(debug_settings
                .recording_mode
                .toggle_and_save_value(settings_ctx));
        });
    }

    fn toggle_in_band_generators(&self, ctx: &mut ViewContext<Self>) {
        DebugSettings::handle(ctx).update(ctx, |debug_settings, settings_ctx| {
            report_if_error!(debug_settings
                .are_in_band_generators_for_all_sessions_enabled
                .toggle_and_save_value(settings_ctx));
        });
    }

    fn toggle_debug_network_status(&self, ctx: &mut ViewContext<Self>) {
        NetworkStatus::handle(ctx).update(ctx, |network_status, network_ctx| {
            let is_reachable = network_status.is_online();
            let new_is_reachable = !is_reachable;
            if new_is_reachable {
                log::info!("Manually toggled network status to be reachable");
            } else {
                log::info!("Manually toggled network status to be not reachable");
            }
            network_status.reachability_changed(new_is_reachable, network_ctx);
        });
    }

    fn toggle_show_memory_stats(&self, ctx: &mut ViewContext<Self>) {
        DebugSettings::handle(ctx).update(ctx, |debug_settings, ctx| {
            report_if_error!(debug_settings.show_memory_stats.toggle_and_save_value(ctx));
        })
    }


    pub fn open_or_toggle_warp_drive(
        &mut self,
        toggle: bool,
        explicit_user_action: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        // Closing all left panels will also close warp drive so we need to retrieve
        // whether warp drive was open first, and toggle based on the initial value.
        let was_warp_drive_open = self.current_workspace_state.is_warp_drive_open;
        self.current_workspace_state.close_all_left_panels();
        self.current_workspace_state.is_warp_drive_open =
            if toggle { !was_warp_drive_open } else { true };

        // Set selected object to None upon toggle close of Warp Drive
        if !self.current_workspace_state.is_warp_drive_open {
            self.set_selected_object(None, ctx);
            self.focus_active_tab(ctx);
        }

        // Reset focused index when opening/toggling Warp Drive open
        if self.current_workspace_state.is_warp_drive_open {
            self.reset_focused_index_in_warp_drive(true, ctx);
        }

        ctx.notify();

        // Telemetry and welcome tip logic is only for when the user explicitly opens Warp Drive
        // AND warp drive wasn't open before. There are other scenarios where we open Warp Drive like:
        // new user onboarding, user joins a team, etc so we want to avoid counting those.
        if explicit_user_action
            && !was_warp_drive_open
            && self.current_workspace_state.is_warp_drive_open
        {
            send_telemetry_from_ctx!(
                TelemetryEvent::WarpDriveOpened {
                    source: WarpDriveSource::Legacy,
                    is_code_mode_v2: false
                },
                ctx
            );
            self.tips_completed.update(ctx, |tips_completed, ctx| {
                mark_feature_used_and_write_to_user_defaults(
                    Tip::Action(TipAction::OpenWarpDrive),
                    tips_completed,
                    ctx,
                );
                ctx.notify();
            });
        }
    }

    fn open_resource_center_main_page(&mut self, ctx: &mut ViewContext<Self>) {
        // Set current page to Main
        self.resource_center_view
            .update(ctx, |resource_center_view, ctx| {
                resource_center_view.set_current_page(ResourceCenterPage::Main, ctx)
            });

        // Open side panel
        self.current_workspace_state.is_resource_center_open = true;
    }

    pub fn toggle_resource_center(&mut self, ctx: &mut ViewContext<Self>) {
        // Close AI Assistant panel when resource center is opened
        if !self.current_workspace_state.is_resource_center_open {
            self.current_workspace_state.is_ai_assistant_panel_open = false;
            self.focus_active_tab(ctx);
        }

        if !self.current_workspace_state.is_resource_center_open {
            self.open_resource_center_main_page(ctx);
            send_telemetry_from_ctx!(TelemetryEvent::ResourceCenterOpened, ctx);
        } else {
            // Close side panel
            self.current_workspace_state.is_resource_center_open = false;
        }

        self.update_resource_center_action_target(ctx);
        ctx.notify();
    }

    fn open_left_panel(&mut self, ctx: &mut ViewContext<Self>) {
        self.left_panel_open = true;

        let active_pane_group = self.active_tab_pane_group().clone();
        active_pane_group.update(ctx, |pane_group, ctx| {
            pane_group.set_left_panel_open(true, ctx);
        });

        ctx.notify();
    }

    fn close_left_panel(&mut self, ctx: &mut ViewContext<Self>) {
        self.left_panel_open = false;

        let active_pane_group = self.active_tab_pane_group().clone();
        active_pane_group.update(ctx, |pane_group, ctx| {
            pane_group.set_left_panel_open(false, ctx);
        });

        ctx.notify();
    }

    fn toggle_vertical_tabs_panel(&mut self, ctx: &mut ViewContext<Self>) {
        self.vertical_tabs_panel_open = !self.vertical_tabs_panel_open;
        if !self.vertical_tabs_panel_open {
            self.close_vertical_tabs_settings_popup();
            self.vertical_tabs_panel.clear_detail_sidecar();
        }
        self.sync_window_button_visibility(ctx);
        ctx.notify();
    }

    fn close_vertical_tabs_settings_popup(&mut self) {
        self.vertical_tabs_panel.show_settings_popup = false;
    }


    fn toggle_left_panel(&mut self, ctx: &mut ViewContext<Self>) {
        let active_pane_group = self.active_tab_pane_group().clone();

        let was_open = active_pane_group.read(ctx, |pane_group, _| pane_group.left_panel_open);
        let new_state = !was_open;

        if new_state {
            self.open_left_panel(ctx);
        } else {
            self.close_left_panel(ctx);
        }

        // If we are opening the panel, set width based on the most recent tab's width if available,
        // otherwise compute default width from current window size. Also auto-expand the project
        // explorer if it's the active left panel view.
        if new_state {
            let window_id = ctx.window_id();
            let resizable_data = ResizableData::handle(ctx);
            if let Some(handle) = resizable_data
                .as_ref(ctx)
                .get_handle(window_id, ModalType::LeftPanelWidth)
            {
                if let Ok(mut state) = handle.lock() {
                    // Get the current width from ResizableData - this reflects the most recent tab's width
                    let current_width = state.size();

                    // Only recompute default if the current width is at the default value
                    // This preserves the width from the most recent tab
                    if current_width == DEFAULT_LEFT_PANEL_WIDTH {
                        let has_horizontal_split = active_pane_group
                            .read(ctx, |pane_group, _| pane_group.has_horizontal_split());
                        let (left_width, _right_width) =
                            compute_default_panel_widths(ctx, window_id, has_horizontal_split);
                        state.set_size(left_width);
                    }
                    // If current_width is not the default, it means we have a width from a previous tab,
                    // so we don't need to do anything - the width is already preserved
                }
            }

            // Auto-expand the file tree when the left panel is opened and the project explorer is
            // the active view.
            let file_tree_active = self
                .left_panel_view
                .read(ctx, |lp, _| lp.is_file_tree_active());
            if file_tree_active {
                self.left_panel_view.update(ctx, |left_panel, ctx| {
                    left_panel.auto_expand_active_file_tree_to_most_recent_directory(ctx);
                });
            }
        }

        if !new_state {
            self.focus_active_tab(ctx);
        }

        ctx.notify();
    }


    fn user_menu_items(&self, app: &AppContext) -> Vec<MenuItem<WorkspaceAction>> {
        let mut items = Vec::new();
        if !self.auth_state.is_anonymous_or_logged_out() {
            let name = self.auth_state.username_for_display().unwrap_or_default();
            items.push(MenuItemFields::new(name).with_disabled(true).into_item())
        }

        let appearance = Appearance::as_ref(app);

        // Render the subtle autoupdate UI if autoupdate is ready and there is no incoming prominent update version.
        if FeatureFlag::Autoupdate.is_enabled()
            && FeatureFlag::AutoupdateUIRevamp.is_enabled()
            && ChannelState::show_autoupdate_menu_items()
        {
            match autoupdate::get_update_state(app) {
                AutoupdateStage::UpdateReady { new_version, .. }
                | AutoupdateStage::UpdatedPendingRestart { new_version }
                    if !is_incoming_version_past_current(
                        new_version.last_prominent_update.as_deref(),
                    ) =>
                {
                    items.push(
                        MenuItemFields::new("Update and relaunch Warp")
                            .with_on_select_action(WorkspaceAction::ApplyUpdate)
                            .with_override_text_color(appearance.theme().ansi_fg_red())
                            .into_item(),
                    )
                }
                AutoupdateStage::Updating { new_version, .. }
                    if !is_incoming_version_past_current(
                        new_version.last_prominent_update.as_deref(),
                    ) =>
                {
                    items.push(
                        MenuItemFields::new(format!("Updating to ({})", new_version.version))
                            .with_disabled(true)
                            .into_item(),
                    )
                }
                AutoupdateStage::UnableToUpdateToNewVersion { new_version }
                    if !is_incoming_version_past_current(
                        new_version.last_prominent_update.as_deref(),
                    ) =>
                {
                    items.push(
                        MenuItemFields::new("Update Warp manually")
                            .with_on_select_action(WorkspaceAction::DownloadNewVersion)
                            .with_override_text_color(appearance.theme().ansi_fg_red())
                            .into_item(),
                    )
                }
                _ => {}
            }
        }

        items.extend([
            MenuItemFields::new("What's new")
                .with_on_select_action(WorkspaceAction::ViewLatestChangelog)
                .into_item(),
            MenuItemFields::new("Settings")
                .with_on_select_action(WorkspaceAction::ShowSettings)
                .into_item(),
            MenuItemFields::new("Keyboard shortcuts")
                .with_on_select_action(WorkspaceAction::ToggleKeybindingsPage)
                .into_item(),
            MenuItem::Separator,
            MenuItemFields::new("Documentation")
                .with_on_select_action(WorkspaceAction::ViewUserDocs)
                .into_item(),
            MenuItemFields::new("Feedback")
                .with_on_select_action(WorkspaceAction::SendFeedback)
                .into_item(),
        ]);

        #[cfg(not(target_family = "wasm"))]
        items.push(
            MenuItemFields::new("View Warp logs")
                .with_on_select_action(WorkspaceAction::ViewLogs)
                .into_item(),
        );

        items.extend([
            MenuItemFields::new("Slack")
                .with_on_select_action(WorkspaceAction::JoinSlack)
                .into_item(),
            MenuItem::Separator,
        ]);

        if self.auth_state.is_anonymous_or_logged_out() {
            items.push(
                MenuItemFields::new("Sign up")
                    .with_on_select_action(WorkspaceAction::SignupAnonymousUser)
                    .into_item(),
            );
        }

        // Check if the user is on any paid plan to determine whether to show "Billing and Usage" or "Upgrade"
        let is_on_paid_plan = UserWorkspaces::as_ref(app)
            .current_workspace()
            .map(|workspace| workspace.billing_metadata.is_user_on_paid_plan())
            .unwrap_or(false);

        if is_on_paid_plan {
            items.push(
                MenuItemFields::new("Billing and usage")
                    .with_on_select_action(WorkspaceAction::ShowSettingsPage(
                        SettingsSection::BillingAndUsage,
                    ))
                    .into_item(),
            );
        } else {
            items.push(
                MenuItemFields::new("Upgrade")
                    .with_on_select_action(WorkspaceAction::ShowUpgrade)
                    .into_item(),
            );
        }

        items.push(
            MenuItemFields::new("Invite a friend")
                .with_on_select_action(WorkspaceAction::ShowReferralSettingsPage)
                .into_item(),
        );

        if !self.auth_state.is_anonymous_or_logged_out() {
            items.push(
                MenuItemFields::new("Log out")
                    .with_on_select_action(WorkspaceAction::LogOut)
                    .into_item(),
            );
        }
        items
    }

    fn selected_new_session_sidecar_selection(
        &self,
        ctx: &AppContext,
    ) -> Option<NewSessionSidecarSelection> {
        self.new_session_sidecar_menu.read(ctx, |menu, _| {
            menu.selected_item().and_then(|item| match item {
                MenuItem::Item(fields) => fields.on_select_action().cloned(),
                _ => None,
            })
        })
    }

    fn execute_new_session_sidecar_selection(
        &mut self,
        selection: NewSessionSidecarSelection,
        ctx: &mut ViewContext<Self>,
    ) {
        match selection {
            NewSessionSidecarSelection::OpenWorktreeRepo { repo_path } => {
                self.open_worktree_in_repo(repo_path, ctx);
            }
        }
    }

    fn toggle_user_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.is_user_menu_open = !self.is_user_menu_open;
        if self.is_user_menu_open {
            let items = self.user_menu_items(ctx);
            self.user_menu.update(ctx, |menu, ctx| {
                menu.set_items(items, ctx);
            });
        }
        ctx.focus(&self.user_menu);
        ctx.notify();
    }

    pub fn toggle_keybindings_page(&mut self, ctx: &mut ViewContext<Self>) {
        let current_page = self
            .resource_center_view
            .read(ctx, |resource_center_view, _ctx| {
                resource_center_view.get_current_page()
            });

        if !self.current_workspace_state.is_resource_center_open {
            // Set current page to Keybindings
            self.resource_center_view
                .update(ctx, |resource_center_view, ctx| {
                    resource_center_view.set_current_page(ResourceCenterPage::Keybindings, ctx)
                });

            // Ensure other right panels are closed
            self.current_workspace_state.is_ai_assistant_panel_open = false;
            // Open side panel
            self.current_workspace_state.is_resource_center_open = true;
            send_telemetry_from_ctx!(TelemetryEvent::KeybindingsPageOpened, ctx);
        } else if current_page != ResourceCenterPage::Keybindings
            && self.current_workspace_state.is_resource_center_open
        {
            // Navigate to keybindings page
            self.resource_center_view
                .update(ctx, |resource_center_view, ctx| {
                    resource_center_view.set_current_page(ResourceCenterPage::Keybindings, ctx)
                });
            send_telemetry_from_ctx!(TelemetryEvent::KeybindingsPageOpened, ctx);
        } else {
            // Close side panel
            self.current_workspace_state.is_resource_center_open = false;
            self.focus_active_tab(ctx);
        }

        ctx.notify();
    }

    fn update_resource_center_action_target(&mut self, ctx: &mut ViewContext<Self>) {
        if self.current_workspace_state.is_resource_center_open {
            let input_id = self.active_input_id(ctx);
            self.resource_center_view
                .update(ctx, |resource_center_view, ctx| {
                    resource_center_view.set_action_target(ctx.window_id(), input_id, ctx)
                });
        }
    }

    fn handle_tab_right_click_menu_event(
        &mut self,
        event: &MenuEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            MenuEvent::Close { via_select_item: _ } => {
                self.show_tab_right_click_menu = None;
                self.show_tab_group_right_click_menu = None;
                self.hide_move_to_group_sidecar(ctx);
                ctx.notify();
            }
            MenuEvent::ItemHovered | MenuEvent::ItemSelected => {
                self.update_move_to_group_sidecar(ctx);
            }
        }
    }

    /// Builds the tab group more-options menu items, grouped into sections.
    fn tab_group_menu_items(&self, group_id: TabGroupId) -> Vec<MenuItem<WorkspaceAction>> {
        let Some((first, last)) = group_member_index_range(&self.tabs, group_id) else {
            return vec![];
        };
        let has_tabs_above = first > 0;
        let has_tabs_below = last + 1 < self.tabs.len();
        let has_tabs_outside = (last - first + 1) < self.tabs.len();

        let move_section = {
            let mut items = vec![];
            if has_tabs_above {
                items.push(
                    MenuItemFields::new("Move group up")
                        .with_on_select_action(WorkspaceAction::MoveTabGroupUp(group_id))
                        .into_item(),
                );
            }
            if has_tabs_below {
                items.push(
                    MenuItemFields::new("Move group down")
                        .with_on_select_action(WorkspaceAction::MoveTabGroupDown(group_id))
                        .into_item(),
                );
            }
            items
        };

        let close_section = {
            let mut items = vec![MenuItemFields::new("Close all tabs in group")
                .with_on_select_action(WorkspaceAction::CloseTabGroup(group_id))
                .into_item()];
            if has_tabs_outside {
                items.push(
                    MenuItemFields::new("Close other tabs")
                        .with_on_select_action(WorkspaceAction::CloseTabsOutsideGroup(group_id))
                        .into_item(),
                );
            }
            if has_tabs_above {
                items.push(
                    MenuItemFields::new("Close tabs above")
                        .with_on_select_action(WorkspaceAction::CloseTabsAboveGroup(group_id))
                        .into_item(),
                );
            }
            if has_tabs_below {
                items.push(
                    MenuItemFields::new("Close tabs below")
                        .with_on_select_action(WorkspaceAction::CloseTabsBelowGroup(group_id))
                        .into_item(),
                );
            }
            items
        };

        let mut menu_items = vec![];
        for section_items in [
            vec![
                MenuItemFields::new("Ungroup tabs")
                    .with_on_select_action(WorkspaceAction::UngroupTabs(group_id))
                    .into_item(),
                MenuItemFields::new("New tab in group")
                    .with_on_select_action(WorkspaceAction::NewTabInGroup(group_id))
                    .into_item(),
            ],
            move_section,
            vec![MenuItemFields::new("Rename")
                .with_on_select_action(WorkspaceAction::RenameTabGroup(group_id))
                .into_item()],
            close_section,
        ] {
            if section_items.is_empty() {
                continue;
            }
            if !menu_items.is_empty() {
                menu_items.push(MenuItem::Separator);
            }
            menu_items.extend(section_items);
        }
        menu_items
    }

    /// Builds the sidecar rows: every group except the tab's current one,
    /// ordered by first member's tab index to match the tabs panel.
    fn build_move_to_group_sidecar_items(
        &self,
        tab_index: usize,
    ) -> Vec<MenuItem<WorkspaceAction>> {
        let Some(tab) = self.tabs.get(tab_index) else {
            return vec![];
        };
        let current_group_id = tab.group_id;

        // Other groups paired with their first member's tab index, sorted so the menu
        // matches panel order.
        let sorted_other_groups = self
            .tab_groups
            .keys()
            .copied()
            .filter(|gid| Some(*gid) != current_group_id)
            .filter_map(|gid| {
                group_member_indices(&self.tabs, gid)
                    .next()
                    .map(|idx| (gid, idx))
            })
            .sorted_by_key(|(_, idx)| *idx);

        sorted_other_groups
            .map(|(group_id, _)| {
                let label = self
                    .tab_groups
                    .get(&group_id)
                    .and_then(|g| g.name.clone())
                    .unwrap_or_else(|| "Untitled group".to_string());
                MenuItemFields::new(label)
                    .with_on_select_action(WorkspaceAction::MoveTabToGroup {
                        tab_index,
                        group_id,
                    })
                    .into_item()
            })
            .collect()
    }

    /// Opens the sidecar when "Move to group" is hovered, hides it otherwise.
    fn update_move_to_group_sidecar(&mut self, ctx: &mut ViewContext<Self>) {
        let Some((tab_index, _)) = self.show_tab_right_click_menu else {
            return;
        };
        // No hovered index = cursor left the menu (possibly onto the sidecar);
        // no label = hovered a non-label row (e.g. separator).
        let hovered = self.tab_right_click_menu.read(ctx, |menu, _| {
            let idx = menu.hovered_index()?;
            let label = match menu.items().get(idx)? {
                MenuItem::Item(fields) => Some(fields.label().to_string()),
                _ => None,
            };
            Some((idx, label))
        });

        let Some((hovered_index, hovered_label)) = hovered else {
            return;
        };

        let Some(label) = hovered_label else {
            if self.show_move_to_group_sidecar {
                self.hide_move_to_group_sidecar(ctx);
            }
            return;
        };

        if label == MOVE_TO_GROUP_LABEL {
            let items = self.build_move_to_group_sidecar_items(tab_index);
            if items.is_empty() {
                self.hide_move_to_group_sidecar(ctx);
                return;
            }
            self.move_to_group_sidecar_menu
                .update(ctx, |menu, view_ctx| {
                    menu.set_items(items, view_ctx);
                });
            self.show_move_to_group_sidecar = true;

            let sidecar_rect = ctx.element_position_by_id_at_last_frame(
                self.window_id,
                MOVE_TO_GROUP_SIDECAR_POSITION_ID,
            );
            self.tab_right_click_menu.update(ctx, |menu, _| {
                menu.set_safe_zone_target(sidecar_rect);
                menu.set_submenu_being_shown_for_item_index(Some(hovered_index));
            });
            ctx.notify();
        } else if self.show_move_to_group_sidecar {
            self.hide_move_to_group_sidecar(ctx);
        }
    }

    fn hide_move_to_group_sidecar(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.show_move_to_group_sidecar {
            return;
        }
        self.show_move_to_group_sidecar = false;
        self.tab_right_click_menu.update(ctx, |menu, _| {
            menu.set_safe_zone_target(None);
            menu.set_submenu_being_shown_for_item_index(None);
        });
        ctx.notify();
    }

    fn handle_move_to_group_sidecar_event(
        &mut self,
        event: &MenuEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            MenuEvent::Close { via_select_item } => {
                // Item dispatch fires `MoveTabToGroup` itself; we just tear
                // down the parent menu on a real pick.
                if *via_select_item {
                    self.show_tab_right_click_menu = None;
                }
                self.show_move_to_group_sidecar = false;
                self.tab_right_click_menu.update(ctx, |menu, _| {
                    menu.set_safe_zone_target(None);
                    menu.set_submenu_being_shown_for_item_index(None);
                });
                ctx.notify();
            }
            MenuEvent::ItemHovered | MenuEvent::ItemSelected => {}
        }
    }

    fn handle_new_session_menu_event(&mut self, event: &MenuEvent, ctx: &mut ViewContext<Self>) {
        match event {
            MenuEvent::Close { .. } => {
                self.close_new_session_dropdown_menu(ctx);
            }
            MenuEvent::ItemHovered => {
                self.update_new_session_sidecar(ctx);
            }
            MenuEvent::ItemSelected => {
                self.update_new_session_sidecar(ctx);
            }
        }
    }

    fn handle_new_session_sidecar_event(&mut self, event: &MenuEvent, ctx: &mut ViewContext<Self>) {
        match event {
            MenuEvent::Close { via_select_item } => {
                let selection = if *via_select_item {
                    self.selected_new_session_sidecar_selection(ctx)
                } else {
                    None
                };
                log::info!(
                    "New-session sidecar closed: worktree_active={}, via_select_item={via_select_item}",
                    self.worktree_sidecar_active
                );
                if let Some(selection) = selection {
                    self.execute_new_session_sidecar_selection(selection, ctx);
                }
                if *via_select_item {
                    // Item clicked in sidecar — also close the main menu.
                    self.show_new_session_dropdown_menu = None;
                }
                self.clear_worktree_sidecar_state(ctx);
                self.new_session_dropdown_menu.update(ctx, |menu, _| {
                    menu.set_safe_zone_target(None);
                    menu.set_submenu_being_shown_for_item_index(None);
                });
                ctx.notify();
            }
            MenuEvent::ItemSelected => {}
            MenuEvent::ItemHovered => {
                self.sync_new_session_sidecar_selection_to_hover(ctx);
            }
        }
    }

    fn should_include_worktree_sidecar_repo(repo_path: &Path, ctx: &AppContext) -> bool {
        #[cfg(not(feature = "local_fs"))]
        {
            let _ = (repo_path, ctx);
            true
        }

        #[cfg(feature = "local_fs")]
        {
            // This performs one repo-metadata lookup per persisted workspace while the
            // sidecar items are rebuilt. That's acceptable for now given the expected
            // repo counts here, and it keeps linked-worktree filtering scoped to the
            // only UI that currently needs it.
            let Some(repository) =
                DetectedRepositories::as_ref(ctx).get_local_watched_repo_for_path(repo_path, ctx)
            else {
                return true;
            };
            // Linked worktrees (and submodules) have an external gitdir; exclude
            // them so only primary repository checkouts appear in the list.

            repository.as_ref(ctx).external_git_directory().is_none()
        }
    }

    fn build_worktree_sidecar_items(
        &self,
        ctx: &AppContext,
    ) -> Vec<MenuItem<NewSessionSidecarSelection>> {
        let search_editor = self.worktree_sidecar_search_editor.clone();
        let search_item = MenuItemFields::new_with_custom_label(
            Arc::new(move |_, _, appearance, _| {
                let theme = appearance.theme();
                let search_icon = ConstrainedBox::new(
                    icons::Icon::SearchSmall
                        .to_warpui_icon(theme.sub_text_color(theme.surface_2()))
                        .finish(),
                )
                .with_width(16.)
                .with_height(16.)
                .finish();
                let search_row = Flex::row()
                    .with_child(Container::new(search_icon).with_margin_right(8.).finish())
                    .with_child(
                        Shrinkable::new(1., ChildView::new(&search_editor).finish()).finish(),
                    )
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .finish();

                ConstrainedBox::new(
                    Container::new(search_row)
                        .with_padding_left(NEW_SESSION_SIDECAR_SEARCH_BOX_HORIZONTAL_PADDING)
                        .with_padding_right(NEW_SESSION_SIDECAR_SEARCH_BOX_HORIZONTAL_PADDING)
                        .with_padding_top(NEW_SESSION_SIDECAR_SEARCH_BOX_VERTICAL_PADDING)
                        .with_padding_bottom(NEW_SESSION_SIDECAR_SEARCH_BOX_VERTICAL_PADDING)
                        .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
                        .with_corner_radius(CornerRadius::with_top(Radius::Pixels(4.)))
                        .finish(),
                )
                .with_height(NEW_SESSION_SIDECAR_SEARCH_BOX_HEIGHT)
                .finish()
            }),
            Some("Search repos".to_string()),
        )
        .with_no_interaction_on_hover()
        .no_highlight_on_hover()
        .with_padding_override(0., 0.)
        .into_item();
        let query = self.worktree_sidecar_search_query.trim().to_lowercase();
        let home = dirs::home_dir().map(|p| p.display().to_string());
        let mut items = vec![search_item];
        items.extend(
            crate::projects::ProjectManagementModel::as_ref(ctx)
                .all_projects()
                .map(|project| PathBuf::from(&project.path))
                .filter(|path| path.exists())
                .filter(|path| Self::should_include_worktree_sidecar_repo(path, ctx))
                .filter(|path| {
                    if query.is_empty() {
                        true
                    } else {
                        path.to_string_lossy()
                            .to_lowercase()
                            .contains(query.as_str())
                    }
                })
                .map(|path| {
                    let path_str = path.to_string_lossy().into_owned();
                    let display = user_friendly_path(&path_str, home.as_deref()).into_owned();
                    MenuItemFields::new(display)
                        .with_on_select_action(NewSessionSidecarSelection::OpenWorktreeRepo {
                            repo_path: path_str.clone(),
                        })
                        .with_icon(icons::Icon::Folder)
                        .with_clip_config(ClipConfig::start())
                        .with_tooltip(path_str)
                        .into_item()
                })
                .collect::<Vec<_>>(),
        );
        items
    }

    fn configure_worktree_new_session_sidecar(
        &mut self,
        hovered_index: usize,
        auto_select_first_repo: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let items = self.build_worktree_sidecar_items(ctx);
        let repo_count = items.len().saturating_sub(1);
        log::info!(
            "Configuring worktree sidecar: hovered_index={hovered_index}, query={:?}, repo_count={repo_count}",
            self.worktree_sidecar_search_query
        );
        let add_repo_mouse_state = self.new_session_sidecar_add_repo_mouse_state.clone();

        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            menu.set_items(items, view_ctx);
            menu.clear_pinned_header_builder();
            menu.set_content_padding_overrides(Some(0.), None);
            menu.set_pinned_footer_builder(move |app| {
                let appearance = Appearance::as_ref(app);
                let theme = appearance.theme();
                let font_family = appearance.ui_font_family();
                let font_size = appearance.ui_font_size();
                let border_fill = theme.outline();
                let mouse_state = add_repo_mouse_state.clone();
                Hoverable::new(mouse_state, move |state| {
                    let bg = if state.is_hovered() {
                        theme.accent_button_color()
                    } else {
                        theme.surface_2()
                    };
                    let text_color = theme.main_text_color(bg);
                    ConstrainedBox::new(
                        Container::new(
                            Flex::row()
                                .with_main_axis_size(MainAxisSize::Max)
                                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                                .with_child(
                                    Text::new_inline(" + Add new repo", font_family, font_size)
                                        .with_color(text_color.into())
                                        .finish(),
                                )
                                .finish(),
                        )
                        .with_padding_left(NEW_SESSION_SIDECAR_FOOTER_HORIZONTAL_PADDING)
                        .with_padding_right(NEW_SESSION_SIDECAR_FOOTER_HORIZONTAL_PADDING)
                        .with_padding_top(NEW_SESSION_SIDECAR_FOOTER_VERTICAL_PADDING)
                        .with_padding_bottom(NEW_SESSION_SIDECAR_FOOTER_VERTICAL_PADDING)
                        .with_background(bg)
                        .with_border(Border::top(1.).with_border_fill(border_fill))
                        .with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(5.)))
                        .finish(),
                    )
                    .with_width(NEW_SESSION_SIDECAR_WIDTH)
                    .finish()
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(|ctx: &mut riftui::elements::EventContext, _, _| {
                    ctx.dispatch_typed_action(WorkspaceAction::OpenWorktreeAddRepoPicker);
                    ctx.dispatch_typed_action(crate::menu::MenuAction::Close(true));
                })
                .finish()
            });
        });
        if auto_select_first_repo {
            self.select_first_worktree_sidecar_repo(ctx);
        } else {
            self.reset_worktree_sidecar_repo_selection(ctx);
        }

        self.worktree_sidecar_active = true;
        self.show_new_session_sidecar = true;
        let sidecar_rect = ctx
            .element_position_by_id_at_last_frame(self.window_id, NEW_SESSION_SIDECAR_POSITION_ID);
        log::info!(
            "Worktree sidecar safe-zone target from previous frame available: {}",
            sidecar_rect.is_some()
        );
        self.new_session_dropdown_menu.update(ctx, |menu, _| {
            menu.set_safe_zone_target(sidecar_rect);
            menu.set_submenu_being_shown_for_item_index(Some(hovered_index));
        });
        ctx.focus(&self.worktree_sidecar_search_editor);
    }

    fn configure_action_sidecar_for_hovered_item(
        &mut self,
        label: &str,
        hovered_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        // Determine the SidecarItemKind from the hovered menu item's label and action.
        let hovered_action = self.new_session_dropdown_menu.read(ctx, |menu, _| {
            menu.items().get(hovered_index).and_then(|item| match item {
                MenuItem::Item(fields) => fields.on_select_action().cloned(),
                _ => None,
            })
        });

        let item_kind = match &hovered_action {
            Some(WorkspaceAction::SelectTabConfig(config)) => SidecarItemKind::UserTabConfig {
                config: config.clone(),
            },
            Some(WorkspaceAction::AddAgentTab) => SidecarItemKind::BuiltIn {
                name: label.to_string(),
                default_mode: DefaultSessionMode::Agent,
                shell: None,
            },
            Some(WorkspaceAction::AddAmbientAgentTab) => SidecarItemKind::BuiltIn {
                name: label.to_string(),
                default_mode: DefaultSessionMode::CloudAgent,
                shell: None,
            },
            Some(WorkspaceAction::AddTerminalTab { .. }) => SidecarItemKind::BuiltIn {
                name: label.to_string(),
                default_mode: DefaultSessionMode::Terminal,
                shell: None,
            },
            Some(WorkspaceAction::AddTabWithShell { shell, .. }) => SidecarItemKind::BuiltIn {
                name: label.to_string(),
                default_mode: DefaultSessionMode::Terminal,
                shell: Some(shell.clone()),
            },
            Some(WorkspaceAction::AddDockerSandboxTab) => SidecarItemKind::BuiltIn {
                name: label.to_string(),
                default_mode: DefaultSessionMode::DockerSandbox,
                shell: None,
            },
            _ => {
                // Hovered item has no associated sidecar. Clear any stale
                // sidecar state left over from a previously-hovered item so
                // the menu doesn't keep rendering that item as the
                // submenu-parent highlight.
                self.tab_config_action_sidecar_item = None;
                self.new_session_dropdown_menu.update(ctx, |menu, _| {
                    menu.set_safe_zone_target(None);
                    menu.set_submenu_being_shown_for_item_index(None);
                });
                return;
            }
        };

        self.tab_config_action_sidecar_item = Some(item_kind);

        let sidecar_rect = ctx
            .element_position_by_id_at_last_frame(self.window_id, NEW_SESSION_SIDECAR_POSITION_ID);
        self.new_session_dropdown_menu.update(ctx, |menu, _| {
            menu.set_safe_zone_target(sidecar_rect);
            menu.set_submenu_being_shown_for_item_index(Some(hovered_index));
        });
    }

    /// Returns `true` when a sidecar of the given width should render on the left
    /// of the menu (because it would overflow the window on the right).
    fn should_render_sidecar_left(
        &self,
        anchor_label: &str,
        sidecar_width: f32,
        app: &AppContext,
    ) -> bool {
        let Some(window) = app.windows().platform_window(self.window_id) else {
            return false;
        };
        let Some(anchor_rect) =
            app.element_position_by_id_at_last_frame(self.window_id, anchor_label)
        else {
            return false;
        };

        let gap = 4.0;

        let would_overflow_right = anchor_rect.max_x() + gap + sidecar_width >= window.size().x();
        let would_overflow_left = anchor_rect.min_x() - gap - sidecar_width < 0.0;

        match (would_overflow_left, would_overflow_right) {
            (true, false) => false, // Only right fits
            (false, true) => true,  // Only left fits
            _ => false,             // Default to right
        }
    }

    fn refresh_worktree_sidecar_if_active(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.worktree_sidecar_active {
            return;
        }
        let Some(hovered_index) = self
            .new_session_dropdown_menu
            .read(ctx, |menu, _| menu.hovered_index())
        else {
            return;
        };
        self.configure_worktree_new_session_sidecar(hovered_index, true, ctx);
    }

    /// Updates the sidecar menu based on which item is hovered in the main
    /// new-session dropdown. If the hovered item is a submenu parent (Terminal
    /// or New worktree config), populates the sidecar with the appropriate items.
    fn update_new_session_sidecar(&mut self, ctx: &mut ViewContext<Self>) {
        // Use hovered_index (not selected_index) as the source of truth.
        // hovered_row_index accurately tracks the mouse position and survives
        // reset_selection (which only clears selected_row/item indices).
        // selected_index can get stuck on a submenu parent when
        // UnhoverSubmenuParent resets the selection.
        let hovered_index = self
            .new_session_dropdown_menu
            .read(ctx, |menu, _| menu.hovered_index());

        // If hovered is None the mouse has left the menu (possibly onto the
        // sidecar) or is on a non-hoverable element. Keep current state.
        let Some(hovered_index) = hovered_index else {
            return;
        };

        // Check what the hovered item is by reading its label.
        let hovered_label = self.new_session_dropdown_menu.read(ctx, |menu, _| {
            menu.items().get(hovered_index).and_then(|item| match item {
                MenuItem::Item(fields) => Some(fields.label().to_string()),
                _ => None,
            })
        });

        // Separator or non-labeled item — hide sidecar.
        let Some(label) = hovered_label else {
            if self.show_new_session_sidecar {
                self.show_new_session_sidecar = false;
                self.new_session_dropdown_menu.update(ctx, |menu, _| {
                    menu.set_safe_zone_target(None);
                    menu.set_submenu_being_shown_for_item_index(None);
                });
                ctx.notify();
            }
            return;
        };

        match label.as_str() {
            "New worktree config" => {
                self.tab_config_action_sidecar_item = None;
                let auto_select_first_repo = self.new_session_dropdown_menu.read(ctx, |menu, _| {
                    menu.last_selection_source() != Some(MenuSelectionSource::Pointer)
                });
                self.configure_worktree_new_session_sidecar(
                    hovered_index,
                    auto_select_first_repo,
                    ctx,
                );
            }
            // Items that don't get any sidecar.
            "New tab config" => {
                self.tab_config_action_sidecar_item = None;
                if self.show_new_session_sidecar {
                    self.show_new_session_sidecar = false;
                    self.worktree_sidecar_active = false;
                    self.new_session_dropdown_menu.update(ctx, |menu, _| {
                        menu.set_safe_zone_target(None);
                        menu.set_submenu_being_shown_for_item_index(None);
                    });
                }
            }
            // All other actionable items get the action sidecar.
            _ => {
                self.show_new_session_sidecar = false;
                self.worktree_sidecar_active = false;
                self.configure_action_sidecar_for_hovered_item(&label, hovered_index, ctx);
            }
        }

        ctx.notify();
    }

    fn handle_tab_bar_overflow_menu_event(
        &mut self,
        event: &MenuEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if let MenuEvent::Close { via_select_item: _ } = event {
            self.close_tab_bar_overflow_menu(ctx)
        }
    }

    fn handle_launch_config_save_modal_event(
        &mut self,
        event: &LaunchConfigModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            LaunchConfigModalEvent::Close => {
                self.current_workspace_state
                    .is_launch_config_save_modal_open = false;
                self.launch_config_save_modal.close();
                ctx.notify();
            }
            LaunchConfigModalEvent::SuccessfullySavedConfig(launch_config) => {
                ctx.update_model(&WarpConfig::handle(ctx), move |warp_config, ctx| {
                    warp_config.append_launch_config(launch_config, ctx);
                });
                ctx.notify();
            }
            #[cfg(feature = "local_fs")]
            LaunchConfigModalEvent::OpenFileWithTarget {
                path,
                target,
                line_col,
            } => {
                self.open_file_with_target(path.clone(), target.clone(), *line_col, ctx);
            }
        }
    }

    fn handle_tab_config_params_modal_event(
        &mut self,
        event: &ModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ModalEvent::Close => {
                self.cancel_tab_config_params_modal(ctx);
            }
        }
    }

    /// Cleans up pending state and closes the tab-config params modal without
    /// creating a tab config. Used when the modal is dismissed or cancelled.
    fn cancel_tab_config_params_modal(&mut self, ctx: &mut ViewContext<Self>) {
        let pending_intention = self.pending_onboarding_intention.take();
        self.pending_session_config_replacement = None;
        self.pending_session_config_tab_config_chip = false;
        self.close_tab_config_params_modal(ctx);

        if let Some(intention) = pending_intention {
            self.dispatch_tutorial_when_bootstrapped(false, intention, ctx);
        }
    }

    fn handle_tab_config_params_modal_body_event(
        &mut self,
        event: &TabConfigParamsModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            TabConfigParamsModalEvent::Submit { config, params } => {
                let pending_intention = self.pending_onboarding_intention.take();
                let should_track_existing_config_open =
                    self.pending_session_config_replacement.is_none();
                let worktree_name = self.maybe_generate_worktree_name(config);
                self.open_tab_config_with_params(
                    config.as_ref().clone(),
                    params.clone(),
                    worktree_name.as_deref(),
                    ctx,
                );
                if should_track_existing_config_open {
                    send_telemetry_from_ctx!(
                        TabConfigsTelemetryEvent::ExistingConfigOpened {
                            open_mode: ExistingTabConfigOpenMode::ParamsModal,
                            is_worktree_config: config.is_worktree(),
                        },
                        ctx
                    );
                }
                self.close_tab_config_params_modal(ctx);
                self.complete_pending_session_config_replacement(ctx);

                // The new tab has setup commands (worktree creation); wait for
                // them to finish before starting the onboarding tutorial, but
                // only after the tab-config chip is dismissed.
                if let Some(intention) = pending_intention {
                    self.queue_onboarding_tutorial_after_session_config_tab_config_chip(
                        PendingSessionConfigTabConfigChipTutorial::AfterSetupCommands { intention },
                        ctx,
                    );
                }

                // Params modal is now closed; show the chip if it was pending.
                self.promote_session_config_tab_config_chip(ctx);
            }
            TabConfigParamsModalEvent::Close => {
                self.cancel_tab_config_params_modal(ctx);
            }
            TabConfigParamsModalEvent::PickNewRepo { param_index } => {
                ctx.dispatch_typed_action_deferred(WorkspaceAction::OpenTabConfigRepoPicker {
                    param_index: *param_index,
                });
            }
        }
    }

    /// Finishes the tab replacement that was deferred while the params modal
    /// was open (worktree flow from the session config modal).
    fn complete_pending_session_config_replacement(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(pending) = self.pending_session_config_replacement.take() else {
            return;
        };

        self.remove_tab_by_pane_group_id(pending.old_pane_group_id, ctx);
    }

    /// Removes the tab whose pane group matches `pane_group_id`, if it exists
    /// and there is more than one tab.
    fn remove_tab_by_pane_group_id(
        &mut self,
        pane_group_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.pane_group.id() == pane_group_id)
        {
            self.remove_tab(index, false, true, ctx);
        }
    }

    /// Opens a native folder picker and, when the user selects a folder, upserts it
    /// into `PersistedWorkspace` and notifies the modal's repo picker at `param_index`.
    fn open_repo_picker_for_tab_config_modal(
        &mut self,
        param_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        let modal_view = self.tab_config_params_modal.view.clone();
        ctx.open_file_picker(
            move |result, ctx| {
                let Ok(paths) = result else { return };
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                // Register the chosen directory as a workspace so it appears in
                // PersistedWorkspace (which is the data source for the repo picker
                // and also triggers codebase indexing / project rules scanning).
                let path_buf: PathBuf = path.clone().into();
                crate::projects::ProjectManagementModel::handle(ctx).update(ctx, |projects, ctx| {
                    projects.upsert_project(path_buf.clone(), ctx);
                });
                // Refresh the repo picker and pre-select the new path.
                modal_view.update(ctx, |modal, ctx| {
                    modal.body().update(ctx, |body, ctx| {
                        body.on_new_repo_selected(path_buf, param_index, ctx);
                    });
                });
            },
            riftui::platform::FilePickerConfiguration::new().folders_only(),
        );
    }

    fn close_tab_config_params_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_tab_config_params_modal_open = false;
        self.tab_config_params_modal.close();
        self.tab_config_params_modal.view.update(ctx, |modal, ctx| {
            modal.body().update(ctx, |body, ctx| {
                body.on_close(ctx);
            });
        });
        ctx.notify();
    }

    fn handle_new_worktree_modal_event(&mut self, event: &ModalEvent, ctx: &mut ViewContext<Self>) {
        match event {
            ModalEvent::Close => self.close_new_worktree_modal(ctx),
        }
    }

    fn handle_new_worktree_modal_body_event(
        &mut self,
        event: &NewWorktreeModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            NewWorktreeModalEvent::Close => self.close_new_worktree_modal(ctx),
            NewWorktreeModalEvent::Submit {
                repo,
                branch,
                worktree_branch_name,
            } => {
                self.handle_new_worktree_submit(repo, branch, worktree_branch_name.as_deref(), ctx);
                self.close_new_worktree_modal(ctx);
            }
            NewWorktreeModalEvent::PickNewRepo => {
                ctx.dispatch_typed_action_deferred(WorkspaceAction::OpenNewWorktreeRepoPicker);
            }
        }
    }

    fn close_new_worktree_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_new_worktree_modal_open = false;
        self.new_worktree_modal.close();
        self.new_worktree_modal.view.update(ctx, |modal, ctx| {
            modal.body().update(ctx, |body, ctx| {
                body.on_close(ctx);
            });
        });
        ctx.notify();
    }

    /// Checks whether the tab config references the special-cased
    /// `autogenerated_branch_name` template var. If so, fetches existing
    /// branches and generates a unique themed name.
    fn maybe_generate_worktree_name(
        &self,
        config: &crate::tab_configs::TabConfig,
    ) -> Option<String> {
        if !config.uses_autogenerated_branch_name() {
            return None;
        }
        let pane = config
            .panes
            .iter()
            .find(|pane| pane.directory.is_some())
            .or_else(|| config.panes.first())?;

        let repo_path = pane.directory.as_deref().map(Path::new);
        let branches = repo_path
            .map(crate::util::git::list_local_branches_sync)
            .unwrap_or_default();
        let branch_refs: HashSet<&str> = branches.iter().map(|s| s.as_str()).collect();
        Some(rift_util::worktree_names::generate_worktree_branch_name(
            &branch_refs,
        ))
    }

    /// Generates a worktree tab config TOML, writes it to `~/.rift/tab_configs/`,
    /// and opens the resulting config as a new tab.
    ///
    /// When `worktree_branch_name` is `None` (autogenerate), the TOML stores
    /// commands with `{autogenerated_branch_name}` template variables that get
    /// substituted with a fresh name on every open.
    /// When `Some(name)` (manual naming), the commands are baked in and a
    /// `worktree_branch_name` param is added so re-opens show the params modal.
    #[cfg(feature = "local_fs")]
    fn handle_new_worktree_submit(
        &mut self,
        repo: &str,
        base_branch: &str,
        worktree_branch_name: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        let repo_display_name = Path::new(repo)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| repo.to_string());
        let config_name = match worktree_branch_name {
            Some(name) if !name.is_empty() => {
                format!("New worktree: {repo_display_name}, {name}")
            }
            _ if !base_branch.is_empty() => {
                format!("New worktree: {repo_display_name}, {base_branch}")
            }
            _ => format!("New worktree: {repo_display_name}"),
        };

        let filename_hint = if let Some(name) = worktree_branch_name {
            name.to_string()
        } else {
            let branches = crate::util::git::list_local_branches_sync(Path::new(repo));
            let branch_refs: HashSet<&str> = branches.iter().map(|s| s.as_str()).collect();
            rift_util::worktree_names::generate_worktree_branch_name(&branch_refs)
        };

        let toml_content = crate::tab_configs::build_worktree_config_toml(
            &config_name,
            repo,
            base_branch,
            worktree_branch_name,
        );

        let dir = tab_configs_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("Failed to create tab_configs dir: {e:?}");
            return;
        }

        let path = find_unused_worktree_config_path(&dir, &filename_hint);
        if let Err(e) = std::fs::write(&path, &toml_content) {
            log::warn!("Failed to write worktree tab config: {e:?}");
            return;
        }

        match toml::from_str::<crate::tab_configs::TabConfig>(&toml_content) {
            Ok(tab_config) => {
                let naming_mode = if worktree_branch_name.is_some() {
                    WorktreeBranchNamingMode::Manual
                } else {
                    WorktreeBranchNamingMode::Auto
                };
                if let Some(name) = worktree_branch_name {
                    // First open with manual name — bypass the params modal.
                    let mut param_values = HashMap::new();
                    param_values.insert("worktree_branch_name".to_string(), name.to_string());
                    self.open_tab_config_with_params(tab_config, param_values, None, ctx);
                } else {
                    // Autogenerate — open with the name we just generated.
                    let param_values = tab_config.default_param_values();
                    self.open_tab_config_with_params(
                        tab_config,
                        param_values,
                        Some(&filename_hint),
                        ctx,
                    );
                }
                send_telemetry_from_ctx!(
                    TabConfigsTelemetryEvent::NewWorktreeConfigOpened {
                        source: NewWorktreeConfigOpenSource::NewWorktreeModal,
                        naming_mode,
                    },
                    ctx
                );
            }
            Err(e) => {
                log::warn!("Failed to parse generated worktree config: {e:?}");
            }
        }
    }

    #[cfg(not(feature = "local_fs"))]
    fn handle_new_worktree_submit(
        &mut self,
        _repo: &str,
        _base_branch: &str,
        _worktree_branch_name: Option<&str>,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    fn open_repo_picker_for_new_worktree_modal(&mut self, ctx: &mut ViewContext<Self>) {
        let modal_view = self.new_worktree_modal.view.clone();
        ctx.open_file_picker(
            move |result, ctx| {
                let Ok(paths) = result else { return };
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                let path_buf: PathBuf = path.clone().into();
                crate::projects::ProjectManagementModel::handle(ctx).update(ctx, |projects, ctx| {
                    projects.upsert_project(path_buf.clone(), ctx);
                });
                modal_view.update(ctx, |modal, ctx| {
                    modal.body().update(ctx, |body, ctx| {
                        body.on_new_repo_selected(path_buf, ctx);
                    });
                });
            },
            riftui::platform::FilePickerConfiguration::new().folders_only(),
        );
    }

    /// Opens a worktree in the given repo using the default worktree tab config,
    /// saving the materialized config to `~/.rift/tab_configs/` first.
    /// The branch name is auto-generated.
    #[cfg(feature = "local_fs")]
    fn open_worktree_in_repo(&mut self, repo_path: String, ctx: &mut ViewContext<Self>) {
        log::info!("open_worktree_in_repo requested: repo_path={repo_path:?}");
        let config_path = ensure_default_worktree_config();
        log::info!("Reading default worktree config from {config_path:?}");
        let template_toml = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to read default worktree config from {config_path:?}: {e:?}");
                return;
            }
        };
        let branches = crate::util::git::list_local_branches_sync(Path::new(&repo_path));
        let branch_refs: HashSet<&str> = branches.iter().map(|s| s.as_str()).collect();
        let branch_name = rift_util::worktree_names::generate_worktree_branch_name(&branch_refs);
        let repo_display_name = Path::new(&repo_path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| repo_path.clone());
        let config_name = format!("Worktree: {repo_display_name}");
        // Use the user's default session mode to decide pane type.
        let pane_type = if AISettings::as_ref(ctx).is_any_ai_enabled(ctx)
            && AISettings::as_ref(ctx).default_session_mode(ctx) == DefaultSessionMode::Agent
        {
            "agent"
        } else {
            "terminal"
        };
        log::info!(
            "Materializing default worktree config: repo_path={repo_path:?}, branch_name={branch_name:?}, pane_type={pane_type}"
        );

        let (toml_content, tab_config) = match materialize_default_worktree_config(
            &template_toml,
            &config_name,
            &repo_path,
            pane_type,
        ) {
            Ok(materialized) => materialized,
            Err(e) => {
                log::warn!(
                    "Failed to materialize default worktree config from {config_path:?}: {e}"
                );
                return;
            }
        };

        let dir = tab_configs_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("Failed to create tab_configs dir: {e:?}");
            return;
        }

        let saved_config_path =
            find_unused_toml_path(&dir, &sanitize_toml_base_name(&repo_display_name));
        if let Err(e) = std::fs::write(&saved_config_path, &toml_content) {
            log::warn!("Failed to write worktree tab config to {saved_config_path:?}: {e:?}");
            return;
        }

        log::info!(
            "Saved default worktree config to {saved_config_path:?}: config_name={:?}",
            tab_config.name
        );

        let param_values = tab_config.default_param_values();
        log::info!("Opening tab from saved worktree config");
        self.open_tab_config_with_params(tab_config, param_values, Some(&branch_name), ctx);
        send_telemetry_from_ctx!(
            TabConfigsTelemetryEvent::NewWorktreeConfigOpened {
                source: NewWorktreeConfigOpenSource::Submenu,
                naming_mode: WorktreeBranchNamingMode::Auto,
            },
            ctx
        );
    }

    #[cfg(not(feature = "local_fs"))]
    fn open_worktree_in_repo(&mut self, _repo_path: String, _ctx: &mut ViewContext<Self>) {}

    /// Opens a native folder picker to add a new repo to PersistedWorkspace,
    /// triggered from the "+ Add new repo..." item in the New worktree config submenu.
    fn open_folder_picker_for_worktree_submenu(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_file_picker(
            move |result, ctx| {
                let Ok(paths) = result else { return };
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                let path_buf: PathBuf = path.into();
                crate::projects::ProjectManagementModel::handle(ctx).update(ctx, |projects, ctx| {
                    projects.upsert_project(path_buf, ctx);
                });
            },
            riftui::platform::FilePickerConfiguration::new().folders_only(),
        );
    }

    fn handle_welcome_tips_event(&mut self, event: &TipsEvent, ctx: &mut ViewContext<Self>) {
        match event {
            TipsEvent::Close => {
                self.welcome_tips_view_state.close_popup();
                ctx.notify();
            }
            TipsEvent::TipsDismissed => {
                self.tips_completed.update(ctx, |tips_completed, ctx| {
                    skip_tips_and_write_to_user_defaults(tips_completed, ctx);
                    ctx.notify();
                });
                self.welcome_tips_view_state = WelcomeTipsViewState::Unavailable;
                ctx.notify();
            }
        }
    }


    fn is_input_box_visible(&self, app: &AppContext) -> bool {
        if let (Some(terminal_model), Some(terminal_view)) = (
            self.get_active_session_terminal_model(app),
            self.active_tab_pane_group()
                .as_ref(app)
                .active_session_view(app),
        ) {
            terminal_view.read(app, |view, ctx| {
                view.is_input_box_visible(&terminal_model.lock(), ctx)
            })
        } else {
            false
        }
    }


    fn handle_require_login_modal_event(
        &mut self,
        event: &AuthViewEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            AuthViewEvent::Close => {
                self.current_workspace_state.is_require_login_modal_open = false;
                ctx.notify();
            }
        }
    }

    fn handle_theme_creator_modal_event(
        &mut self,
        event: &ThemeCreatorModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ThemeCreatorModalEvent::Close => {
                self.current_workspace_state.is_theme_creator_modal_open = false;
                ctx.notify();
            }
            ThemeCreatorModalEvent::SetCustomTheme { theme } => {
                self.theme_chooser_view
                    .update(ctx, |theme_chooser_view, ctx| {
                        theme_chooser_view.reload_and_set_custom_theme(theme.clone(), ctx);
                    });
            }
            ThemeCreatorModalEvent::ShowErrorToast { message } => {
                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast = DismissibleToast::error(message.clone());
                    view.add_ephemeral_toast(new_toast, ctx);
                });
            }
        }
    }

    fn handle_theme_deletion_modal_event(
        &mut self,
        event: &ThemeDeletionModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ThemeDeletionModalEvent::Close => {
                self.current_workspace_state.is_theme_deletion_modal_open = false;
                ctx.notify();
            }
            ThemeDeletionModalEvent::ShowErrorToast { message } => {
                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast = DismissibleToast::error(message.clone());
                    view.add_ephemeral_toast(new_toast, ctx);
                });
            }
            ThemeDeletionModalEvent::DeleteCurrentTheme => {
                self.theme_chooser_view
                    .update(ctx, |theme_chooser_view, ctx| {
                        // Reset theme to Dark if we are deleting the current theme
                        theme_chooser_view.select_and_save_theme(&ThemeKind::Dark, ctx);
                    });
            }
        }
    }

    /// Returns the pane group with the matching EntityId, or None if it doesn't exist.
    fn get_pane_group_view_with_id(&self, id: EntityId) -> Option<&ViewHandle<PaneGroup>> {
        self.tab_views().find(|view| view.id() == id)
    }

    // The workspace manages the close confirmation dialog, so it may need to close a pane after the user confirms in the dialog.
    // The flow is:
    // - User closes pane in pane group, which emits event to workspace
    // - Workspace shows confirmation dialog, and calls back into pane group to close pane here if user confirms
    fn close_pane(
        &mut self,
        pane_group_id: EntityId,
        pane_id: PaneId,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(pane_group_view) = self.get_pane_group_view_with_id(pane_group_id) else {
            log::error!("Could not close pane because pane group doesn't exist");
            return;
        };
        pane_group_view.update(ctx, |pane_group, ctx| {
            pane_group.close_pane(pane_id, ctx);
        });
    }

    fn handle_close_session_confirmation_dialog_event(
        &mut self,
        event: &CloseSessionConfirmationEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            CloseSessionConfirmationEvent::Cancel => {
                self.current_workspace_state
                    .is_close_session_confirmation_dialog_open = false;
                ctx.notify();
            }
            CloseSessionConfirmationEvent::CloseSession {
                dont_show_again,
                open_confirmation_source,
            } => {
                if *dont_show_again {
                    if let Err(e) = SessionSettings::handle(ctx).update(ctx, |settings, ctx| {
                        settings.should_confirm_close_session.set_value(false, ctx)
                    }) {
                        log::error!(
                            "Failed to set should_confirm_close_session setting to false: {e}"
                        );
                    };
                }
                match *open_confirmation_source {
                    OpenDialogSource::CloseTab { tab_index } => {
                        self.remove_tab(tab_index, true, true, ctx);
                    }
                    OpenDialogSource::ClosePane {
                        pane_group_id,
                        pane_id,
                    } => {
                        self.close_pane(pane_group_id, pane_id, ctx);
                    }
                    OpenDialogSource::CloseTabsDirection {
                        tab_index,
                        direction,
                    } => {
                        self.close_tabs_direction(tab_index, direction, true, ctx);
                    }
                    OpenDialogSource::CloseOtherTabs { tab_index } => {
                        self.close_other_tabs(tab_index, true, ctx);
                    }
                }
                self.current_workspace_state
                    .is_close_session_confirmation_dialog_open = false;
                ctx.notify();
            }
        }
    }



    pub fn handle_network_status_event(
        &mut self,
        _handle: ModelHandle<NetworkStatus>,
        _event: &NetworkStatusEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.notify();
    }

    fn handle_auth_manager_event(
        &mut self,
        _handle: ModelHandle<AuthManager>,
        event: &AuthManagerEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            AuthManagerEvent::AttemptedLoginGatedFeature { auth_view_variant } => {
                self.open_require_login_modal(*auth_view_variant, ctx)
            }
            AuthManagerEvent::LoginOverrideDetected(interrupted_auth_payload) => {
                self.open_auth_override_warning_modal(interrupted_auth_payload.clone(), ctx);
            }
            AuthManagerEvent::AuthComplete => {
                // Only show the telemetry banner if the user is an existing user. The new user flow
                // for this is handled in the onboarding flow.
                if self.auth_state.is_onboarded().unwrap_or_default() {
                    // Need to check this AFTER we fetch any billing metadata associated with the team,
                    // to make sure we don't show the banner if the user is an enterprise user.
                    self.check_and_trigger_telemetry_banner_for_existing_users(ctx);
                }
            }
            _ => {
                ctx.notify();
            }
        }
    }

    pub fn toggle_block_snackbar(&mut self, ctx: &mut ViewContext<Self>) {
        BlockListSettings::handle(ctx).update(ctx, |blocklist_settings, ctx| {
            report_if_error!(blocklist_settings
                .snackbar_enabled
                .toggle_and_save_value(ctx));
        });
    }

    pub fn toggle_error_underlining(&mut self, ctx: &mut ViewContext<Self>) {
        InputSettings::handle(ctx).update(ctx, |input_settings, ctx| {
            report_if_error!(input_settings.error_underlining.toggle_and_save_value(ctx));
        });
    }

    pub fn toggle_syntax_highlighting(&mut self, ctx: &mut ViewContext<Self>) {
        InputSettings::handle(ctx).update(ctx, |input_settings, ctx| {
            report_if_error!(input_settings
                .syntax_highlighting
                .toggle_and_save_value(ctx));
        });
    }

    pub fn change_cursor(&mut self, cursor_shape: Cursor, ctx: &mut ViewContext<Self>) {
        ctx.set_cursor_shape(cursor_shape);
        ctx.notify();
    }

    pub fn set_a11y_verbosity(
        &mut self,
        verbosity: AccessibilityVerbosity,
        ctx: &mut ViewContext<Self>,
    ) {
        AccessibilitySettings::handle(ctx).update(ctx, |accessibility_settings, ctx| {
            report_if_error!(accessibility_settings
                .a11y_verbosity
                .set_value(verbosity, ctx));
        });
    }

    pub fn snapshot(
        &self,
        window_id: WindowId,
        quake_mode: bool,
        app: &AppContext,
    ) -> WindowSnapshot {
        let window_bounds = app.window_bounds(&window_id);
        let window_fullscreen_state = app
            .windows()
            .platform_window(window_id)
            .map(|window| window.fullscreen_state())
            .unwrap_or_default();
        let active_tab_index = self.active_tab_index();
        let drag_model = CrossWindowTabDrag::as_ref(app);
        // Use the placeholder-aware getter so we don't skip an unrelated
        // tab at a stale `source_tab_index` after a put-back handoff has
        // already removed the real placeholder.
        let transferred_tab_index = if drag_model.is_active()
            && drag_model.source_window_id() == Some(window_id)
        {
            if drag_model.has_dedicated_preview_window() {
                // Multi-tab drag: skip the dedicated-preview placeholder.
                drag_model.source_placeholder_tab_index()
            } else if drag_model.source_was_single_tab() && drag_model.handed_off_target().is_some()
            {
                // Single-tab drag in InsertedInTarget phase: the source's
                // only tab has been transferred to the target window's live
                // view context.  Snapshotting it here would call
                // `terminal_view.as_ref()` while that view is being updated
                // in the target window, triggering a circular view reference
                // panic.  Skip index 0 (the sole tab).
                Some(0)
            } else {
                None
            }
        } else {
            None
        };
        let tabs = self
            .tab_views()
            .enumerate()
            .filter(|(tab_index, _)| Some(*tab_index) != transferred_tab_index)
            .map(|(tab_index, pane_group_view)| {
                let resizable_data = ResizableData::handle(app);
                let modal_sizes = resizable_data.as_ref(app).get_all_handles(window_id);

                let left_panel_width = modal_sizes.map(|ms| {
                    ms.left_panel_width
                        .lock()
                        .expect("should be able to lock left panel handle")
                        .size()
                });

                let right_panel_width = modal_sizes.map(|ms| {
                    ms.right_panel_width
                        .lock()
                        .expect("should be able to lock right panel handle")
                        .size()
                });

                let pane_group = pane_group_view.as_ref(app);
                let root = pane_group.snapshot(app);
                let left_panel =
                    self.compute_left_panel_snapshot(pane_group_view, left_panel_width, app);
                let right_panel =
                    self.compute_right_panel_snapshot(pane_group_view, right_panel_width, app);
                TabSnapshot {
                    root,
                    custom_title: pane_group.custom_title(app),
                    default_directory_color: self
                        .tabs
                        .get(tab_index)
                        .and_then(|tab| tab.default_directory_color),
                    selected_color: self
                        .tabs
                        .get(tab_index)
                        .map(|tab| tab.selected_color)
                        .unwrap_or_default(),
                    left_panel,
                    right_panel,
                }
            })
            .filter(|tab| {
                // Filter out any tab that contains a single, read-only session.
                !matches!(
                    tab.root,
                    PaneNodeSnapshot::Leaf(LeafSnapshot {
                        contents: LeafContents::Terminal(TerminalPaneSnapshot {
                            is_read_only: true,
                            ..
                        }),
                        ..
                    })
                )
            })
            .collect();

        let resizable_data = ResizableData::handle(app);
        let modal_sizes = resizable_data.as_ref(app).get_all_handles(window_id);

        // Reads the current width of the universal search modal, to store with the window snapshot
        let universal_search_width = modal_sizes.map(|ms| {
            ms.universal_search_width
                .lock()
                .expect("should be able to lock universal search resizable state handle")
                .size()
        });

        let warp_ai_width = modal_sizes.map(|ms| {
            ms.warp_ai_width
                .lock()
                .expect("should be able to lock warp_ai resizable state handle")
                .size()
        });

        let voltron_width = modal_sizes.map(|ms| {
            ms.voltron_width
                .lock()
                .expect("should be able to lock voltron resizable state handle")
                .size()
        });

        let warp_drive_index_width = modal_sizes.map(|ms| {
            ms.warp_drive_index_width
                .lock()
                .expect("should be able to lock warp drive resizable state handle")
                .size()
        });

        let left_panel_width = modal_sizes.map(|ms| {
            ms.left_panel_width
                .lock()
                .map(|guard| guard.size())
                .unwrap_or(DEFAULT_LEFT_PANEL_WIDTH)
        });

        let right_panel_width = modal_sizes.map(|ms| {
            ms.right_panel_width
                .lock()
                .map(|guard| guard.size())
                .unwrap_or(DEFAULT_RIGHT_PANEL_WIDTH)
        });

        let agent_management_filters = Some(
            self.agent_management_view
                .read(app, |view, _| view.get_filters()),
        );

        WindowSnapshot {
            tabs,
            active_tab_index,
            bounds: window_bounds,
            fullscreen_state: window_fullscreen_state,
            quake_mode,
            universal_search_width,
            warp_ai_width,
            voltron_width,
            warp_drive_index_width,
            left_panel_open: self.left_panel_open,
            vertical_tabs_panel_open: self.vertical_tabs_panel_open,
            left_panel_width,
            right_panel_width,
            agent_management_filters,
        }
    }

    fn compute_left_panel_snapshot(
        &self,
        pane_group: &ViewHandle<PaneGroup>,
        left_panel_width: Option<f32>,
        app: &AppContext,
    ) -> Option<LeftPanelSnapshot> {
        let pane_group_ref = pane_group.as_ref(app);
        if !pane_group_ref.left_panel_open {
            return None;
        }

        let pane_group_id = pane_group.id();

        self.left_panel_view.read(app, |lp, _| {
            Some(LeftPanelSnapshot {
                left_panel_displayed_tab: lp.active_view().into(),
                pane_group_id: pane_group_id.to_string(),
                width: left_panel_width.unwrap_or(DEFAULT_LEFT_PANEL_WIDTH) as usize,
            })
        })
    }

    fn compute_right_panel_snapshot(
        &self,
        pane_group: &ViewHandle<PaneGroup>,
        right_panel_width: Option<f32>,
        app: &AppContext,
    ) -> Option<RightPanelSnapshot> {
        let pane_group_ref = pane_group.as_ref(app);
        if !pane_group_ref.right_panel_open {
            return None;
        }

        let pane_group_id = pane_group.id();
        let is_maximized = pane_group_ref.is_right_panel_maximized;

        Some(RightPanelSnapshot {
            pane_group_id: pane_group_id.to_string(),
            width: right_panel_width.unwrap_or(DEFAULT_RIGHT_PANEL_WIDTH) as usize,
            is_maximized,
        })
    }

    pub fn open_launch_config_save_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.close_palette(true, None, ctx); // close palettes if any are open
        self.launch_config_save_modal.open();
        self.current_workspace_state
            .is_launch_config_save_modal_open = true;

        self.launch_config_save_modal.view.update(ctx, |view, ctx| {
            view.set_snapshot_source(ctx);
            view.reset_editor(ctx); // placeholder and clear editor
            ctx.notify();
        });

        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::SaveNewLaunchConfig),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        ctx.focus(&self.launch_config_save_modal.view);
        ctx.notify();
    }

    pub fn cycle_prev_session(&mut self, ctx: &mut ViewContext<Self>) {
        self.cycle_session(SessionCycleDirection::Previous, ctx);
    }

    pub fn cycle_next_session(&mut self, ctx: &mut ViewContext<Self>) {
        self.cycle_session(SessionCycleDirection::Next, ctx);
    }

    fn cycle_session(&mut self, direction: SessionCycleDirection, ctx: &mut ViewContext<Self>) {
        let keys_settings = KeysSettings::as_ref(ctx);
        let ctrl_tab_behavior = *keys_settings.ctrl_tab_behavior;
        match ctrl_tab_behavior {
            CtrlTabBehavior::ActivatePrevNextTab => match direction {
                SessionCycleDirection::Next => {
                    self.activate_next_tab(ctx);
                }
                SessionCycleDirection::Previous => {
                    self.activate_prev_tab(ctx);
                }
            },
            CtrlTabBehavior::CycleMostRecentSession | CtrlTabBehavior::CycleMostRecentTab => {
                self.current_workspace_state.is_palette_open = false;
                let palette_was_open = self.current_workspace_state.is_ctrl_tab_palette_open;
                if !palette_was_open {
                    self.open_palette_action(
                        PaletteMode::Navigation,
                        PaletteSource::CtrlTab {
                            shift_pressed_initially: matches!(
                                direction,
                                SessionCycleDirection::Previous
                            ),
                        },
                        None,
                        ctx,
                    );
                }
                // CycleMostRecentSession: always advance (async sources need explicit
                // advance after palette open). CycleMostRecentTab: advance only when
                // palette was already open (sync offset handles first-open selection).
                if palette_was_open
                    || matches!(ctrl_tab_behavior, CtrlTabBehavior::CycleMostRecentSession)
                {
                    self.ctrl_tab_palette
                        .update(ctx, |palette, ctx| match direction {
                            SessionCycleDirection::Next => {
                                palette.select_next_item(ctx);
                            }
                            SessionCycleDirection::Previous => {
                                palette.select_prev_item(ctx);
                            }
                        });
                }
                ctx.notify();
            }
        }
    }

    pub fn activate_prev_tab(&mut self, ctx: &mut ViewContext<Self>) {
        let index = if self.vertical_tabs_panel.search_query.is_empty() {
            if self.active_tab_index > 0 {
                self.active_tab_index - 1
            } else {
                self.tabs.len() - 1
            }
        } else {
            let matching = self.vertical_tabs_panel.matching_tab_indices(
                &self.tabs,
                self.active_tab_index,
                ctx,
            );
            matching
                .iter()
                .rev()
                .find(|&&i| i < self.active_tab_index)
                .or_else(|| matching.last())
                .copied()
                .unwrap_or(self.active_tab_index)
        };
        self.activate_tab(index, ctx);
    }

    pub fn activate_next_tab(&mut self, ctx: &mut ViewContext<Self>) {
        let index = if self.vertical_tabs_panel.search_query.is_empty() {
            if self.active_tab_index + 1 < self.tabs.len() {
                self.active_tab_index + 1
            } else {
                0
            }
        } else {
            let matching = self.vertical_tabs_panel.matching_tab_indices(
                &self.tabs,
                self.active_tab_index,
                ctx,
            );
            matching
                .iter()
                .find(|&&i| i > self.active_tab_index)
                .or_else(|| matching.first())
                .copied()
                .unwrap_or(self.active_tab_index)
        };
        self.activate_tab(index, ctx);
    }

    pub fn activate_last_tab(&mut self, ctx: &mut ViewContext<Self>) {
        if self.tabs.len() > 1 {
            let target_index = self.tabs.len() - 1;
            self.activate_tab(target_index, ctx);
        }
    }

    /// If a closing tab is an untouched split-off child-agent tab, move its
    /// pane back to the original tab instead of closing it. Returns true if
    /// handled.
    fn try_re_adopt_split_off_child_agent_tab(
        &mut self,
        index: usize,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let Some(tab_data) = self.tabs.get(index) else {
            return false;
        };
        let Some(origin) = tab_data
            .pane_group
            .as_ref(ctx)
            .child_agent_origin()
            .cloned()
        else {
            return false;
        };
        let Some(source_pane_group) = origin.source_pane_group.upgrade(ctx) else {
            return false;
        };

        let pane_group = tab_data.pane_group.clone();
        // Only re-adopt untouched split-off tabs; changed layouts use normal
        // close handling.
        let pane_ids: Vec<PaneId> = pane_group.as_ref(ctx).pane_ids().collect();
        if pane_ids.len() != 1 {
            return false;
        }
        let pane_id = pane_ids[0];

        let Some(pane_content) =
            pane_group.update(ctx, |pg, ctx| pg.remove_pane_for_move(&pane_id, ctx))
        else {
            return false;
        };

        source_pane_group.update(ctx, |pg, ctx| {
            pg.re_adopt_child_agent_pane(pane_content, origin.conversation_id, ctx);
        });

        true
    }

    fn remove_tab(
        &mut self,
        index: usize,
        add_to_undo_stack: bool,
        detach_panes_for_close: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(pane_group) = self.tabs.get(index).map(|t| t.pane_group.clone()) else {
            debug_assert!(false, "Tried to remove a tab with an invalid index");
            return;
        };

        // Clear a detail sidecar anchored to this tab before the tab disappears.
        self.vertical_tabs_panel
            .clear_detail_sidecar_if_for_pane_group(pane_group.id());

        // If this is the last tab, close the window instead of actually removing
        // the tab.
        if self.tabs.len() == 1 {
            if ContextFlag::CloseWindow.is_enabled() {
                ctx.close_window();
            }
            return;
        }

        // Preserve split-off child-agent tabs by moving their lone pane back
        // before close cleanup. Skip tab moves so the destination keeps the
        // pane.
        let re_adopted =
            detach_panes_for_close && self.try_re_adopt_split_off_child_agent_tab(index, ctx);

        if !re_adopted && detach_panes_for_close {
            let working_directories_model = self.working_directories_model.clone();
            pane_group.update(ctx, |pane_group, ctx| {
                pane_group.for_all_terminal_panes(
                    |terminal_view, ctx| {
                        if terminal_view
                            .model
                            .lock()
                            .block_list()
                            .active_block()
                            .is_active_and_long_running()
                        {
                            terminal_view.shutdown_pty(ctx);
                        }
                    },
                    ctx,
                );

                pane_group.detach_panes_for_close(&working_directories_model, ctx);
            });
        }

        let tab_data = self.tabs.remove(index);

        let removed_pane_group_id = tab_data.pane_group.id();
        self.tab_mru_order.retain(|id| *id != removed_pane_group_id);

        // Re-adopted child tabs leave no useful tab contents to restore; the
        // live pane already moved back.
        if add_to_undo_stack && !re_adopted {
            let handle = ctx.handle();
            UndoCloseStack::handle(ctx).update(ctx, |stack, ctx| {
                log::info!("storing data for closed tab");
                stack.handle_tab_closed(handle, index, tab_data, ctx);
            });
        }

        match index.cmp(&self.active_tab_index) {
            Ordering::Equal => {
                // Horizontal tabs should activate the tab that was immediately to the
                // right of the closed tab. After removal, that tab has the same index.
                // If the closed tab was the last tab, fall back to the previous tab.
                let active_index = if uses_vertical_tabs(ctx) {
                    index.saturating_sub(1)
                } else {
                    index.min(self.tabs.len() - 1)
                };
                self.activate_tab_internal(active_index, ctx);
            }
            Ordering::Less => {
                // If we are closing a tab before the active tab we need to adjust
                // the active tab index.
                self.active_tab_index -= 1;
            }
            _ => {}
        }

        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    fn should_confirm_close_session(&self, ctx: &mut ViewContext<Self>) -> bool {
        // If we're closing the only remaining tab, we're actually going to close the window.
        // We don't need a user confirmation here because there's already another one on window close.
        if self.tab_count() == 1 {
            return false;
        }
        // TODO: remove session sharing flag check when long-running commands are included
        FeatureFlag::CreatingSharedSessions.is_enabled()
            && ContextFlag::CreateSharedSession.is_enabled()
            && *SessionSettings::as_ref(ctx).should_confirm_close_session
    }

    /// Checks if the provided tab indices need to be confirmed before closing, unless skip_confirmation is true.
    /// If none of them need confirmation (or the confirm setting is turned off), we close all the provided tabs.
    /// Returns true iff all of the tabs were closed.
    fn close_tabs(
        &mut self,
        tab_indices: impl Iterator<Item = usize>,
        dialog_source: OpenDialogSource,
        skip_confirmation: bool,
        add_to_undo_stack: bool,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let tab_indices_vec = tab_indices.collect_vec();
        // Check if there are any tabs that can't be closed without confirmation
        if !skip_confirmation && self.should_confirm_close_session(ctx) {
            for i in tab_indices_vec.iter() {
                let is_tab_shared = self
                    .get_pane_group_view(*i)
                    .is_some_and(|view| view.as_ref(ctx).is_terminal_pane_being_shared(ctx));
                if is_tab_shared {
                    self.show_close_session_confirmation_dialog(dialog_source, ctx);
                    return false;
                }
            }
        }

        if !skip_confirmation {
            let tabs = tab_indices_vec
                .iter()
                .filter_map(|i| self.get_pane_group_view(*i))
                .map(|tab| tab.downgrade())
                .collect_vec();
            let summary = UnsavedStateSummary::for_tabs(tabs, ctx);

            if summary.should_display_warning(ctx) {
                // The quit-warning dialog uses app-scoped callbacks (ironically, because that's
                // what Self::show_native_modal expects). That means we need a handle to the
                // current workspace here.
                let confirm_self = ctx.handle();
                let navigate_self = ctx.handle();
                let confirm_tabs = tab_indices_vec.clone();
                let dialog = summary
                    .dialog()
                    .on_confirm(move |ctx| {
                        if let Some(workspace) = confirm_self.upgrade(ctx) {
                            workspace.update(ctx, |workspace, ctx| {
                                workspace.close_tabs(
                                    confirm_tabs.into_iter(),
                                    dialog_source,
                                    true,
                                    add_to_undo_stack,
                                    ctx,
                                );
                            });
                        }
                    })
                    .on_cancel(|_ctx| { /* No action needed besides dismissing the dialog. */ })
                    .on_show_processes(move |ctx| {
                        if let Some(workspace) = navigate_self.upgrade(ctx) {
                            workspace.update(ctx, |workspace, ctx| {
                                // TODO(ben): Ideally, this would filter to the relevant tabs.
                                workspace.open_palette_action(
                                    PaletteMode::Navigation,
                                    PaletteSource::QuitModal,
                                    Some("running"),
                                    ctx,
                                );
                            })
                        }
                    })
                    .build();

                send_telemetry_from_ctx!(
                    TelemetryEvent::QuitModalShown {
                        running_processes: summary.total_long_running_commands as u32,
                        shared_sessions: summary.shared_sessions as u32,
                        modal_for: CloseTarget::Tab,
                    },
                    ctx
                );

                if cfg!(all(not(target_family = "wasm"), target_os = "macos")) {
                    AppContext::show_native_platform_modal(ctx, dialog);
                    return false;
                } else if cfg!(all(
                    not(target_family = "wasm"),
                    any(target_os = "linux", windows)
                )) {
                    self.show_native_modal(dialog, ctx);
                    return false;
                }
            }
        }

        // If we are renaming a tab, cancel that.  Closing tabs causes the renamed tab index
        // to fall out of sync.  This can cause inconsistencies.
        self.cancel_tab_rename(ctx);

        // Remove the tabs in reverse order to avoid indexing OOB.
        for i in tab_indices_vec.into_iter().sorted().rev() {
            self.remove_tab(i, add_to_undo_stack, true, ctx);
        }
        true
    }

    /// Opens a confirmation dialog if necessary, or closes immediately if not.
    /// Always closes immediately if skip_confirmation is true.
    fn close_tab(
        &mut self,
        index: usize,
        skip_confirmation: bool,
        add_to_undo_stack: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let is_last_tab = self.tabs.len() == 1;
        if !ContextFlag::CloseWindow.is_enabled() && is_last_tab {
            return;
        }

        let tabs_closed = self.close_tabs(
            vec![index].into_iter(),
            OpenDialogSource::CloseTab { tab_index: index },
            skip_confirmation || is_last_tab, // If this is the last tab, the confirmation dialog will be handled by the window close.
            add_to_undo_stack,
            ctx,
        );

        // Telemetry whenever tabs actually closed, not when confirmation dialog comes up.
        if tabs_closed {
            ctx.dispatch_global_action("workspace:save_app", ());
            send_telemetry_from_ctx!(
                TelemetryEvent::TabOperations {
                    action: TabTelemetryAction::CloseTab,
                },
                ctx
            );
        }
    }

    /// Opens a confirmation dialog if necessary, or closes immediately if not.
    /// Always closes immediately if skip_confirmation is true.
    pub fn close_other_tabs(
        &mut self,
        index: usize,
        skip_confirmation: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        // Figure out what indices we want to delete for the "other tabs" case.
        let indices_to_remove = (0..self.tabs.len()).filter(|i| *i != index);

        let tabs_closed = self.close_tabs(
            indices_to_remove,
            OpenDialogSource::CloseOtherTabs { tab_index: index },
            skip_confirmation,
            true,
            ctx,
        );

        // Telemetry whenever tabs actually closed, not when confirmation dialog comes up.
        if tabs_closed {
            send_telemetry_from_ctx!(
                TelemetryEvent::TabOperations {
                    action: TabTelemetryAction::CloseOtherTabs,
                },
                ctx
            );
        }
    }

    /// Opens a confirmation dialog if necessary, or closes immediately if not.
    /// Always closes immediately if skip_confirmation is true.
    pub fn close_tabs_direction(
        &mut self,
        index: usize,
        direction: TabMovement,
        skip_confirmation: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let indices_to_remove = match direction {
            TabMovement::Left => 0..index,
            TabMovement::Right => (index + 1)..self.tabs.len(),
        };
        let tabs_closed = self.close_tabs(
            indices_to_remove,
            OpenDialogSource::CloseTabsDirection {
                tab_index: index,
                direction,
            },
            skip_confirmation,
            true,
            ctx,
        );

        // Telemetry whenever tabs actually closed, not when confirmation dialog comes up.
        if tabs_closed {
            match direction {
                TabMovement::Right if self.active_tab_index > index => {
                    send_telemetry_from_ctx!(
                        TelemetryEvent::TabOperations {
                            action: TabTelemetryAction::CloseTabsToRight,
                        },
                        ctx
                    );
                }
                _ => (),
            }
        }
    }

    /// Closes all tabs that have code panes with the specified file path open.
    /// This is used when a file is renamed or deleted in the file tree
    #[cfg(feature = "local_fs")]
    fn close_tabs_with_file_path(&mut self, old_path: &Path, ctx: &mut ViewContext<Self>) {
        // Find all code panes across all tabs that have this file open
        for tab_data in &self.tabs {
            // Check if this tab has any code panes with the old file path open
            tab_data.pane_group.update(ctx, |pane_group, ctx| {
                // Collect code panes first to avoid borrowing issues
                let code_panes: Vec<_> = pane_group.code_panes(ctx).collect();
                for (_, code_pane) in code_panes {
                    code_pane.update(ctx, |code_view, ctx| {
                        code_view.close_tabs_with_path(old_path, ctx);
                    });
                }
            });
        }

        ctx.notify();
    }

    /// Renames all open code tabs that point to `old_path` to now point to `new_path`,
    /// updating their contents in-place rather than closing them.
    #[cfg(feature = "local_fs")]
    fn rename_tabs_with_file_path(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        ctx: &mut ViewContext<Self>,
    ) {
        for tab_data in &self.tabs {
            tab_data.pane_group.update(ctx, |pane_group, ctx| {
                // Collect code panes first to avoid borrowing issues
                let code_panes: Vec<_> = pane_group.code_panes(ctx).collect();
                for (_, code_pane) in code_panes {
                    code_pane.update(ctx, |code_view, ctx| {
                        code_view.rename_tabs_with_path(old_path, new_path, ctx);
                    });
                }
            });
        }
        ctx.notify();
    }

    /// Update this workspace when it is reopened after being closed.
    pub fn handle_reopen(&mut self, ctx: &mut ViewContext<Self>) {
        self.sync_window_button_visibility(ctx);
        for pane_group in self.tab_views() {
            pane_group.update(ctx, |pane_group, ctx| {
                pane_group.reattach_panes(ctx);
            })
        }
        self.update_active_session(ctx);
    }

    pub fn restore_closed_tab(
        &mut self,
        tab_index: usize,
        tab_data: TabData,
        ctx: &mut ViewContext<Self>,
    ) {
        // When restoring a closed tab, we have to reattach its panes so that they know they're
        // user-accessible again.
        tab_data.pane_group.update(ctx, |pane_group, ctx| {
            pane_group.reattach_panes(ctx);
        });

        self.tabs.insert(tab_index, tab_data);
        self.tab_mru_order
            .push(self.tabs[tab_index].pane_group.id());
        self.activate_tab(tab_index, ctx);

        ctx.notify();
    }

    pub fn open_autoupdate_failure_link(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_url(
            "https://docs.warp.dev/support-and-community/troubleshooting-and-support/updating-warp",
        );
    }

    pub fn add_terminal_tab(&mut self, hide_homepage: bool, ctx: &mut ViewContext<Self>) {
        self.add_new_session_tab_with_default_mode(
            NewSessionSource::Tab,
            Some(ctx.window_id()),
            None,
            hide_homepage,
            ctx,
        );
        ctx.notify();
    }

    fn add_welcome_tab(&mut self, ctx: &mut ViewContext<Self>) {
        let startup_directory = self.get_new_tab_startup_directory(
            NewSessionSource::Tab,
            Some(ctx.window_id()),
            None,
            ctx,
        );
        self.add_tab_with_pane_layout(
            PanesLayout::Snapshot(Box::new(PaneNodeSnapshot::Leaf(LeafSnapshot {
                is_focused: true,
                custom_vertical_tabs_title: None,
                contents: LeafContents::Welcome { startup_directory },
            }))),
            Arc::new(HashMap::new()),
            None,
            ctx,
        );
        ctx.notify();
    }

    fn add_get_started_tab(&mut self, ctx: &mut ViewContext<Self>) {
        self.add_tab_with_pane_layout(
            PanesLayout::Snapshot(Box::new(PaneNodeSnapshot::Leaf(LeafSnapshot {
                is_focused: true,
                custom_vertical_tabs_title: None,
                contents: LeafContents::GetStarted,
            }))),
            Arc::new(HashMap::new()),
            None,
            ctx,
        );
        ctx.notify();
    }



    // Adds a tab with a specific shell, only meant to be dispatched directly by actions.
    fn add_tab_with_shell(
        &mut self,
        shell: AvailableShell,
        source: AddTabWithShellSource,
        ctx: &mut ViewContext<Self>,
    ) {
        send_telemetry_from_ctx!(
            TelemetryEvent::AddTabWithShell {
                source,
                shell: shell.telemetry_value()
            },
            ctx
        );
        self.add_new_session_tab_with_default_mode(
            NewSessionSource::Tab,
            Some(ctx.window_id()),
            Some(shell),
            false,
            ctx,
        );
        ctx.notify();
    }

    fn add_new_session_tab_with_default_mode(
        &mut self,
        new_session_source: NewSessionSource,
        previous_session_window_id: Option<WindowId>,
        chosen_shell: Option<AvailableShell>,
        hide_homepage: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        self.add_new_session_tab_internal_with_default_session_mode_behavior(
            new_session_source,
            previous_session_window_id,
            chosen_shell,
            hide_homepage,
            DefaultSessionModeBehavior::Apply,
            ctx,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn add_new_session_tab_internal_with_default_session_mode_behavior(
        &mut self,
        new_session_source: NewSessionSource,
        previous_session_window_id: Option<WindowId>,
        chosen_shell: Option<AvailableShell>,
        hide_homepage: bool,
        default_session_mode_behavior: DefaultSessionModeBehavior,
        ctx: &mut ViewContext<Self>,
    ) {
        // Check if we should default to agent mode (only for new sessions, not restorations)
        let should_enter_agent_view = matches!(
            default_session_mode_behavior,
            DefaultSessionModeBehavior::Apply
        ) && AISettings::as_ref(ctx).default_session_mode(ctx) == DefaultSessionMode::Agent;
        #[cfg(feature = "local_tty")]
        let is_docker_sandbox = chosen_shell
            .as_ref()
            .is_some_and(AvailableShell::is_docker_sandbox);
        #[cfg(not(feature = "local_tty"))]
        let is_docker_sandbox = {
            let _ = chosen_shell.as_ref();
            false
        };

        let startup_directory = self.get_new_tab_startup_directory(
            new_session_source,
            previous_session_window_id,
            chosen_shell.as_ref(),
            ctx,
        );

        self.add_tab_with_pane_layout(
            PanesLayout::SingleTerminal(Box::new(NewTerminalOptions {
                shell: chosen_shell,
                initial_directory: startup_directory,
                hide_homepage,
                ..Default::default()
            })),
            Arc::new(HashMap::new()),
            None, /*custom_tab_title*/
            ctx,
        );

        #[cfg(all(feature = "local_tty", not(target_family = "wasm")))]
        if is_docker_sandbox {
            if let Some(terminal_view) = self
                .active_tab_pane_group()
                .as_ref(ctx)
                .active_session_view(ctx)
            {
                TerminalView::initialize_docker_sandbox_environment(&terminal_view, ctx);
            } else {
                log::warn!("Could not find docker sandbox terminal view after creating new tab");
            }
        }
        #[cfg(not(all(feature = "local_tty", not(target_family = "wasm"))))]
        let _ = is_docker_sandbox;
        // If the default session mode is Agent and AI is enabled, enter agent view
        if should_enter_agent_view {
            self.enter_agent_view_on_active_tab(ctx);
        }
    }


    pub fn add_tab_with_pane_layout(
        &mut self,
        panes_layout: PanesLayout,
        block_lists: Arc<HashMap<PaneUuid, Vec<SerializedBlockListItem>>>,
        custom_tab_title: Option<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        // Remember whether the left panel was open on the current active pane group
        // before creating a new active pane group.
        let left_panel_was_open = if self.tabs.is_empty() {
            false
        } else {
            self.active_tab_pane_group().as_ref(ctx).left_panel_open
        };

        // Capture the active tab's colors before creating the new tab.
        let active_tab = self.tabs.get(self.active_tab_index);
        let active_tab_selected_color = active_tab.map(|tab| tab.selected_color);
        let active_tab_default_color = active_tab.and_then(|tab| tab.default_directory_color);

        let is_new_terminal = matches!(panes_layout, PanesLayout::SingleTerminal(_));
        let is_restoration = matches!(panes_layout, PanesLayout::Snapshot(_));
        // Capture the active tab's group membership so the new tab can inherit it.
        let active_tab_group_id = if FeatureFlag::GroupedTabs.is_enabled() && !is_restoration {
            active_tab.and_then(|tab| tab.group_id)
        } else {
            None
        };
        let new_pane_group = ctx.add_typed_action_view(|ctx| {
            let mut pane_group = PaneGroup::new_with_panes_layout(
                self.tips_completed.clone(),
                self.user_default_shell_unsupported_banner_model_handle
                    .clone(),
                self.server_api.clone(),
                panes_layout,
                block_lists,
                self.model_event_sender.clone(),
                ctx,
            );
            if let Some(title) = custom_tab_title {
                pane_group.set_title(&title, ctx);
            }
            pane_group
        });

        ctx.subscribe_to_view(&new_pane_group, move |me, pane_group, event, ctx| {
            me.handle_file_tree_event(pane_group, event, ctx)
        });

        let new_tab_placement_setting = TabSettings::as_ref(ctx).new_tab_placement;

        match new_tab_placement_setting {
            NewTabPlacement::AfterAllTabs => {
                // When inheriting a group, land at the end of the group's
                // contiguous run instead of past it so the setting is
                // honored within the group's bounds.
                let insert_idx = active_tab_group_id
                    .and_then(|gid| {
                        group_member_indices(&self.tabs, gid)
                            .last()
                            .map(|last| last + 1)
                    })
                    .unwrap_or(self.tabs.len());
                self.tabs.insert(insert_idx, TabData::new(new_pane_group));
                self.tab_mru_order
                    .push(self.tabs[insert_idx].pane_group.id());
                self.activate_tab_internal(insert_idx, ctx);
            }
            // Add tab after current tab
            _ => {
                if self.tab_count() == 0 {
                    self.tabs.push(TabData::new(new_pane_group));
                    self.tab_mru_order
                        .push(self.tabs.last().unwrap().pane_group.id());
                    self.activate_tab_internal(self.tab_count() - 1, ctx);
                } else {
                    let insert_idx = self.active_tab_index + 1;
                    self.tabs.insert(insert_idx, TabData::new(new_pane_group));
                    self.tab_mru_order
                        .push(self.tabs[insert_idx].pane_group.id());
                    self.activate_tab_internal(insert_idx, ctx);
                }
            }
        }

        // Inherit the active tab's group membership. D
        if let Some(group_id) = active_tab_group_id {
            let new_idx = self.active_tab_index;
            if let Some(new_tab) = self.tabs.get_mut(new_idx) {
                new_tab.group_id = Some(group_id);
            }
        }

        if !is_restoration {
            if *TabSettings::as_ref(ctx).preserve_active_tab_color.value() {
                if let Some(SelectedTabColor::Color(color)) = active_tab_selected_color {
                    self.tabs[self.active_tab_index].selected_color =
                        SelectedTabColor::Color(color);
                }
            }

            // preserve the current tab's default directory color when the new tab inherits the working directory
            // (otherwise the new tab's color flashes from no-color to default color during bootstrapping).
            if FeatureFlag::DirectoryTabColors.is_enabled() && is_new_terminal {
                let wd_config = &SessionSettings::as_ref(ctx).working_directory_config;
                let inherits_cwd = wd_config.config_for_source(NewSessionSource::Tab).mode
                    == WorkingDirectoryMode::PreviousDir
                    || wd_config.config_for_source(NewSessionSource::Window).mode
                        == WorkingDirectoryMode::PreviousDir;
                if inherits_cwd {
                    if let Some(color) = active_tab_default_color {
                        self.tabs[self.active_tab_index].default_directory_color = Some(color);
                    }
                }
            }
        }

        // If the previous tab's left panel was open, maintain that state with the new tab
        // (unless we're restoring the tab from a persisted snapshot).
        if FeatureFlag::AgentViewConversationListView.is_enabled()
            && !is_restoration
            && left_panel_was_open
        {
            self.active_tab_pane_group().update(ctx, |pg, ctx| {
                pg.set_left_panel_open(true, ctx);
            });
        }
    }

    pub fn add_tab_from_existing_pane(
        &mut self,
        pane: Box<dyn AnyPaneContent>,
        new_idx: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        let new_pane_group = ctx.add_typed_action_view(|ctx| {
            PaneGroup::new_from_existing_pane(
                pane,
                self.tips_completed.clone(),
                self.user_default_shell_unsupported_banner_model_handle
                    .clone(),
                self.server_api.clone(),
                self.model_event_sender.clone(),
                ctx,
            )
        });
        ctx.subscribe_to_view(&new_pane_group, move |me, pane_group, event, ctx| {
            me.handle_file_tree_event(pane_group, event, ctx)
        });

        if self.tab_count() == 0 {
            self.tabs.push(TabData::new(new_pane_group));
            self.tab_mru_order
                .push(self.tabs.last().unwrap().pane_group.id());
            self.activate_tab_internal(self.tab_count() - 1, ctx);
        } else {
            self.tabs.insert(new_idx, TabData::new(new_pane_group));
            self.tab_mru_order.push(self.tabs[new_idx].pane_group.id());
            self.activate_tab_internal(new_idx, ctx);
        }
    }



    /// The built-in notebook viewer was removed; open the file in the external editor instead.
    pub fn add_tab_for_file_notebook(
        &mut self,
        file_path: Option<PathBuf>,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(path) = file_path {
            crate::util::file::open_file_path_with_editor(None, path, None, ctx);
        }
    }

    pub fn add_tab_for_assisted_autoupdate<V: View>(
        &mut self,
        update_command_fn: impl 'static + Fn(ShellType) -> String,
        context_block: ViewHandle<V>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.add_tab_with_pane_layout(
            Default::default(),
            Arc::new(HashMap::new()),
            Some("Install Update".to_owned()),
            ctx,
        );

        let Some(terminal_view) = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .active_session_view(ctx)
        else {
            log::error!("Could not access terminal view after creating a new tab!");
            return;
        };
        terminal_view.update(ctx, |terminal_view, ctx| {
            terminal_view.insert_rich_content(
                None,
                context_block,
                None,
                terminal::view::rich_content::RichContentInsertionPosition::Append {
                    insert_below_long_running_block: false,
                },
                ctx,
            );
        });

        // Once we know what shell is being used, update the input with the
        // appropriate assisted auto-update command.
        ctx.subscribe_to_view(&terminal_view, move |me, _, event, ctx| {
            if let terminal::Event::ShellSpawned(shell_type) = event {
                me.set_active_terminal_input_contents_and_focus_app(
                    &(update_command_fn)(*shell_type),
                    ctx,
                );
            }
        });
    }

    fn open_repository(&mut self, path: Option<&str>, ctx: &mut ViewContext<Self>) {
        match path {
            Some(path) => self.handle_open_repository(path, ctx),
            None => ctx.open_file_picker(
                |result, ctx| match result {
                    Ok(paths) => {
                        let Some(path) = paths.into_iter().next() else {
                            return;
                        };

                        if let Some(handle) = ctx.handle().upgrade(ctx) {
                            handle.update(ctx, |workspace, ctx| {
                                workspace.handle_open_repository(&path, ctx);
                            });
                        }
                    }
                    Err(err) => {
                        let window_id = ctx.window_id();
                        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                            toast_stack.add_ephemeral_toast(
                                DismissibleToast::error(format!("{err}")),
                                window_id,
                                ctx,
                            );
                        });
                    }
                },
                FilePickerConfiguration::new().folders_only(),
            ),
        }
    }

    fn handle_open_repository(&mut self, path: &str, ctx: &mut ViewContext<Self>) {
        let path_buf = PathBuf::from(path);
        ProjectManagementModel::handle(ctx).update(ctx, |projects, ctx| {
            projects.upsert_project(path_buf.clone(), ctx);
        });
        self.add_tab_with_pane_layout(
            PanesLayout::SingleTerminal(Box::new(NewTerminalOptions {
                initial_directory: Some(path_buf.clone()),
                hide_homepage: true,
                ..Default::default()
            })),
            Arc::new(HashMap::new()),
            None,
            ctx,
        );
        self.active_tab_pane_group().update(ctx, |tab, ctx| {
            if let Some(active_terminal) = tab.active_session_view(ctx) {
                active_terminal.update(ctx, |terminal, _| {
                    terminal.maybe_set_pending_repo_init_path(path_buf);
                });
            }
        });
    }












    /// Moves the tab at `index` one slot left/right, where a "slot" is either a
    /// single tab or an entire adjacent group. If the neighbor in the move
    /// direction belongs to a *different* group, the tab hops over that whole
    /// group so it never lands inside the group's contiguous run. Moving past an
    /// ungrouped tab, or reordering within the tab's own group, is an ordinary
    /// one-slot move.
    fn move_tab(&mut self, index: usize, direction: TabMovement, ctx: &mut ViewContext<Self>) {
        let tabs_len = self.tabs.len();
        // The group the moved tab belongs to (if any), so we can distinguish
        // "reorder within my own group" from "hop over a different group".
        let moved_group_id = self.tabs.get(index).and_then(|tab| tab.group_id);
        let target = match direction {
            TabMovement::Left if index > 0 => {
                let neighbor = index - 1;
                match self.tabs[neighbor].group_id {
                    // A different group sits to the left: target its first
                    // member so the tab lands just before the whole group.
                    Some(group_id) if Some(group_id) != moved_group_id => {
                        group_member_index_range(&self.tabs, group_id)
                            .map(|(first, _)| first)
                            .unwrap_or(neighbor)
                    }
                    // Ungrouped neighbor, or a member of our own group: step one.
                    Some(_) | None => neighbor,
                }
            }
            TabMovement::Right if index < tabs_len - 1 => {
                let neighbor = index + 1;
                match self.tabs[neighbor].group_id {
                    // A different group sits to the right: target its last
                    // member; the remove+insert leaves the tab just past it.
                    Some(group_id) if Some(group_id) != moved_group_id => {
                        group_member_index_range(&self.tabs, group_id)
                            .map(|(_, last)| last)
                            .unwrap_or(neighbor)
                    }
                    // Ungrouped neighbor, or a member of our own group: step one.
                    Some(_) | None => neighbor,
                }
            }
            // Already at the start/end of the list: nothing to move past.
            TabMovement::Left | TabMovement::Right => return,
        };

        // `hop_tab_to_index` keeps the same tab active across the move, so we
        // only capture whether the moved tab was the active one for telemetry.
        let moving_active_tab = index == self.active_tab_index;
        self.hop_tab_to_index(index, target, ctx);

        if moving_active_tab {
            send_telemetry_from_ctx!(TelemetryEvent::MoveActiveTab { direction }, ctx);
        } else {
            send_telemetry_from_ctx!(TelemetryEvent::MoveTab { direction }, ctx);
        }
    }

    /// How to render the tab bar.
    fn tab_bar_mode(&self, app: &AppContext) -> ShowTabBar {
        // Drag-preview windows always show the tab bar inline; the user
        // is literally holding the tab they detached, so it must remain
        // visible regardless of the user's hover/fullscreen settings.
        if self.is_tab_drag_preview {
            return ShowTabBar::Stacked;
        }

        // Always show the tab bar during HoA onboarding so that callouts
        // pointing at tabs/inbox render correctly even when the user has
        // "show tab bar on hover" enabled.
        if self.hoa_onboarding_flow.is_some() || self.should_show_session_config_tab_config_chip() {
            return ShowTabBar::Stacked;
        }

        if !FeatureFlag::FullScreenZenMode.is_enabled() {
            return ShowTabBar::default();
        }

        let is_fullscreen = app
            .windows()
            .platform_window(self.window_id)
            .is_some_and(|window| window.fullscreen_state() == FullscreenState::Fullscreen);

        let is_hovered = self
            .tab_bar_hover_state
            .lock()
            .is_ok_and(|state| state.is_hovered())
            || self.traffic_light_mouse_states.are_traffic_lights_hovered();

        // Check if any of the menus/popups rendered relative to the tab bar are open.
        let is_vertical_tabs_active = FeatureFlag::VerticalTabs.is_enabled()
            && *TabSettings::as_ref(app).use_vertical_tabs
            && self.vertical_tabs_panel_open;
        let is_tab_menu_open = self.show_tab_bar_overflow_menu
            || (self.show_tab_right_click_menu.is_some() && !is_vertical_tabs_active)
            || (self.show_new_session_dropdown_menu.is_some() && !is_vertical_tabs_active)
            || (!FeatureFlag::AgentMode.is_enabled() && self.should_show_ai_assistant_warm_welcome)
            || self.is_user_menu_open
            || self.tab_bar_pinned_by_popup;

        // Check if any panes are being dragged (potentially into a new tab).
        let is_pane_being_dragged = self
            .active_tab_pane_group()
            .as_ref(app)
            .any_pane_being_dragged(app);

        let workspace_decoration_visibility = TabSettings::as_ref(app)
            .workspace_decoration_visibility
            .value();

        let hovered_visibility = if is_pane_being_dragged || is_hovered || is_tab_menu_open {
            ShowTabBar::Stacked
        } else {
            ShowTabBar::Hidden
        };

        match workspace_decoration_visibility {
            WorkspaceDecorationVisibility::OnHover => hovered_visibility,
            // If the tab bar is hidden when fullscreen, show/hide on hover.
            WorkspaceDecorationVisibility::HideFullscreen if is_fullscreen => hovered_visibility,
            // If the user always wants a tab bar OR the window isn't fullscreen, make it
            // persistently stacked above the content area.
            _ => ShowTabBar::Stacked,
        }
    }

    #[cfg(target_os = "macos")]
    pub fn sync_window_button_visibility(&self, ctx: &mut ViewContext<Self>) {
        use riftui::platform::mac::WindowExt;
        let show = if FeatureFlag::FullScreenZenMode.is_enabled()
            && TabSettings::as_ref(ctx)
                .workspace_decoration_visibility
                .value()
                == &WorkspaceDecorationVisibility::OnHover
        {
            self.tab_bar_mode(ctx).has_tab_bar()
        } else {
            TabSettings::as_ref(ctx)
                .workspace_decoration_visibility
                .show_window_decorations()
        };
        if let Some(platform_window) = ctx.windows().platform_window(ctx.window_id()) {
            platform_window.as_ref().set_window_buttons(show);
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn sync_window_button_visibility(&self, _: &mut ViewContext<Self>) {
        // Only macOS uses native window buttons.
    }

    /// Updates the titlebar height to match the scaled tab bar height.
    pub fn update_titlebar_height(&self, ctx: &mut ViewContext<Self>) {
        let zoom_factor = WindowSettings::as_ref(ctx).zoom_level.as_zoom_factor();
        let scaled_tab_bar_height = (TOTAL_TAB_BAR_HEIGHT * zoom_factor) as f64;

        if let Some(platform_window) = ctx.windows().platform_window(ctx.window_id()) {
            platform_window
                .as_ref()
                .set_titlebar_height(scaled_tab_bar_height);
        }
    }

    fn request_notification_permissions_if_needed(&mut self, ctx: &mut ViewContext<Self>) {
        // Request permissions any time notifications are currently enabled.
        let current_mode = SessionSettings::as_ref(ctx).notifications.value().mode;

        if current_mode == NotificationsMode::Enabled {
            ctx.request_desktop_notification_permissions(move |view, outcome, ctx| {
                match &outcome {
                    RequestPermissionsOutcome::Accepted => (),
                    RequestPermissionsOutcome::PermissionsDenied => {
                        // Show a helpful toast if the user denied permissions.
                        let url = NOTIFICATIONS_TROUBLESHOOT_URL.to_string();
                        view.toast_stack.update(ctx, |toast_stack, ctx| {
                            let toast = DismissibleToast::error(
                                "Warp doesn't have permission to send desktop notifications.".to_string(),
                            )
                            .with_link(ToastLink::new("Troubleshoot notifications".to_string()).with_href(url));
                            toast_stack.add_persistent_toast(toast, ctx);
                        });
                    }
                    RequestPermissionsOutcome::OtherError { error_message } => {
                        log::error!(
                            "Unknown error when requesting notification permissions. error_msg: {error_message}"
                        );
                    }
                }
                send_telemetry_from_ctx!(
                    TelemetryEvent::NotificationsRequestPermissionsOutcome { outcome },
                    ctx
                );
            });

            send_telemetry_from_ctx!(
                TelemetryEvent::NotificationPermissionsRequested {
                    source: NotificationsTurnedOnSource::Settings,
                    trigger: None
                },
                ctx
            );
        }
    }

    fn toggle_notifications(&mut self, ctx: &mut ViewContext<Self>) {
        let current_settings = SessionSettings::as_ref(ctx).notifications.value().clone();
        let previous_mode = current_settings.mode;
        let new_mode = match previous_mode {
            NotificationsMode::Unset | NotificationsMode::Dismissed => NotificationsMode::Enabled,
            NotificationsMode::Enabled => NotificationsMode::Disabled,
            NotificationsMode::Disabled => NotificationsMode::Enabled,
        };

        SessionSettings::handle(ctx).update(ctx, |settings, ctx| {
            let new_notifications = NotificationsSettings {
                mode: new_mode,
                ..current_settings
            };
            if let Err(e) = settings.notifications.set_value(new_notifications, ctx) {
                log::error!("Error persisting notifications setting: {e}");
            }
        });
    }

    fn open_command_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::CommandPalette),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
        });
    }

    fn open_files_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::CommandPalette),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
            // Reset mixer with correct file data source before setting filter
            let mixer = view.search_bar.as_ref(ctx).mixer().clone();
            view.data_source_store.update(ctx, |store, ctx| {
                store.reset_search_mixer(mixer, false, ctx);
            });
            view.set_active_query_filter(QueryFilter::Files, ctx);
        });
    }
    fn set_command_palette_binding_source(
        &mut self,
        source: PaletteSource,
        ctx: &mut ViewContext<Self>,
    ) {
        let window_id = ctx.window_id();
        // Safety: Unwrap is okay here because we just retrieved the window_id from the context
        // so we know it exists
        let view_id = ctx
            .focused_view_id(window_id)
            .expect("Just retrieved the window_id from the context.");

        let active_palette_handle = if matches!(source, PaletteSource::CtrlTab { .. }) {
            &self.ctrl_tab_palette
        } else {
            &self.palette
        };
        active_palette_handle.update(ctx, |view, ctx| {
            view.set_binding_source(window_id, view_id, ctx);
            ctx.notify();
        });
    }

    fn open_navigation_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
            view.set_active_query_filter(QueryFilter::Sessions, ctx);
            view.set_initial_selection_offset(0, ctx);
        });
        ctx.notify();
    }

    fn open_conversations_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
            view.set_active_query_filter(QueryFilter::Conversations, ctx);
            view.set_initial_selection_offset(0, ctx);
        });
        ctx.notify();
    }

    fn open_ctrl_tab_palette(
        &mut self,
        query_filter: QueryFilter,
        shift_pressed_initially: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let offset = if shift_pressed_initially { -1 } else { 1 };

        self.ctrl_tab_palette.update(ctx, |view, ctx| {
            view.reset(ctx);
        });

        let mixer = self
            .ctrl_tab_palette
            .as_ref(ctx)
            .search_bar
            .as_ref(ctx)
            .mixer()
            .clone();
        let data_source_store = self.ctrl_tab_palette.as_ref(ctx).data_source_store.clone();

        match query_filter {
            QueryFilter::Tabs => {
                let window_id = ctx.window_id();
                let tabs = self.tab_navigation_data(window_id, ctx.as_ref());
                data_source_store.update(ctx, |store, ctx| {
                    store.reset_ctrl_tab_mixer(mixer, tabs, ctx);
                });
            }
            QueryFilter::Sessions => {
                data_source_store.update(ctx, |store, ctx| {
                    store.restore_ctrl_tab_session_mixer(mixer, ctx);
                });
            }
            _ => {}
        }

        self.ctrl_tab_palette.update(ctx, |view, ctx| {
            match query_filter {
                QueryFilter::Tabs => {
                    // Set offset BEFORE filter: the tabs query is synchronous, so results
                    // arrive during set_active_query_filter. The offset must already be
                    // stored so on_mixer_results_changed picks it up.
                    view.set_initial_selection_offset(offset, ctx);
                    view.set_active_query_filter(query_filter, ctx);
                }
                _ => {
                    // Sessions (and other async sources): set filter first, then offset.
                    // The existing post-open select_next_item handles initial selection.
                    view.set_active_query_filter(query_filter, ctx);
                    view.set_initial_selection_offset(offset, ctx);
                }
            }
        });

        ctx.notify();
    }

    fn open_warp_drive_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
            view.set_active_query_filter(QueryFilter::Drive, ctx);
            ctx.focus(&self.palette);
        });
        ctx.notify();
    }

    fn set_navigation_palette_session_source(
        &mut self,
        source: PaletteSource,
        ctx: &mut ViewContext<Self>,
    ) {
        let active_pane_id = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .focused_pane_id(ctx);
        let active_tab_id = self
            .tabs
            .get(self.active_tab_index)
            .map(|tab| tab.pane_group.id());
        let active_window_id = ctx.window_id();

        let active_palette_handle = if matches!(source, PaletteSource::CtrlTab { .. }) {
            &self.ctrl_tab_palette
        } else {
            &self.palette
        };
        active_palette_handle.update(ctx, |view, ctx| {
            // Set the session source when the active_tab_id is Some.
            if let Some(active_tab_id) = active_tab_id {
                view.set_session_source(
                    SessionSource::Set {
                        active_pane_id,
                        active_tab_id,
                        active_window_id,
                    },
                    ctx,
                );
                ctx.notify();
            }
            ctx.notify();
        });
    }

    fn set_palette_sources(&mut self, source: PaletteSource, ctx: &mut ViewContext<Self>) {
        self.set_command_palette_binding_source(source, ctx);
        self.set_navigation_palette_session_source(source, ctx);
    }

    fn open_launch_config_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
            view.set_active_query_filter(QueryFilter::LaunchConfigurations, ctx);
        });
    }

    fn close_palette(
        &mut self,
        focus_active_tab: bool,
        accepted_action_type: Option<&'static str>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.current_workspace_state.is_palette_open = false;
        self.current_workspace_state.is_ctrl_tab_palette_open = false;
        self.tab_bar_pinned_by_popup = false;
        self.sync_window_button_visibility(ctx);
        if focus_active_tab
            // If the user did not do any action on the command palette (eg. closed via shortcut or clicking away)
            // we always force the focus back onto the terminal input
            // Otherwise we check if any other views are open before moving focus back to terminal input
            && (accepted_action_type.is_none()
                || !self
                    .current_workspace_state
                    .is_any_non_terminal_view_open(ctx))
        {
            self.focus_active_tab(ctx);
        }
        ctx.notify();
    }

    /// Close all overlays in this workspace and the active pane group.
    fn close_all_overlays(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.close_all_modals();
        self.close_tab_bar_overflow_menu(ctx);
        self.close_all_chip_menus(ctx);

        self.active_tab_pane_group()
            .update(ctx, |pane_group, ctx| pane_group.close_overlays(ctx));
    }

    /// Close all chip menus across all inputs to prevent overlapping with modals.
    /// This is a defensive measure to ensure chip menus don't stay open when focus-stealing modals appear.
    fn close_all_chip_menus(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(active_input_handle) = self.get_active_input_view_handle(ctx) {
            active_input_handle.update(ctx, |input, ctx| {
                input.prompt_render_helper.prompt_view().update(
                    ctx,
                    |prompt_display, prompt_ctx| {
                        prompt_display.close_all_chip_menus(prompt_ctx);
                    },
                );
            });
        }
    }

    fn open_require_login_modal(&mut self, variant: AuthViewVariant, ctx: &mut ViewContext<Self>) {
        self.require_login_modal.update(ctx, |modal, ctx| {
            modal.set_variant(ctx, variant);
        });

        self.close_all_overlays(ctx);
        self.current_workspace_state.is_require_login_modal_open = true;
        ctx.focus(&self.require_login_modal);
        ctx.notify();
    }

    fn open_auth_override_warning_modal(
        &mut self,
        auth_payload: AuthRedirectPayload,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_all_overlays(ctx);
        self.auth_override_warning_modal.update(ctx, |modal, _| {
            modal.set_interrupted_auth_payload(auth_payload);
        });
        self.current_workspace_state.is_auth_override_modal_open = true;
        ctx.focus(&self.auth_override_warning_modal);
        ctx.notify();
    }

    fn open_palette(
        &mut self,
        mode: PaletteMode,
        source: PaletteSource,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_all_overlays(ctx);

        let active_palette = if matches!(source, PaletteSource::CtrlTab { .. }) {
            &self.ctrl_tab_palette
        } else {
            &self.palette
        };
        active_palette.update(ctx, |palette, ctx| {
            palette.set_is_shared_session_viewer(false, ctx);
        });

        if matches!(source, PaletteSource::TitleBarSearchBar) {
            self.tab_bar_pinned_by_popup = true;
        }
        if matches!(source, PaletteSource::CtrlTab { .. }) {
            self.current_workspace_state.is_ctrl_tab_palette_open = true;
        } else {
            self.current_workspace_state.is_palette_open = true;
        }
        match mode {
            PaletteMode::Command => self.open_command_palette(ctx),
            PaletteMode::Navigation => match source {
                PaletteSource::CtrlTab {
                    shift_pressed_initially,
                } => {
                    let filter = match *KeysSettings::as_ref(ctx).ctrl_tab_behavior {
                        CtrlTabBehavior::CycleMostRecentTab => QueryFilter::Tabs,
                        _ => QueryFilter::Sessions,
                    };
                    self.open_ctrl_tab_palette(filter, shift_pressed_initially, ctx);
                }
                _ => self.open_navigation_palette(ctx),
            },
            PaletteMode::LaunchConfig => self.open_launch_config_palette(ctx),
            PaletteMode::WarpDrive => self.open_warp_drive_palette(ctx),
            PaletteMode::Files => self.open_files_palette(ctx),
            PaletteMode::Conversations => self.open_conversations_palette(ctx),
        }

        ctx.focus(&self.palette);

        send_telemetry_from_ctx!(TelemetryEvent::PaletteSearchOpened { mode, source }, ctx);

        ctx.notify();
    }

    /// Implements the WorkspaceAction::OpenPalette. This method makes sure the palette is open and
    /// has up-to-date sources. Use this if you don't want toggle semantics.
    fn open_palette_action(
        &mut self,
        palette_mode: PaletteMode,
        source: PaletteSource,
        with_content: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        // ensure the palette sources are up-to-date, e.g. maybe there is already a navigation
        // palette open and then new sessions were opened after that
        self.set_palette_sources(source, ctx);
        self.open_palette(palette_mode, source, ctx);
        if let Some(text) = with_content {
            self.palette.update(ctx, |palette, ctx| {
                palette.insert_query_text(text, ctx);
            });
        }
    }

    pub fn is_palette_mode_enabled(&self, palette_mode: PaletteMode, app: &AppContext) -> bool {
        self.palette.as_ref(app).is_mode_enabled(palette_mode, app)
    }

    /// Toggle the open / closed state of the palette (so that hitting shortcut a second time
    /// will close the palette)
    fn toggle_palette(
        &mut self,
        palette_mode: PaletteMode,
        source: PaletteSource,
        ctx: &mut ViewContext<Self>,
    ) {
        // If the invite modal is open, don't show the palette since it won't be visible anyway
        if !self
            .current_workspace_state
            .is_any_non_palette_modal_open(ctx)
        {
            let is_palette_mode_already_open =
                self.palette.as_ref(ctx).is_mode_enabled(palette_mode, ctx)
                    && ((matches!(source, PaletteSource::CtrlTab { .. })
                        && self.current_workspace_state.is_ctrl_tab_palette_open)
                        || self.current_workspace_state.is_palette_open);
            if is_palette_mode_already_open {
                self.close_palette(true, None, ctx);
            } else {
                self.set_palette_sources(source, ctx);
                self.open_palette(palette_mode, source, ctx);
            }
        }
    }

    fn handle_palette_event(&mut self, event: &CommandPaletteEvent, ctx: &mut ViewContext<Self>) {
        match event {
            CommandPaletteEvent::Close {
                accepted_action_type,
            } => self.close_palette(true, *accepted_action_type, ctx),
            CommandPaletteEvent::ExecuteWorkflow { .. }
            | CommandPaletteEvent::InvokeEnvironmentVariables { .. }
            | CommandPaletteEvent::OpenNotebook { .. }
            | CommandPaletteEvent::ViewInWarpDrive { .. }
            | CommandPaletteEvent::OpenFile { .. } => {}
            CommandPaletteEvent::OpenDirectory { path } => {
                let active_terminal_view = self
                    .active_tab_pane_group()
                    .as_ref(ctx)
                    .active_session_view(ctx);

                if let Some(terminal_view) = active_terminal_view {
                    terminal_view.update(ctx, |terminal_view, ctx| {
                        terminal_view.open_repo_folder(path.to_string(), false, ctx);
                    });
                }
            }
        }
    }






    fn set_focused_index(&mut self, index: Option<usize>, ctx: &mut ViewContext<Self>) {
        self.update_warp_drive_view(ctx, |warp_drive, ctx| {
            warp_drive.set_focused_index(index, ctx);
        });
        ctx.notify();
    }

    fn handle_changelog_event(&mut self, event: &ChangelogEvent, ctx: &mut ViewContext<Self>) {
        // For certain contexts, like shared sessions, we do not want to force open the side panel
        // or display the reward modal.
        if !ContextFlag::ForceSidePanelOpen.is_enabled() {
            return;
        }
        // Don't show changelog if user has disabled it in settings.
        let show_changelog_setting = *ChangelogSettings::as_ref(ctx).show_changelog_after_update;

        let mut request_type = None;
        let should_show_changelog = match event {
            ChangelogEvent::ChangelogRequestFailed {
                request_type: ChangelogRequestType::UserAction,
            }
            | ChangelogEvent::ChangelogRequestComplete {
                request_type: ChangelogRequestType::UserAction,
                ..
            } => {
                request_type = Some(ChangelogRequestType::UserAction);
                true
            }
            ChangelogEvent::ChangelogRequestComplete {
                request_type: ChangelogRequestType::WindowLaunch,
                ..
            } => match ChannelState::app_version() {
                Some(version) => {
                    let opening_warp_drive_on_start_up = OPENING_WARP_DRIVE_ON_START_UP
                        .lock()
                        .expect("Should be able to access OPENING_WARP_DRIVE_ON_START_UP");

                    request_type = Some(ChangelogRequestType::WindowLaunch);
                    // Do not show changelog on quake mode window or if it has already been shown
                    // or if we are opening Warp Drive on start up
                    quake_mode_window_id() != Some(ctx.window_id())
                        && !Settings::has_changelog_been_shown(version, ctx)
                        && !*opening_warp_drive_on_start_up
                }
                None => false,
            },
            ChangelogEvent::ChangelogRequestFailed {
                request_type: ChangelogRequestType::WindowLaunch,
            } => false,
            ChangelogEvent::ImageRequestComplete => false,
        } && show_changelog_setting;

        match (should_show_changelog, request_type) {
            (true, Some(ChangelogRequestType::WindowLaunch)) => {
                if let Some(version) = ChannelState::app_version() {
                    Settings::mark_changelog_shown(version, ctx);
                    if FeatureFlag::AvatarInTabBar.is_enabled() {
                        self.update_toast_stack.update(ctx, |stack, ctx| {
                            // Get keybinding for view changelog action
                            let keystroke = ctx
                                .editable_bindings()
                                .find(|binding| binding.name == "workspace:view_changelog")
                                .and_then(|binding| trigger_to_keystroke(binding.trigger));

                            let mut link = ToastLink::new("View changelog".to_owned())
                                .with_onclick_action(WorkspaceAction::ViewLatestChangelog);
                            if let Some(keystroke) = keystroke {
                                link = link.with_keystroke(keystroke);
                            }

                            let toast = DismissibleToast::default(String::from("Warp updated!"))
                                .with_link(link);

                            stack.add_ephemeral_toast(toast, ctx);
                        });
                    } else {
                        // If resource center isn't already open and Warp AI isn't open, then open resource center
                        if !self.current_workspace_state.is_resource_center_open
                            && !self.current_workspace_state.is_ai_assistant_panel_open
                        {
                            self.open_resource_center_main_page(ctx);
                            self.update_resource_center_action_target(ctx);
                            ctx.notify();
                        }
                    }
                }
            }
            (_, Some(ChangelogRequestType::UserAction)) => {
                if !self.current_workspace_state.is_resource_center_open
                    && !self.current_workspace_state.is_ai_assistant_panel_open
                {
                    self.open_resource_center_main_page(ctx);
                    self.update_resource_center_action_target(ctx);
                    ctx.notify();
                }
            }
            _ => {}
        }
    }

    fn manual_check_for_update(&self, ctx: &mut ViewContext<Self>) {
        AutoupdateState::handle(ctx).update(ctx, |autoupdate_state, ctx| {
            autoupdate_state.manually_check_for_update(ctx);
        });
    }

    pub fn is_theme_creator_modal_open(&self) -> bool {
        self.current_workspace_state.is_theme_creator_modal_open
    }

    pub fn is_theme_deletion_modal_open(&self) -> bool {
        self.current_workspace_state.is_theme_deletion_modal_open
    }

    pub fn is_palette_open(&self) -> bool {
        self.current_workspace_state.is_palette_open
            || self.current_workspace_state.is_ctrl_tab_palette_open
    }

    pub fn is_workflow_modal_open(&self) -> bool {
        self.current_workspace_state.is_workflow_modal_open
    }

    pub fn is_warp_drive_open(&self) -> bool {
        self.current_workspace_state.is_warp_drive_open
    }

    pub fn is_left_panel_open(&self, ctx: &AppContext) -> bool {
        self.active_tab_pane_group().as_ref(ctx).left_panel_open
    }

    fn handle_settings_pane_event(
        &mut self,
        event: &SettingsViewEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            SettingsViewEvent::CheckForUpdate => {
                self.manual_check_for_update(ctx);
            }
            SettingsViewEvent::LaunchNetworkLogging => {
                self.open_network_log_pane(ctx);
            }
            SettingsViewEvent::OpenWarpDrive => {
                self.close_all_overlays(ctx);
                self.open_or_toggle_warp_drive(
                    false, /* toggle */
                    false, /* explicit_user_action */
                    ctx,
                );
                ctx.notify();
            }
            SettingsViewEvent::SignupAnonymousUser => {
                self.initiate_user_signup(AnonymousUserSignupEntrypoint::SignUpButton, ctx);
            }
            SettingsViewEvent::Pane(_) | SettingsViewEvent::StartResize => {}
            SettingsViewEvent::ShowToast { message, flavor } => {
                self.toast_stack.update(ctx, |toast_stack, ctx| {
                    toast_stack
                        .add_ephemeral_toast(DismissibleToast::new(message.clone(), *flavor), ctx);
                });
            }
            SettingsViewEvent::OpenAIFactCollection => {
                self.open_ai_fact_collection_pane(Some(Direction::Right), None, ctx);
                send_telemetry_from_ctx!(
                    TelemetryEvent::KnowledgePaneOpened {
                        entrypoint: KnowledgePaneEntrypoint::Settings,
                    },
                    ctx
                );
            }
            SettingsViewEvent::OpenMCPServerCollection => {
                self.show_settings_with_section(Some(SettingsSection::MCPServers), ctx);

                send_telemetry_from_ctx!(
                    TelemetryEvent::MCPServerCollectionPaneOpened {
                        entrypoint: MCPServerCollectionPaneEntrypoint::Settings,
                    },
                    ctx
                );
            }
            SettingsViewEvent::OpenExecutionProfileEditor(profile_id) => {
                self.open_execution_profile_editor_pane(None, *profile_id, ctx);
            }
            SettingsViewEvent::OpenLspLogs { log_path } => {
                self.open_lsp_logs(log_path, ctx);
            }
            SettingsViewEvent::OpenProjectRulesPane { rule_paths } => {
                #[cfg(feature = "local_fs")]
                if let Some(first) = rule_paths.first() {
                    crate::util::file::open_file_path_with_editor(None, first.clone(), None, ctx);
                }
                #[cfg(not(feature = "local_fs"))]
                let _ = rule_paths;
            }
        }
    }

    fn refresh_working_directories_for_pane_group(
        &mut self,
        pane_group: &ViewHandle<PaneGroup>,
        ctx: &mut ViewContext<Self>,
    ) {
        let pane_group_id = pane_group.id();
        let terminal_cwds: Vec<(EntityId, LocalOrRemotePath)> = pane_group
            .as_ref(ctx)
            .terminal_view_working_directories(ctx)
            .filter_map(|(id, cwd)| cwd.map(|c| (id, c)))
            .collect();
        let code_paths: Vec<(EntityId, LocalOrRemotePath)> = pane_group
            .as_ref(ctx)
            .code_view_paths(ctx)
            .filter_map(|(id, cwd)| cwd.map(|c| (id, c)))
            .collect();
        let code_diff_paths: Vec<(EntityId, LocalOrRemotePath)> = pane_group
            .as_ref(ctx)
            .code_diff_view_paths(ctx)
            .filter_map(|(id, cwd)| cwd.map(|c| (id, c)))
            .collect();
        let notebook_paths: Vec<(EntityId, LocalOrRemotePath)> = pane_group
            .as_ref(ctx)
            .file_notebook_paths(ctx)
            .filter_map(|(id, path)| path.map(|p| (id, p)))
            .collect();
        let local_paths: Vec<(EntityId, LocalOrRemotePath)> = code_paths
            .into_iter()
            .chain(notebook_paths)
            .chain(code_diff_paths)
            .collect();

        // Get the focused terminal ID to prioritize it in the repo_to_terminal map
        let focused_terminal_id = pane_group
            .as_ref(ctx)
            .active_session_view(ctx)
            .map(|terminal_view| terminal_view.id());

        self.working_directories_model.update(ctx, |model, ctx| {
            model.refresh_working_directories_for_pane_group(
                pane_group_id,
                terminal_cwds,
                local_paths,
                focused_terminal_id,
                ctx,
            );
        });
    }

    /// Opens the in-app network log pane as a right-split of the active pane
    /// group. If a pane already exists for the current window, refreshes its
    /// snapshot from the in-memory model and focuses it instead of opening
    /// another one.
    pub(crate) fn open_network_log_pane(&mut self, ctx: &mut ViewContext<Self>) {
        let manager = NetworkLogPaneManager::handle(ctx);

        if let Some(locator) = manager.as_ref(ctx).find_pane(ctx.window_id()) {
            // Pane is already open: refresh its snapshot so any items
            // captured since the last open are reflected, then focus it.
            if let Some(tab) = self
                .tabs
                .iter()
                .find(|tab| tab.pane_group.id() == locator.pane_group_id)
            {
                let pane_group = tab.pane_group.clone();
                let network_log_view = pane_group.read(ctx, |pane_group, ctx| {
                    pane_group
                        .downcast_pane_by_id::<NetworkLogPane>(locator.pane_id)
                        .map(|pane| pane.network_log_view(ctx))
                });
                if let Some(network_log_view) = network_log_view {
                    network_log_view.update(ctx, |view, ctx| view.reload_snapshot(ctx));
                }
            }
            self.focus_pane(locator, ctx);
            return;
        }

        let pane = NetworkLogPane::new(ctx);
        self.active_tab_pane_group().update(ctx, |pane_group, ctx| {
            pane_group.add_pane_with_direction(
                Direction::Right,
                pane,
                true, /* focus_new_pane */
                ctx,
            );
        });
    }



    fn dismiss_create_auth_secret_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if self.create_auth_secret_modal.take().is_some() {
            self.focus_active_tab(ctx);
            ctx.notify();
        }
    }



    #[cfg(all(feature = "local_fs", not(target_family = "wasm")))]
    fn show_handoff_success_toast(ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        WorkspaceToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(
                DismissibleToast::default(
                    "Starting cloud environment for this session...".to_owned(),
                ),
                window_id,
                ctx,
            );
        });
    }

    /// Resolves the terminal view that should receive the handoff cloud-mode
    /// pane push and prepares it for the transition:
    ///
    /// 1. Finds the pane group that owns `source_view` (rather than the
    ///    currently-active tab) so focus changes during an async fork RPC
    ///    cannot mis-target the handoff.
    /// 2. If the active session slot holds a swapped-in child agent, reverts
    ///    the swap so the push lands on the orchestrator's PaneStack.
    /// 3. If the resolved view's agent view is fullscreen, exits it so the
    ///    cloud pane is visible at the terminal level.
    #[cfg(all(feature = "local_fs", not(target_family = "wasm")))]
    fn prepare_handoff_target(
        &mut self,
        source_view: &ViewHandle<TerminalView>,
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<TerminalView> {
        let source_view_id = source_view.id();
        let pane_group = self
            .tabs
            .iter()
            .find(|tab| {
                tab.pane_group
                    .as_ref(ctx)
                    .contains_terminal_view(source_view_id, ctx)
            })
            .map(|tab| tab.pane_group.clone())
            .unwrap_or_else(|| self.active_tab_pane_group().clone());
        let target = if let Some((original_pane_id, orchestrator_view)) =
            pane_group.as_ref(ctx).original_session_if_swapped(ctx)
        {
            pane_group.update(ctx, |group, ctx| {
                group.reveal_and_focus_pane(original_pane_id, ctx);
            });
            orchestrator_view
        } else {
            source_view.clone()
        };

        let agent_view_controller = target.as_ref(ctx).agent_view_controller().clone();
        if agent_view_controller
            .as_ref(ctx)
            .agent_view_state()
            .is_fullscreen()
        {
            agent_view_controller.update(ctx, |controller, ctx| {
                controller.exit_agent_view_without_confirmation(ctx);
            });
        }

        target
    }







    fn handle_theme_chooser_event(
        &mut self,
        event: &ThemeChooserEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ThemeChooserEvent::Click => self.focus_theme_chooser(ctx),
            ThemeChooserEvent::Close(mode) => {
                self.save_theme_chooser(mode, ctx);
                self.restore_previous_workspace_state(ctx);
            }
            ThemeChooserEvent::OpenThemeCreatorModal => {
                self.open_theme_creator_modal(ctx);
            }
            ThemeChooserEvent::OpenThemeDeletionModal(theme_kind) => {
                self.open_theme_deletion_modal(theme_kind.clone(), ctx);
            }
        };
    }

    fn handle_resource_center_event(
        &mut self,
        event: &ResourceCenterEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ResourceCenterEvent::Close => {
                self.current_workspace_state.is_resource_center_open = false;
                ctx.notify();
            }
            ResourceCenterEvent::Escape => {
                // Calls terminal view focus to determine where focus should be
                if let Some(pane_group_handle) = self.get_pane_group_view(self.active_tab_index) {
                    pane_group_handle.update(ctx, |pane_group, ctx| {
                        if let Some(terminal_view_handle) = pane_group.active_session_view(ctx) {
                            terminal_view_handle.update(ctx, |terminal, ctx| {
                                terminal.redetermine_global_focus(ctx);
                            });
                        }
                    });
                }
            }
        };
    }


    fn get_active_input_view_handle(&self, app: &AppContext) -> Option<ViewHandle<Input>> {
        app.view(self.active_tab_pane_group())
            .active_session_view(app)
            .map(|terminal_view_handle| app.view(&terminal_view_handle).input().clone())
    }

    fn get_active_session_terminal_model(
        &self,
        app: &AppContext,
    ) -> Option<Arc<FairMutex<TerminalModel>>> {
        self.active_tab_pane_group()
            .as_ref(app)
            .active_session_terminal_model(app)
    }

    /// Replace the active terminal input's buffer with `contents`. Adds to the
    /// undo stack.
    pub fn set_active_terminal_input_contents_and_focus_app(
        &mut self,
        contents: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        let window_id = ctx.window_id();

        if let Some(active_input_view_handle) = self.get_active_input_view_handle(ctx) {
            active_input_view_handle.update(ctx, |input_view, input_ctx| {
                input_view.replace_buffer_content(contents, input_ctx);
            });

            ctx.windows().show_window_and_focus_app(window_id);

            ctx.notify();
        } else {
            log::error!("workspace::view::fill_input(): no active input view handle to fill");
        }
    }

    /// Insert the given command that should open a subshell. And set a flag that we should
    /// automatically bootstrap AKA "warpify" that subshell if we support it. No-op if there is
    /// no active terminal session.
    pub fn insert_subshell_command_and_bootstrap_if_supported(
        &mut self,
        command: &str,
        shell: Option<ShellType>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.active_tab_pane_group()
            .update(ctx, |pane_group_view, pane_group_ctx| {
                pane_group_view
                    .active_session_view(pane_group_ctx)
                    .map(|terminal_view_handle| {
                        terminal_view_handle.update(
                            pane_group_ctx,
                            |terminal_view, terminal_view_ctx| {
                                terminal_view.insert_subshell_command_and_bootstrap_if_supported(
                                    command,
                                    shell,
                                    terminal_view_ctx,
                                );
                            },
                        )
                    })
            });
    }

    /// Update the active session model state.
    fn update_active_session(&mut self, ctx: &mut ViewContext<Self>) {
        let pane_group_handle = self.active_tab_pane_group();
        let file_tree_and_global_search_are_enabled = {
            #[cfg(feature = "local_fs")]
            {
                Self::should_enable_file_tree_and_global_search_for_pane_group(
                    self.active_tab_pane_group().as_ref(ctx),
                )
            }

            #[cfg(not(feature = "local_fs"))]
            {
                false
            }
        };

        // Update working directories for the current pane group
        let pane_group_handle = pane_group_handle.clone();
        self.refresh_working_directories_for_pane_group(&pane_group_handle, ctx);

        if let Some(terminal_handle) = pane_group_handle.as_ref(ctx).active_session_view(ctx) {
            #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
            let (
                session,
                pwd_location,
                path_if_local,
                is_local,
                is_wsl_session,
                session_id,
                has_pending_ssh,
            ) = terminal_handle.read(ctx, |terminal, ctx| {
                let active_session_id = terminal.active_block_session_id();
                let session =
                    active_session_id.and_then(|id| terminal.sessions_model().as_ref(ctx).get(id));
                let pwd_location = terminal.pwd_as_local_or_remote(ctx);
                let path_if_local = terminal.active_session_path_if_local(ctx);
                let is_local = terminal.active_session_is_local(ctx);
                let is_wsl_session = session.as_ref().map(|s| s.is_wsl()).unwrap_or(false);
                let has_pending_ssh = terminal.has_pending_ssh_command();
                (
                    session,
                    pwd_location,
                    path_if_local,
                    is_local,
                    is_wsl_session,
                    active_session_id,
                    has_pending_ssh,
                )
            });

            let window_id = ctx.window_id();
            let working_directory_clone = path_if_local.clone();
            ActiveSession::handle(ctx).update(ctx, |active_session, ctx| {
                active_session.set_session_state(
                    window_id,
                    session,
                    pwd_location,
                    Some(terminal_handle.id()),
                    ctx,
                );
            });

            CodebaseIndexManager::handle(ctx).update(ctx, |manager, _ctx| {
                if let Some(working_directory) = working_directory_clone {
                    manager.handle_active_session_changed(working_directory.as_path());
                }
            });

            let is_remote = matches!(is_local, Some(false));
            let is_unsupported_session = is_wsl_session;

            // Check whether this remote session has an active remote server
            // connection (or is in the process of connecting). This is only
            // true for Auto SSH Warpification (mode 1) sessions where
            // `connect_session` was called at `InitShell` time.
            let has_remote_server = is_remote
                && FeatureFlag::SshRemoteServer.is_enabled()
                && session_id.is_some_and(|sid| {
                    RemoteServerManager::as_ref(ctx).is_session_potentially_active(sid)
                });

            let enablement = CodingPanelEnablementState::from_session_env(
                file_tree_and_global_search_are_enabled,
                is_remote,
                is_unsupported_session,
                has_remote_server,
            );

            // When an SSH command is running (pending host set + block
            // still long-running), the old local session is still active
            // so the enablement computes as `Enabled`. Override to
            // `PendingRemoteSession` so the file tree shows loading
            // instead of the stale local tree.
            let enablement =
                if has_pending_ssh && matches!(enablement, CodingPanelEnablementState::Enabled) {
                    CodingPanelEnablementState::PendingRemoteSession
                } else {
                    enablement
                };

            self.left_panel_view.update(ctx, |left_panel, ctx| {
                left_panel.update_coding_panel_enablement(enablement, ctx);
            });

            let _ = (is_remote, is_wsl_session);
        } else {
            let enablement = CodingPanelEnablementState::from_session_env(
                file_tree_and_global_search_are_enabled,
                false,
                false,
                false,
            );

            self.left_panel_view.update(ctx, |left_panel, ctx| {
                left_panel.update_coding_panel_enablement(enablement, ctx);
            });
        }
    }




    /// Focus and return the active terminal input. If there is no active terminal input (either
    /// because a command is running or because there are no terminal panes), this may create a new
    /// terminal pane according to the [`UnavailableTerminalBehavior`].
    fn focus_terminal_input(
        &mut self,
        fallback_behavior: TerminalSessionFallbackBehavior,
        ctx: &mut ViewContext<Self>,
    ) -> Option<ViewHandle<TerminalView>> {
        let active_pane_group = self.active_tab_pane_group();

        // If there's an active terminal session and it's not busy, return it.
        // If there is no terminal session open, add a terminal pane to the right and return the new terminal view handle.
        let terminal_view_handle = active_pane_group
            .as_ref(ctx)
            .active_session_view(ctx)
            .unwrap_or_else(|| {
                let active_pane_group = self.active_tab_pane_group();
                active_pane_group.update(ctx, |pane_group, ctx| {
                    pane_group.add_terminal_pane(Direction::Right, None /*chosen_shell*/, ctx);
                });
                active_pane_group
                    .as_ref(ctx)
                    .active_session_view(ctx)
                    .unwrap()
            });

        let is_env_var_block = terminal_view_handle.read(ctx, |terminal_view, ctx| {
            terminal_view.has_active_env_var_block(ctx)
        });

        if self.is_input_box_visible(ctx) {
            active_pane_group.update(ctx, |pane_group, ctx| pane_group.focus_active_session(ctx));
            return Some(terminal_view_handle);
        } else if is_env_var_block {
            terminal_view_handle.update(ctx, |terminal_view, ctx| {
                terminal_view.cancel_env_var_block(ctx);
            });
            active_pane_group.update(ctx, |pane_group, ctx| pane_group.focus_active_session(ctx));
            return Some(terminal_view_handle);
        } else if fallback_behavior != TerminalSessionFallbackBehavior::OpenIfNeeded {
            // The active terminal exists but is busy, and the fallback behavior is
            // RequireExisting or OpenIfNone. In those cases, show a toast and no-op.
            self.toast_stack.update(ctx, |toast_stack, ctx| {
                let toast = DismissibleToast::error(
                    "A command in this session is still running.".to_string(),
                );
                toast_stack.add_ephemeral_toast(toast, ctx);
            });
            return None;
        }

        // There's no available session and we were asked not to create one.
        if fallback_behavior == TerminalSessionFallbackBehavior::RequireExisting {
            return None;
        }

        // Either:
        // * There's no active session
        // * The active session is busy but the fallback behavior is OpenIfNeeded
        // In this case, open a new terminal pane to the right.

        if !ContextFlag::CreateNewSession.is_enabled() {
            self.toast_stack.update(ctx, |toast_stack, ctx| {
                let toast =
                    DismissibleToast::error("Cannot open a new terminal session".to_string());
                toast_stack.add_ephemeral_toast(toast, ctx);
            });
            return None;
        }

        active_pane_group.as_ref(ctx).active_session_view(ctx)
    }

    /// Opens the LSP log file in a new terminal pane using `tail -f`.
    fn open_lsp_logs(&mut self, log_path: &PathBuf, ctx: &mut ViewContext<Self>) {

        let active_pane_group = self.active_tab_pane_group();

        // Add a terminal pane to the right
        active_pane_group.update(ctx, |pane_group, ctx| {
            pane_group.add_terminal_pane(PaneGroupDirection::Right, None, ctx);
        });

        let Some(terminal_view_handle) = active_pane_group.as_ref(ctx).active_session_view(ctx)
        else {
            log::error!("Could not get terminal view handle when attempting to open LSP logs.");
            return;
        };

        terminal_view_handle.update(ctx, |terminal, ctx| {
            let shell_family = terminal.shell_family(ctx);
            // Recovered inline from the deleted workflows module.
            let tail_command = match shell_family {
                rift_util::path::ShellFamily::Posix => format!("tail -f {log_path:?}"),
                rift_util::path::ShellFamily::PowerShell => {
                    format!("Get-Content -Wait -Tail 10 -Path \"{}\"", log_path.display())
                }
            };
            terminal.set_pending_command(&tail_command, ctx);
        });
    }

    fn run_tab_config_skill(&mut self, path: &Path, ctx: &mut ViewContext<Self>) {
        if !AISettings::as_ref(ctx).is_any_ai_enabled(ctx) {
            return;
        }

        let Some(terminal_view_handle) =
            self.focus_terminal_input(TerminalSessionFallbackBehavior::OpenIfNeeded, ctx)
        else {
            return;
        };

        let prefix = CLIAgentSessionsModel::as_ref(ctx)
            .session(terminal_view_handle.id())
            .map(|session| session.agent.skill_command_prefix())
            .unwrap_or("/");
        let prompt = format!("{prefix}update-tab-config Update {} to...", path.display());

        terminal_view_handle.update(ctx, |terminal_view, ctx| {
            terminal_view.input().update(ctx, |input, ctx| {
                input.clear_buffer_and_reset_undo_stack(ctx);
                input.set_input_mode_agent(true, ctx);
                input.ensure_agent_mode_for_ai_features(true, None, ctx);
                input.replace_buffer_content(&prompt, ctx);
                input.focus_input_box(ctx);
            });
        });
    }


    /// Inserts given command into active Input Editor, optionally replacing the current buffer. No-ops if
    /// there is no active terminal pane open, with an input box active.
    fn insert_in_input(
        &mut self,
        content: &str,
        replace_buffer: bool,
        should_submit: bool,
        ensure_agent_mode: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let active_input_handle = self.get_active_input_view_handle(ctx);

        if let Some(active_input_handle) = active_input_handle {
            active_input_handle.update(ctx, |input, ctx| {
                if replace_buffer {
                    input.replace_buffer_content(content, ctx);
                } else {
                    input.append_to_buffer(content, ctx);
                }

                if ensure_agent_mode {
                    input.ensure_agent_mode_for_ai_features(true, None, ctx);
                }

                if should_submit {
                    input.input_enter(ctx);
                }
                ctx.notify();
            });
        }
    }



    fn handle_window_settings_changed_event(
        &mut self,
        event: &WindowSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            WindowSettingsChangedEvent::BackgroundOpacity { .. } => {
                ctx.notify();
            }
            WindowSettingsChangedEvent::LeftPanelVisibilityAcrossTabs { .. } => {
                if self.left_panel_visibility_across_tabs_enabled(ctx) {
                    self.left_panel_open = self
                        .active_tab_pane_group()
                        .read(ctx, |pane_group, _| pane_group.left_panel_open);
                }
            }
            WindowSettingsChangedEvent::ZoomLevel { .. } => {
                self.update_titlebar_height(ctx);
            }
            _ => {}
        }
    }


    fn restore_previous_workspace_state(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(previous_state) = self.previous_workspace_state.take() {
            self.current_workspace_state = previous_state;

            // Assumption: at most one of the states will be active.
            // If none are, then we focus the terminal view instead.
            if self.current_workspace_state.is_palette_open {
                self.open_command_palette(ctx);
            } else if self.current_workspace_state.is_theme_chooser_open {
                self.focus_theme_chooser(ctx);
            } else if self.current_workspace_state.is_resource_center_open {
                ctx.focus(&self.resource_center_view);
            } else if self.current_workspace_state.is_ai_assistant_panel_open {
                ctx.focus(&self.ai_assistant_panel);
            } else if self
                .current_workspace_state
                .is_close_session_confirmation_dialog_open
            {
                ctx.focus(&self.close_session_confirmation_dialog);
            } else if self
                .current_workspace_state
                .is_rewind_confirmation_dialog_open
            {
                ctx.focus(&self.rewind_confirmation_dialog);
            } else if self.current_workspace_state.is_native_quit_modal_open {
                ctx.focus(&self.native_modal);
            } else {
                ctx.focus_self();
            }

            self.cancel_tab_rename(ctx);
        }
    }

    fn should_keep_theme(system_theme: SystemTheme, ctx: &mut ViewContext<Self>) -> bool {
        if system_theme == ctx.system_theme() {
            let respect_system_theme = respect_system_theme(ThemeSettings::as_ref(ctx));
            if let RespectSystemTheme::On { .. } = respect_system_theme {
                return true;
            }
        }
        false
    }

    fn save_theme_chooser(&mut self, mode: &ThemeChooserMode, ctx: &mut ViewContext<Self>) {
        let keep_theme = match mode {
            ThemeChooserMode::SystemAgnostic => true,
            ThemeChooserMode::SystemLight => Workspace::should_keep_theme(SystemTheme::Light, ctx),
            ThemeChooserMode::SystemDark => Workspace::should_keep_theme(SystemTheme::Dark, ctx),
        };
        if keep_theme {
            self.keep_theme(ctx);
        } else {
            self.revert_theme(ctx);
        }
    }

    fn revert_theme(&mut self, ctx: &mut ViewContext<Self>) {
        AppearanceManager::handle(ctx).update(ctx, |appearance_manager, ctx| {
            appearance_manager.clear_transient_theme(ctx);
        });
        self.current_workspace_state.is_theme_chooser_open = false;
        self.previous_theme = None;
        ctx.notify();
    }

    fn keep_theme(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_theme_chooser_open = false;
        self.previous_theme = None;
        ctx.notify();
    }

    fn apply_update(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(autoupdate::ReadyForRelaunch::Yes) = autoupdate::apply_update(self, ctx) {
            autoupdate::initiate_relaunch_for_update(ctx);
        }
        self.close_tab_bar_overflow_menu(ctx);
    }

    fn download_new_version(&mut self, ctx: &mut ViewContext<Self>) {
        autoupdate::manually_download_new_version(ctx);
        self.close_tab_bar_overflow_menu(ctx);
    }

    fn active_session_ps1_grid_info(&self, app: &AppContext) -> Option<(BlockGrid, SizeInfo)> {
        self.get_active_session_terminal_model(app)
            .and_then(|model| {
                let lock = model.lock();
                lock.prompt_grid()
                    .cloned()
                    .zip(Some(*lock.block_list().size()))
            })
            .or_else(|| {
                (0..self.tabs.len()).find_map(|i| {
                    self.get_pane_group_view(i)?
                        .as_ref(app)
                        .active_session_terminal_model(app)
                        .and_then(|model| {
                            let lock = model.lock();
                            lock.prompt_grid()
                                .cloned()
                                .zip(Some(*lock.block_list().size()))
                        })
                })
            })
    }

    fn show_close_session_confirmation_dialog(
        &mut self,
        source: OpenDialogSource,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_session_confirmation_dialog
            .update(ctx, |view, _| {
                view.set_open_confirmation_source(source);
            });
        self.current_workspace_state
            .is_close_session_confirmation_dialog_open = true;
        ctx.focus(&self.close_session_confirmation_dialog);
        ctx.notify();
    }

    pub fn show_native_modal(
        &mut self,
        dialog: AlertDialogWithCallbacks<AppModalCallback>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.native_modal.update(ctx, |view, ctx| {
            view.set_alert_dialog(dialog);
            ctx.notify();
        });
        self.current_workspace_state.is_native_quit_modal_open = true;
        ctx.focus(&self.native_modal);
        ctx.notify();
    }

    fn handle_native_modal_event(&mut self, event: &NativeModalEvent, ctx: &mut ViewContext<Self>) {
        match event {
            NativeModalEvent::Close => {
                self.current_workspace_state.is_native_quit_modal_open = false;
                ctx.notify();
            }
        }
    }

    /// Mock pressing a button on the native quit modal. This function has an unusual signature so
    /// that the workspace view is not borrowed while the button press is handled.
    #[cfg(any(test, feature = "integration_tests"))]
    pub fn press_native_modal_button(
        handle: &ViewHandle<Self>,
        button_index: usize,
        app: &mut AppContext,
    ) {
        use super::native_modal::NativeModalAction;
        let modal_handle = handle.as_ref(app).native_modal.clone();
        modal_handle.update(app, |modal, ctx| {
            modal.handle_action(&NativeModalAction::TriggerButtonCallback(button_index), ctx);
        });
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub fn is_native_quit_modal_open(&self, ctx: &AppContext) -> bool {
        self.current_workspace_state.is_native_quit_modal_open
            && self.native_modal.as_ref(ctx).has_alert_dialog()
    }

    fn show_settings(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_settings_with_section(None, ctx);
    }

    fn show_settings_with_section(
        &mut self,
        section: Option<SettingsSection>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_all_overlays(ctx);
        self.open_settings_pane(section, None, ctx);
    }

    fn show_settings_with_search(
        &mut self,
        search_query: &str,
        section: Option<SettingsSection>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_all_overlays(ctx);
        self.open_settings_pane(section, Some(search_query), ctx);
    }

    /// Opens the team settings page and fills the invite field with the given email. This is used when linking directing to
    /// settings with the intent of inviting a user.
    pub fn show_team_settings_page_with_email_invite(
        &mut self,
        email_invite: Option<&String>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.show_settings_with_section(Some(SettingsSection::Teams), ctx);

        self.settings_pane.update(ctx, |view, ctx| {
            view.open_teams_page_email_invite(email_invite, ctx);
        });
    }


    /// Shows the theme chooser so the user can change the active theme.
    pub fn show_theme_chooser_for_active_theme(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_theme_chooser(Some(ThemeChooserMode::for_active_theme(ctx)), ctx)
    }

    pub fn show_theme_chooser_for_custom_theme(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_theme_chooser(None, ctx)
    }

    /// Shows the theme chooser so the user can change a specific theme.
    pub fn show_theme_chooser(
        &mut self,
        theme_chooser_mode: Option<ThemeChooserMode>,
        ctx: &mut ViewContext<Self>,
    ) {
        let current_theme = active_theme_kind(ThemeSettings::as_ref(ctx), ctx);

        self.close_tab_bar_overflow_menu(ctx);

        self.current_workspace_state.close_all_left_panels();

        // When showing the theme chooser, let's close the command palette
        // in case it was used to open the theme chooser.
        self.current_workspace_state.is_palette_open = false;
        self.current_workspace_state.is_ctrl_tab_palette_open = false;
        self.previous_workspace_state = Some(self.current_workspace_state);
        self.current_workspace_state.is_ai_assistant_panel_open = false;
        self.current_workspace_state.is_theme_chooser_open = true;

        self.previous_theme = Some(current_theme);

        self.theme_chooser_view.update(ctx, |view, ctx| {
            view.record_open_theme(ctx);
            if let Some(theme_chooser_mode) = theme_chooser_mode {
                view.select_theme(theme_chooser_mode.into_theme_kind(ctx), ctx);
                view.set_mode(theme_chooser_mode);
            } else {
                view.reload_and_set_latest_theme(ctx);
            }
        });

        self.focus_theme_chooser(ctx);
    }

    pub fn show_keyboard_settings(
        &mut self,
        keybinding_name: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.show_settings_with_section(Some(SettingsSection::Keybindings), ctx);
        if let Some(keybinding_name) = keybinding_name {
            self.settings_pane.update(ctx, |settings_pane, ctx| {
                settings_pane.search_for_keybinding(keybinding_name, ctx);
            });
        }
    }

    pub fn is_theme_chooser_open(&self) -> bool {
        self.current_workspace_state.is_theme_chooser_open
    }

    /// Returns whether the workspace is currently showing a settings file
    /// error banner (i.e. settings_file_error is set and not dismissed).
    #[cfg(feature = "integration_tests")]
    pub fn has_settings_file_error_banner(&self) -> bool {
        self.settings_file_error.is_some() && !self.settings_error_banner_dismissed
    }

    fn increase_font_size(&mut self, ctx: &mut ViewContext<Self>) {
        self.adjust_terminal_font_size(FONT_SIZE_INCREMENT, ctx);
    }

    fn decrease_font_size(&mut self, ctx: &mut ViewContext<Self>) {
        self.adjust_terminal_font_size(-FONT_SIZE_INCREMENT, ctx);
    }

    fn reset_font_size(&mut self, ctx: &mut ViewContext<Self>) {
        self.set_terminal_font_size(MonospaceFontSize::default_value(), ctx);
    }

    fn increase_zoom(&mut self, ctx: &mut ViewContext<Self>) {
        self.adjust_zoom(true /* increase */, ctx);
    }

    fn decrease_zoom(&mut self, ctx: &mut ViewContext<Self>) {
        self.adjust_zoom(false /* increase */, ctx);
    }

    fn reset_zoom(&mut self, ctx: &mut ViewContext<Self>) {
        WindowSettings::handle(ctx).update(ctx, |window_settings, ctx| {
            report_if_error!(window_settings
                .zoom_level
                .set_value(ZoomLevel::default_value(), ctx));
        });
    }

    fn adjust_zoom(&mut self, increase: bool, ctx: &mut ViewContext<Self>) {
        let current_zoom = *WindowSettings::as_ref(ctx).zoom_level.value();
        let Some(current_index) = crate::window_settings::ZoomLevel::VALUES
            .iter()
            .position(|zoom| *zoom == current_zoom)
        else {
            return;
        };

        let next_index = if increase {
            (current_index + 1).min(crate::window_settings::ZoomLevel::VALUES.len() - 1)
        } else {
            current_index.saturating_sub(1)
        };

        WindowSettings::handle(ctx).update(ctx, |window_settings, ctx| {
            report_if_error!(window_settings
                .zoom_level
                .set_value(crate::window_settings::ZoomLevel::VALUES[next_index], ctx));
        });
    }

    fn adjust_terminal_font_size(&mut self, font_size_delta: f32, ctx: &mut ViewContext<Self>) {
        let appearance = Appearance::as_ref(ctx);
        let new_font_size = (appearance.monospace_font_size() + font_size_delta)
            .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.set_terminal_font_size(new_font_size, ctx);
    }

    fn set_terminal_font_size(&mut self, new_font_size: f32, ctx: &mut ViewContext<Self>) {
        FontSettings::handle(ctx).update(ctx, |font_settings, ctx| {
            report_if_error!(font_settings
                .monospace_font_size
                .set_value(new_font_size, ctx));
        });
    }


    fn toggle_mouse_reporting(&mut self, ctx: &mut ViewContext<Self>) {
        let prev_mouse_reporting_enabled =
            AltScreenReporting::handle(ctx).update(ctx, |reporting, ctx| {
                let prev_mouse_reporting_enabled = *reporting.mouse_reporting_enabled.value();
                reporting
                    .mouse_reporting_enabled
                    .set_value(!prev_mouse_reporting_enabled, ctx)
                    .expect("MouseReportingEnabled failed to serialize");
                prev_mouse_reporting_enabled
            });

        let verb = if prev_mouse_reporting_enabled {
            "disabled"
        } else {
            "enabled"
        };
        let mut message = format!("You {verb} mouse reporting.");
        if let Some(keystroke) =
            keybinding_name_to_keystroke("workspace:toggle_mouse_reporting", ctx)
        {
            let _ = write!(message, " Press {} to undo.", keystroke.displayed());
        }

        self.toast_stack.update(ctx, |view, ctx| {
            let new_toast = DismissibleToast::default(message);
            view.add_ephemeral_toast(new_toast, ctx);
        });
    }

    fn toggle_scroll_reporting(&mut self, ctx: &mut ViewContext<Self>) {
        AltScreenReporting::handle(ctx).update(ctx, |reporting, ctx| {
            reporting
                .scroll_reporting_enabled
                .toggle_and_save_value(ctx)
                .expect("ScrollReportingEnabled failed to serialize");
        });
    }

    fn toggle_focus_reporting(&mut self, ctx: &mut ViewContext<Self>) {
        AltScreenReporting::handle(ctx).update(ctx, |reporting, ctx| {
            reporting
                .focus_reporting_enabled
                .toggle_and_save_value(ctx)
                .expect("FocusReportingEnabled failed to serialize");
        });
    }

    /// Handle an event from the referral theme status model, showing the reward modal if necessary
    /// This listens for changes to keybindings and keeps the cached versions up-to-date in our
    /// tooltips.
    fn handle_keybinding_changed(
        &mut self,
        event: &KeybindingChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match &event {
            KeybindingChangedEvent::BindingChanged {
                binding_name,
                new_trigger: new_trigger_option,
            } => self
                .cached_keybindings
                .entry(binding_name.to_owned())
                .and_modify(|keystroke| {
                    *keystroke = new_trigger_option.as_ref().map(|key| key.displayed())
                }),
        };
        ctx.notify()
    }

    fn handle_window_state_change(&mut self, event: &StateEvent, ctx: &mut ViewContext<Self>) {
        match &event {
            StateEvent::ValueChanged { current, previous } => {
                let did_window_change_focus =
                    WindowManager::did_window_change_focus(self.window_id, current, previous);
                let cached_window_is_active = current.active_window == Some(self.window_id);
                let app_became_active = previous.stage != ApplicationStage::Active
                    && current.stage == ApplicationStage::Active;
                let platform_window_is_active =
                    ctx.windows().active_window() == Some(self.window_id);

                // Notify focus listeners when this window is active after either a window focus
                // change or app reactivation while the active window stayed the same.
                // On macOS, app activation can beat the deferred key-window update, so
                // reactivation also verifies the live platform window.
                if cached_window_is_active
                    && (did_window_change_focus || (app_became_active && platform_window_is_active))
                {
                    if let Some(terminal_view) = self
                        .active_tab_pane_group()
                        .as_ref(ctx)
                        .focused_session_view(ctx)
                    {
                        self.notify_terminal_focus_change(Some(terminal_view.id()), ctx);
                    }
                }

                // Re-render if fullscreen state for active window has changed.
                if current.is_active_window_fullscreen != previous.is_active_window_fullscreen {
                    ctx.notify();
                } else if did_window_change_focus {
                    // Re-render if this window's focus state has changed.
                    ctx.notify();
                } else if current.stage != previous.stage {
                    // Re-render if the app's focus state has changed (Active/Inactive)
                    // This ensures dimming updates properly when the app gains/loses focus
                    ctx.notify();
                }
            }
        };
    }


    fn handle_openwarp_launch_modal_event(
        &mut self,
        event: &OpenWarpLaunchModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            OpenWarpLaunchModalEvent::Close => {
                OneTimeModalModel::handle(ctx).update(ctx, |model, ctx| {
                    model.mark_openwarp_launch_modal_dismissed(ctx);
                });
                self.focus_active_tab(ctx);
                ctx.notify();
            }
        }
    }

    fn handle_orchestration_launch_modal_event(
        &mut self,
        event: &OrchestrationLaunchModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            OrchestrationLaunchModalEvent::Close => {
                OneTimeModalModel::handle(ctx).update(ctx, |model, ctx| {
                    model.mark_orchestration_launch_modal_dismissed(ctx);
                });
                self.focus_active_tab(ctx);
                ctx.notify();
            }
        }
    }

    fn handle_oz_launch_modal_event(
        &mut self,
        event: &LaunchModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            LaunchModalEvent::Close => {
                OneTimeModalModel::handle(ctx).update(ctx, |model, ctx| {
                    model.mark_oz_launch_modal_dismissed(ctx);
                });

                // Clear the "Introducing Oz" custom tab name so normal tab naming rules apply.
                if let Some(pane_group_id) = self.oz_launch_modal.tab_pane_group_id.take() {
                    if let Some(tab) = self
                        .tabs
                        .iter()
                        .find(|tab| tab.pane_group.id() == pane_group_id)
                    {
                        tab.pane_group.update(ctx, |view, ctx| {
                            view.clear_title(ctx);
                        });
                    }
                }

                self.focus_active_tab(ctx);
            }
            LaunchModalEvent::ToggleCheckbox => {
                PrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
                    let current_value = settings.is_cloud_conversation_storage_enabled;
                    settings.set_is_cloud_conversation_storage_enabled(!current_value, ctx);
                });
            }
        }
    }

    fn handle_build_plan_migration_modal_event(
        &mut self,
        event: &BuildPlanMigrationModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            BuildPlanMigrationModalEvent::Close => {
                OneTimeModalModel::handle(ctx).update(ctx, |model, ctx| {
                    model.mark_build_plan_migration_modal_dismissed(ctx);
                });
                self.focus_active_tab(ctx);
                ctx.notify();
            }
            BuildPlanMigrationModalEvent::ShowToast { message, flavor } => {
                use crate::view_components::{DismissibleToast, ToastFlavor};
                self.toast_stack.update(ctx, |toast_stack, ctx| {
                    let toast = match flavor {
                        ToastFlavor::Success => DismissibleToast::success(message.clone()),
                        ToastFlavor::Error => DismissibleToast::error(message.clone()),
                        _ => DismissibleToast::error(message.clone()),
                    };
                    toast_stack.add_ephemeral_toast(toast, ctx);
                });
            }
        }
    }


    #[cfg(not(target_family = "wasm"))]
    fn open_plugin_instructions_pane(
        &mut self,
        agent: crate::terminal::CLIAgent,
        kind: PluginModalKind,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::terminal::model::rich_content::RichContentType;
        use crate::terminal::view::plugin_instructions_block::{
            PluginInstructionsBlock, PluginInstructionsBlockEvent,
        };
        use crate::terminal::view::rich_content::{
            RichContentInsertionPosition, RichContentMetadata,
        };

        let Some(manager) = plugin_manager_for(agent) else {
            return;
        };

        let instructions = match kind {
            PluginModalKind::Install => manager.install_instructions(),
            PluginModalKind::Update => manager.update_instructions(),
        };

        // Read session metadata from the originating terminal before creating the instructions pane.
        let active_view = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .active_session_view(ctx);

        let is_remote_session = active_view
            .as_ref()
            .and_then(|view| view.as_ref(ctx).active_session_is_local(ctx))
            .is_some_and(|is_local| !is_local);

        let custom_command_prefix = active_view.and_then(|view| {
            CLIAgentSessionsModel::as_ref(ctx)
                .session(view.id())
                .and_then(|s| s.custom_command_prefix.clone())
        });

        self.active_tab_pane_group().update(ctx, |pane_group, ctx| {
            let pane_id = pane_group.add_terminal_pane_ignoring_default_session_mode(
                pane_group::Direction::Right,
                None,
                ctx,
            );

            if let Some(terminal_view) = pane_group.terminal_view_from_pane_id(pane_id, ctx) {
                terminal_view.update(ctx, |view, ctx| {
                    let custom_command_prefix = custom_command_prefix.clone();
                    let block = ctx.add_typed_action_view(|ctx| {
                        PluginInstructionsBlock::new(
                            instructions,
                            agent,
                            custom_command_prefix,
                            is_remote_session,
                            ctx,
                        )
                    });
                    ctx.subscribe_to_view(&block, |view, block, event, ctx| match event {
                        PluginInstructionsBlockEvent::Close => {
                            view.remove_plugin_instructions_block(block.clone(), ctx);
                        }
                    });
                    view.insert_rich_content(
                        Some(RichContentType::PluginInstructionsBlock),
                        block,
                        Some(RichContentMetadata::PluginInstructionsBlock),
                        RichContentInsertionPosition::Append {
                            insert_below_long_running_block: false,
                        },
                        ctx,
                    );
                });
            }
        });
    }

    /// Opens the Codex modal.
    pub fn open_codex_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_codex_modal_open = true;
        ctx.focus(&self.codex_modal);
        ctx.notify();
        send_telemetry_from_ctx!(TelemetryEvent::CodexModalOpened, ctx);
    }


    fn handle_cloud_agent_capacity_modal_event(
        &mut self,
        event: &CloudAgentCapacityModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            CloudAgentCapacityModalEvent::Close => {
                self.current_workspace_state
                    .is_cloud_agent_capacity_modal_open = false;
                self.focus_active_tab(ctx);
                ctx.notify();
            }
        }
    }

    pub fn open_cloud_agent_capacity_modal(
        &mut self,
        variant: CloudAgentCapacityModalVariant,
        ctx: &mut ViewContext<Self>,
    ) {
        if !FeatureFlag::CloudMode.is_enabled() {
            return;
        }
        self.cloud_agent_capacity_modal.update(ctx, |modal, ctx| {
            modal.set_variant(variant);
            ctx.notify();
        });
        self.current_workspace_state
            .is_cloud_agent_capacity_modal_open = true;
        ctx.focus(&self.cloud_agent_capacity_modal);
        ctx.notify();
        send_telemetry_from_ctx!(TelemetryEvent::CloudAgentCapacityModalOpened, ctx);
    }

    fn handle_free_tier_limit_modal_event(
        &mut self,
        event: &FreeTierLimitHitModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            FreeTierLimitHitModalEvent::MaybeOpen => {
                if self.free_tier_limit_check_triggered
                    && self.check_and_open_free_tier_limit_modal(ctx)
                {
                    self.free_tier_limit_check_triggered = false;
                }
            }
            FreeTierLimitHitModalEvent::Close => {
                self.current_workspace_state
                    .is_free_tier_limit_hit_modal_open = false;
                GeneralSettings::handle(ctx).update(ctx, |settings, ctx| {
                    if let Err(e) = settings
                        .free_tier_limit_hit_modal_dismissed
                        .set_value(true, ctx)
                    {
                        log::warn!("Failed to mark free tier limit hit modal as dismissed: {e}");
                    }
                });
                self.focus_active_tab(ctx);
                ctx.notify();
            }
        }
    }

    pub fn check_and_open_free_tier_limit_modal(&mut self, _ctx: &mut ViewContext<Self>) -> bool {
        // The free-tier AI request-limit interstitial was removed along with AI.
        false
    }


    pub(crate) fn focus_active_tab(&mut self, ctx: &mut ViewContext<Self>) {
        self.active_tab_pane_group().update(ctx, |tab, ctx| {
            tab.focus(ctx);
        })
    }

    fn focus_theme_chooser(&self, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.theme_chooser_view);
        ctx.notify();
    }

    fn open_prompt_editor(
        &mut self,
        open_source: PromptEditorOpenSource,
        ctx: &mut ViewContext<Self>,
    ) {
        // Try to get a prompt preview from an active session. Otherwise, read it from the settings
        // view.
        let ps1_grid_info = self.active_session_ps1_grid_info(ctx).or_else(|| {
            self.settings_pane
                .read(ctx, |settings, app| settings.get_ps1_info(app))
        });
        let chip_runtime_capabilities = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .active_session_view(ctx)
            .and_then(|terminal_view| {
                terminal_view.read(ctx, |terminal, ctx| {
                    let required_executables = crate::context_chips::available_chips()
                        .into_iter()
                        .filter_map(|kind| kind.to_chip())
                        .flat_map(|chip| chip.runtime_policy().required_executables().to_vec())
                        .collect::<std::collections::HashSet<_>>();
                    terminal
                        .active_block_session_id()
                        .and_then(|id| terminal.sessions_model().as_ref(ctx).get(id))
                        .as_deref()
                        .map(|session| {
                            ChipRuntimeCapabilities::from_session_with_external_command_queries(
                                session,
                                required_executables.iter().map(String::as_str),
                                false,
                            )
                        })
                })
            })
            .unwrap_or_default();
        self.prompt_editor_modal.update(ctx, |prompt_editor, ctx| {
            prompt_editor.open(ps1_grid_info, chip_runtime_capabilities, ctx);
        });
        self.close_all_overlays(ctx);
        self.current_workspace_state.is_prompt_editor_open = true;
        ctx.focus(&self.prompt_editor_modal);

        send_telemetry_from_ctx!(
            TelemetryEvent::OpenPromptEditor {
                entrypoint: open_source
            },
            ctx
        );
    }


    fn open_theme_creator_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_theme_creator_modal_open = true;
        ctx.focus(&self.theme_creator_modal);
        ctx.notify();
    }

    fn open_theme_deletion_modal(&mut self, theme_kind: ThemeKind, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_theme_deletion_modal_open = true;
        self.theme_deletion_modal
            .update(ctx, |theme_deletion_modal, ctx| {
                theme_deletion_modal.set_theme_kind(theme_kind, ctx);
            });
        ctx.focus(&self.theme_deletion_modal);
        ctx.notify();
    }






    fn render_tab_in_tab_bar(
        &self,
        tab_index: usize,
        tab_bar_state: TabBarState,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let tab = &self.tabs[tab_index];
        let close_button_position = if FeatureFlag::TabCloseButtonOnLeft.is_enabled() {
            TabSettings::as_ref(ctx).close_button_position
        } else {
            TabCloseButtonPosition::default()
        };

        let is_drag_target = self
            .hovered_tab_index
            .as_ref()
            .is_some_and(|hovered_index| match hovered_index {
                TabBarHoverIndex::OverTab(idx) => *idx == tab_index,
                TabBarHoverIndex::BeforeTab(_) => false,
            });

        TabComponent::new(
            tab_index,
            tab_bar_state,
            tab,
            self.tab_rename_editor.clone(),
            close_button_position,
            is_drag_target,
            ctx,
        )
        .build()
        .finish()
    }

    /// Renders the tab at `tab_index` using the same render code path the live
    /// tab bar uses, so the floating chip during a cross-window tab drag
    /// matches the source tab exactly. Dispatches to the vertical tab group
    /// renderer when the source layout was vertical, and to `TabComponent`
    /// (via `render_tab_in_tab_bar`) when it was horizontal. Constructed with
    /// neutral `TabBarState` so the snapshot doesn't carry over local-drag or
    /// rename state.
    pub(crate) fn render_tab_for_drag_ghost(
        &self,
        tab_index: usize,
        was_vertical: bool,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        if tab_index >= self.tabs.len() {
            return Empty::new().finish();
        }
        if was_vertical {
            vertical_tabs::render_tab_group_for_drag_ghost(self, tab_index, ctx)
        } else {
            let tab = &self.tabs[tab_index];
            let close_button_position = if FeatureFlag::TabCloseButtonOnLeft.is_enabled() {
                TabSettings::as_ref(ctx).close_button_position
            } else {
                TabCloseButtonPosition::default()
            };
            let tab_bar_state = TabBarState {
                tab_count: self.tabs.len(),
                active_tab_index: Some(tab_index),
                is_any_tab_renaming: false,
                is_any_tab_dragging: false,
                hover_fixed_width: None,
            };
            // `.for_drag_ghost()` makes the resulting element skip the
            // outer `SavePosition`, `Draggable`, and `DropTarget` wrappers
            // so the chip overlay doesn't pollute the target window's
            // position cache (see `TabComponent::for_drag_ghost`).
            TabComponent::new(
                tab_index,
                tab_bar_state,
                tab,
                self.tab_rename_editor.clone(),
                close_button_position,
                false,
                ctx,
            )
            .for_drag_ghost()
            .build()
            .finish()
        }
    }

    fn render_agent_management_view_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let is_active = self.current_workspace_state.is_agent_management_view_open;

        SavePosition::new(
            Container::new(
                Align::new(
                    self.render_tab_bar_icon_button(
                        appearance,
                        icons::Icon::Grid,
                        &self.mouse_states.agent_management_view_button,
                        WorkspaceAction::ToggleAgentManagementView,
                        "Agent management panel".to_string(),
                        keybinding_name_to_display_string(
                            "workspace:toggle_agent_management_view",
                            ctx,
                        ),
                        is_active,
                        false,
                    )
                    .finish(),
                )
                .finish(),
            )
            .finish(),
            "workspace:toggle_agent_management_view",
        )
        .finish()
    }

    fn render_left_toggle_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let vertical_tabs_active =
            FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(ctx).use_vertical_tabs;

        let (is_active, tooltip_text, action, keybinding_name, save_position_id) =
            if vertical_tabs_active {
                (
                    self.vertical_tabs_panel_open,
                    "Tabs panel",
                    WorkspaceAction::ToggleVerticalTabsPanel,
                    "workspace:toggle_vertical_tabs_panel",
                    "workspace:toggle_vertical_tabs_panel",
                )
            } else {
                let tooltip = if self.left_panel_views.len() <= 1 {
                    match self
                        .left_panel_views
                        .first()
                        .copied()
                        .unwrap_or(ToolPanelView::WarpDrive)
                    {
                        ToolPanelView::ProjectExplorer => "Project explorer",
                        ToolPanelView::GlobalSearch { .. } => "Global search",
                        ToolPanelView::WarpDrive => "Warp Drive",
                        ToolPanelView::ConversationListView => "Agent conversations",
                    }
                } else {
                    "Tools panel"
                };
                (
                    self.active_tab_pane_group().as_ref(ctx).left_panel_open,
                    tooltip,
                    WorkspaceAction::ToggleLeftPanel,
                    "workspace:toggle_left_panel",
                    "workspace:toggle_left_panel",
                )
            };

        SavePosition::new(
            Container::new(
                Align::new(
                    self.render_tab_bar_icon_button(
                        appearance,
                        icons::Icon::Menu,
                        &self.mouse_states.left_panel_icon,
                        action,
                        tooltip_text.to_string(),
                        keybinding_name_to_display_string(keybinding_name, ctx),
                        is_active,
                        false,
                    )
                    .finish(),
                )
                .finish(),
            )
            .finish(),
            save_position_id,
        )
        .finish()
    }

    fn render_tools_panel_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let is_active = self.active_tab_pane_group().as_ref(ctx).left_panel_open;

        let tooltip_text = if self.left_panel_views.len() <= 1 {
            match self
                .left_panel_views
                .first()
                .copied()
                .unwrap_or(ToolPanelView::WarpDrive)
            {
                ToolPanelView::ProjectExplorer => "Project explorer",
                ToolPanelView::GlobalSearch { .. } => "Global search",
                ToolPanelView::WarpDrive => "Warp Drive",
                ToolPanelView::ConversationListView => "Agent conversations",
            }
        } else {
            "Tools panel"
        };

        SavePosition::new(
            Container::new(
                Align::new(
                    self.render_tab_bar_icon_button(
                        appearance,
                        icons::Icon::Tool2,
                        &self.mouse_states.tools_panel_icon,
                        WorkspaceAction::ToggleLeftPanel,
                        tooltip_text.to_string(),
                        keybinding_name_to_display_string("workspace:toggle_left_panel", ctx),
                        is_active,
                        false,
                    )
                    .finish(),
                )
                .finish(),
            )
            .finish(),
            "workspace:toggle_left_panel",
        )
        .finish()
    }

    fn should_enable_file_tree_and_global_search_for_pane_group(pane_group: &PaneGroup) -> bool {
        pane_group
            .pane_ids()
            .filter(|id| !pane_group.is_pane_hidden_for_close(*id))
            .any(|id| {
                id.is_terminal_pane()
                    || id.is_file_pane()
                    || id.is_code_pane()
                    || id.is_code_diff_pane()
            })
    }

    fn render_right_panel_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let is_active = self.active_tab_pane_group().as_ref(ctx).right_panel_open;
        let is_enabled = Self::should_enable_file_tree_and_global_search_for_pane_group(
            self.active_tab_pane_group().as_ref(ctx),
        );
        let disable = !is_enabled;

        let theme = appearance.theme();
        let font_color = if disable {
            theme.disabled_text_color(theme.background())
        } else if is_active {
            theme.main_text_color(theme.background())
        } else {
            theme.sub_text_color(theme.background())
        };

        // Build the button content: Diff icon + optional diff stats
        let icon = ConstrainedBox::new(icons::Icon::Diff.to_warpui_icon(font_color).finish())
            .with_width(16.)
            .with_height(16.)
            .finish();

        let show_diff_stats = *TabSettings::as_ref(ctx).show_code_review_diff_stats;

        let line_changes = if show_diff_stats {
            self.active_tab_pane_group()
                .as_ref(ctx)
                .active_session_view(ctx)
                .and_then(|tv| tv.as_ref(ctx).current_diff_line_changes(ctx))
                .filter(|lc| {
                    // Only show the stat badge when there are actual line-level changes
                    // (files_changed alone, e.g. mode-only changes, is not surfaced here).
                    lc.lines_added > 0 || lc.lines_removed > 0
                })
        } else {
            None
        };

        let has_stats = line_changes.is_some();

        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        row.add_child(icon);

        if let Some(lc) = line_changes {
            let stat = |value: u32, prefix: &str, color: ColorU| -> Box<dyn Element> {
                Container::new(
                    Text::new_inline(format!("{prefix}{value}"), appearance.ui_font_family(), 12.)
                        .with_color(color)
                        .with_style(Properties::default().weight(Weight::Semibold))
                        .finish(),
                )
                .with_margin_left(4.)
                .finish()
            };
            let add_color = AnsiColorIdentifier::Green
                .to_ansi_color(&appearance.theme().terminal_colors().normal)
                .into();
            let remove_color = AnsiColorIdentifier::Red
                .to_ansi_color(&appearance.theme().terminal_colors().normal)
                .into();
            row.add_child(stat(lc.lines_added, "+", add_color));
            row.add_child(stat(lc.lines_removed, "-", remove_color));
        }

        let label = row.finish();

        // The diff icon SVG has intrinsic horizontal whitespace in its 14px viewBox: its visible
        // paths start around x=3 and end around x=11. When stats are shown, equal container padding
        // makes the gap between the button edge and the visible icon look wider than the gap after
        // the text. Locally compensate for that artwork padding without changing the shared icon.
        let (header_padding_left, header_padding_right) =
            if has_stats { (5., 8.) } else { (4., 4.) };
        let default_styles = UiComponentStyles {
            font_color: Some(font_color.into()),
            font_size: Some(12.),
            font_weight: Some(Weight::Medium),
            font_family_id: Some(appearance.ui_font_family()),
            height: Some(24.),
            border_radius: Some(CornerRadius::with_all(Radius::Pixels(4.))),
            border_width: Some(0.),
            padding: Some(Coords {
                top: 0.,
                bottom: 0.,
                left: header_padding_left,
                right: header_padding_right,
            }),
            ..Default::default()
        };

        let hover_styles = UiComponentStyles {
            background: Some(theme.surface_2().into()),
            ..default_styles
        };

        let clicked_styles = UiComponentStyles {
            background: Some(theme.background().into()),
            ..default_styles
        };

        let mut button = Button::new(
            self.mouse_states.right_panel_icon.clone(),
            default_styles,
            Some(hover_styles),
            Some(clicked_styles),
            None,
        )
        .with_custom_label(label);

        if is_active {
            button = button.active().with_active_styles(UiComponentStyles {
                background: Some(internal_colors::fg_overlay_3(theme).into()),
                ..UiComponentStyles::default()
            });
        }

        let hoverable = if disable {
            button.build().disable()
        } else {
            button
                .with_tooltip(self.render_tab_bar_icon_button_tooltip(
                    appearance,
                    "Code review panel".to_string(),
                    keybinding_name_to_display_string("workspace:toggle_right_panel", ctx),
                ))
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(WorkspaceAction::ToggleRightPanel);
                })
        };

        SavePosition::new(
            Container::new(Align::new(hoverable.finish()).finish()).finish(),
            "workspace:right_panel_button",
        )
        .finish()
    }

    /// Renders an invisible rect for detecting hovers over the tab bar.
    fn render_tab_bar_hover_area(&self) -> Box<dyn Element> {
        self.render_tab_bar_hoverable(
            ConstrainedBox::new(Empty::new().finish())
                .with_height(TAB_BAR_HOVER_HEIGHT)
                .finish(),
        )
    }

    /// Renders the provided content wrapped in the tab bar hover behavior.
    fn render_tab_bar_hoverable(&self, content: Box<dyn Element>) -> Box<dyn Element> {
        Hoverable::new(self.tab_bar_hover_state.clone(), |_| content)
            .with_hover_out_delay(Duration::from_millis(500))
            .on_hover(|_is_hovered, ctx, _app, _position| {
                ctx.dispatch_typed_action(WorkspaceAction::SyncTrafficLights);
            })
            .finish()
    }

    fn render_tab_hover_indicator(&self, appearance: &Appearance) -> Box<dyn Element> {
        ConstrainedBox::new(
            Rect::new()
                .with_background(appearance.theme().accent())
                .finish(),
        )
        .with_height(32.)
        .with_width(4.)
        .finish()
    }

    /// Renders the insertion slot for a cross-window ghost drag in the
    /// horizontal tab bar. Shows an empty space with `fg_overlay_1`
    /// background — identical to same-window drag's origin slot.
    fn render_ghost_tab_slot(&self, appearance: &Appearance, ctx: &AppContext) -> Box<dyn Element> {
        let theme = appearance.theme();
        let width = self.tab_fixed_width.or_else(|| {
            self.tabs.first().and_then(|_| {
                ctx.element_position_by_id_at_last_frame(self.window_id, tab_position_id(0))
                    .map(|rect| rect.width())
            })
        });
        let slot = Container::new(Empty::new().finish())
            .with_background(internal_colors::fg_overlay_1(theme))
            .finish();
        let inner = if let Some(w) = width {
            ConstrainedBox::new(slot).with_width(w).finish()
        } else {
            ConstrainedBox::new(slot)
                .with_min_width(80.)
                .with_max_width(200.)
                .finish()
        };
        Shrinkable::new(1.0, inner).finish()
    }

    fn render_title_bar_search_bar(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let text_color = theme.sub_text_color(theme.background());

        Hoverable::new(
            self.mouse_states.title_bar_search_bar.clone(),
            |mouse_state| {
                let row = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_spacing(10.)
                    .with_child(
                        ConstrainedBox::new(
                            icons::Icon::Search.to_warpui_icon(text_color).finish(),
                        )
                        .with_width(16.)
                        .with_height(16.)
                        .finish(),
                    )
                    .with_child(
                        Shrinkable::new(
                            1.,
                            Text::new_inline(
                                "Search sessions, agents, files...",
                                appearance.ui_font_family(),
                                14.,
                            )
                            .with_color(text_color.into())
                            .with_clip(ClipConfig::ellipsis())
                            .finish(),
                        )
                        .finish(),
                    )
                    .finish();

                ConstrainedBox::new(
                    Container::new(row)
                        .with_background(if mouse_state.is_hovered() {
                            internal_colors::fg_overlay_2(theme)
                        } else {
                            internal_colors::fg_overlay_1(theme)
                        })
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                        .with_padding_left(16.)
                        .with_padding_right(16.)
                        .with_padding_top(4.)
                        .with_padding_bottom(4.)
                        .finish(),
                )
                .with_width(TITLE_BAR_SEARCH_BAR_MAX_WIDTH)
                .finish()
            },
        )
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::OpenPalette {
                mode: PaletteMode::Command,
                source: PaletteSource::TitleBarSearchBar,
                query: None,
            });
        })
        .finish()
    }

    fn render_tab_bar_contents(
        &self,
        hover_fixed_width: Option<f32>,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let mut tab_bar = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        let is_web_anonymous_user = self
            .auth_state
            .is_user_web_anonymous_user()
            .unwrap_or_default();

        // Simplified mode for viewing Warp Drive objects, shared sessions, or conversation transcripts on WASM
        #[cfg(target_family = "wasm")]
        if let Some(content_type) = self.get_simplified_wasm_tab_bar_content(ctx) {
            // Use MainAxisAlignment::SpaceBetween and expand to fill width
            tab_bar = tab_bar
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_main_axis_size(MainAxisSize::Max);
            let bg_color = blended_colors::neutral_1(appearance.theme());

            // Left: Warp logo - clickable to link to warp.dev
            let warp_logo = Hoverable::new(self.mouse_states.warp_logo.clone(), |_state| {
                ConstrainedBox::new(
                    rift_core::ui::Icon::Warp
                        .to_warpui_icon(appearance.theme().foreground())
                        .finish(),
                )
                .with_height(24.)
                .with_width(24.)
                .finish()
            })
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::OpenLink("https://warp.dev".to_owned()));
            })
            .with_cursor(Cursor::PointingHand)
            .finish();
            tab_bar.add_child(warp_logo);

            // Right: Info button + "View all cloud runs" button (for ambient agent sessions) + "Open in Warp" button
            let mut right_row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);

            // Extract task_id from conversation transcripts and shared sessions
            let task_id = match content_type {
                SimplifiedWasmTabBarContent::ConversationTranscript { task_id }
                | SimplifiedWasmTabBarContent::SharedSession { task_id } => task_id,
                SimplifiedWasmTabBarContent::WarpDriveObject => None,
            };

            // Show info button for conversation transcripts and shared sessions (if there's content to display)
            let should_show_info_button =
                !matches!(content_type, SimplifiedWasmTabBarContent::WarpDriveObject)
                    && self
                        .active_tab_pane_group()
                        .as_ref(ctx)
                        .focused_session_view(ctx)
                        .is_some_and(|view| {
                            Self::should_show_conversation_details_panel(&view, ctx)
                        });

            if should_show_info_button {
                right_row.add_child(
                    Container::new(ChildView::new(&self.transcript_info_button).finish())
                        .with_margin_right(8.)
                        .finish(),
                );

                // Add "View all cloud runs" button when task_id exists (with 4px gap)
                if task_id.is_some() {
                    right_row.add_child(
                        Container::new(ChildView::new(&self.view_cloud_runs_button).finish())
                            .with_margin_right(4.)
                            .finish(),
                    );
                }
            }

            // Hide "Open in Warp" button on mobile devices
            if !riftui::platform::wasm::is_mobile_device() {
                right_row.add_child(ChildView::new(&self.open_in_warp_button).finish());
            }
            tab_bar.add_child(right_row.finish());

            return Container::new(tab_bar.finish())
                .with_background_color(bg_color)
                .with_border(
                    Border::bottom(1.0)
                        .with_border_fill(blended_colors::neutral_2(appearance.theme())),
                )
                .with_padding_left(24.)
                .with_padding_right(24.)
                .with_padding_top(4.)
                .with_padding_bottom(4.)
                .finish();
        }

        // Check if vertical tabs mode is active
        let vertical_tabs_active =
            FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(ctx).use_vertical_tabs;

        // Render config-driven left-side toolbar buttons (both horizontal and vertical tabs)
        let knowledge_center_closed = true;
        let config = TabSettings::as_ref(ctx)
            .header_toolbar_chip_selection
            .clone();
        if knowledge_center_closed && !self.is_theme_chooser_open() {
            let left_toolbar_buttons = config
                .left_items()
                .into_iter()
                .filter_map(|item| self.render_header_toolbar_button(&item, appearance, ctx))
                .collect::<Vec<_>>();
            let left_toolbar_button_count = left_toolbar_buttons.len();
            for (index, button) in left_toolbar_buttons.into_iter().enumerate() {
                let is_last_left_toolbar_button = index + 1 == left_toolbar_button_count;
                if !vertical_tabs_active && is_last_left_toolbar_button {
                    tab_bar.add_child(Container::new(button).with_margin_right(8.).finish());
                } else {
                    tab_bar.add_child(button);
                }
            }
        }

        if vertical_tabs_active {
            let mut right_controls = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);

            self.add_configurable_right_side_tab_bar_controls(
                &mut right_controls,
                &config,
                is_web_anonymous_user,
                appearance,
                ctx,
            );

            let left_padding = self.compute_tab_bar_left_padding(ctx);

            let tab_bar = Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(tab_bar.finish())
                .with_child(
                    Shrinkable::new(
                        1.,
                        Clipped::new(
                            Container::new(
                                Align::new(self.render_title_bar_search_bar(appearance)).finish(),
                            )
                            .with_padding_left(TITLE_BAR_SEARCH_BAR_SLOT_PADDING)
                            .with_padding_right(TITLE_BAR_SEARCH_BAR_SLOT_PADDING)
                            .finish(),
                        )
                        .finish(),
                    )
                    .finish(),
                )
                .with_child(right_controls.finish())
                .finish();

            return EventHandler::new(
                Container::new(tab_bar)
                    .with_padding_left(left_padding)
                    .with_padding_right(TAB_BAR_PADDING_RIGHT)
                    .finish(),
            )
            .on_right_mouse_down(|ctx, _, position| {
                ctx.dispatch_typed_action(WorkspaceAction::ShowHeaderToolbarContextMenu {
                    position,
                });
                DispatchEventResult::StopPropagation
            })
            .finish();
        } else {
            // Copy from our saved tab_bar_state to ensure all tabs get rendered with the same state
            let active_tab_index = if FeatureFlag::AgentManagementView.is_enabled()
                && self.current_workspace_state.is_agent_management_view_open
            {
                None
            } else {
                Some(self.active_tab_index)
            };

            let drag_model = CrossWindowTabDrag::as_ref(ctx);
            let tab_bar_state = TabBarState {
                tab_count: self.tabs.len(),
                active_tab_index,
                is_any_tab_renaming: self.current_workspace_state.is_tab_being_renamed(),
                is_any_tab_dragging: self.current_workspace_state.is_tab_being_dragged
                    || drag_model.is_active(),
                hover_fixed_width,
            };
            // Collapse the detached-placeholder slot to 0 width while it
            // exists in this (source) window. After a put-back handoff the
            // placeholder has been removed and the real tab re-inserted at a
            // different index, so `source_placeholder_tab_index()` returns
            // `None` and nothing is hidden — otherwise the stale
            // `source_tab_index` would collapse an unrelated tab (e.g. the
            // first tab shifting into that slot after a leftward put-back).
            let transferred_tab_index = if drag_model.is_active()
                && drag_model.source_window_id() == Some(self.window_id)
            {
                let has_dedicated_preview = drag_model.has_dedicated_preview_window();
                let has_handoff = drag_model.handed_off_target().is_some();
                if has_dedicated_preview || has_handoff {
                    drag_model.source_placeholder_tab_index()
                } else {
                    None
                }
            } else {
                None
            };
            // Ghost state for cross-window drag hovering over this tab bar.
            let ghost = drag_model.ghost_state_for_window(self.window_id);

            for i in 0..self.tabs.len() {
                // Insert ghost slot before tab `i` if the drag would land here.
                if ghost.as_ref().is_some_and(|g| g.insertion_index == i) {
                    tab_bar.add_child(self.render_ghost_tab_slot(appearance, ctx));
                }
                let is_transferred = transferred_tab_index == Some(i);
                if !is_transferred
                    && self
                        .hovered_tab_index
                        .as_ref()
                        .is_some_and(|hovered_index| match hovered_index {
                            TabBarHoverIndex::BeforeTab(idx) => i == *idx,
                            TabBarHoverIndex::OverTab(_) => false,
                        })
                {
                    tab_bar.add_child(self.render_tab_hover_indicator(appearance));
                }
                if is_transferred {
                    tab_bar.add_child(
                        ConstrainedBox::new(self.render_tab_in_tab_bar(i, tab_bar_state, ctx))
                            .with_width(0.)
                            .finish(),
                    );
                } else {
                    tab_bar.add_child(self.render_tab_in_tab_bar(i, tab_bar_state, ctx));
                }
            }

            // Fencepost: ghost slot or hover indicator after all tabs.
            if ghost
                .as_ref()
                .is_some_and(|g| g.insertion_index == self.tabs.len())
            {
                tab_bar.add_child(self.render_ghost_tab_slot(appearance, ctx));
            } else if self
                .hovered_tab_index
                .as_ref()
                .is_some_and(|hovered_index| match hovered_index {
                    TabBarHoverIndex::BeforeTab(idx) => self.tabs.len() == *idx,
                    TabBarHoverIndex::OverTab(_) => false,
                })
            {
                tab_bar.add_child(self.render_tab_hover_indicator(appearance));
            }

            if ContextFlag::CreateNewSession.is_enabled() {
                tab_bar.add_child(self.render_new_session_button(ctx));
            }
        }

        // Placeholder to make sure the flex row expands across the entire width of the app.
        tab_bar.add_child(Shrinkable::new(0.5, Empty::new().finish()).finish());

        self.add_configurable_right_side_tab_bar_controls(
            &mut tab_bar,
            &config,
            is_web_anonymous_user,
            appearance,
            ctx,
        );

        let left_padding = self.compute_tab_bar_left_padding(ctx);

        EventHandler::new(
            Container::new(tab_bar.finish())
                .with_padding_left(left_padding)
                .with_padding_right(TAB_BAR_PADDING_RIGHT)
                .finish(),
        )
        .on_right_mouse_down(|ctx, _, position| {
            ctx.dispatch_typed_action(WorkspaceAction::ShowHeaderToolbarContextMenu { position });
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    /// Renders a single header toolbar button for the given item kind.
    /// Returns `None` if the item is not currently available.
    /// The button is wrapped with a right-click handler that opens the
    /// toolbar configurator.
    fn render_header_toolbar_button(
        &self,
        item: &HeaderToolbarItemKind,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Option<Box<dyn Element>> {
        if !item.is_available(ctx) {
            return None;
        }
        let vertical_tabs_active =
            FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(ctx).use_vertical_tabs;
        let inner = match item {
            HeaderToolbarItemKind::TabsPanel => self.render_left_toggle_button(appearance, ctx),
            HeaderToolbarItemKind::ToolsPanel => {
                if self.left_panel_views.is_empty() {
                    return None;
                }
                if vertical_tabs_active {
                    self.render_tools_panel_button(appearance, ctx)
                } else {
                    self.render_left_toggle_button(appearance, ctx)
                }
            }
            HeaderToolbarItemKind::AgentManagement => {
                self.render_agent_management_view_button(appearance, ctx)
            }
            HeaderToolbarItemKind::CodeReview => self.render_right_panel_button(appearance, ctx),
            HeaderToolbarItemKind::NotificationsMailbox => {
                self.render_notifications_mailbox_button(appearance, ctx)
            }
        };
        Some(
            Container::new(
                EventHandler::new(inner)
                    .on_right_mouse_down(|ctx, _, position| {
                        ctx.dispatch_typed_action(WorkspaceAction::ShowHeaderToolbarContextMenu {
                            position,
                        });
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            )
            .with_margin_left(TAB_BAR_ICON_PADDING)
            .finish(),
        )
    }

    /// Renders the notifications mailbox button (extracted for reuse from
    /// add_right_side_tab_bar_controls).
    fn render_notifications_mailbox_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let is_inbox_active = self.current_workspace_state.is_notification_mailbox_open
            || self.hoa_onboarding_flow.as_ref().is_some_and(|flow| {
                flow.as_ref(ctx).step() == HoaOnboardingStep::AgentInboxCallout
            });
        let mailbox_button = self
            .render_tab_bar_icon_button(
                appearance,
                icons::Icon::Inbox,
                &self.mouse_states.notifications_mailbox,
                WorkspaceAction::ToggleNotificationMailbox {
                    select_first: false,
                },
                "Notifications".to_string(),
                keybinding_name_to_display_string(TOGGLE_NOTIFICATION_MAILBOX_BINDING_NAME, ctx),
                is_inbox_active,
                false,
            )
            .finish();

        // Agent notifications were removed; there is never an unread count.
        let unread_count = 0usize;
        let mailbox_element = if unread_count > 0 {
            let indicator = Container::new(
                ConstrainedBox::new(
                    Rect::new()
                        .with_background(appearance.theme().accent())
                        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                        .finish(),
                )
                .with_width(6.)
                .with_height(6.)
                .finish(),
            )
            .finish();
            let mut stack = Stack::new();
            stack.add_child(mailbox_button);
            stack.add_positioned_child(
                indicator,
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
            stack.finish()
        } else {
            mailbox_button
        };

        SavePosition::new(
            Container::new(Align::new(mailbox_element).finish()).finish(),
            NOTIFICATIONS_MAILBOX_POSITION_ID,
        )
        .finish()
    }

    /// Adds the configurable right-side toolbar items plus the fixed controls
    /// (update pill, offline indicator, avatar, etc.) that are not configurable.
    fn add_configurable_right_side_tab_bar_controls(
        &self,
        target: &mut Flex,
        config: &crate::workspace::tab_settings::HeaderToolbarChipSelection,
        is_web_anonymous_user: bool,
        appearance: &Appearance,
        ctx: &AppContext,
    ) {
        if let Some(update_pill) = self.render_tab_overflow_menu(ctx, appearance) {
            target.add_child(
                Container::new(update_pill)
                    .with_margin_left(TAB_BAR_PADDING_LEFT)
                    .finish(),
            );
        }

        let is_online = NetworkStatus::as_ref(ctx).is_online();

        if !is_online {
            target.add_child(
                Container::new(self.render_offline_button(appearance))
                    .with_margin_right(4.)
                    .finish(),
            );
        }

        for item in config.right_items() {
            if let Some(button) = self.render_header_toolbar_button(&item, appearance, ctx) {
                target.add_child(button);
            }
        }

        // Legacy AI assistant button (non-agent-mode only)
        if is_online
            && !FeatureFlag::AgentMode.is_enabled()
            && !is_web_anonymous_user
            && !self.current_workspace_state.is_ai_assistant_panel_open
        {
            target.add_child(
                Container::new(
                    SavePosition::new(
                        self.render_legacy_warp_ai_entrypoint_button(appearance),
                        AI_ASSISTANT_BUTTON_ID,
                    )
                    .finish(),
                )
                .with_margin_left(TAB_BAR_PADDING_LEFT)
                .finish(),
            );
        }

        if FeatureFlag::AvatarInTabBar.is_enabled() {
            target.add_child(
                Container::new(self.render_avatar_button(appearance, ctx))
                    .with_margin_left(TAB_BAR_PADDING_LEFT)
                    .finish(),
            );
        } else {
            let resource_center_closed = !self.current_workspace_state.is_resource_center_open;
            if resource_center_closed && ContextFlag::WarpEssentials.is_enabled() {
                target.add_child(
                    Container::new(self.render_resource_center_button(appearance, ctx))
                        .with_margin_left(TAB_BAR_PADDING_LEFT)
                        .finish(),
                );
            }

            target.add_child(
                Container::new(self.render_settings_button(appearance))
                    .with_margin_left(TAB_BAR_PADDING_LEFT)
                    .finish(),
            );
        }

        if self.auth_state.is_anonymous_or_logged_out()
            && !FeatureFlag::OpenWarpNewSettingsModes.is_enabled()
        {
            if is_web_anonymous_user {
                target.add_child(
                    Container::new(self.render_web_anonymous_user_sign_in_button(appearance))
                        .with_margin_left(8.)
                        .finish(),
                );
            } else {
                target.add_child(
                    Container::new(self.render_anonymous_sign_up_user_button(appearance))
                        .with_margin_left(8.)
                        .finish(),
                );
            }
        }

        let zoom_factor = WindowSettings::as_ref(ctx).zoom_level.as_zoom_factor();
        let traffic_light_data = traffic_light_data(ctx, self.window_id);
        if let Some(traffic_light_data) = traffic_light_data.as_ref() {
            if should_reserve_traffic_light_space_in_tab_bar(traffic_light_data.side) {
                target.add_child(
                    ConstrainedBox::new(Empty::new().finish())
                        .with_width(traffic_light_data.width(zoom_factor))
                        .finish(),
                );
            }
        }
    }

    fn compute_tab_bar_left_padding(&self, ctx: &AppContext) -> f32 {
        let zoom_factor = WindowSettings::as_ref(ctx).zoom_level.as_zoom_factor();
        let traffic_light_data = traffic_light_data(ctx, self.window_id);
        let is_window_fullscreen = ctx
            .windows()
            .platform_window(self.window_id)
            .map(|window| window.fullscreen_state() == FullscreenState::Fullscreen)
            .unwrap_or(false);
        if self.current_workspace_state.is_left_panel_open() {
            0.
        } else if is_window_fullscreen && cfg!(target_os = "macos") {
            // Full-screen mode on MacOS does not need as much padding (traffic lights are hidden).
            TAB_BAR_PADDING_LEFT
        } else {
            traffic_light_data
                .as_ref()
                .filter(|data| data.side == TrafficLightSide::Left)
                .map(|data| data.width(zoom_factor))
                .unwrap_or(0.)
                + 16.
        }
    }

    /// Renders the tab bar contents, wrapped in hover and drag-drop behaviors.
    fn render_tab_bar(
        &self,
        tab_fixed_width: Option<f32>,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let bar_contents = ConstrainedBox::new(
            // We can wrap the whole tab bar in the a drop target with the `AfterTabIndex` drop target data since the API for accepting a drop target with nested
            // drop target elements will default to the inner ones (in this case the tabs or the button before the tabs)
            DropTarget::new(
                self.render_tab_bar_contents(tab_fixed_width, appearance, ctx),
                TabBarDropTargetData {
                    tab_bar_location: TabBarLocation::AfterTabIndex(self.tabs.len()),
                },
            )
            .finish(),
        )
        .with_height(TAB_BAR_HEIGHT)
        .finish();

        let tab_bar_border =
            Border::bottom(TAB_BAR_BORDER_HEIGHT).with_border_fill(appearance.theme().outline());

        let mut tab_bar_container = Container::new(
            EventHandler::new(Clipped::new(self.render_tab_bar_hoverable(bar_contents)).finish())
                .on_back_mouse_down(move |ctx, _app, _position| {
                    ctx.dispatch_typed_action(WorkspaceAction::ActivatePrevTab);
                    DispatchEventResult::StopPropagation
                })
                .on_forward_mouse_down(move |ctx, _app, _position| {
                    ctx.dispatch_typed_action(WorkspaceAction::ActivateNextTab);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_border(tab_bar_border);
        if FeatureFlag::NewTabStyling.is_enabled() {
            tab_bar_container = tab_bar_container
                .with_background(internal_colors::fg_overlay_1(appearance.theme()));
        }
        let tab_bar_element = tab_bar_container.finish();

        let dimming_color = appearance.theme().background().into();
        SavePosition::new(
            WindowFocusDimming::apply_panel_header_dimming(
                tab_bar_element,
                self.mouse_states.header_dimming.clone(),
                TAB_BAR_HEIGHT,
                dimming_color,
                self.window_id,
                ctx,
            ),
            TAB_BAR_POSITION_ID,
        )
        .finish()
    }

    // Render traffic lights, if appropriate for the current platform.
    fn maybe_render_traffic_lights(&self, stack: &mut Stack, app: &AppContext) {
        let Some(traffic_light_data) = traffic_light_data(app, self.window_id) else {
            return;
        };

        let appearance = Appearance::as_ref(app);
        let fullscreen_state = app
            .windows()
            .platform_window(self.window_id)
            .map(|window| window.fullscreen_state())
            .unwrap_or_default();
        stack.add_positioned_child(
            traffic_light_data.render(
                fullscreen_state,
                &self.traffic_light_mouse_states,
                appearance.theme(),
                app,
            ),
            OffsetPositioning::offset_from_parent(
                Vector2F::zero(),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::TopRight,
                ChildAnchor::TopRight,
            ),
        );
    }

    fn render_new_session_button(&self, ctx: &AppContext) -> Box<dyn Element> {
        const CORNER_RADIUS: Radius = Radius::Pixels(4.);
        const BUTTON_HEIGHT: f32 = 24.;
        const SIDE_MENU_WIDTH: f32 = 16.;
        const BUTTON_WIDTH: f32 = 24. + SIDE_MENU_WIDTH;
        const BUTTON_LEFT_MARGIN: f32 = 4.;

        let new_tab_tool_tip_label_text = "New Tab".to_string();
        let new_tab_tool_tip_sublabel_text =
            keybinding_name_to_display_string(NEW_TAB_BINDING_NAME, ctx);
        let tab_configs_tool_tip_label_text = "Tab configs".to_string();
        let tab_configs_tool_tip_sublabel_text =
            keybinding_name_to_display_string(TOGGLE_TAB_CONFIGS_MENU_BINDING_NAME, ctx);
        let appearance = Appearance::as_ref(ctx);

        if !FeatureFlag::ShellSelector.is_enabled() {
            // Legacy new tab button, which shows the menu on right click.
            let new_tab_button = self
                .render_tab_bar_icon_button(
                    appearance,
                    icons::Icon::Plus,
                    &self.mouse_states.new_tab_button.clone(),
                    WorkspaceAction::AddDefaultTab,
                    new_tab_tool_tip_label_text,
                    new_tab_tool_tip_sublabel_text,
                    false,
                    false,
                )
                .on_right_click(move |ctx, _, position| {
                    ctx.dispatch_typed_action(WorkspaceAction::ToggleNewSessionMenu {
                        anchor: NewSessionMenuAnchor::AddTabButton(position),
                    });
                })
                .finish();
            return Container::new(
                SavePosition::new(
                    Align::new(new_tab_button).finish(),
                    NEW_TAB_BUTTON_POSITION_ID,
                )
                .finish(),
            )
            .with_margin_left(BUTTON_LEFT_MARGIN)
            .finish();
        }

        let theme = appearance.theme();

        Hoverable::new(self.mouse_states.new_tab.clone(), |state| {
            let window_id = self.window_id;
            let is_active = self.show_new_session_dropdown_menu.is_some();

            let new_tab_button = combo_inner_button(
                appearance,
                icons::Icon::Plus,
                false,
                self.mouse_states.new_tab_button.clone(),
            )
            .with_style(
                UiComponentStyles::default()
                    .set_border_radius(CornerRadius::with_left(CORNER_RADIUS)),
            )
            .with_tooltip(self.render_tab_bar_icon_button_tooltip(
                appearance,
                new_tab_tool_tip_label_text.clone(),
                new_tab_tool_tip_sublabel_text.clone(),
            ))
            .build()
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::AddDefaultTab);
            })
            .finish();

            let new_session_menu_button = combo_inner_button(
                appearance,
                icons::Icon::ChevronDown,
                is_active,
                self.mouse_states.new_tab_menu.clone(),
            )
            .with_style(
                UiComponentStyles::default()
                    .set_border_radius(CornerRadius::with_right(CORNER_RADIUS))
                    .set_width(SIDE_MENU_WIDTH),
            )
            .with_active_styles(
                UiComponentStyles::default()
                    .set_background(internal_colors::fg_overlay_3(theme).into()),
            )
            .with_tooltip(self.render_tab_bar_icon_button_tooltip(
                appearance,
                tab_configs_tool_tip_label_text.clone(),
                tab_configs_tool_tip_sublabel_text.clone(),
            ))
            .build()
            .on_click(move |ctx, app, _| {
                // We are positioning the menu to the lower-left corner of the new tab button.
                // This gives the impression that both individual buttons are one big button.
                if let Some(position) =
                    app.element_position_by_id_at_last_frame(window_id, NEW_TAB_BUTTON_POSITION_ID)
                {
                    ctx.dispatch_typed_action(WorkspaceAction::ToggleNewSessionMenu {
                        anchor: NewSessionMenuAnchor::AddTabButton(position.lower_left()),
                    });
                }
            })
            .finish();

            let row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    SavePosition::new(
                        Align::new(new_tab_button).finish(),
                        NEW_TAB_BUTTON_POSITION_ID,
                    )
                    .finish(),
                )
                .with_child(
                    SavePosition::new(
                        Align::new(new_session_menu_button).finish(),
                        NEW_SESSION_MENU_BUTTON_POSITION_ID,
                    )
                    .finish(),
                )
                .finish();

            let mut ret = Container::new(
                ConstrainedBox::new(row)
                    .with_height(BUTTON_HEIGHT)
                    .with_width(BUTTON_WIDTH)
                    .finish(),
            )
            .with_corner_radius(CornerRadius::with_all(CORNER_RADIUS))
            .with_margin_left(BUTTON_LEFT_MARGIN);

            if state.is_hovered() {
                ret = ret.with_background(internal_colors::neutral_1(theme));
            }
            ret.finish()
        })
        .finish()
    }

    fn render_avatar_button(&self, appearance: &Appearance, ctx: &AppContext) -> Box<dyn Element> {
        let is_anonymous = self.auth_state.is_anonymous_or_logged_out();
        let display_name = self
            .auth_state
            .username_for_display()
            .unwrap_or(DEFAULT_USER_DISPLAY_NAME.to_owned());

        let avatar_content = if self.auth_state.is_anonymous_or_logged_out() {
            AvatarContent::Icon(icons::Icon::Gear)
        } else {
            self.auth_state
                .user_photo_url()
                .map(|url| AvatarContent::Image {
                    url,
                    display_name: display_name.clone(),
                })
                .unwrap_or(AvatarContent::DisplayName(display_name.clone()))
        };

        let mut avatar = Avatar::new(
            avatar_content,
            UiComponentStyles {
                width: Some(20.),
                height: Some(20.),
                border_radius: Some(CornerRadius::with_all(Radius::Percentage(50.))),
                font_family_id: Some(appearance.ui_font_family()),
                font_weight: Some(Weight::Bold),
                background: Some(appearance.theme().accent().into()),
                font_size: Some(12.),
                font_color: Some(ColorU::black()),
                ..Default::default()
            },
        );

        // Render the subtle autoupdate UI if autoupdate is ready and there is no incoming prominent update version.
        let autoupdate_stage = autoupdate::get_update_state(ctx);
        if FeatureFlag::AutoupdateUIRevamp.is_enabled()
            && autoupdate_stage.ready_for_update()
            && autoupdate_stage
                .available_new_version()
                .map(|version| {
                    !is_incoming_version_past_current(version.last_prominent_update.as_deref())
                })
                .unwrap_or(false)
        {
            avatar = avatar.with_status_element(
                StatusElementTypes::Circle,
                RedNotificationDot::default_styles(appearance),
            );
        }

        let button = Hoverable::new(self.mouse_states.avatar_icon.clone(), |state| {
            let mut stack = Stack::new();
            let mut container = Container::new(avatar.build().finish())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .with_uniform_padding(2.);

            if state.is_mouse_over_element() {
                if !state.is_clicked() {
                    container = container.with_background(appearance.theme().surface_2());
                }
                // On hover, show tooltip of user's display name (if it exists)
                if !self.is_user_menu_open && !is_anonymous {
                    stack.add_positioned_overlay_child(
                        appearance
                            .ui_builder()
                            .tool_tip(display_name.clone())
                            .with_style(UiComponentStyles {
                                background: Some(appearance.theme().tooltip_background().into()),
                                font_color: Some(appearance.theme().background().into_solid()),
                                ..Default::default()
                            })
                            .build()
                            .finish(),
                        OffsetPositioning::offset_from_parent(
                            vec2f(0., 4.),
                            ParentOffsetBounds::WindowByPosition,
                            ParentAnchor::BottomMiddle,
                            ChildAnchor::TopMiddle,
                        ),
                    );
                }
            }
            stack.add_child(container.finish());
            stack.finish()
        })
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::ToggleUserMenu);
        })
        .with_cursor(Cursor::PointingHand)
        .finish();

        SavePosition::new(Align::new(button).finish(), USER_AVATAR_BUTTON_POSITION_ID).finish()
    }

    fn render_resource_center_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        // only show the unread indicator if the tips are NOT completed
        let should_show_unread_indicator = !self.tips_completed.as_ref(ctx).skipped_or_completed;
        let mut button = self
            .render_tab_bar_icon_button(
                appearance,
                icons::Icon::Lightbulb,
                &self.mouse_states.resource_center_icon,
                WorkspaceAction::ToggleResourceCenter,
                "Warp Essentials".to_string(),
                self.cached_keybindings[TOGGLE_RESOURCE_CENTER_KEYBINDING_NAME].clone(),
                false,
                false,
            )
            .finish();

        if should_show_unread_indicator {
            const INDICATOR_DIAMETER: f32 = 6.;
            let indicator = Container::new(
                ConstrainedBox::new(
                    WarpUiIcon::new(ELLIPSE_SVG_PATH, appearance.theme().accent()).finish(),
                )
                .with_height(INDICATOR_DIAMETER)
                .with_width(INDICATOR_DIAMETER)
                .finish(),
            )
            .finish();
            let mut stack = Stack::new();
            stack.add_child(button);
            stack.add_positioned_child(
                indicator,
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
            button = stack.finish();
        }

        Align::new(button).finish()
    }

    fn render_settings_button(&self, appearance: &Appearance) -> Box<dyn Element> {
        Align::new(
            self.render_tab_bar_icon_button(
                appearance,
                icons::Icon::Gear,
                &self.mouse_states.settings_icon,
                WorkspaceAction::ShowSettings,
                "Settings".to_string(),
                self.cached_keybindings[SHOW_SETTINGS_KEYBINDING_NAME].clone(),
                false,
                false,
            )
            .finish(),
        )
        .finish()
    }

    fn render_web_anonymous_user_sign_in_button(
        &self,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let default_styles = UiComponentStyles {
            font_color: Some(appearance.theme().active_ui_text_color().into()),
            font_size: Some(12.),
            font_weight: Some(Weight::Light),
            font_family_id: Some(appearance.ui_font_family()),
            border_color: None,
            border_radius: Some(CornerRadius::with_all(Radius::Pixels(5.))),
            border_width: Some(1.),
            width: Some(80.),
            height: Some(24.),
            ..Default::default()
        };
        let hovered_styles = UiComponentStyles {
            font_color: Some(appearance.theme().accent().into()),
            border_color: Some(appearance.theme().accent().into()),
            ..default_styles
        };
        let button = appearance
            .ui_builder()
            .button_with_custom_styles(
                ButtonVariant::Text,
                self.mouse_states.sign_in_button.clone(),
                default_styles,
                Some(hovered_styles),
                Some(hovered_styles),
                None,
            )
            .with_centered_text_label(String::from("Sign up"));

        Align::new(
            button
                .build()
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(WorkspaceAction::SignInAnonymousWebUser)
                })
                .finish(),
        )
        .finish()
    }

    fn render_anonymous_sign_up_user_button(&self, appearance: &Appearance) -> Box<dyn Element> {
        let default_styles = UiComponentStyles {
            font_color: Some(appearance.theme().active_ui_text_color().into()),
            font_size: Some(12.),
            font_weight: Some(Weight::Semibold),
            font_family_id: Some(appearance.ui_font_family()),
            border_color: Some(appearance.theme().active_ui_text_color().into()),
            border_radius: Some(CornerRadius::with_all(Radius::Pixels(5.))),
            border_width: Some(1.),
            width: Some(80.),
            height: Some(24.),
            ..Default::default()
        };
        let hovered_styles = UiComponentStyles {
            font_color: Some(appearance.theme().accent().into()),
            border_color: Some(appearance.theme().accent().into()),
            ..default_styles
        };

        let button = appearance
            .ui_builder()
            .button_with_custom_styles(
                ButtonVariant::Text,
                self.mouse_states.sign_up_button.clone(),
                default_styles,
                Some(hovered_styles),
                Some(hovered_styles),
                None,
            )
            .with_centered_text_label(String::from("Sign up"));

        Align::new(
            button
                .build()
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(WorkspaceAction::SignupAnonymousUser)
                })
                .finish(),
        )
        .finish()
    }

    fn render_offline_button(&self, appearance: &Appearance) -> Box<dyn Element> {
        let ui_builder = appearance.ui_builder().clone();

        let tool_tip_label_text = "Some features may be unavailable offline".to_string();
        let icon = ConstrainedBox::new(
            Container::new(
                icons::Icon::CloudOffline
                    .to_warpui_icon(appearance.theme().foreground())
                    .finish(),
            )
            .with_uniform_padding(3.)
            .finish(),
        )
        .with_width(icons::ICON_DIMENSIONS)
        .with_height(icons::ICON_DIMENSIONS)
        .finish();

        let hoverable = Hoverable::new(self.mouse_states.offline_icon.clone(), |state| {
            let mut stack = Stack::new().with_child(icon);
            if state.is_hovered() {
                let tool_tip = ui_builder.tool_tip(tool_tip_label_text);
                stack.add_positioned_overlay_child(
                    tool_tip.build().finish(),
                    OffsetPositioning::offset_from_parent(
                        vec2f(0., 4.),
                        ParentOffsetBounds::WindowByPosition,
                        ParentAnchor::BottomMiddle,
                        ChildAnchor::TopMiddle,
                    ),
                );
            }
            stack.finish()
        });

        Align::new(hoverable.finish()).finish()
    }


    fn render_tab_bar_icon_button_tooltip(
        &self,
        appearance: &Appearance,
        tool_tip_label_text: String,
        tool_tip_sublabel_text: Option<String>,
    ) -> Box<dyn FnOnce() -> Box<dyn Element>> {
        let ui_builder = appearance.ui_builder().clone();

        Box::new(move || {
            if let Some(tool_tip_sublabel_text) = tool_tip_sublabel_text {
                ui_builder
                    .tool_tip_with_sublabel(tool_tip_label_text, tool_tip_sublabel_text)
                    .build()
                    .finish()
            } else {
                ui_builder.tool_tip(tool_tip_label_text).build().finish()
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tab_bar_icon_button(
        &self,
        appearance: &Appearance,
        icon_type: icons::Icon,
        mouse_state_handle: &MouseStateHandle,
        action: WorkspaceAction,
        tool_tip_label_text: String,
        tool_tip_sublabel_text: Option<String>,
        is_active: bool,
        disable: bool,
    ) -> Hoverable {
        let theme = appearance.theme();
        let icon_color = if is_active {
            theme.main_text_color(theme.background())
        } else {
            theme.sub_text_color(theme.background())
        };
        let mut button = icon_button_with_color(
            appearance,
            icon_type,
            is_active,
            mouse_state_handle.clone(),
            icon_color,
        );
        button = button
            .with_hovered_styles(UiComponentStyles {
                font_color: Some(icon_color.into()),
                background: Some(theme.surface_2().into()),
                ..UiComponentStyles::default()
            })
            .with_clicked_styles(UiComponentStyles {
                font_color: Some(icon_color.into()),
                background: Some(theme.background().into()),
                ..UiComponentStyles::default()
            });

        if is_active {
            button = button.with_active_styles(UiComponentStyles {
                background: Some(internal_colors::fg_overlay_3(theme).into()),
                ..UiComponentStyles::default()
            });
        }

        if disable {
            button = button.with_style(UiComponentStyles {
                font_color: Some(theme.disabled_text_color(theme.background()).into()),
                ..UiComponentStyles::default()
            });
            button.build().disable()
        } else {
            button
                .with_tooltip(self.render_tab_bar_icon_button_tooltip(
                    appearance,
                    tool_tip_label_text,
                    tool_tip_sublabel_text,
                ))
                .build()
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        }
    }

    fn render_tab_overflow_menu(
        &self,
        app: &AppContext,
        appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        if !ContextFlag::PromptForVersionUpdates.is_enabled() {
            return None;
        }

        let autoupdate_stage = autoupdate::get_update_state(app);
        // Render the prominent autoupdate pill if autoupdate is ready and the current version is behind a prominent update version.
        if autoupdate_stage.ready_for_update()
            && (!FeatureFlag::AutoupdateUIRevamp.is_enabled()
                || autoupdate_stage
                    .available_new_version()
                    .map(|version| {
                        is_incoming_version_past_current(version.last_prominent_update.as_deref())
                    })
                    .unwrap_or(false))
        {
            let pill = ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_child(
                            Text::new_inline(
                                UPDATE_READY_TEXT,
                                appearance.ui_font_family(),
                                PILL_FONT_SIZE,
                            )
                            .with_color(Fill::warn().into())
                            .finish(),
                        )
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .finish(),
                )
                .with_border(Border::all(1.).with_border_color(Fill::warn().into()))
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                .with_uniform_margin(4.)
                .with_uniform_padding(4.)
                .with_margin_right(5.)
                .finish(),
            )
            .with_width(TAB_BAR_PILL_WIDTH)
            .finish();

            let button = if self.show_tab_bar_overflow_menu {
                pill
            } else {
                // Only attach the event handler in the case where the menu isn't already showing
                // Otherwise we have a race condition in the case that someone clicks on the button
                // where the menu tries to dismiss itself onclick and the menu gets reshown on mouseup
                Hoverable::new(self.mouse_states.overflow_button.clone(), |_state| pill)
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(WorkspaceAction::ToggleTabBarOverflowMenu);
                    })
                    .with_reset_cursor_after_click()
                    .with_cursor(Cursor::PointingHand)
                    .finish()
            };

            Some(Align::new(SavePosition::new(button, "tab_bar_overflow_button").finish()).finish())
        } else {
            None
        }
    }

    fn render_banner_and_active_tab(
        &self,
        app: &AppContext,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let active_tab_data = &self.tabs[self.active_tab_index];

        let active_content = if FeatureFlag::AgentManagementView.is_enabled()
            && self.current_workspace_state.is_agent_management_view_open
        {
            ChildView::new(&self.agent_management_view).finish()
        } else {
            ChildView::new(&active_tab_data.pane_group).finish()
        };

        let terminal_content = match self.maybe_render_workspace_banner(app, appearance) {
            Some(banner_element) => Flex::column()
                .with_child(banner_element)
                .with_child(Shrinkable::new(1., active_content).finish())
                .finish(),
            None => active_content,
        };

        let vertical_tabs_active =
            FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(app).use_vertical_tabs;
        let pane_group = self.active_tab_pane_group().as_ref(app);
        let is_right_open = pane_group.right_panel_open;
        let is_right_maximized = is_right_open && pane_group.is_right_panel_maximized;

        let mut main_content = Flex::row();

        // In horizontal tabs mode, config-driven panels render inside this row
        // alongside the terminal area.
        // In vertical tabs mode, panels are rendered in render_panels instead.
        if !vertical_tabs_active {
            let config = TabSettings::as_ref(app)
                .header_toolbar_chip_selection
                .clone();
            let mut prev_panel_added = false;
            for item in config.left_items() {
                Self::add_panel_with_separator(
                    &mut main_content,
                    &mut prev_panel_added,
                    self.render_config_panel(&item, pane_group, &config, app),
                    app,
                );
            }

            if !is_right_maximized {
                if prev_panel_added {
                    main_content.add_child(Self::render_panel_separator(app));
                }
                main_content =
                    main_content.with_child(Shrinkable::new(1.0, terminal_content).finish());
                prev_panel_added = true;
            }

            for item in config.right_items() {
                Self::add_panel_with_separator(
                    &mut main_content,
                    &mut prev_panel_added,
                    self.render_config_panel(&item, pane_group, &config, app),
                    app,
                );
            }

            if is_right_maximized {
                Self::add_panel_with_separator(
                    &mut main_content,
                    &mut prev_panel_added,
                    self.render_config_panel_maximized(pane_group, &config, app),
                    app,
                );
            } else if !config.contains_item(&HeaderToolbarItemKind::CodeReview) {
                Self::add_panel_with_separator(
                    &mut main_content,
                    &mut prev_panel_added,
                    self.render_config_panel(
                        &HeaderToolbarItemKind::CodeReview,
                        pane_group,
                        &config,
                        app,
                    ),
                    app,
                );
            }
        } else if !is_right_maximized {
            main_content = main_content.with_child(Shrinkable::new(1.0, terminal_content).finish());
        }

        let clickable_element = EventHandler::new(main_content.finish())
            .on_back_mouse_down(|ctx, _app, _position| {
                ctx.dispatch_typed_action(WorkspaceAction::ActivatePrevTab);
                DispatchEventResult::StopPropagation
            })
            .on_forward_mouse_down(|ctx, _app, _position| {
                ctx.dispatch_typed_action(WorkspaceAction::ActivateNextTab);
                DispatchEventResult::StopPropagation
            })
            .finish();

        Shrinkable::new(
            THEME_CHOOSER_RATIO,
            SavePosition::new(clickable_element, TAB_CONTENT_POSITION_ID).finish(),
        )
        .finish()
    }

    fn render_theme_chooser(&self) -> Box<dyn Element> {
        let theme_chooser = ChildView::new(&self.theme_chooser_view).finish();
        ConstrainedBox::new(theme_chooser)
            .with_max_width(240.0)
            .finish()
    }

    #[cfg(not(target_family = "wasm"))]
    fn render_resource_center(&self) -> Box<dyn Element> {
        ConstrainedBox::new(ChildView::new(&self.resource_center_view).finish())
            .with_width(RESOURCE_CENTER_WIDTH)
            .finish()
    }

    // Allow let and return because of the conditional linux compilation (otherwise we get a clippy
    // warning on mac)
    #[allow(clippy::let_and_return)]
    fn banner_fields(&self, app: &AppContext) -> Option<WorkspaceBannerFields> {
        // The settings error banner sits just below reauth in priority — it's
        // more important that users are notified their settings file is broken
        // than that they continue to see any of the autoupdate or crash recovery
        // banners.
        let banner_fields = self
            .render_reauth_banner_element()
            .or_else(|| self.render_settings_error_banner(app))
            .or_else(|| self.render_autoupdate_banner_element(app));

        #[cfg(enable_crash_recovery)]
        let banner_fields = banner_fields.or_else(|| crash_recovery::banner_metadata(app));

        banner_fields
    }

    fn render_settings_error_banner(&self, app: &AppContext) -> Option<WorkspaceBannerFields> {
        if self.settings_error_banner_dismissed {
            return None;
        }
        let error = self.settings_file_error.as_ref()?;
        let (heading, description) = error.heading_and_description();
        let secondary_button =
            AISettings::as_ref(app)
                .is_any_ai_enabled(app)
                .then(|| WorkspaceBannerButtonDetails {
                    text: "Fix with Oz".to_owned(),
                    action: WorkspaceAction::FixSettingsWithOz {
                        error_description: error.to_string(),
                    },
                    variant: BannerButtonVariant::Naked,
                    icon: Some(Icon::Oz),
                    more_info_button_action: None,
                });
        Some(WorkspaceBannerFields {
            banner_type: WorkspaceBanner::InvalidSettings,
            severity: BannerSeverity::Warning,
            heading: Some(heading),
            description,
            secondary_button,
            button: Some(WorkspaceBannerButtonDetails {
                text: "Open file".to_owned(),
                action: WorkspaceAction::OpenSettingsFile,
                variant: BannerButtonVariant::Outlined,
                icon: None,
                more_info_button_action: None,
            }),
        })
    }

    fn maybe_render_workspace_banner(
        &self,
        app: &AppContext,
        appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        self.banner_fields(app)
            .map(|fields| self.render_workspace_banner(fields, appearance))
    }

    fn render_reauth_banner_element(&self) -> Option<WorkspaceBannerFields> {
        if self.reauth_banner_dismissed || !self.auth_state.needs_reauth() {
            return None;
        }

        Some(WorkspaceBannerFields {
            banner_type: WorkspaceBanner::Reauth,
            severity: BannerSeverity::Warning,
            heading: Some("Your login has expired.".into()),
            description: "Please sign in again to restore access to cloud-based features.".into(),
            secondary_button: None,
            button: Some(WorkspaceBannerButtonDetails {
                text: "Sign in".into(),
                action: WorkspaceAction::Reauth,
                variant: BannerButtonVariant::Outlined,
                icon: None,
                more_info_button_action: None,
            }),
        })
    }

    fn render_autoupdate_banner_element(&self, app: &AppContext) -> Option<WorkspaceBannerFields> {
        if FeatureFlag::Autoupdate.is_enabled() {
            match autoupdate::get_update_state(app) {
                AutoupdateStage::UnableToUpdateToNewVersion { new_version }
                    if !self.autoupdate_unable_to_update_banner_dismissed =>
                {
                    let description =
                        if is_incoming_version_past_current(new_version.soft_cutoff.as_deref()) {
                            VERSION_DEPRECATION_WITHOUT_PERMISSIONS_BANNER_TEXT.to_owned()
                        } else {
                            "A new version is available but Warp is unable to perform the update."
                                .to_owned()
                        };

                    Some(WorkspaceBannerFields {
                        banner_type: WorkspaceBanner::UnableToUpdateToNewVersion,
                        severity: BannerSeverity::Error,
                        heading: None,
                        description,
                        secondary_button: None,
                        button: Some(WorkspaceBannerButtonDetails {
                            text: "Update Warp manually".to_string(),
                            action: WorkspaceAction::DownloadNewVersion,
                            variant: BannerButtonVariant::Outlined,
                            icon: None,
                            more_info_button_action: Some(WorkspaceAction::AutoupdateFailureLink),
                        }),
                    })
                }
                AutoupdateStage::UnableToLaunchNewVersion { new_version }
                    if !self.autoupdate_unable_to_launch_new_version =>
                {
                    let description =
                        if is_incoming_version_past_current(new_version.soft_cutoff.as_deref()) {
                            VERSION_DEPRECATION_WITHOUT_PERMISSIONS_BANNER_TEXT.to_owned()
                        } else {
                            "Warp was unable to launch the new installed version.".to_owned()
                        };

                    Some(WorkspaceBannerFields {
                        banner_type: WorkspaceBanner::UnableToLaunchNewVersion,
                        severity: BannerSeverity::Error,
                        heading: None,
                        description,
                        secondary_button: None,
                        button: Some(WorkspaceBannerButtonDetails {
                            text: "Update Warp manually".to_string(),
                            action: WorkspaceAction::DownloadNewVersion,
                            variant: BannerButtonVariant::Outlined,
                            icon: None,
                            more_info_button_action: Some(WorkspaceAction::AutoupdateFailureLink),
                        }),
                    })
                }
                AutoupdateStage::UpdateReady { new_version, .. }
                | AutoupdateStage::UpdatedPendingRestart { new_version } => {
                    if is_incoming_version_past_current(new_version.soft_cutoff.as_deref()) {
                        Some(WorkspaceBannerFields {
                            banner_type: WorkspaceBanner::VersionDeprecated,
                            severity: BannerSeverity::Error,
                            heading: None,
                            description: VERSION_DEPRECATION_BANNER_TEXT.to_string(),
                            secondary_button: None,
                            button: Some(WorkspaceBannerButtonDetails {
                                text: "Update now".to_string(),
                                action: WorkspaceAction::ApplyUpdate,
                                variant: BannerButtonVariant::Outlined,
                                icon: None,
                                more_info_button_action: None,
                            }),
                        })
                    } else if let Some(update_by) = new_version.update_by {
                        self.server_time.as_ref().and_then(|server_time| {
                            (server_time.current_time() > update_by).then(|| {
                                WorkspaceBannerFields {
                                    banner_type: WorkspaceBanner::VersionDeprecated,
                                    severity: BannerSeverity::Warning,
                                    heading: None,
                                    description: "Your app is out of date and needs to update."
                                        .to_string(),
                                    secondary_button: None,
                                    button: Some(WorkspaceBannerButtonDetails {
                                        text: "Restart app and update now".to_string(),
                                        action: WorkspaceAction::ApplyUpdate,
                                        variant: BannerButtonVariant::Outlined,
                                        icon: None,
                                        more_info_button_action: None,
                                    }),
                                }
                            })
                        })
                    } else {
                        None
                    }
                }
                AutoupdateStage::NoUpdateAvailable
                | AutoupdateStage::CheckingForUpdate
                | AutoupdateStage::DownloadingUpdate
                | AutoupdateStage::Updating { .. }
                | AutoupdateStage::UnableToUpdateToNewVersion { .. }
                | AutoupdateStage::UnableToLaunchNewVersion { .. } => None,
            }
        } else {
            None
        }
    }

    fn render_workspace_banner(
        &self,
        fields: WorkspaceBannerFields,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let bg_color = match fields.severity {
            BannerSeverity::Warning => theme.ansi_fg_yellow(),
            BannerSeverity::Error => theme.ansi_fg_red(),
        };
        let text_color = theme.main_text_color(Fill::Solid(bg_color)).into_solid();

        // Left side: alert icon + bold heading + regular description, all inline.
        let icon =
            ConstrainedBox::new(Icon::AlertCircle.to_warpui_icon(text_color.into()).finish())
                .with_width(16.)
                .with_height(16.)
                .finish();

        let ui_font_family = appearance.ui_font_family();
        const BANNER_FONT_SIZE: f32 = 12.;

        // Combine heading and description into a single `Text` so it can
        // elide with a trailing ellipsis when there isn't enough room for the
        // buttons. The heading portion is highlighted with Semibold weight.
        // See `ConversationSearchItem::render_item` for the same pattern.
        let heading_char_count = fields
            .heading
            .as_ref()
            .map(|heading| heading.chars().count())
            .unwrap_or(0);
        let combined_text = match fields.heading {
            Some(heading) => format!("{heading} {}", fields.description),
            None => fields.description,
        };
        let mut text = Text::new_inline(combined_text, ui_font_family, BANNER_FONT_SIZE)
            .with_color(text_color)
            .with_clip(ClipConfig::ellipsis());
        if heading_char_count > 0 {
            text = text.with_single_highlight(
                Highlight::new().with_properties(Properties::default().weight(Weight::Semibold)),
                (0..heading_char_count).collect(),
            );
        }

        let mut banner = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Container::new(icon).with_margin_right(8.).finish())
            // `Expanded` (not `Shrinkable`) so the text fills the remaining
            // row width and pushes the action buttons to the right even when
            // the text is short. Truncation still applies when the text would
            // otherwise overflow.
            .with_child(Expanded::new(1., text.finish()).finish());

        if let Some(secondary_button) = fields.secondary_button {
            banner.add_child(
                Container::new(self.render_banner_action_button(
                    secondary_button,
                    self.mouse_states.banner_secondary_button.clone(),
                    text_color,
                    appearance,
                ))
                .with_margin_left(4.)
                .finish(),
            );
        }

        if let Some(button) = fields.button {
            let more_info_button_action = button.more_info_button_action.clone();
            banner.add_child(
                Container::new(self.render_banner_action_button(
                    button,
                    self.mouse_states.banner_button.clone(),
                    text_color,
                    appearance,
                ))
                .with_margin_left(4.)
                .finish(),
            );

            if let Some(more_info_button_action) = more_info_button_action {
                let more_info_details = WorkspaceBannerButtonDetails {
                    text: "More info".to_owned(),
                    action: more_info_button_action,
                    variant: BannerButtonVariant::Outlined,
                    icon: None,
                    more_info_button_action: None,
                };
                banner.add_child(
                    Container::new(self.render_banner_action_button(
                        more_info_details,
                        self.mouse_states.more_info_banner_button.clone(),
                        text_color,
                        appearance,
                    ))
                    .with_margin_left(4.)
                    .finish(),
                );
            }
        }

        if fields.banner_type.is_dismissible() {
            let dismiss_target = fields.banner_type;
            banner.add_child(
                Container::new(
                    Hoverable::new(
                        self.mouse_states.dismiss_banner_button.clone(),
                        move |state| {
                            let mut container = Container::new(
                                ConstrainedBox::new(
                                    // Plain x-close glyph (`Icon::X` →
                                    // `x-close.svg`), matching the Figma
                                    // design. `Icon::XCircle` wraps the x in
                                    // a circle which is not what we want.
                                    Icon::X.to_warpui_icon(text_color.into()).finish(),
                                )
                                .with_width(16.)
                                .with_height(16.)
                                .finish(),
                            )
                            .with_uniform_padding(2.)
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));
                            if state.is_hovered() {
                                container = container
                                    .with_background_color(coloru_with_opacity(text_color, 20));
                            }
                            container.finish()
                        },
                    )
                    .with_cursor(Cursor::PointingHand)
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(WorkspaceAction::DismissWorkspaceBanner(
                            dismiss_target,
                        ));
                    })
                    .finish(),
                )
                .with_margin_left(4.)
                .finish(),
            );
        }

        ConstrainedBox::new(
            Container::new(banner.finish())
                .with_background_color(bg_color)
                .with_uniform_padding(8.)
                .finish(),
        )
        .finish()
    }

    /// Renders a single banner action button using the Figma-spec'd Naked or
    /// Secondary variants: no fill by default, optional 1px border, text and
    /// icon tinted with the banner's contrast-safe text color.
    fn render_banner_action_button(
        &self,
        details: WorkspaceBannerButtonDetails,
        mouse_state: MouseStateHandle,
        text_color: ColorU,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let WorkspaceBannerButtonDetails {
            text,
            action,
            variant,
            icon,
            ..
        } = details;
        let ui_font_family = appearance.ui_font_family();
        Hoverable::new(mouse_state, move |state| {
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);
            if let Some(icon) = icon {
                row.add_child(
                    Container::new(
                        ConstrainedBox::new(icon.to_warpui_icon(text_color.into()).finish())
                            .with_width(14.)
                            .with_height(14.)
                            .finish(),
                    )
                    .with_margin_right(4.)
                    .finish(),
                );
            }
            row.add_child(
                Text::new_inline(text.clone(), ui_font_family, 12.)
                    .with_color(text_color)
                    .with_style(Properties {
                        weight: Weight::Semibold,
                        ..Default::default()
                    })
                    .finish(),
            );

            let mut container = Container::new(row.finish())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .with_horizontal_padding(8.);
            if matches!(variant, BannerButtonVariant::Outlined) {
                container = container.with_border(Border::all(1.).with_border_color(text_color));
            }
            if state.is_hovered() {
                container = container.with_background_color(coloru_with_opacity(text_color, 20));
            }

            ConstrainedBox::new(container.finish())
                .with_height(24.)
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    fn dismiss_workspace_banner(
        &mut self,
        ctx: &mut ViewContext<Self>,
        banner_type: &WorkspaceBanner,
    ) {
        match banner_type {
            WorkspaceBanner::UnableToUpdateToNewVersion => {
                self.autoupdate_unable_to_update_banner_dismissed = true;
            }
            WorkspaceBanner::UnableToLaunchNewVersion => {
                self.autoupdate_unable_to_launch_new_version = true;
            }
            WorkspaceBanner::VersionDeprecated => {}
            WorkspaceBanner::AnonymousUserAuth => {}
            WorkspaceBanner::Reauth => {
                self.reauth_banner_dismissed = true;
            }
            #[cfg(all(enable_crash_recovery, target_os = "linux"))]
            WorkspaceBanner::WaylandCrashRecovery => {
                crash_recovery::dismiss_workspace_banner(ctx);
            }
            WorkspaceBanner::InvalidSettings => {
                self.settings_error_banner_dismissed = true;
                self.sync_settings_error_state_into_settings_pane(ctx);
            }
        }
        ctx.notify();
    }

    fn render_panel(
        &self,
        app: &AppContext,
        contents: Box<dyn Element>,
        side: &PanelPosition,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Max);
        let mut contents = contents;

        let traffic_light_data = traffic_light_data(app, self.window_id);
        let vertical_tabs_active =
            FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(app).use_vertical_tabs;
        // Add a spacer for the traffic light buttons on Windows/Linux.
        if traffic_light_data.is_some_and(|data| data.side == TrafficLightSide::Right)
            && *side == PanelPosition::Right
            && !vertical_tabs_active
        {
            col.add_child(
                ConstrainedBox::new(Empty::new().finish())
                    .with_height(TAB_BAR_HEIGHT)
                    .finish(),
            );
            contents = Container::new(contents)
                .with_border(Border::top(1.).with_border_fill(appearance.theme().surface_2()))
                .finish();
        }
        col.add_child(Shrinkable::new(1.0, contents).finish());

        self.wrap_in_panel_surface(appearance, side, col.finish(), *PANEL_CORNER_RADIUS)
    }

    fn wrap_in_panel_surface(
        &self,
        appearance: &Appearance,
        side: &PanelPosition,
        contents: Box<dyn Element>,
        corner_radius: CornerRadius,
    ) -> Box<dyn Element> {
        let mut container = Container::new(contents)
            .with_background(appearance.theme().surface_1().with_opacity(90))
            .with_corner_radius(corner_radius);

        match side {
            PanelPosition::Left => container = container.with_margin_right(2.0),
            PanelPosition::Right => container = container.with_margin_left(2.0),
        };

        container.finish()
    }

    fn render_panel_separator(app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        ConstrainedBox::new(
            Rect::new()
                .with_background_color(appearance.theme().outline().into_solid())
                .finish(),
        )
        .with_width(1.0)
        .finish()
    }

    fn add_panel_with_separator(
        panels_view: &mut Flex,
        prev_panel_added: &mut bool,
        panel: Option<Box<dyn Element>>,
        app: &AppContext,
    ) {
        if let Some(panel) = panel {
            if *prev_panel_added {
                panels_view.add_child(Self::render_panel_separator(app));
            }
            panels_view.add_child(panel);
            *prev_panel_added = true;
        }
    }

    fn render_panels(
        &self,
        app: &AppContext,
        terminal_view: Box<dyn Element>,
        hide_vertical_tabs: bool,
    ) -> Box<dyn Element> {
        let mut panels_view = Flex::row();
        let mut prev_panel_added = false;

        // Config-driven vertical-tabs-era panels (left side).
        // Hidden for simplified WASM views (notebooks, shared sessions, etc.)
        // where these panels are unnecessary.
        let vertical_tabs_active = !hide_vertical_tabs
            && FeatureFlag::VerticalTabs.is_enabled()
            && *TabSettings::as_ref(app).use_vertical_tabs;

        // In vertical tabs mode, config-driven panels are rendered here.
        // In horizontal tabs mode, they're rendered inside render_banner_and_active_tab.
        if vertical_tabs_active {
            let config = TabSettings::as_ref(app)
                .header_toolbar_chip_selection
                .clone();
            let pane_group = self.active_tab_pane_group().as_ref(app);

            for item in config.left_items() {
                Self::add_panel_with_separator(
                    &mut panels_view,
                    &mut prev_panel_added,
                    self.render_config_panel(&item, pane_group, &config, app),
                    app,
                );
            }
        }

        // Theme chooser (workspace-level, not configurable).
        // Uses wrap_in_panel_surface which adds margin for its own visual separation,
        // so we add a separator before it only if a config panel is to its left, then
        // reset the flag so no separator is added between the theme chooser and the terminal.
        if self.current_workspace_state.is_theme_chooser_open {
            if prev_panel_added {
                panels_view.add_child(Self::render_panel_separator(app));
            }
            panels_view.add_child(self.render_panel(
                app,
                self.render_theme_chooser(),
                &PanelPosition::Left,
            ));
            prev_panel_added = false;
        }

        if prev_panel_added {
            panels_view.add_child(Self::render_panel_separator(app));
        }
        // The outer workspace container in `render` already paints the terminal
        // background fill, so don't paint it again here (see APP-4328).
        panels_view = panels_view.with_child(Shrinkable::new(1.0, terminal_view).finish());
        prev_panel_added = true;

        if vertical_tabs_active {
            let config = TabSettings::as_ref(app)
                .header_toolbar_chip_selection
                .clone();
            let pane_group = self.active_tab_pane_group().as_ref(app);

            for item in config.right_items() {
                Self::add_panel_with_separator(
                    &mut panels_view,
                    &mut prev_panel_added,
                    self.render_config_panel(&item, pane_group, &config, app),
                    app,
                );
            }

            if pane_group.right_panel_open && pane_group.is_right_panel_maximized {
                Self::add_panel_with_separator(
                    &mut panels_view,
                    &mut prev_panel_added,
                    self.render_config_panel_maximized(pane_group, &config, app),
                    app,
                );
            } else if !config.contains_item(&HeaderToolbarItemKind::CodeReview) {
                Self::add_panel_with_separator(
                    &mut panels_view,
                    &mut prev_panel_added,
                    self.render_config_panel(
                        &HeaderToolbarItemKind::CodeReview,
                        pane_group,
                        &config,
                        app,
                    ),
                    app,
                );
            }
        }

        #[cfg(target_family = "wasm")]
        if !riftui::platform::wasm::is_mobile_device()
            && self
                .current_workspace_state
                .is_transcript_details_panel_open
        {
            if let Some(panel_content) = self.render_transcript_details_panel(app) {
                panels_view = panels_view.with_child(panel_content);
            }
        }

        // Resource center and AI assistant are workspace-level panels, not configurable.
        #[cfg(not(target_family = "wasm"))]
        if self.current_workspace_state.is_right_panel_open() {
            let right_panel_content = if self.current_workspace_state.is_resource_center_open {
                Some(self.render_panel(app, self.render_resource_center(), &PanelPosition::Right))
            } else if self.current_workspace_state.is_ai_assistant_panel_open {
                Some(self.render_panel(
                    app,
                    ChildView::new(&self.ai_assistant_panel).finish(),
                    &PanelPosition::Right,
                ))
            } else {
                log::warn!(
                    "is_right_panel_open() returned true, but neither the resource center nor AI \
                    assistant are open"
                );
                None
            };

            if let Some(right_panel_content) = right_panel_content {
                panels_view = panels_view.with_child(right_panel_content);
            }
        }

        panels_view.finish()
    }

    fn is_mailbox_on_left(config: &HeaderToolbarChipSelection) -> bool {
        config
            .left_items()
            .contains(&HeaderToolbarItemKind::NotificationsMailbox)
    }

    fn tabs_panel_side(config: &HeaderToolbarChipSelection) -> PanelPosition {
        if config
            .left_items()
            .contains(&HeaderToolbarItemKind::TabsPanel)
        {
            PanelPosition::Left
        } else {
            PanelPosition::Right
        }
    }

    /// Renders a configurable panel for the given toolbar item, if it is open.
    /// Returns `None` if the panel should not be rendered (item not supported,
    /// panel not open, or item is not a panel type).
    fn render_config_panel(
        &self,
        item: &HeaderToolbarItemKind,
        pane_group: &PaneGroup,
        config: &HeaderToolbarChipSelection,
        app: &AppContext,
    ) -> Option<Box<dyn Element>> {
        if !item.is_supported(app) || !item.is_panel() {
            return None;
        }
        match item {
            HeaderToolbarItemKind::TabsPanel => {
                if !self.vertical_tabs_panel_open {
                    return None;
                }
                Some(
                    SavePosition::new(
                        self.render_vertical_tabs_panel(Self::tabs_panel_side(config), app),
                        VERTICAL_TABS_PANEL_POSITION_ID,
                    )
                    .finish(),
                )
            }
            HeaderToolbarItemKind::ToolsPanel => {
                if !pane_group.left_panel_open || riftui::platform::is_mobile_device() {
                    return None;
                }
                Some(ChildView::new(&self.left_panel_view).finish())
            }
            HeaderToolbarItemKind::CodeReview
            | HeaderToolbarItemKind::AgentManagement
            | HeaderToolbarItemKind::NotificationsMailbox => None,
        }
    }

    /// Renders the maximized code review panel if it is configured and maximized.
    fn render_config_panel_maximized(
        &self,
        pane_group: &PaneGroup,
        _config: &HeaderToolbarChipSelection,
        app: &AppContext,
    ) -> Option<Box<dyn Element>> {
        let _ = (pane_group, app);
        None
    }

    /// Offset positioning for agent toasts.
    /// TODO: update positioning based on input mode.
    fn agent_toast_positioning(&self) -> OffsetPositioning {
        OffsetPositioning::offset_from_save_position_element(
            TAB_CONTENT_POSITION_ID,
            vec2f(0., 16.),
            PositionedElementOffsetBounds::WindowByPosition,
            PositionedElementAnchor::TopRight,
            ChildAnchor::TopRight,
        )
    }

    /// Offset positioning for global toasts.
    // TODO: update positioning based on input mode.
    fn global_toast_positioning(&self) -> OffsetPositioning {
        OffsetPositioning::offset_from_save_position_element(
            TAB_CONTENT_POSITION_ID,
            vec2f(0., 16.),
            PositionedElementOffsetBounds::WindowByPosition,
            PositionedElementAnchor::TopMiddle,
            ChildAnchor::TopMiddle,
        )
    }

    /// Offset positioning for the update toast.
    fn update_toast_positioning(
        &self,
        input_position_id: String,
        app: &AppContext,
    ) -> OffsetPositioning {
        let input_mode = InputModeSettings::as_ref(app).input_mode.value();

        match input_mode {
            InputMode::PinnedToBottom => OffsetPositioning::offset_from_save_position_element(
                input_position_id,
                vec2f(-16., -16.),
                PositionedElementOffsetBounds::WindowByPosition,
                PositionedElementAnchor::TopRight,
                ChildAnchor::BottomRight,
            ),
            InputMode::PinnedToTop => OffsetPositioning::offset_from_save_position_element(
                input_position_id,
                vec2f(-16., 16.),
                PositionedElementOffsetBounds::WindowByPosition,
                PositionedElementAnchor::BottomRight,
                ChildAnchor::TopRight,
            ),
            InputMode::Waterfall => OffsetPositioning::offset_from_parent(
                vec2f(-16., -16.),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::BottomRight,
                ChildAnchor::BottomRight,
            ),
        }
    }

    fn add_toggle_setting_context_flags(&self, app: &AppContext, context: &mut Context) {
        let privacy_settings = PrivacySettings::as_ref(app);
        let editor_settings = AppEditorSettings::as_ref(app);
        let semantic_selection_settings = SemanticSelection::as_ref(app);
        let selection_settings = SelectionSettings::as_ref(app);
        let session_settings = SessionSettings::as_ref(app);
        let safe_mode_settings: &SafeModeSettings = SafeModeSettings::as_ref(app);
        let block_list_settings = BlockListSettings::as_ref(app);
        let tab_settings = TabSettings::as_ref(app);
        let alias_expansion_settings = AliasExpansionSettings::as_ref(app);
        let code_settings = CodeSettings::as_ref(app);
        let input_settings = InputSettings::as_ref(app);
        let font_settings = FontSettings::as_ref(app);
        let reporting_setings = AltScreenReporting::as_ref(app);
        let general_settings = GeneralSettings::as_ref(app);
        let theme_settings = ThemeSettings::as_ref(app);
        let ssh_settings = SshSettings::as_ref(app);
        let warpify_settings = WarpifySettings::as_ref(app);
        let terminal_settings = TerminalSettings::as_ref(app);
        let window_settings = WindowSettings::as_ref(app);
        let pane_settings = PaneSettings::as_ref(app);
        let keys_settings = KeysSettings::as_ref(app);
        let command_search_settings = CommandSearchSettings::as_ref(app);

        let is_compact_mode =
            matches!(terminal_settings.spacing_mode.value(), SpacingMode::Compact);
        if is_compact_mode {
            context.set.insert(flags::COMPACT_MODE_CONTEXT_FLAG);
        }

        let respect_system_theme = respect_system_theme(theme_settings);
        if matches!(respect_system_theme, RespectSystemTheme::On(_)) {
            context.set.insert(flags::RESPECT_SYSTEM_THEME_CONTEXT_FLAG);
        }

        if SelectionSettings::as_ref(app).copy_on_select_enabled() {
            context.set.insert(flags::COPY_ON_SELECT_CONTEXT_FLAG);
        }

        if SelectionSettings::as_ref(app).linux_selection_clipboard_enabled() {
            context.set.insert(flags::LINUX_SELECTION_CLIPBOARD_FLAG);
        }

        if *editor_settings.autocomplete_symbols {
            context.set.insert(flags::AUTOCOMPLETE_SYMBOLS_CONTEXT_FLAG);
        }

        if *general_settings.restore_session {
            context.set.insert(flags::RESTORE_SESSION_CONTEXT_FLAG);
        }

        if *session_settings.honor_ps1 {
            context.set.insert(flags::HONOR_PS1_CONTEXT_FLAG);
        }

        if session_settings
            .saved_prompt
            .value()
            .same_line_prompt_enabled()
        {
            context.set.insert(flags::RIFT_SAME_LINE_PROMPT_FLAG);
        }

        if *ssh_settings.enable_legacy_ssh_wrapper.value() {
            #[allow(deprecated)]
            context.set.insert(flags::LEGACY_SSH_WRAPPER_CONTEXT_FLAG);
        }
        if *warpify_settings.enable_ssh_warpification.value() {
            context.set.insert(flags::SSH_WARPIFICATION_CONTEXT_FLAG);
        }

        if *warpify_settings.use_ssh_tmux_wrapper.value() {
            context.set.insert(flags::SSH_TMUX_WRAPPER_CONTEXT_FLAG);
        }

        if keys_settings.extra_meta_keys.left_alt {
            context.set.insert(flags::EXTRA_META_KEYS_LEFT_CONTEXT_FLAG);
        }

        if keys_settings.extra_meta_keys.right_alt {
            context
                .set
                .insert(flags::EXTRA_META_KEYS_RIGHT_CONTEXT_FLAG);
        }

        if *reporting_setings.scroll_reporting_enabled.value() {
            context.set.insert(flags::SCROLL_REPORTING_CONTEXT_FLAG);
        }
        if *reporting_setings.mouse_reporting_enabled.value() {
            context.set.insert(flags::MOUSE_REPORTING_CONTEXT_FLAG);
        }

        if *reporting_setings.focus_reporting_enabled.value() {
            context.set.insert(flags::FOCUS_REPORTING_CONTEXT_FLAG);
        }

        if *KeysSettings::as_ref(app).quake_mode_enabled {
            context.set.insert(flags::QUAKE_MODE_ENABLED_CONTEXT_FLAG);
        }

        if matches!(
            SessionSettings::as_ref(app).notifications.mode,
            NotificationsMode::Enabled
        ) {
            context.set.insert(flags::NOTIFICATIONS_CONTEXT_FLAG);
        }
        if session_settings.notifications.is_long_running_enabled {
            context.set.insert(flags::LONG_RUNNING_NOTIFICATIONS_FLAG);
        }
        if session_settings
            .notifications
            .is_agent_task_completed_enabled
        {
            context
                .set
                .insert(flags::AGENT_TASK_COMPLETED_NOTIFICATIONS_FLAG);
        }
        if session_settings.notifications.is_needs_attention_enabled {
            context
                .set
                .insert(flags::NEEDS_ATTENTION_NOTIFICATIONS_FLAG);
        }
        if session_settings.notifications.play_notification_sound {
            context.set.insert(flags::NOTIFICATION_SOUND_FLAG);
        }

        if *general_settings.link_tooltip {
            context.set.insert(flags::LINK_TOOLTIP_CONTEXT_FLAG);
        }

        if *input_settings.completions_open_while_typing.value() {
            context
                .set
                .insert(flags::COMPLETIONS_OPEN_WHILE_TYPING_CONTEXT_FLAG);
        }

        if *input_settings.command_corrections.value() {
            context.set.insert(flags::COMMAND_CORRECTIONS_CONTEXT_FLAG);
        }

        if *input_settings.error_underlining.value() {
            context.set.insert(flags::ERROR_UNDERLINING_FLAG);
        }

        if *input_settings.syntax_highlighting.value() {
            context.set.insert(flags::SYNTAX_HIGHLIGHTING_FLAG);
        }

        if privacy_settings.is_telemetry_enabled {
            context.set.insert(flags::TELEMETRY_FLAG);
        }

        if *block_list_settings
            .show_jump_to_bottom_of_block_button
            .value()
        {
            context
                .set
                .insert(flags::JUMP_TO_BOTTOM_OF_BLOCK_BUTTON_CONTEXT_FLAG);
        }

        if *block_list_settings.show_block_dividers.value() {
            context.set.insert(flags::BLOCK_DIVIDERS_CONTEXT_FLAG);
        }

        if *safe_mode_settings.safe_mode_enabled.value() {
            context.set.insert(flags::SAFE_MODE_FLAG);
        }
        if !privacy_settings.is_telemetry_force_enabled()
            && matches!(
                UserWorkspaces::as_ref(app).get_cloud_conversation_storage_enablement_setting(),
                AdminEnablementSetting::RespectUserSetting
            )
        {
            context
                .set
                .insert(flags::CLOUD_CONVERSATION_STORAGE_EDITABLE_FLAG);
        }
        if privacy_settings.is_cloud_conversation_storage_enabled {
            context.set.insert(flags::CLOUD_CONVERSATION_STORAGE_FLAG);
        }

        if privacy_settings.is_crash_reporting_enabled {
            context.set.insert(flags::CRASH_REPORTING_FLAG);
        }

        if editor_settings.cursor_blink.value() == &CursorBlink::Enabled {
            context.set.insert(flags::CURSOR_BLINK_CONTEXT_FLAG);
        }

        if *editor_settings.vim_mode.value() {
            context.set.insert(flags::VIM_MODE_CONTEXT_FLAG);
            if *editor_settings.vim_unnamed_system_clipboard.value() {
                context.set.insert(flags::VIM_UNNAMED_SYSTEM_CLIPBOARD);
            }
            if *editor_settings.vim_status_bar.value() {
                context.set.insert(flags::VIM_SHOW_STATUS_BAR);
            }
        }

        if *pane_settings.should_dim_inactive_panes {
            context.set.insert(flags::DIM_INACTIVE_PANES_FLAG);
        }
        if *window_settings.open_windows_at_custom_size {
            context.set.insert(flags::OPEN_WINDOWS_AT_CUSTOM_SIZE_FLAG);
        }

        if *window_settings.background_blur_texture {
            context.set.insert(flags::WINDOW_BLUR_TEXTURE_FLAG);
        }

        if *window_settings.left_panel_visibility_across_tabs {
            context
                .set
                .insert(flags::LEFT_PANEL_VISIBILITY_ACROSS_TABS_FLAG);
        }

        if *font_settings.match_ai_font_to_terminal_font {
            context
                .set
                .insert(flags::MATCH_AI_FONT_TO_TERMINAL_FONT_FLAG);
        }

        if *font_settings.match_notebook_to_monospace_font_size {
            context
                .set
                .insert(flags::MATCH_NOTEBOOK_FONT_SIZE_TO_TERMINAL_FONT_SIZE_FLAG);
        }

        if *pane_settings.focus_panes_on_hover {
            context.set.insert(flags::FOCUS_PANES_ON_HOVER_CONTEXT_FLAG);
        }

        if *general_settings.show_warning_before_quitting.value() {
            context.set.insert(flags::QUIT_WARNING_MODAL);
        }

        if semantic_selection_settings.smart_select_enabled() {
            context.set.insert(flags::SMART_SELECT_FLAG);
        }

        if *KeysSettings::as_ref(app).activation_hotkey_enabled.value() {
            context.set.insert(flags::ACTIVATION_HOTKEY_FLAG);
        }

        if *tab_settings.show_indicators.value() {
            context.set.insert(flags::TAB_INDICATORS_FLAG);
        }
        if *tab_settings.show_code_review_button.value() {
            context.set.insert(flags::SHOW_CODE_REVIEW_BUTTON_FLAG);
        }
        if *tab_settings.show_code_review_diff_stats.value() {
            context.set.insert(flags::SHOW_CODE_REVIEW_DIFF_STATS_FLAG);
        }
        if *general_settings
            .auto_open_code_review_pane_on_first_agent_change
            .value()
        {
            context.set.insert(flags::AUTO_OPEN_CODE_REVIEW_PANE_FLAG);
        }
        if *tab_settings.use_vertical_tabs.value() {
            context.set.insert(flags::USE_VERTICAL_TABS_FLAG);
        }
        if *tab_settings.preserve_active_tab_color.value() {
            context.set.insert(flags::PRESERVE_ACTIVE_TAB_COLOR_FLAG);
        }
        if *tab_settings
            .show_vertical_tab_panel_in_restored_windows
            .value()
        {
            context
                .set
                .insert(flags::SHOW_VERTICAL_TAB_PANEL_IN_RESTORED_WINDOWS_FLAG);
        }
        if *tab_settings
            .use_latest_user_prompt_as_conversation_title_in_tab_names
            .value()
        {
            context
                .set
                .insert(flags::USE_LATEST_USER_PROMPT_AS_CONVERSATION_TITLE_IN_TAB_NAMES_FLAG);
        }
        if self.should_show_session_config_tab_config_chip() {
            context
                .set
                .insert(flags::SESSION_CONFIG_TAB_CONFIG_CHIP_OPEN);
        }

        if tab_settings
            .workspace_decoration_visibility
            .value()
            .hides_decorations_by_default()
        {
            context
                .set
                .insert(flags::HIDE_WORKSPACE_DECORATIONS_CONTEXT_FLAG);
        }

        if *alias_expansion_settings.alias_expansion_enabled.value() {
            context.set.insert(flags::ALIAS_EXPANSION_FLAG);
        }

        if *selection_settings.middle_click_paste_enabled.value() {
            context.set.insert(flags::MIDDLE_CLICK_PASTE_FLAG);
        }

        if *code_settings.code_as_default_editor.value() {
            context.set.insert(flags::CODE_AS_DEFAULT_EDITOR);
        }

        if *code_settings.codebase_context_enabled.value() {
            context.set.insert(flags::IS_CODEBASE_INDEXING_ENABLED);
        }

        if *code_settings.auto_indexing_enabled.value() {
            context.set.insert(flags::IS_AUTOINDEXING_ENABLED);
        }

        if *input_settings.show_hint_text.value() {
            context.set.insert(flags::SHOW_INPUT_HINT_TEXT_CONTEXT_FLAG);
        }

        if *input_settings.show_agent_tips.value() {
            context.set.insert(flags::SHOW_AGENT_TIPS_FLAG);
        }
        if *editor_settings.enable_autosuggestions {
            context.set.insert(flags::AUTOSUGGESTIONS_ENABLED_FLAG);
        }

        if *editor_settings.autosuggestion_keybinding_hint.value() {
            context
                .set
                .insert(flags::AUTOSUGGESTION_KEYBINDING_HINT_FLAG);
        }
        if *editor_settings.show_autosuggestion_ignore_button.value() {
            context
                .set
                .insert(flags::SHOW_AUTOSUGGESTION_IGNORE_BUTTON_FLAG);
        }

        #[cfg(target_os = "linux")]
        {
            let force_x11 = *crate::settings::LinuxAppConfiguration::as_ref(app)
                .force_x11
                .value();

            if !force_x11 {
                context.set.insert(flags::ALLOW_NATIVE_WAYLAND);
            }
        }

        let terminal_settings = TerminalSettings::as_ref(app);
        if *terminal_settings.use_audible_bell {
            context.set.insert(flags::USE_AUDIBLE_BELL_CONTEXT_FLAG);
        }
        if *terminal_settings.show_terminal_zero_state_block.value() {
            context
                .set
                .insert(flags::SHOW_TERMINAL_ZERO_STATE_BLOCK_FLAG);
        }
        if matches!(
            terminal_settings.alt_screen_padding.value(),
            crate::terminal::settings::AltScreenPaddingMode::Custom { .. }
        ) {
            context.set.insert(flags::ALT_SCREEN_PADDING_FLAG);
        }

        let gpu_settings = GPUSettings::as_ref(app);
        if *gpu_settings.prefer_low_power_gpu {
            context.set.insert(flags::PREFER_LOW_POWER_GPU_FLAG);
        }

        let ai_settings = AISettings::as_ref(app);
        if ai_settings.is_ai_autodetection_enabled(app) {
            context.set.insert(flags::AI_INPUT_AUTODETECTION_FLAG);
        }
        if ai_settings.is_nld_in_terminal_enabled(app) {
            context.set.insert(flags::NLD_IN_TERMINAL_FLAG);
        }
        if ai_settings.is_intelligent_autosuggestions_enabled(app) {
            context.set.insert(flags::INTELLIGENT_AUTOSUGGESTIONS_FLAG);
        }
        if ai_settings.is_prompt_suggestions_enabled(app) {
            context.set.insert(flags::PROMPT_SUGGESTIONS_FLAG);
        }
        if ai_settings.is_code_suggestions_enabled(app) {
            context.set.insert(flags::CODE_SUGGESTIONS_FLAG);
        }
        if ai_settings.is_natural_language_autosuggestions_enabled(app) {
            context
                .set
                .insert(flags::NATURAL_LANGUAGE_AUTOSUGGESTIONS_FLAG);
        }

        if ai_settings.is_shared_block_title_generation_enabled(app) {
            context
                .set
                .insert(flags::SHARED_BLOCK_TITLE_GENERATION_FLAG);
        }

        if *ai_settings.should_show_oz_updates_in_zero_state.value() {
            context
                .set
                .insert(flags::SHOW_OZ_UPDATES_IN_ZERO_STATE_FLAG);
        }
        if *ai_settings.git_operations_autogen_enabled_internal.value() {
            context.set.insert(flags::GIT_OPERATIONS_AUTOGEN_FLAG);
        }
        if *ai_settings.include_agent_commands_in_history.value() {
            context
                .set
                .insert(flags::INCLUDE_AGENT_COMMANDS_IN_HISTORY_FLAG);
        }
        if *ai_settings.memory_enabled.value() {
            context.set.insert(flags::AI_RULES_FLAG);
        }
        if *ai_settings.rule_suggestions_enabled_internal.value() {
            context.set.insert(flags::SUGGESTED_RULES_FLAG);
        }
        if *ai_settings.warp_drive_context_enabled.value() {
            context.set.insert(flags::RIFT_DRIVE_CONTEXT_FLAG);
        }
        if *ai_settings.file_based_mcp_enabled.value() {
            context.set.insert(flags::FILE_BASED_MCP_FLAG);
        }
        if *ai_settings.can_use_warp_credits_for_fallback.value() {
            context.set.insert(flags::RIFT_CREDIT_FALLBACK_FLAG);
        }
        if *session_settings.show_model_selectors_in_prompt.value() {
            context
                .set
                .insert(flags::SHOW_BASE_MODEL_PICKER_IN_PROMPT_FLAG);
        }
        if *ai_settings.should_render_cli_agent_footer.value() {
            context.set.insert(flags::CLI_AGENT_FOOTER_ENABLED);
        }
        if *ai_settings.auto_toggle_rich_input.value() {
            context.set.insert(flags::AUTO_TOGGLE_RICH_INPUT_FLAG);
        }
        if *ai_settings.auto_open_rich_input_on_cli_agent_start.value() {
            context
                .set
                .insert(flags::AUTO_OPEN_RICH_INPUT_ON_CLI_AGENT_START_FLAG);
        }
        if *ai_settings.auto_dismiss_rich_input_after_submit.value() {
            context
                .set
                .insert(flags::AUTO_DISMISS_RICH_INPUT_AFTER_SUBMIT_FLAG);
        }
        if *ai_settings.show_agent_notifications.value() {
            context.set.insert(flags::AGENT_IN_APP_NOTIFICATIONS_FLAG);
        }

        if *ai_settings
            .should_render_use_agent_footer_for_user_commands
            .value()
        {
            context.set.insert(flags::USE_AGENT_FOOTER_FLAG);
        }

        match ai_settings.thinking_display_mode {
            crate::settings::ThinkingDisplayMode::ShowAndCollapse => {
                context
                    .set
                    .insert(flags::THINKING_DISPLAY_SHOW_AND_COLLAPSE);
            }
            crate::settings::ThinkingDisplayMode::AlwaysShow => {
                context.set.insert(flags::THINKING_DISPLAY_ALWAYS_SHOW);
            }
            crate::settings::ThinkingDisplayMode::NeverShow => {
                context.set.insert(flags::THINKING_DISPLAY_NEVER_SHOW);
            }
        }

        match ai_settings.orchestration_message_display_mode {
            crate::settings::OrchestrationMessageDisplayMode::ShowAndCollapse => {
                context
                    .set
                    .insert(flags::ORCHESTRATION_MESSAGE_DISPLAY_SHOW_AND_COLLAPSE);
            }
            crate::settings::OrchestrationMessageDisplayMode::AlwaysShow => {
                context
                    .set
                    .insert(flags::ORCHESTRATION_MESSAGE_DISPLAY_ALWAYS_SHOW);
            }
            crate::settings::OrchestrationMessageDisplayMode::AlwaysCollapse => {
                context
                    .set
                    .insert(flags::ORCHESTRATION_MESSAGE_DISPLAY_ALWAYS_COLLAPSE);
            }
        }

        match ai_settings.default_prompt_submission_mode {
            crate::settings::PromptSubmissionMode::Interrupt => {
                context.set.insert(flags::PROMPT_SUBMISSION_INTERRUPT);
            }
            crate::settings::PromptSubmissionMode::Queue => {
                context.set.insert(flags::PROMPT_SUBMISSION_QUEUE);
            }
        }

        if input_settings.is_terminal_input_message_bar_enabled() {
            context
                .set
                .insert(flags::SHOW_TERMINAL_INPUT_MESSAGE_LINE_FLAG);
        }

        if *input_settings.enable_slash_commands_in_terminal.value() {
            context.set.insert(flags::SLASH_COMMANDS_IN_TERMINAL_FLAG);
        }
        if *input_settings.at_context_menu_in_terminal_mode.value() {
            context.set.insert(flags::AT_CONTEXT_MENU_IN_TERMINAL_FLAG);
        }

        if *input_settings
            .outline_codebase_symbols_for_at_context_menu
            .value()
        {
            context
                .set
                .insert(flags::OUTLINE_CODEBASE_SYMBOLS_FOR_AT_CONTEXT_MENU_FLAG);
        }
        if *command_search_settings
            .show_global_workflows_in_universal_search
            .value()
        {
            context
                .set
                .insert(flags::GLOBAL_WORKFLOWS_IN_COMMAND_SEARCH_FLAG);
        }

        if ChannelState::enable_debug_features() {
            let block_visibility_settings = BlockVisibilitySettings::as_ref(app);
            if *block_visibility_settings
                .should_show_bootstrap_block
                .value()
            {
                context.set.insert(flags::INITIALIZATION_BLOCK_FLAG);
            }
            if *block_visibility_settings
                .should_show_in_band_command_blocks
                .value()
            {
                context.set.insert(flags::IN_BAND_COMMAND_BLOCKS_FLAG);
            }
        }

        if should_use_ligature_rendering(app) {
            context.set.insert(flags::LIGATURE_RENDERING_CONTEXT_FLAG);
        }
    }

    /// Send SyncEvent to all synced pane groups.
    fn process_sync_event_for_all_synced_pane_groups(
        &mut self,
        event: &SyncEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        for tab in self.tab_views() {
            // We have to get the latest SyncInputStatus each iteration because
            // tab.update below could potentially change it.
            let synced_pane_group_ids = SyncedInputState::as_ref(ctx);

            if synced_pane_group_ids.should_sync_this_pane_group(tab.id(), ctx.window_id()) {
                tab.update(ctx, |pane_group, ctx| {
                    pane_group.send_sync_event_to_panes(event, ctx);
                });
            }
        }

        self.update_pane_dimming_for_current_focus_region(ctx);
    }

    /// Sends SyncEvent to all synced terminal views.
    /// The purpose of the event could be match the active terminal input,
    /// expand the terminal input box, or collapse the terminal input box.
    fn process_updated_sync_state(&self, ctx: &mut ViewContext<Self>) {
        // If there is an active terminal, return a sync event that all
        // other synced terminals should apply to match it.
        // If there is no active terminal (like when all Warp windows are
        // minimized), return an event to start syncing.
        let sync_event = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .active_session_view(ctx)
            .map_or(
                SyncEvent {
                    source_view_id: ctx.view_id(),
                    data: SyncInputType::StartSyncing,
                },
                |terminal_view_handle| {
                    terminal_view_handle
                        .as_ref(ctx)
                        .create_sync_event_based_on_terminal_state(ctx)
                },
            );

        let stop_syncing_event = SyncEvent {
            source_view_id: ctx.view_id(),
            data: terminal::view::SyncInputType::StopSyncing,
        };

        for tab in self.tab_views() {
            // We have to get the latest SyncInputStatus each iteration because
            // tab.update below could potentially change it.
            let synced_pane_group_ids = SyncedInputState::as_ref(ctx);

            if synced_pane_group_ids.should_sync_this_pane_group(tab.id(), ctx.window_id()) {
                tab.update(ctx, |pane_group, pane_group_ctx| {
                    pane_group.send_sync_event_to_panes(&sync_event, pane_group_ctx);
                });
            } else {
                // Note: we're sending StopSyncing to tabs that could already
                // know they're not syncing. We can optimize this later.
                tab.update(ctx, |pane_group, pane_group_ctx| {
                    pane_group.send_sync_event_to_panes(&stop_syncing_event, pane_group_ctx);
                });
            }
        }

        // Update tab indicators based on the new sync state.
        ctx.notify();
    }

    fn all_pane_group_ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.tab_views().map(|tab| tab.id())
    }

    fn team_uid(&self, app: &AppContext) -> Option<ServerId> {
        // TODO this is a stop gap for now - ideally a specific team uid should
        // be passed into each event
        UserWorkspaces::as_ref(app).current_team_uid()
    }

    fn initiate_user_signup(
        &mut self,
        entrypoint: AnonymousUserSignupEntrypoint,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.auth_state.is_user_anonymous().unwrap_or_default() {
            // User has a Firebase anonymous account — use the linking flow.
            AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                auth_manager.initiate_anonymous_user_linking(entrypoint, ctx);
            });
        } else {
            // User is fully logged out (no Firebase user) — open the regular sign-up page.
            AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                let sign_up_url = auth_manager.sign_up_url();
                ctx.open_url(&sign_up_url);
            });
        }
        self.require_login_modal.update(ctx, |auth_modal, ctx| {
            auth_modal.skip_to_browser_open_step(ctx);
        });
        self.open_require_login_modal(AuthViewVariant::RequireLoginCloseable, ctx);
    }

    fn redirect_to_sign_in(&mut self) {
        #[cfg(target_family = "wasm")]
        if let Some(current_url) = parse_current_url() {
            update_browser_url(
                Url::parse(&format!(
                    "{}/login?redirect_to={}",
                    ChannelState::server_root_url(),
                    current_url.path()
                ))
                .ok(),
                true,
            );
        } else {
            update_browser_url(
                Url::parse(&format!("{}/login", ChannelState::server_root_url())).ok(),
                true,
            );
        }
    }

    /// Triggers the necessary cleanup for when a user logs out.
    pub fn on_log_out(&mut self, ctx: &mut ViewContext<Self>) {
        // Logging out should mimic the same behaviour as closing a window.
        // This gives views a chance to clean up any state through on_view_detached before being dropped.
        self.on_window_closed(ctx);
    }

    fn focus_openwarp_launch_modal(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.openwarp_launch_modal);
    }

    fn focus_orchestration_launch_modal(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.orchestration_launch_modal);
    }

    fn open_tab_and_focus_oz_launch_modal(&mut self, ctx: &mut ViewContext<Self>) {
        // Create a new tab with one terminal session titled "Introducing Oz"
        self.add_tab_with_pane_layout(
            PanesLayout::SingleTerminal(Box::new(NewTerminalOptions {
                shell: None,
                initial_directory: None,
                hide_homepage: false,
                ..Default::default()
            })),
            Arc::new(HashMap::new()),
            Some("Introducing Oz".to_string()),
            ctx,
        );
        self.oz_launch_modal.tab_pane_group_id = self
            .tabs
            .get(self.active_tab_index)
            .map(|tab| tab.pane_group.id());
        ctx.focus(&self.oz_launch_modal.view);
    }

    fn focus_build_plan_migration_modal(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.build_plan_migration_modal);
    }

    fn open_left_panel_view(&mut self, action: &LeftPanelAction, ctx: &mut ViewContext<Self>) {
        if !self.active_tab_pane_group().as_ref(ctx).left_panel_open {
            self.toggle_left_panel(ctx);
        }

        if self.active_tab_pane_group().as_ref(ctx).left_panel_open {
            self.left_panel_view.update(ctx, |left_panel, ctx| {
                left_panel.handle_action_with_force_open(action, false, ctx);
                left_panel.focus_active_view_on_entry(ctx);
            });
        }
    }

    fn toggle_left_panel_view(
        &mut self,
        action: &LeftPanelAction,
        is_showing_target_view: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let is_left_panel_open = self.active_tab_pane_group().as_ref(ctx).left_panel_open;

        if is_left_panel_open && is_showing_target_view {
            // If we're showing the target view for this action,
            // toggle the left panel closed.
            self.toggle_left_panel(ctx);
        } else {
            self.open_left_panel_view(action, ctx);
        }
    }

    /// Computes the list of available left panel views based on current AI settings and feature flags.
    fn compute_left_panel_views(ctx: &AppContext) -> Vec<ToolPanelView> {
        let mut views = vec![];
        if cfg!(feature = "local_fs") && *CodeSettings::as_ref(ctx).show_project_explorer.value() {
            views.push(ToolPanelView::ProjectExplorer);
        }
        if cfg!(feature = "local_fs")
            && FeatureFlag::GlobalSearch.is_enabled()
            && *CodeSettings::as_ref(ctx).show_global_search.value()
        {
            views.push(ToolPanelView::GlobalSearch {
                entry_focus: GlobalSearchEntryFocus::Results,
            });
        }
        views
    }

    /// Recomputes the available left panel views based on current AI settings and feature flags,
    /// then updates both the workspace's left_panel_views and the LeftPanelView's toolbelt buttons.
    fn update_left_panel_available_views(&mut self, ctx: &mut ViewContext<Self>) {
        let views = Self::compute_left_panel_views(ctx);
        self.left_panel_views = views.clone();
        self.left_panel_view.update(ctx, |left_panel, ctx| {
            left_panel.update_available_views(views, ctx);
        });
    }

    /// Opens a given URL in the desktop Warp app if installed, or redirects to download page.
    #[cfg(target_family = "wasm")]
    fn open_link_on_desktop(&mut self, url: &Url, ctx: &mut ViewContext<Self>) {
        use crate::settings::app_installation_detection::{
            UserAppInstallDetectionSettings, UserAppInstallStatus,
        };

        // Check if the desktop app is installed
        let is_app_installed = *UserAppInstallDetectionSettings::as_ref(ctx)
            .user_app_installation_detected
            .value()
            == UserAppInstallStatus::Detected;

        if !is_app_installed {
            // App not installed - redirect to download page
            ctx.open_url("https://warp.dev/download");
            // In webapp code we cannot distinguish between
            // the localhost:9277/install_detection endpoint not running (not installed) vs
            // the browser blocking Local Network Access which results in CORS error;
            // the browser intentionally obscures the error root cause for privacy reasons.
            // Many users' browser settings will block Local Network Access so this will end up redirecting to download page,
            // even if they have the app installed.
            let toast_message = format!(
                "Have Warp installed but redirecting to download page?\nEnable Local Network Access for {} in your browser.",
                ChannelState::server_root_url()
            );
            self.toast_stack.update(ctx, |toast_stack, ctx| {
                toast_stack.add_persistent_toast(DismissibleToast::default(toast_message), ctx)
            });
            // Still try to open the url on desktop below
        }

        // Open the URL on desktop. This does nothing if the app isn't installed.
        crate::uri::web_intent_parser::open_url_on_desktop(url);
    }
}

impl Entity for Workspace {
    type Event = ();
}

impl TypedActionView for Workspace {
    type Action = WorkspaceAction;

    fn action_accessibility_contents(
        &mut self,
        action: &WorkspaceAction,
        _: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        match action {
            WorkspaceAction::SetA11yVerbosityLevel(verbosity) => {
                ActionAccessibilityContent::Custom(AccessibilityContent::new_without_help(
                    format!("{verbosity:?} accessibility announcements set"),
                    WarpA11yRole::UserAction,
                ))
            }
            _ => ActionAccessibilityContent::from_debug(),
        }
    }

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        use WorkspaceAction::*;
        let window_id = ctx.window_id();

        if self.auth_state.is_anonymous_or_logged_out() && action.blocked_for_anonymous_user() {
            AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                auth_manager.attempt_login_gated_feature(
                    action.into(),
                    AuthViewVariant::RequireLoginCloseable,
                    ctx,
                )
            });
            return;
        }

        match action {
            ActivateTab(index) => self.activate_tab(*index, ctx),
            ActivateTabByNumber(num) => self.activate_tab(num.saturating_sub(1), ctx),
            ActivatePrevTab => self.activate_prev_tab(ctx),
            OpenLaunchConfigSaveModal => self.open_launch_config_save_modal(ctx),
            ActivateNextTab => self.activate_next_tab(ctx),
            ActivateLastTab => self.activate_last_tab(ctx),
            CyclePrevSession => self.cycle_prev_session(ctx),
            CycleNextSession => self.cycle_next_session(ctx),
            MoveActiveTabLeft => self.move_tab(self.active_tab_index, TabMovement::Left, ctx),
            MoveActiveTabRight => self.move_tab(self.active_tab_index, TabMovement::Right, ctx),
            MoveTabLeft(index) => self.move_tab(*index, TabMovement::Left, ctx),
            MoveTabRight(index) => self.move_tab(*index, TabMovement::Right, ctx),
            RenameTab(index) => self.rename_tab(*index, ctx),
            ResetTabName(index) => self.clear_tab_name(*index, ctx),
            RenamePane(locator) => self.rename_pane(*locator, ctx),
            ResetPaneName(locator) => self.clear_pane_name(*locator, ctx),
            RenameActiveTab => self.rename_tab(self.active_tab_index, ctx),
            RenameActivePane => {
                let pane_group = self.active_tab_pane_group().clone();
                let pane_group_id = pane_group.id();
                let pane_id = pane_group.as_ref(ctx).focused_pane_id(ctx);
                self.rename_pane(
                    PaneViewLocator {
                        pane_group_id,
                        pane_id,
                    },
                    ctx,
                );
            }
            SetActiveTabName(name) => self.set_active_tab_name(name, ctx),
            SetActiveTabColor(color) => self.set_tab_color(self.active_tab_index, *color, ctx),
            ToggleTabRightClickMenu { tab_index, anchor } => {
                self.toggle_tab_right_click_menu(*tab_index, *anchor, ctx)
            }
            ToggleVerticalTabsPaneContextMenu {
                tab_index,
                target,
                position,
            } => self.toggle_vertical_tabs_pane_context_menu(*tab_index, *target, *position, ctx),
            ToggleTabBarOverflowMenu => self.toggle_tab_bar_overflow_menu(ctx),
            ToggleBlockSnackbar => self.toggle_block_snackbar(ctx),
            ToggleWelcomeTips => self.toggle_welcome_tips_visiblity(ctx),
            CloseTab(index) => self.close_tab(*index, false, true, ctx),
            CloseActiveTab => self.close_tab(self.active_tab_index, false, true, ctx),
            CloseOtherTabs(index) => self.close_other_tabs(*index, false, ctx),
            CloseNonActiveTabs => self.close_other_tabs(self.active_tab_index, false, ctx),
            CloseTabsRight(index) => {
                self.close_tabs_direction(*index, TabMovement::Right, false, ctx)
            }
            CloseTabsRightActiveTab => {
                self.close_tabs_direction(self.active_tab_index, TabMovement::Right, false, ctx)
            }
            CloseTabGroup(group_id) => self.close_tab_group(*group_id, ctx),
            ToggleTabGroupCollapsed(group_id) => self.toggle_tab_group_collapsed(*group_id, ctx),
            RenameTabGroup(group_id) => self.rename_tab_group(*group_id, ctx),
            NewTabGroupFromTab(tab_index) => self.new_tab_group_from_tab(*tab_index, ctx),
            MoveTabToGroup {
                tab_index,
                group_id,
            } => self.move_tab_to_group(*tab_index, *group_id, ctx),
            RemoveTabFromGroup(tab_index) => self.remove_tab_from_group(*tab_index, ctx),
            ToggleTabGroupRightClickMenu { group_id, anchor } => {
                self.toggle_tab_group_right_click_menu(*group_id, *anchor, ctx)
            }
            UngroupTabs(group_id) => self.ungroup_tabs(*group_id, ctx),
            NewTabInGroup(group_id) => self.new_tab_in_group(*group_id, ctx),
            MoveTabGroupUp(group_id) => self.move_tab_group(*group_id, TabMovement::Left, ctx),
            MoveTabGroupDown(group_id) => self.move_tab_group(*group_id, TabMovement::Right, ctx),
            CloseTabsOutsideGroup(group_id) => self.close_tabs_outside_group(*group_id, ctx),
            CloseTabsAboveGroup(group_id) => self.close_tabs_above_group(*group_id, ctx),
            CloseTabsBelowGroup(group_id) => self.close_tabs_below_group(*group_id, ctx),
            AddDefaultTab => {
                let effective_mode = AISettings::as_ref(ctx).default_session_mode(ctx);
                match effective_mode {
                    DefaultSessionMode::TabConfig => {
                        let ai_settings = AISettings::as_ref(ctx);
                        if let Some(config) = ai_settings.resolved_default_tab_config(ctx) {
                            self.open_tab_config(config, ctx);
                        } else {
                            // Config missing or deleted — clear and fall through to Terminal.
                            AISettings::handle(ctx).update(ctx, |settings, ctx| {
                                report_if_error!(settings
                                    .default_session_mode_internal
                                    .set_value(DefaultSessionMode::Terminal, ctx));
                                report_if_error!(settings
                                    .default_tab_config_path
                                    .set_value(String::new(), ctx));
                            });
                            self.add_terminal_tab(false, ctx);
                        }
                    }
                    DefaultSessionMode::CloudAgent => {
                        self.add_ambient_agent_tab(ctx);
                    }
                    DefaultSessionMode::DockerSandbox => {
                        self.add_docker_sandbox_tab(ctx);
                    }
                    // Terminal and Agent are handled by the existing path
                    // (add_terminal_tab applies DefaultSessionMode::Agent internally).
                    DefaultSessionMode::Terminal | DefaultSessionMode::Agent => {
                        if FeatureFlag::WelcomeTab.is_enabled() {
                            self.add_welcome_tab(ctx);
                        } else {
                            self.add_terminal_tab(false, ctx);
                        }
                    }
                }
            }
            AddTerminalTab { hide_homepage } => {
                self.add_new_session_tab_internal_with_default_session_mode_behavior(
                    NewSessionSource::Tab,
                    Some(window_id),
                    None,
                    *hide_homepage,
                    DefaultSessionModeBehavior::Ignore,
                    ctx,
                );
                ctx.notify();
            }
            AddTabWithShell { shell, source } => {
                self.add_tab_with_shell(shell.clone(), *source, ctx)
            }
            AddGetStartedTab => self.add_get_started_tab(ctx),
            OpenNewSessionMenu { anchor } => self.open_new_session_dropdown_menu(*anchor, ctx),
            ToggleTabConfigsMenu => self.toggle_tab_configs_menu(ctx),
            ShowSessionConfigModal => self.show_session_config_modal(ctx),
            DismissSessionConfigTabConfigChip => {
                self.dismiss_session_config_tab_config_chip(ctx);
            }
            SaveCurrentTabAsNewConfig(tab_index) => {
                self.save_current_tab_as_new_config(*tab_index, ctx)
            }
            ToggleNewSessionMenu { anchor } => self.toggle_new_session_dropdown_menu(*anchor, ctx),
            SelectNewSessionMenuItem(new_session_menu_item) => {
                self.open_launch_config_from_menu(new_session_menu_item.clone(), ctx)
            }
            SelectTabConfig(tab_config) => {
                self.open_tab_config(tab_config.clone(), ctx);
            }
            OpenNewWorktreeModal => {
                let cwd = self
                    .active_session_view(ctx)
                    .and_then(|view| view.as_ref(ctx).pwd())
                    .map(PathBuf::from);
                self.new_worktree_modal.view.update(ctx, |modal, ctx| {
                    modal.body().update(ctx, |body, ctx| {
                        body.on_open(cwd, ctx);
                    });
                });
                self.new_worktree_modal.open();
                self.current_workspace_state.is_new_worktree_modal_open = true;
                ctx.notify();
            }
            OpenNewWorktreeRepoPicker => {
                self.open_repo_picker_for_new_worktree_modal(ctx);
            }
            OpenTabConfigErrorFile {
                #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
                path,
                toast_object_id,
            } => {
                #[cfg(feature = "local_fs")]
                {
                    let settings = EditorSettings::as_ref(ctx);
                    let target = resolve_file_target_with_editor_choice(
                        path,
                        *settings.open_code_panels_file_editor,
                        *settings.prefer_markdown_viewer,
                        *settings.open_file_layout,
                        None,
                    );
                    self.open_file_with_target(path.clone(), target, None, ctx);
                }
                self.dismiss_older_toasts(toast_object_id, ctx);
            }
            TabConfigSidecarMakeDefault {
                mode,
                tab_config_path,
                #[cfg_attr(not(feature = "local_tty"), allow(unused_variables))]
                shell,
            } => {
                AISettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.default_session_mode_internal.set_value(*mode, ctx));
                    if let Some(path) = tab_config_path {
                        report_if_error!(settings
                            .default_tab_config_path
                            .set_value(path.to_string_lossy().into_owned(), ctx));
                    }
                });
                #[cfg(feature = "local_tty")]
                if let Some(shell) = shell {
                    use crate::terminal::available_shells::AvailableShells;
                    AvailableShells::handle(ctx).update(ctx, |model, ctx| {
                        let _ = model.set_user_preferred_shell(shell.clone(), ctx);
                    });
                }
                self.close_new_session_dropdown_menu(ctx);
            }
            TabConfigSidecarEditConfig {
                #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
                path,
            } => {
                #[cfg(feature = "local_fs")]
                {
                    let settings = EditorSettings::as_ref(ctx);
                    let target = resolve_file_target_with_editor_choice(
                        path,
                        *settings.open_code_panels_file_editor,
                        *settings.prefer_markdown_viewer,
                        *settings.open_file_layout,
                        None,
                    );
                    self.open_file_with_target(path.clone(), target, None, ctx);
                }
                self.close_new_session_dropdown_menu(ctx);
            }
            TabConfigSidecarRemoveConfig { name, path } => {
                self.remove_tab_config_confirmation_dialog
                    .update(ctx, |dialog, ctx| {
                        dialog.set_config(name.clone(), path.clone());
                        ctx.notify();
                    });
                self.close_new_session_dropdown_menu(ctx);
                self.current_workspace_state
                    .is_remove_tab_config_dialog_open = true;
                ctx.focus(&self.remove_tab_config_confirmation_dialog);
                ctx.notify();
            }
            OpenSettingsFile => {
                let path = crate::settings::user_preferences_toml_file_path();
                crate::util::file::open_file_path_with_editor(None, path, None, ctx);
            }
            OpenNetworkLogPane => {
                self.open_network_log_pane(ctx);
            }
            OpenWorktreeInRepo { repo_path } => {
                self.open_worktree_in_repo(repo_path.clone(), ctx);
            }
            OpenWorktreeAddRepoPicker => {
                self.close_new_session_dropdown_menu(ctx);
                self.open_folder_picker_for_worktree_submenu(ctx);
            }
            AutoupdateFailureLink => self.open_autoupdate_failure_link(ctx),
            ApplyUpdate => self.apply_update(ctx),
            LogOut => {
                // Need to dispatch global action, or else we will not be able to retrieve
                // the currently active session in the log out modal.
                ctx.dispatch_global_action("app:maybe_log_out", ());
            }
            CopyVersion(version) => self.copy_version(version, ctx),
            DownloadNewVersion => self.download_new_version(ctx),
            ConfigureKeybindingSettings { keybinding_name } => {
                self.show_keyboard_settings(keybinding_name.as_deref(), ctx)
            }
            ShowSettings => self.show_settings(ctx),
            ShowSettingsPage(section) => self.show_settings_with_section(Some(*section), ctx),
            ShowSettingsPageWithSearch {
                search_query,
                section,
            } => self.show_settings_with_search(search_query, *section, ctx),
            ShowThemeChooser(mode) => self.show_theme_chooser(Some(*mode), ctx),
            ShowThemeChooserForActiveTheme => self.show_theme_chooser_for_active_theme(ctx),
            IncreaseFontSize => self.increase_font_size(ctx),
            DecreaseFontSize => self.decrease_font_size(ctx),
            ResetFontSize => self.reset_font_size(ctx),
            IncreaseZoom => self.increase_zoom(ctx),
            DecreaseZoom => self.decrease_zoom(ctx),
            ResetZoom => self.reset_zoom(ctx),
            OpenPalette {
                mode,
                source,
                query,
            } => self.open_palette_action(*mode, *source, query.as_deref(), ctx),
            TogglePalette {
                mode: palette_mode,
                source,
            } => self.toggle_palette(*palette_mode, *source, ctx),
            JoinSlack => self.join_slack(ctx),
            ViewUserDocs => self.view_user_docs(ctx),
            ViewLatestChangelog => self.view_latest_changelog(ctx),
            ViewPrivacyPolicy => self.view_privacy_policy(ctx),
            SendFeedback => self.send_feedback(ctx),
            #[cfg(not(target_family = "wasm"))]
            ViewLogs => self.view_logs(ctx),
            ChangeCursor(cursor) => self.change_cursor(*cursor, ctx),
            ToggleErrorUnderlining => self.toggle_error_underlining(ctx),
            ToggleSyntaxHighlighting => self.toggle_syntax_highlighting(ctx),
            CheckForUpdate => self.manual_check_for_update(ctx),
            SetA11yVerbosityLevel(verbosity) => self.set_a11y_verbosity(*verbosity, ctx),
            ToggleNotifications => self.toggle_notifications(ctx),
            ToggleTabColor { color, tab_index } => self.toggle_tab_color(*tab_index, *color, ctx),
            DispatchToSettingsTab(action) => {
                let window_id = ctx.window_id();
                ctx.dispatch_typed_action_for_view(window_id, self.settings_pane.id(), action)
            }
            OpenLink(link) => ctx.open_url(link),
            #[cfg(target_family = "wasm")]
            OpenLinkOnDesktop(url) => self.open_link_on_desktop(url, ctx),
            DumpDebugInfo => self.dump_debug_info(ctx),
            #[cfg(target_os = "macos")]
            InstallCLI => self.install_cli(ctx),
            #[cfg(target_os = "macos")]
            UninstallCLI => self.uninstall_cli(ctx),
            ToggleRecordingMode => self.toggle_recording_mode(ctx),
            ToggleInBandGenerators => self.toggle_in_band_generators(ctx),
            ToggleDebugNetworkStatus => self.toggle_debug_network_status(ctx),
            ToggleShowMemoryStats => self.toggle_show_memory_stats(ctx),
            ToggleResourceCenter => self.toggle_resource_center(ctx),
            ToggleUserMenu => self.toggle_user_menu(ctx),
            ToggleKeybindingsPage => self.toggle_keybindings_page(ctx),
            ShowCommandSearch(CommandSearchOptions {
                filter,
                init_content,
            }) => self.show_command_search(*filter, init_content, ctx),
            ToggleMouseReporting => self.toggle_mouse_reporting(ctx),
            ToggleScrollReporting => self.toggle_scroll_reporting(ctx),
            ToggleFocusReporting => self.toggle_focus_reporting(ctx),
            StartTabDrag => {
                // If we are renaming a tab, finish the rename before dragging.
                self.finish_tab_rename(ctx);
                self.current_workspace_state.is_tab_being_dragged = true;
            }
            StartGroupDrag(_group_id) => {
                self.finish_tab_group_rename(ctx);
            }
            DragGroup { group_id, position } => {
                self.on_group_drag(*group_id, *position, ctx);
            }
            DropGroup => {
                send_telemetry_from_ctx!(TelemetryEvent::DragAndDropTabGroup, ctx);
                ctx.notify();
            }
            ToggleLeftPanel => {
                let active_pane_group = self.active_tab_pane_group().clone();
                let was_open = active_pane_group.read(ctx, |pg, _| pg.left_panel_open);

                // Don't open the panel if no views are available.
                if !was_open && self.left_panel_views.is_empty() {
                    return;
                }

                let file_tree_active = self
                    .left_panel_view
                    .read(ctx, |lp, _| lp.is_file_tree_active());
                let warp_drive_active = self
                    .left_panel_view
                    .read(ctx, |lp, _| lp.is_warp_drive_active());

                self.toggle_left_panel(ctx);

                let is_open = active_pane_group.read(ctx, |pg, _| pg.left_panel_open);

                if !was_open && is_open {
                    self.left_panel_view.update(ctx, |left_panel, ctx| {
                        left_panel.focus_active_view_on_entry(ctx);
                    });

                    if file_tree_active {
                        send_telemetry_from_ctx!(
                            TelemetryEvent::FileTreeToggled {
                                source: FileTreeSource::LeftPanelToolbelt,
                                is_code_mode_v2: true,
                                cli_agent: None,
                            },
                            ctx
                        );
                    } else if warp_drive_active {
                        // Tools panel opened with Warp Drive as the active view
                        send_telemetry_from_ctx!(
                            TelemetryEvent::WarpDriveOpened {
                                source: WarpDriveSource::LeftPanelToolbelt,
                                is_code_mode_v2: true
                            },
                            ctx
                        );
                    }
                }
            }
            ToggleRightPanel => {}
            ToggleVerticalTabsPanel => {
                self.toggle_vertical_tabs_panel(ctx);
            }
            ToggleNotificationMailbox { select_first } => {
                if FeatureFlag::HOANotifications.is_enabled()
                    && *AISettings::as_ref(ctx).show_agent_notifications
                {
                    let opening = !self.current_workspace_state.is_notification_mailbox_open;
                    self.current_workspace_state.is_notification_mailbox_open = opening;
                    if let Some(stack) = &self.notification_toast_stack {
                        stack.update(ctx, |stack, ctx| stack.set_mailbox_open(opening, ctx));
                    }
                    if opening {
                        if self.tab_bar_mode(ctx).has_tab_bar() {
                            self.tab_bar_pinned_by_popup = true;
                        }
                        if let Some(view) = &self.notification_mailbox_view {
                            view.update(ctx, |mailbox, ctx| {
                                mailbox.reset_for_open(*select_first, ctx);
                            });
                            ctx.focus(view);
                        }
                    } else {
                        self.tab_bar_pinned_by_popup = false;
                        self.sync_window_button_visibility(ctx);
                        self.focus_active_tab(ctx);
                    }
                    ctx.notify();
                }
            }
            ToggleVerticalTabsSettingsPopup => {
                if FeatureFlag::VerticalTabs.is_enabled()
                    && *TabSettings::as_ref(ctx).use_vertical_tabs
                    && self.vertical_tabs_panel_open
                {
                    self.vertical_tabs_panel.show_settings_popup =
                        !self.vertical_tabs_panel.show_settings_popup;
                    ctx.notify();
                }
            }
            SetVerticalTabsDisplayGranularity(granularity) => {
                let granularity = *granularity;
                TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let _ = settings
                        .vertical_tabs_display_granularity
                        .set_value(granularity, ctx);
                });
                send_telemetry_from_ctx!(
                    VerticalTabsTelemetryEvent::DisplayOptionChanged(
                        VerticalTabsDisplayOption::DisplayGranularity(granularity),
                    ),
                    ctx
                );
                ctx.notify();
            }
            SetVerticalTabsTabItemMode(mode) => {
                let mode = *mode;
                TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let _ = settings.vertical_tabs_tab_item_mode.set_value(mode, ctx);
                });
                send_telemetry_from_ctx!(
                    VerticalTabsTelemetryEvent::DisplayOptionChanged(
                        VerticalTabsDisplayOption::TabItemMode(mode),
                    ),
                    ctx
                );
                ctx.notify();
            }
            SetVerticalTabsViewMode(mode) => {
                let mode = *mode;
                TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let _ = settings.vertical_tabs_view_mode.set_value(mode, ctx);
                });
                send_telemetry_from_ctx!(
                    VerticalTabsTelemetryEvent::DisplayOptionChanged(
                        VerticalTabsDisplayOption::ViewMode(mode),
                    ),
                    ctx
                );
                ctx.notify();
            }
            SetVerticalTabsPrimaryInfo(primary_info) => {
                let primary_info = *primary_info;
                TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let _ = settings
                        .vertical_tabs_primary_info
                        .set_value(primary_info, ctx);
                });
                send_telemetry_from_ctx!(
                    VerticalTabsTelemetryEvent::DisplayOptionChanged(
                        VerticalTabsDisplayOption::PrimaryInfo(primary_info),
                    ),
                    ctx
                );
                ctx.notify();
            }
            SetVerticalTabsCompactSubtitle(subtitle) => {
                let subtitle = *subtitle;
                TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let _ = settings
                        .vertical_tabs_compact_subtitle
                        .set_value(subtitle, ctx);
                });
                send_telemetry_from_ctx!(
                    VerticalTabsTelemetryEvent::DisplayOptionChanged(
                        VerticalTabsDisplayOption::CompactSubtitle(subtitle),
                    ),
                    ctx
                );
                ctx.notify();
            }
            ToggleVerticalTabsShowPrLink => {
                let new_value = TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let new_value = !*settings.vertical_tabs_show_pr_link.value();
                    let _ = settings
                        .vertical_tabs_show_pr_link
                        .set_value(new_value, ctx);
                    new_value
                });
                send_telemetry_from_ctx!(
                    VerticalTabsTelemetryEvent::DisplayOptionChanged(
                        VerticalTabsDisplayOption::ShowPrLink(new_value),
                    ),
                    ctx
                );
                ctx.notify();
            }
            ToggleVerticalTabsShowDiffStats => {
                let new_value = TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let new_value = !*settings.vertical_tabs_show_diff_stats.value();
                    let _ = settings
                        .vertical_tabs_show_diff_stats
                        .set_value(new_value, ctx);
                    new_value
                });
                send_telemetry_from_ctx!(
                    VerticalTabsTelemetryEvent::DisplayOptionChanged(
                        VerticalTabsDisplayOption::ShowDiffStats(new_value),
                    ),
                    ctx
                );
                ctx.notify();
            }
            ToggleVerticalTabsShowDetailsOnHover => {
                let new_value = TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                    let new_value = !*settings.vertical_tabs_show_details_on_hover.value();
                    let _ = settings
                        .vertical_tabs_show_details_on_hover
                        .set_value(new_value, ctx);
                    new_value
                });
                send_telemetry_from_ctx!(
                    VerticalTabsTelemetryEvent::DisplayOptionChanged(
                        VerticalTabsDisplayOption::ShowDetailsOnHover(new_value),
                    ),
                    ctx
                );
                ctx.notify();
            }
            ClosePanel => {
                if self.left_panel_view.is_self_or_child_focused(ctx) {
                    self.close_left_panel(ctx);
                }
            }
            OpenInExplorer { path } => {
                ctx.open_file_path_in_explorer(path);
            }
            OpenFilePath { path } => {
                ctx.open_file_path(path);
            }
            DragTab {
                tab_index,
                tab_position,
            } => self.on_tab_drag(*tab_index, *tab_position, ctx),
            DropTab => {
                let is_cross_window = CrossWindowTabDrag::as_ref(ctx).is_active();
                let handed_off_tab_index =
                    CrossWindowTabDrag::as_ref(ctx)
                        .handed_off_target()
                        .map(|_| {
                            CrossWindowTabDrag::as_ref(ctx)
                                .transferred_tab_index()
                                .unwrap_or(0)
                        });
                self.current_workspace_state.is_tab_being_dragged = false;
                // Clear the per-tab `detached` flag set by `on_tab_drag` when
                // the drag first left the tab bar. Skip the tab that has
                // already been handed off to another window — its source-side
                // cleanup runs below via `handle_drop_result`.
                for (i, tab) in self.tabs.iter_mut().enumerate() {
                    if handed_off_tab_index == Some(i) {
                        continue;
                    }
                    tab.detached = false;
                }
                send_telemetry_from_ctx!(TelemetryEvent::DragAndDropTab, ctx);
                if is_cross_window {
                    let drop_result =
                        CrossWindowTabDrag::handle(ctx).update(ctx, |drag, ctx| drag.on_drop(ctx));
                    self.handle_drop_result(drop_result, ctx);
                    // Don't clear any pending source-close here; that happens
                    // from `Workspace::on_window_closed` once the source /
                    // preview window actually closes. See the field doc on
                    // `CrossWindowTabDrag::pending_source_window_closes`.
                }
            }
            CopyAccessTokenToClipboard => {
                // Blocking is ok here only because this action is only registered in dev and local
                // builds to aid in debugging and development.
                let access_token =
                    riftui::r#async::block_on(self.server_api.get_or_refresh_access_token());
                if let Ok(token) = access_token {
                    if let Some(bearer) = token.bearer_token() {
                        ctx.clipboard().write(ClipboardContent::plain_text(bearer));
                    }
                }
            }
            CopyTextToClipboard(text) => {
                ctx.clipboard()
                    .write(ClipboardContent::plain_text(text.to_string()));
            }
            DismissWorkspaceBanner(banner_type) => self.dismiss_workspace_banner(ctx, banner_type),
            Crash => {
                #[cfg(feature = "crash_reporting")]
                crate::crash_reporting::crash();
            }
            Panic => {
                panic!("WorkspaceAction::Panic triggered from command palette");
            }
            DumpHeapProfile => {
                #[cfg(feature = "dhat_heap_profiling")]
                crate::profiling::dump_dhat_heap_profile();
            }
            OpenViewTreeDebugWindow => {
                let window_id = ctx.window_id();
                ctx.open_view_tree_debug_window(window_id);
            }
            ToggleSyncAllTerminalInputsInAllTabs => {
                let enabled = SyncedInputState::handle(ctx).update(ctx, |status, _| {
                    status.toggle_sync_all_terminal_inputs_in_all_tabs(window_id);

                    status.is_syncing_all_inputs(window_id)
                });
                let verb = if enabled { "enabled" } else { "disabled" };
                let mut message = format!("You {verb} synchronized inputs in all tabs.");
                if let Some(keystroke) = keybinding_name_to_keystroke(
                    "workspace:toggle_sync_all_terminal_inputs_in_all_tabs",
                    ctx,
                ) {
                    let _ = write!(message, " Press {} to undo.", keystroke.displayed());
                }
                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast = DismissibleToast::default(message);
                    view.add_ephemeral_toast(new_toast, ctx);
                });

                send_telemetry_from_ctx!(
                    TelemetryEvent::ToggleSyncAllPanesInAllTabs { enabled },
                    ctx
                );

                self.process_updated_sync_state(ctx);
            }
            ToggleSyncTerminalInputsInTab => {
                let enabled = SyncedInputState::handle(ctx).update(ctx, |status, _| {
                    let current_pane_group_id = self.active_tab_pane_group().id();

                    status.toggle_sync_terminal_inputs_in_tab(
                        current_pane_group_id,
                        self.all_pane_group_ids(),
                        self.tab_count(),
                        window_id,
                    );

                    status.should_sync_this_pane_group(current_pane_group_id, window_id)
                });
                let verb = if enabled { "enabled" } else { "disabled" };
                let mut message = format!("You {verb} synchronized inputs in this tab.");
                if let Some(keystroke) = keybinding_name_to_keystroke(
                    "workspace:toggle_sync_terminal_inputs_in_tab",
                    ctx,
                ) {
                    let _ = write!(message, " Press {} to undo.", keystroke.displayed());
                }
                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast = DismissibleToast::default(message);
                    view.add_ephemeral_toast(new_toast, ctx);
                });

                send_telemetry_from_ctx!(TelemetryEvent::ToggleSyncAllPanesInTab { enabled }, ctx);

                self.process_updated_sync_state(ctx);
            }
            DisableTerminalInputSync => {
                SyncedInputState::handle(ctx).update(ctx, |status, _| {
                    status.disable_sync_terminal_inputs(window_id);
                });

                self.process_updated_sync_state(ctx);

                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast =
                        DismissibleToast::success("Disabled all synchronized inputs.".to_string());
                    view.add_ephemeral_toast(new_toast, ctx);
                });
                send_telemetry_from_ctx!(TelemetryEvent::DisableInputSync, ctx);
            }
            OpenHeaderToolbarEditor => {
                self.open_header_toolbar_editor(ctx);
            }
            ShowHeaderToolbarContextMenu { position } => {
                self.show_header_toolbar_context_menu(*position, ctx);
            }
            ReopenClosedSession => {
                // While we could grab the UndoCloseStack singleton entity and
                // directly call undo_close(), it would fail when attempting to
                // restore a closed tab as we would attempt to update the
                // workspace while we are currently updating the workspace.
                // Instead, we use a global action to ensure we don't try to
                // perform nested updates on the workspace.
                ctx.dispatch_global_action("app:undo_close", ());
            }
            AddWindow => {
                ctx.dispatch_global_action("root_view:open_new", ());
            }
            AddWindowWithShell { shell } => {
                ctx.dispatch_global_action("root_view:open_new_with_shell", Some(shell.clone()));
            }
            NavigatePrevPaneOrPanel => {
                self.navigate_pane_or_panel(PanePanelDirection::Prev, ctx);
            }
            NavigateNextPaneOrPanel => {
                self.navigate_pane_or_panel(PanePanelDirection::Next, ctx);
            }
            FocusLeftPanel => self.focus_left_panel(ctx),
            FocusRightPanel => self.focus_right_panel(ctx),
            TerminateApp => {
                ctx.terminate_app(TerminationMode::Cancellable, None);
            }
            CloseWindow => {
                if ContextFlag::CloseWindow.is_enabled() {
                    ctx.close_window();
                }
            }
            RunCommand(code) => {
                let command = code.trim().to_string();
                self.insert_in_input(&command, true, true, false, ctx);
                ctx.notify();
            }
            InsertInInput {
                content,
                replace_buffer,
                ensure_agent_mode,
            } => {
                self.insert_in_input(content, *replace_buffer, false, *ensure_agent_mode, ctx);
                ctx.notify();
            }
            #[cfg(all(enable_crash_recovery, target_os = "linux"))]
            DismissWaylandCrashRecoveryBannerAndOpenLink => {
                self.dismiss_workspace_banner(ctx, &WorkspaceBanner::WaylandCrashRecovery);
                ctx.open_url("https://docs.warp.dev/terminal/more-features/linux#native-wayland");
            }
            TabHoverWidthStart { width } => {
                // Store the fixed width value for the tab to maintain consistent size during hover
                self.tab_fixed_width = Some(*width);
                ctx.notify();
            }
            TabHoverWidthEnd => {
                // Clear the stored width when hover ends
                self.tab_fixed_width = None;
                ctx.notify();
            }
            FocusPane(locator) => {
                self.focus_pane(*locator, ctx);
            }
            ScrollToSettingsWidget { page, widget_id } => {
                self.open_settings_pane(Some(*page), None, ctx);
                self.settings_pane.update(ctx, |settings, ctx| {
                    settings.scroll_to_settings_widget(*page, widget_id, ctx);
                });
                ctx.notify();
            }
            OpenRepository { path } => {
                self.open_repository(path.as_deref(), ctx);
            }
            OpenTabConfigRepoPicker { param_index } => {
                self.open_repo_picker_for_tab_config_modal(*param_index, ctx);
            }
            #[cfg(target_os = "macos")]
            SampleProcess => {
                let pid = process::id();
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let output_path = env::temp_dir()
                    .join(format!("warp_sample_{timestamp}.txt"))
                    .display()
                    .to_string();

                self.toast_stack.update(ctx, |view, ctx| {
                    view.add_ephemeral_toast(
                        DismissibleToast::default("Sampling process for 3 seconds...".to_string()),
                        ctx,
                    );
                });

                let output_path_clone = output_path.clone();
                ctx.spawn(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            // `sample` is the macOS CLI that Activity Monitor uses for "Sample Process".
                            Command::new("sample")
                                .args([
                                    // process ID
                                    &pid.to_string(),
                                    // duration in seconds
                                    "3",
                                    // sampling interval in milliseconds
                                    "1",
                                    // write output to file
                                    "-file",
                                    &output_path_clone,
                                ])
                                .output()
                        })
                        .await
                    },
                    move |me, result, ctx| {
                        let message = match result {
                            Ok(Ok(output)) if output.status.success() => {
                                ctx.open_file_path_in_explorer(Path::new(&output_path));

                                #[cfg(feature = "crash_reporting")]
                                if ChannelState::channel().is_dogfood() {
                                    // For dogfood process samples, we raise a sentry warning with the sample attatched.
                                    // We do this so that our performance bot can then read through the performance logs
                                    // in sentry and write up a report of findings/possible optimizations.
                                    if let Ok(sample_data) = fs::read(&output_path) {
                                        let filename = Path::new(&output_path)
                                            .file_name()
                                            .map(|f| f.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "process_sample.txt".to_string());
                                        let attachment = Attachment {
                                            buffer: sample_data,
                                            filename,
                                            ty: Some(AttachmentType::Attachment),
                                            ..Default::default()
                                        };
                                        sentry::with_scope(
                                            |scope| {
                                                scope.add_attachment(attachment);
                                            },
                                            || {
                                                sentry::capture_message(
                                                    "[FOR PERFORMANCE BOT] Dev took performance sample with results: ",
                                                    sentry::Level::Warning,
                                                )
                                            },
                                        );
                                    }
                                }

                                format!("Process sample saved to {output_path}")
                            }
                            Ok(Ok(output)) => {
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                log::error!("sample command failed ({}): {stderr}", output.status);
                                "Failed to sample process (check logs)".to_string()
                            }
                            Ok(Err(io_err)) => {
                                log::error!("Failed to run sample command: {io_err}");
                                "Failed to sample process (check logs)".to_string()
                            }
                            Err(join_err) => {
                                log::error!("Sample task panicked: {join_err}");
                                "Failed to sample process (check logs)".to_string()
                            }
                        };
                        me.toast_stack.update(ctx, |view, ctx| {
                            view.add_ephemeral_toast(DismissibleToast::default(message), ctx);
                        });
                    },
                );
            }
            OpenLightbox {
                images,
                initial_index,
            } => {
                let params = LightboxParams {
                    images: images.clone(),
                    initial_index: *initial_index,
                };
                if let Some(handle) = &self.lightbox_view {
                    handle.update(ctx, |view, ctx| view.update_params(params, ctx));
                } else {
                    let handle = ctx.add_typed_action_view(|ctx| LightboxView::new(params, ctx));
                    ctx.subscribe_to_view(&handle, |me, _, event, ctx| match event {
                        LightboxViewEvent::Close => {
                            me.lightbox_view = None;
                            me.focus_active_tab(ctx);
                            ctx.notify();
                        }
                        LightboxViewEvent::FocusLost => {
                            // Focus already moved elsewhere; just tear down the view.
                            me.lightbox_view = None;
                            ctx.notify();
                        }
                    });
                    ctx.focus(&handle);
                    self.lightbox_view = Some(handle);
                }
                ctx.notify();
            }
            UpdateLightboxImage { index, image } => {
                if let Some(handle) = &self.lightbox_view {
                    handle.update(ctx, |view, ctx| {
                        view.update_image_at(*index, image.clone(), ctx);
                    });
                    ctx.notify();
                }
            }
            SyncTrafficLights => {
                self.sync_window_button_visibility(ctx);
            }
        };
        if action.should_save_app_state_on_action() {
            ctx.dispatch_global_action("workspace:save_app", ());
        }
    }
}

impl View for Workspace {
    fn ui_name() -> &'static str {
        "Workspace"
    }

    fn self_or_child_interacted_with(&self, ctx: &mut ViewContext<Self>) {
        self.sync_window_button_visibility(ctx);
    }

    fn keymap_context(&self, app: &AppContext) -> riftui::keymap::Context {
        let mut context = Self::default_keymap_context();

        if NetworkStatus::as_ref(app).is_online() {
            context.set.insert("IsOnline");
        }

        if AISettings::as_ref(app).is_any_ai_enabled(app) {
            context.set.insert(flags::IS_ANY_AI_ENABLED);
        }

        if AISettings::as_ref(app).is_active_ai_enabled(app) {
            context.set.insert(flags::IS_ACTIVE_AI_ENABLED);
        }
        if AISettings::as_ref(app).is_voice_input_enabled(app)
            && UserWorkspaces::as_ref(app).is_voice_enabled()
        {
            context.set.insert(flags::IS_VOICE_INPUT_ENABLED);
        }

        if self
            .active_tab_pane_group()
            .as_ref(app)
            .any_pane_being_dragged(app)
        {
            context.set.insert("Workspace_PaneDragging");
        }

        // TODO: This is temporary. We currently check if any code pane is open where it should
        // really be whether the code pane is opened and focused.
        if self
            .active_tab_pane_group()
            .as_ref(app)
            .pane_ids()
            .any(|id| id.is_code_pane())
        {
            context.set.insert("Workspace_TextOpen");
        }

        if matches!(
            autoupdate::get_update_state(app),
            AutoupdateStage::UpdateReady { .. } | AutoupdateStage::UpdatedPendingRestart { .. }
        ) {
            context.set.insert("AutoupdateState_UpdateReady");
        }

        if matches!(
            *AccessibilitySettings::as_ref(app).a11y_verbosity,
            AccessibilityVerbosity::Verbose
        ) {
            context.set.insert("AccessibilityVerbosity_Verbose");
        }

        if ContextFlag::CloseWindow.is_enabled() {
            context.set.insert("Workspace_CloseWindow");
        }

        match self.tab_count() {
            0 => {
                debug_assert!(false, "Should always be at least one tab");
            }
            1 => {
                context.set.insert("Workspace_SingleTab");
            }
            n => {
                context.set.insert("Workspace_MultipleTabs");
                if self.active_tab_index == 0 {
                    context.set.insert("Workspace_LeftmostTabActive");
                } else if self.active_tab_index == n - 1 {
                    context.set.insert("Workspace_RightmostTabActive");
                }
            }
        };

        if AISettings::as_ref(app).is_any_ai_enabled(app)
            && *AISettings::as_ref(app).show_conversation_history
        {
            context.set.insert(flags::SHOW_CONVERSATION_HISTORY);
        }

        if *CodeSettings::as_ref(app).show_project_explorer {
            context.set.insert(flags::SHOW_PROJECT_EXPLORER);
        }
        if *CodeSettings::as_ref(app).show_global_search {
            context.set.insert(flags::SHOW_GLOBAL_SEARCH);
        }

        if self.team_uid(app).is_some() {
            context.set.insert("WarpDrive_BelongsToTeam");
        }

        if self.auth_state.is_anonymous_or_logged_out() {
            context.set.insert("IsAnonymousUser");
        }

        self.add_toggle_setting_context_flags(app, &mut context);

        let sync_state = SyncedInputState::as_ref(app);

        if sync_state.is_syncing_all_inputs(self.window_id) {
            context.set.insert(flags::SYNC_ALL_TABS_FLAG);
        } else if sync_state
            .is_syncing_all_panes_in_pane_group(self.window_id, self.active_tab_pane_group().id())
        {
            context.set.insert(flags::SYNC_ALL_PANES_IN_CURRENT_TAB);
        }

        let is_universal_developer_input_enabled =
            InputSettings::as_ref(app).is_universal_developer_input_enabled(app);

        if is_universal_developer_input_enabled {
            context.set.insert(flags::UNIVERSAL_DEVELOPER_INPUT_ENABLED);
        }

        let default_terminal = DefaultTerminal::as_ref(app);
        if default_terminal.is_warp_default() {
            context.set.insert(flags::RIFT_IS_DEFAULT_TERMINAL);
        }

        if FeatureFlag::DebugMode.is_enabled() {
            let debug_settings = DebugSettings::as_ref(app);
            if *debug_settings.recording_mode.value() {
                context.set.insert(flags::RECORDING_MODE_FLAG);
            }
            if *debug_settings
                .are_in_band_generators_for_all_sessions_enabled
                .value()
            {
                context.set.insert(flags::IN_BAND_GENERATORS_FLAG);
            }

            let network_status = NetworkStatus::as_ref(app);
            if network_status.is_online() {
                context.set.insert(flags::DEBUG_NETWORK_ONLINE_FLAG);
            }

            if debug_settings.should_show_memory_stats() {
                context.set.insert(flags::DEBUG_SHOW_MEMORY_STATS_FLAG);
            }
        }

        // We only want to register the temporary changelog shortcut if the changelog toast is
        // visible.
        // There is a collision between the default shortcut and `/open-repo`, so durable changelog
        // access lives in the command palette and slash-command menu instead.
        if self.update_toast_stack.as_ref(app).has_toasts() {
            context.set.insert("UpdateToastVisible");
        }

        if let Some(terminal_view) = self
            .active_tab_pane_group()
            .as_ref(app)
            .focused_session_view(app)
        {
            let terminal_view = terminal_view.as_ref(app);
            if terminal_view.is_long_running() {
                context.set.insert("LongRunningCommand");
            }

            if FeatureFlag::AgentView.is_enabled() {
                let agent_view_state = terminal_view
                    .agent_view_controller()
                    .as_ref(app)
                    .agent_view_state();
                if agent_view_state.is_fullscreen() {
                    context.set.insert(flags::ACTIVE_AGENT_VIEW);
                } else if agent_view_state.is_inline() {
                    context.set.insert(flags::ACTIVE_INLINE_AGENT_VIEW);
                }
            }
        }

        #[cfg(target_family = "wasm")]
        if self.is_conversation_transcript_viewer_focused(app) {
            context.set.insert("Workspace_CloudConversationWebViewer");
        }

        context
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let tab_bar_mode = self.tab_bar_mode(app);

        // For WASM simplified tab bar views (Warp Drive objects, shared sessions, conversation transcripts),
        // we render the tab bar outside of panels so that the details panel only affects content below the tab bar.
        cfg_if::cfg_if! {
            if #[cfg(target_family = "wasm")] {
                let use_simplified_wasm_tab_bar = self.get_simplified_wasm_tab_bar_content(app).is_some();
            } else {
                let use_simplified_wasm_tab_bar = false;
            }
        }

        let panels = if use_simplified_wasm_tab_bar {
            // For the simplified WASM tab bar, we want to render the tab bar on top of all other content
            // so that content being added/moved around in the workspace (for example the details panel being toggled)
            // does not affect the tab.
            let mut outer_column = Flex::column();
            if tab_bar_mode == ShowTabBar::Stacked {
                outer_column.add_child(self.render_tab_bar(self.tab_fixed_width, appearance, app));
            }
            let content = self.render_banner_and_active_tab(app, appearance);
            // Hide the vertical tab rail for simplified WASM views (notebooks, shared sessions, etc.)
            let panels_row = self.render_panels(app, Shrinkable::new(1.0, content).finish(), true);
            outer_column.add_child(Shrinkable::new(1.0, panels_row).finish());
            Container::new(outer_column.finish())
                .with_background(util::get_terminal_background_fill(self.window_id, app))
                .finish()
        } else {
            let mut outer_column = Flex::column();
            if tab_bar_mode == ShowTabBar::Stacked {
                outer_column.add_child(self.render_tab_bar(self.tab_fixed_width, appearance, app));
            }
            let content = self.render_banner_and_active_tab(app, appearance);
            let panels_row = self.render_panels(app, Shrinkable::new(1.0, content).finish(), false);
            outer_column.add_child(Shrinkable::new(1.0, panels_row).finish());
            Container::new(outer_column.finish())
                .with_background(util::get_terminal_background_fill(self.window_id, app))
                .finish()
        };
        let mut stack = Stack::new();

        #[cfg(target_family = "wasm")]
        {
            let pane_group = self.active_tab_pane_group().as_ref(app);
            if riftui::platform::wasm::is_mobile_device() && pane_group.left_panel_open {
                let scrim = Rect::new()
                    .with_background(Fill::Solid(ColorU::new(
                        0,
                        0,
                        0,
                        MOBILE_OVERLAY_SCRIM_ALPHA,
                    )))
                    .finish();
                let clickable_scrim = EventHandler::new(scrim)
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(WorkspaceAction::ToggleLeftPanel);
                        DispatchEventResult::StopPropagation
                    })
                    .finish();
                stack.add_positioned_overlay_child(
                    Percentage::width(1.0 - MOBILE_OVERLAY_PANEL_WIDTH_RATIO, clickable_scrim)
                        .finish(),
                    OffsetPositioning::offset_from_save_position_element(
                        TAB_BAR_POSITION_ID,
                        vec2f(0., 0.),
                        PositionedElementOffsetBounds::WindowBySize,
                        PositionedElementAnchor::BottomRight,
                        ChildAnchor::TopRight,
                    ),
                );

                let panel_content = Container::new(ChildView::new(&self.left_panel_view).finish())
                    .with_background(appearance.theme().surface_1())
                    .finish();
                stack.add_positioned_overlay_child(
                    Percentage::width(MOBILE_OVERLAY_PANEL_WIDTH_RATIO, panel_content).finish(),
                    OffsetPositioning::offset_from_save_position_element(
                        TAB_BAR_POSITION_ID,
                        vec2f(0., 0.),
                        PositionedElementOffsetBounds::WindowBySize,
                        PositionedElementAnchor::BottomLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        }

        stack.add_child(
            Container::new(panels)
                .with_uniform_padding(WORKSPACE_PADDING)
                .finish(),
        );

        if !use_simplified_wasm_tab_bar
            && FeatureFlag::VerticalTabs.is_enabled()
            && *TabSettings::as_ref(app).use_vertical_tabs
            && self.vertical_tabs_panel_open
            && self.vertical_tabs_panel.show_settings_popup
        {
            stack.add_positioned_overlay_child(
                Dismiss::new(render_settings_popup(&self.vertical_tabs_panel, app))
                    .prevent_interaction_with_other_elements()
                    .on_dismiss(|ctx, _| {
                        ctx.dispatch_typed_action(WorkspaceAction::ToggleVerticalTabsSettingsPopup);
                    })
                    .finish(),
                OffsetPositioning::offset_from_save_position_element(
                    VERTICAL_TABS_SETTINGS_BUTTON_POSITION_ID,
                    vec2f(0., 4.),
                    PositionedElementOffsetBounds::WindowByPosition,
                    PositionedElementAnchor::BottomLeft,
                    ChildAnchor::TopLeft,
                ),
            );
        }

        if FeatureFlag::VerticalTabs.is_enabled()
            && *TabSettings::as_ref(app).use_vertical_tabs
            && self.vertical_tabs_panel_open
        {
            if let Some(vertical_tabs::DetailSidecarOverlay {
                anchor_position_id,
                offset,
                bounds,
                parent_anchor,
                child_anchor,
                sidecar,
            }) = render_detail_sidecar(
                &self.vertical_tabs_panel,
                self,
                Self::tabs_panel_side(&TabSettings::as_ref(app).header_toolbar_chip_selection),
                app,
            ) {
                stack.add_positioned_overlay_child(
                    sidecar,
                    OffsetPositioning::offset_from_save_position_element(
                        &anchor_position_id,
                        offset,
                        bounds,
                        parent_anchor,
                        child_anchor,
                    ),
                );
            }
        }

        // Transcript details panel overlay (right side, mobile only)
        #[cfg(target_family = "wasm")]
        if riftui::platform::wasm::is_mobile_device()
            && self
                .current_workspace_state
                .is_transcript_details_panel_open
        {
            // Dimming scrim on the left (10% width); tapping closes the panel
            let scrim = Rect::new()
                .with_background(Fill::Solid(ColorU::new(
                    0,
                    0,
                    0,
                    MOBILE_OVERLAY_SCRIM_ALPHA,
                )))
                .finish();
            let clickable_scrim = EventHandler::new(scrim)
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(
                        WorkspaceAction::ToggleConversationTranscriptDetailsPanel,
                    );
                    DispatchEventResult::StopPropagation
                })
                .finish();
            stack.add_positioned_overlay_child(
                Percentage::width(1.0 - MOBILE_OVERLAY_PANEL_WIDTH_RATIO, clickable_scrim).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    TAB_BAR_POSITION_ID,
                    vec2f(0., 0.),
                    PositionedElementOffsetBounds::WindowBySize,
                    PositionedElementAnchor::BottomLeft,
                    ChildAnchor::TopLeft,
                ),
            );

            // Details panel overlay (90% width, positioned on the right)
            let panel_content = ChildView::new(&self.transcript_details_panel).finish();
            stack.add_positioned_overlay_child(
                Percentage::width(MOBILE_OVERLAY_PANEL_WIDTH_RATIO, panel_content).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    TAB_BAR_POSITION_ID,
                    vec2f(0., 0.),
                    PositionedElementOffsetBounds::WindowBySize,
                    PositionedElementAnchor::BottomRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        if let Some(position) = self.show_header_toolbar_context_menu {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.header_toolbar_context_menu).finish(),
                OffsetPositioning::offset_from_parent(
                    position,
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopLeft,
                    ChildAnchor::TopLeft,
                ),
            );
        }

        match tab_bar_mode {
            ShowTabBar::Stacked => (), // The tab bar was rendered in the content column.
            ShowTabBar::Hidden => {
                // Hide the tab bar, but include a hover area.
                stack.add_positioned_child(
                    self.render_tab_bar_hover_area(),
                    OffsetPositioning::offset_from_parent(
                        Vector2F::zero(),
                        ParentOffsetBounds::WindowByPosition,
                        ParentAnchor::TopLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        }

        // If the tab bar is being shown in "stacked" mode, we want to render
        // the traffic lights relative to the full workspace, so they appear
        // in the top-right corner even if a right-side panel is open.
        if tab_bar_mode == ShowTabBar::Stacked {
            self.maybe_render_traffic_lights(&mut stack, app);
        }

        // Conditionally render tab bar menus. These must be added after the tab bar itself
        // (whether stacked inside panels or as an overlay) so that tab bar button save
        // positions are committed to the position cache before these menus read them.
        if tab_bar_mode.has_tab_bar() && self.show_tab_bar_overflow_menu {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.tab_bar_overflow_menu).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    "tab_bar_overflow_button",
                    vec2f(0., 10.),
                    PositionedElementOffsetBounds::Unbounded,
                    PositionedElementAnchor::BottomRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        if let Some((tab_idx, right_click_menu_anchor)) = self.show_tab_right_click_menu {
            let is_vertical = FeatureFlag::VerticalTabs.is_enabled()
                && *TabSettings::as_ref(app).use_vertical_tabs
                && self.vertical_tabs_panel_open;
            if tab_bar_mode.has_tab_bar() || is_vertical {
                let positioning = match (is_vertical, right_click_menu_anchor) {
                    (true, TabContextMenuAnchor::VerticalTabsKebab) => {
                        // Anchor depends on which side the tabs panel is configured on.
                        let tabs_side = Self::tabs_panel_side(
                            &TabSettings::as_ref(app).header_toolbar_chip_selection,
                        );
                        let (anchor, child_anchor) = if tabs_side == PanelPosition::Left {
                            (PositionedElementAnchor::BottomLeft, ChildAnchor::TopLeft)
                        } else {
                            (PositionedElementAnchor::BottomRight, ChildAnchor::TopRight)
                        };
                        Some(OffsetPositioning::offset_from_save_position_element(
                            vertical_tabs::vtab_action_buttons_position_id(tab_idx),
                            vec2f(0., 4.),
                            PositionedElementOffsetBounds::WindowByPosition,
                            anchor,
                            child_anchor,
                        ))
                    }
                    (true, TabContextMenuAnchor::Pointer(position)) => {
                        Some(OffsetPositioning::offset_from_parent(
                            position,
                            ParentOffsetBounds::WindowByPosition,
                            ParentAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        ))
                    }
                    (false, TabContextMenuAnchor::Pointer(position)) => {
                        Some(OffsetPositioning::offset_from_parent(
                            position,
                            ParentOffsetBounds::Unbounded,
                            ParentAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        ))
                    }
                    (false, TabContextMenuAnchor::VerticalTabsKebab) => None,
                };
                if let Some(positioning) = positioning {
                    stack.add_positioned_overlay_child(
                        ChildView::new(&self.tab_right_click_menu).finish(),
                        positioning,
                    );
                }

                // Sidecar menu for the "Move to group" submenu parent. Mirrors
                // the new-session sidecar's overflow-aware left/right anchoring.
                if self.show_move_to_group_sidecar {
                    let sidecar_element = SavePosition::new(
                        ChildView::new(&self.move_to_group_sidecar_menu).finish(),
                        MOVE_TO_GROUP_SIDECAR_POSITION_ID,
                    )
                    .finish();

                    let render_left = self.should_render_sidecar_left(
                        MOVE_TO_GROUP_LABEL,
                        MOVE_TO_GROUP_SIDECAR_WIDTH,
                        app,
                    );
                    let (offset, parent_anchor, child_anchor) = if render_left {
                        (
                            vec2f(-4., 0.),
                            PositionedElementAnchor::TopLeft,
                            ChildAnchor::TopRight,
                        )
                    } else {
                        (
                            vec2f(4., 0.),
                            PositionedElementAnchor::TopRight,
                            ChildAnchor::TopLeft,
                        )
                    };

                    stack.add_positioned_overlay_child(
                        sidecar_element,
                        OffsetPositioning::offset_from_save_position_element(
                            MOVE_TO_GROUP_LABEL,
                            offset,
                            PositionedElementOffsetBounds::WindowByPosition,
                            parent_anchor,
                            child_anchor,
                        ),
                    );
                }
            }
        }

        // Tab group more-options menu (reuses the `tab_right_click_menu` view).
        if let Some((group_id, anchor)) = self.show_tab_group_right_click_menu {
            let is_vertical = FeatureFlag::VerticalTabs.is_enabled()
                && *TabSettings::as_ref(app).use_vertical_tabs
                && self.vertical_tabs_panel_open;
            if is_vertical {
                let positioning = match anchor {
                    TabContextMenuAnchor::VerticalTabsKebab => {
                        let tabs_side = Self::tabs_panel_side(
                            &TabSettings::as_ref(app).header_toolbar_chip_selection,
                        );
                        let (anchor, child_anchor) = if tabs_side == PanelPosition::Left {
                            (PositionedElementAnchor::BottomLeft, ChildAnchor::TopLeft)
                        } else {
                            (PositionedElementAnchor::BottomRight, ChildAnchor::TopRight)
                        };
                        OffsetPositioning::offset_from_save_position_element(
                            vertical_tabs::vtab_group_kebab_position_id(group_id),
                            vec2f(0., 4.),
                            PositionedElementOffsetBounds::WindowByPosition,
                            anchor,
                            child_anchor,
                        )
                    }
                    TabContextMenuAnchor::Pointer(position) => {
                        OffsetPositioning::offset_from_parent(
                            position,
                            ParentOffsetBounds::WindowByPosition,
                            ParentAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        )
                    }
                };
                stack.add_positioned_overlay_child(
                    ChildView::new(&self.tab_right_click_menu).finish(),
                    positioning,
                );
            }
        }

        // Render the new session dropdown menu. This is outside the tab bar visibility
        // gate because it can also be opened from the vertical tabs panel.
        if let Some(menu_anchor) = self.show_new_session_dropdown_menu {
            let is_vertical = FeatureFlag::VerticalTabs.is_enabled()
                && *TabSettings::as_ref(app).use_vertical_tabs
                && self.vertical_tabs_panel_open;

            match (is_vertical, menu_anchor) {
                (true, NewSessionMenuAnchor::AddTabButton(_)) => {
                    // Anchor the menu below the vertical-tabs + button. The anchor
                    // side mirrors which side the tabs panel itself is on, so the
                    // menu always expands inward and stays inside the window.
                    let tabs_side = Self::tabs_panel_side(
                        &TabSettings::as_ref(app).header_toolbar_chip_selection,
                    );
                    let (anchor, child_anchor) = match tabs_side {
                        PanelPosition::Left => {
                            (PositionedElementAnchor::BottomLeft, ChildAnchor::TopLeft)
                        }
                        PanelPosition::Right => {
                            (PositionedElementAnchor::BottomRight, ChildAnchor::TopRight)
                        }
                    };
                    stack.add_positioned_overlay_child(
                        ChildView::new(&self.new_session_dropdown_menu).finish(),
                        OffsetPositioning::offset_from_save_position_element(
                            vertical_tabs::VERTICAL_TABS_ADD_TAB_POSITION_ID,
                            vec2f(0., 4.),
                            PositionedElementOffsetBounds::WindowBySize,
                            anchor,
                            child_anchor,
                        ),
                    );
                }
                (true, NewSessionMenuAnchor::Pointer(_))
                | (false, NewSessionMenuAnchor::AddTabButton(_))
                | (false, NewSessionMenuAnchor::Pointer(_)) => {
                    // TODO(CORE-2300): In the new version of the shell selector, this is not a
                    // context menu but a dropdown. Since it is quite wide, we need to reposition
                    // it so it does not render outside the bounds of the window.
                    let bounds = if FeatureFlag::ShellSelector.is_enabled() {
                        ParentOffsetBounds::WindowByPosition
                    } else {
                        ParentOffsetBounds::Unbounded
                    };
                    stack.add_positioned_overlay_child(
                        ChildView::new(&self.new_session_dropdown_menu).finish(),
                        OffsetPositioning::offset_from_parent(
                            menu_anchor.position(),
                            bounds,
                            ParentAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        ),
                    );
                }
            }

            // Sidecar menu for submenu parents (New worktree config).
            if self.show_new_session_sidecar {
                let anchor_label = self.new_session_dropdown_menu.read(app, |menu, _| {
                    menu.hovered_index().and_then(|idx| {
                        menu.items().get(idx).and_then(|item| match item {
                            MenuItem::Item(fields) => Some(fields.label().to_string()),
                            _ => None,
                        })
                    })
                });

                if let Some(anchor_label) = anchor_label {
                    let sidecar_element = SavePosition::new(
                        ChildView::new(&self.new_session_sidecar_menu).finish(),
                        NEW_SESSION_SIDECAR_POSITION_ID,
                    )
                    .finish();

                    let render_left = self.should_render_sidecar_left(
                        &anchor_label,
                        NEW_SESSION_SIDECAR_WIDTH,
                        app,
                    );
                    let (offset, parent_anchor, child_anchor) = if render_left {
                        (
                            vec2f(-4., 0.),
                            PositionedElementAnchor::TopLeft,
                            ChildAnchor::TopRight,
                        )
                    } else {
                        (
                            vec2f(4., 0.),
                            PositionedElementAnchor::TopRight,
                            ChildAnchor::TopLeft,
                        )
                    };

                    stack.add_positioned_overlay_child(
                        sidecar_element,
                        OffsetPositioning::offset_from_save_position_element(
                            anchor_label,
                            offset,
                            PositionedElementOffsetBounds::WindowByPosition,
                            parent_anchor,
                            child_anchor,
                        ),
                    );
                }
            }

            // Action sidecar for actionable items (Terminal, Agent, Cloud Agent, tab configs).
            if let Some(sidecar_item) = &self.tab_config_action_sidecar_item {
                let anchor_label = self.new_session_dropdown_menu.read(app, |menu, _| {
                    menu.hovered_index().and_then(|idx| {
                        menu.items().get(idx).and_then(|item| match item {
                            MenuItem::Item(fields) => Some(fields.label().to_string()),
                            _ => None,
                        })
                    })
                });

                if let Some(anchor_label) = anchor_label {
                    let is_already_default = {
                        let ai_settings = AISettings::as_ref(app);
                        let current_mode = ai_settings.default_session_mode(app);
                        let current_path = ai_settings.default_tab_config_path();
                        match sidecar_item {
                            SidecarItemKind::BuiltIn {
                                default_mode,
                                shell,
                                ..
                            } => {
                                current_mode == *default_mode
                                    && *default_mode != DefaultSessionMode::TabConfig
                                    && shell.is_none()
                            }
                            SidecarItemKind::UserTabConfig { config } => {
                                current_mode == DefaultSessionMode::TabConfig
                                    && config
                                        .source_path
                                        .as_ref()
                                        .is_some_and(|p| p.to_string_lossy() == current_path)
                            }
                        }
                    };
                    let sidecar_content = crate::tab_configs::action_sidecar::render_action_sidecar(
                        sidecar_item,
                        &self.tab_config_action_sidecar_mouse_states,
                        is_already_default,
                        app,
                    );
                    let sidecar_element =
                        SavePosition::new(sidecar_content, NEW_SESSION_SIDECAR_POSITION_ID)
                            .finish();

                    let render_left = self.should_render_sidecar_left(
                        &anchor_label,
                        crate::tab_configs::action_sidecar::SIDECAR_WIDTH,
                        app,
                    );
                    let (offset, parent_anchor, child_anchor) = if render_left {
                        (
                            vec2f(-4., 0.),
                            PositionedElementAnchor::TopLeft,
                            ChildAnchor::TopRight,
                        )
                    } else {
                        (
                            vec2f(4., 0.),
                            PositionedElementAnchor::TopRight,
                            ChildAnchor::TopLeft,
                        )
                    };

                    stack.add_positioned_overlay_child(
                        sidecar_element,
                        OffsetPositioning::offset_from_save_position_element(
                            anchor_label,
                            offset,
                            PositionedElementOffsetBounds::WindowByPosition,
                            parent_anchor,
                            child_anchor,
                        ),
                    );
                }
            }
        }


        if self.welcome_tips_view_state.is_popup_open() {
            stack.add_child(ChildView::new(&self.welcome_tips_view).finish());
        }

        if self.current_workspace_state.is_palette_open {
            stack.add_overlay_child(ChildView::new(&self.palette).finish());
        }

        if self.current_workspace_state.is_ctrl_tab_palette_open {
            stack.add_child(ChildView::new(&self.ctrl_tab_palette).finish());
        }

        if self.current_workspace_state.is_require_login_modal_open {
            stack.add_child(ChildView::new(&self.require_login_modal).finish());
        }

        if self.current_workspace_state.is_auth_override_modal_open {
            stack.add_child(ChildView::new(&self.auth_override_warning_modal).finish());
        }

        if self.current_workspace_state.is_theme_creator_modal_open {
            stack.add_child(ChildView::new(&self.theme_creator_modal).finish());
        }

        if self.current_workspace_state.is_theme_deletion_modal_open {
            stack.add_child(ChildView::new(&self.theme_deletion_modal).finish());
        }

        if self.launch_config_save_modal.is_open() {
            stack.add_child(self.launch_config_save_modal.render());
        }

        if self.tab_config_params_modal.is_open() {
            stack.add_child(self.tab_config_params_modal.render());
        }

        if self.session_config_modal.is_open() {
            stack.add_child(self.session_config_modal.render());
        }

        if self.should_show_session_config_tab_config_chip() {
            let use_vertical = FeatureFlag::VerticalTabs.is_enabled()
                && *TabSettings::as_ref(app).use_vertical_tabs
                && self.vertical_tabs_panel_open;
            let chip =
                self.render_session_config_tab_config_chip(use_vertical, Appearance::as_ref(app));
            if use_vertical {
                stack.add_positioned_overlay_child(
                    chip,
                    OffsetPositioning::offset_from_save_position_element(
                        vertical_tabs::VERTICAL_TABS_ADD_TAB_POSITION_ID,
                        vec2f(8., -20.),
                        PositionedElementOffsetBounds::WindowByPosition,
                        PositionedElementAnchor::MiddleRight,
                        ChildAnchor::TopLeft,
                    ),
                );
            } else {
                let anchor_id = if FeatureFlag::ShellSelector.is_enabled() {
                    NEW_SESSION_MENU_BUTTON_POSITION_ID
                } else {
                    NEW_TAB_BUTTON_POSITION_ID
                };
                stack.add_positioned_overlay_child(
                    chip,
                    OffsetPositioning::offset_from_save_position_element(
                        anchor_id,
                        vec2f(0., 8.),
                        PositionedElementOffsetBounds::WindowByPosition,
                        PositionedElementAnchor::BottomMiddle,
                        ChildAnchor::TopMiddle,
                    ),
                );
            }
        }

        if self.new_worktree_modal.is_open() {
            stack.add_child(self.new_worktree_modal.render());
        }

        if self.current_workspace_state.is_prompt_editor_open {
            stack.add_child(ChildView::new(&self.prompt_editor_modal).finish());
        }

        if self.current_workspace_state.is_header_toolbar_editor_open {
            stack.add_child(ChildView::new(&self.header_toolbar_editor_modal).finish());
        }

        let one_time_modal_model = OneTimeModalModel::as_ref(app);
        let should_show_modal = one_time_modal_model.target_window_id() == Some(self.window_id);

        if should_show_modal && one_time_modal_model.is_oz_launch_modal_open() {
            stack.add_child(ChildView::new(&self.oz_launch_modal.view).finish());
        }

        if should_show_modal && one_time_modal_model.is_openwarp_launch_modal_open() {
            stack.add_child(ChildView::new(&self.openwarp_launch_modal).finish());
        }

        if should_show_modal && one_time_modal_model.is_orchestration_launch_modal_open() {
            stack.add_child(ChildView::new(&self.orchestration_launch_modal).finish());
        }

        if let Some(hoa_flow) = &self.hoa_onboarding_flow {
            let step = hoa_flow.as_ref(app).step();

            // Block all mouse events from reaching the workspace underneath.
            // The onboarding flow elements are rendered on top and receive events normally.
            stack.add_child(
                Dismiss::new(Empty::new().finish())
                    .prevent_interaction_with_other_elements()
                    .finish(),
            );

            match step {
                HoaOnboardingStep::WelcomeBanner => {
                    stack.add_child(ChildView::new(hoa_flow).finish());
                }
                HoaOnboardingStep::VerticalTabsCallout => {
                    if let Some(pinned) = self.hoa_vtabs_callout_pinned_position {
                        let use_vertical = *TabSettings::as_ref(app).use_vertical_tabs;
                        // The pinned position is the bubble body's top-left when
                        // using a Left arrow. With Left arrow, the element origin
                        // is at (0, 0) and the body starts at (~21, 0) due to the
                        // arrow width. With Up arrow, the body starts at (0, ~21).
                        // To keep the body in the same window position:
                        // - Left arrow: element origin = pinned (arrow is to the left)
                        // - Up arrow: shift left by ~21 (no arrow width) and up by ~21 (arrow height)
                        let offset = if use_vertical {
                            pinned
                        } else {
                            // Left arrow: body at (21, 0) from origin.
                            // Up arrow: body at (0, 21) from origin.
                            // To keep body fixed: origin.x += 21, origin.y -= 21.
                            vec2f(pinned.x() + 21., pinned.y() - 21.)
                        };
                        stack.add_positioned_child(
                            ChildView::new(hoa_flow).finish(),
                            OffsetPositioning::offset_from_parent(
                                offset,
                                ParentOffsetBounds::WindowByPosition,
                                ParentAnchor::TopLeft,
                                ChildAnchor::TopLeft,
                            ),
                        );
                    } else {
                        stack.add_positioned_child(
                            ChildView::new(hoa_flow).finish(),
                            OffsetPositioning::offset_from_save_position_element(
                                VERTICAL_TABS_PANEL_POSITION_ID,
                                vec2f(0., 8.),
                                PositionedElementOffsetBounds::WindowByPosition,
                                PositionedElementAnchor::TopRight,
                                ChildAnchor::TopLeft,
                            ),
                        );
                    }
                }
                HoaOnboardingStep::AgentInboxCallout => {
                    // Up arrow with End(24.) = arrow center ~36px from right edge.
                    // The save position wraps the icon + left margin. The icon is
                    // ~14px wide near the right edge. Shift right by ~22px so the
                    // arrow center lands on the icon center.
                    stack.add_positioned_child(
                        ChildView::new(hoa_flow).finish(),
                        OffsetPositioning::offset_from_save_position_element(
                            NOTIFICATIONS_MAILBOX_POSITION_ID,
                            vec2f(24., 4.),
                            PositionedElementOffsetBounds::WindowByPosition,
                            PositionedElementAnchor::BottomRight,
                            ChildAnchor::TopRight,
                        ),
                    );
                }
                HoaOnboardingStep::TabConfig => {
                    let use_vertical = *TabSettings::as_ref(app).use_vertical_tabs;
                    if use_vertical {
                        // Left arrow: anchor to the vertical tabs panel.
                        if let Some(pinned) = self.hoa_vtabs_callout_pinned_position {
                            stack.add_positioned_child(
                                ChildView::new(hoa_flow).finish(),
                                OffsetPositioning::offset_from_parent(
                                    pinned,
                                    ParentOffsetBounds::WindowByPosition,
                                    ParentAnchor::TopLeft,
                                    ChildAnchor::TopLeft,
                                ),
                            );
                        } else {
                            stack.add_positioned_child(
                                ChildView::new(hoa_flow).finish(),
                                OffsetPositioning::offset_from_save_position_element(
                                    VERTICAL_TABS_PANEL_POSITION_ID,
                                    vec2f(0., 8.),
                                    PositionedElementOffsetBounds::WindowByPosition,
                                    PositionedElementAnchor::TopRight,
                                    ChildAnchor::TopLeft,
                                ),
                            );
                        }
                    } else {
                        // Up arrow centered: anchor the callout's top-center
                        // to the + button's bottom-center so the arrow points
                        // at the button.
                        stack.add_positioned_child(
                            ChildView::new(hoa_flow).finish(),
                            OffsetPositioning::offset_from_save_position_element(
                                NEW_SESSION_MENU_BUTTON_POSITION_ID,
                                vec2f(0., 8.),
                                PositionedElementOffsetBounds::WindowByPosition,
                                PositionedElementAnchor::BottomMiddle,
                                ChildAnchor::TopMiddle,
                            ),
                        );
                    }
                }
            }
        }

        if should_show_modal && one_time_modal_model.is_build_plan_migration_modal_open() {
            stack.add_child(ChildView::new(&self.build_plan_migration_modal).finish());
        }

        if self.current_workspace_state.is_codex_modal_open {
            stack.add_child(ChildView::new(&self.codex_modal).finish());
        }

        if FeatureFlag::CloudMode.is_enabled()
            && self
                .current_workspace_state
                .is_cloud_agent_capacity_modal_open
        {
            stack.add_child(ChildView::new(&self.cloud_agent_capacity_modal).finish());
        }

        if self
            .current_workspace_state
            .is_free_tier_limit_hit_modal_open
        {
            stack.add_child(ChildView::new(&self.free_tier_limit_hit_modal).finish());
        }

        if let Some(lightbox_view) = &self.lightbox_view {
            stack.add_child(ChildView::new(lightbox_view).finish());
        }

        if FeatureFlag::CreatingSharedSessions.is_enabled()
            && ContextFlag::CreateSharedSession.is_enabled()
            && self
                .current_workspace_state
                .is_close_session_confirmation_dialog_open
        {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.close_session_confirmation_dialog).finish(),
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::Center,
                    ChildAnchor::Center,
                ),
            );
        }

        if self.current_workspace_state.is_native_quit_modal_open {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.native_modal).finish(),
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::Center,
                    ChildAnchor::Center,
                ),
            );
        }

        if self
            .current_workspace_state
            .is_remove_tab_config_dialog_open
        {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.remove_tab_config_confirmation_dialog).finish(),
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::Center,
                    ChildAnchor::Center,
                ),
            );
        }

        if FeatureFlag::AvatarInTabBar.is_enabled() && self.is_user_menu_open {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.user_menu).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    USER_AVATAR_BUTTON_POSITION_ID,
                    Vector2F::zero(),
                    PositionedElementOffsetBounds::WindowByPosition,
                    PositionedElementAnchor::BottomRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        if self.current_workspace_state.is_notification_mailbox_open {
            if let Some(view) = &self.notification_mailbox_view {
                let mailbox_on_left = Self::is_mailbox_on_left(
                    &TabSettings::as_ref(app).header_toolbar_chip_selection,
                );
                let (anchor, child_anchor) = if mailbox_on_left {
                    (PositionedElementAnchor::BottomLeft, ChildAnchor::TopLeft)
                } else {
                    (PositionedElementAnchor::BottomRight, ChildAnchor::TopRight)
                };
                stack.add_positioned_overlay_child(
                    ChildView::new(view).finish(),
                    OffsetPositioning::offset_from_save_position_element(
                        NOTIFICATIONS_MAILBOX_POSITION_ID,
                        Vector2F::zero(),
                        PositionedElementOffsetBounds::WindowByPosition,
                        anchor,
                        child_anchor,
                    ),
                );
            }
        }

        if !FeatureFlag::AgentMode.is_enabled()
            && AISettings::as_ref(app).is_any_ai_enabled(app)
            && self.should_show_ai_assistant_warm_welcome
            && !self.current_workspace_state.is_changelog_modal_open
            && !self.current_workspace_state.is_resource_center_open
            && !self.current_workspace_state.is_ai_assistant_panel_open
            && tab_bar_mode.has_tab_bar()
        {
            stack.add_positioned_child(
                self.render_ai_assistant_warm_welcome(appearance),
                OffsetPositioning::offset_from_save_position_element(
                    AI_ASSISTANT_BUTTON_ID,
                    vec2f(0., 10.),
                    PositionedElementOffsetBounds::Unbounded,
                    PositionedElementAnchor::BottomRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        // Cross-window ghost drag: floating chip that follows the cursor in the target window.
        // Added last so it renders on top of all other content.
        if FeatureFlag::DragTabsToWindows.is_enabled() {
            if let Some(ghost) =
                CrossWindowTabDrag::as_ref(app).ghost_state_for_window(self.window_id)
            {
                // Place the chip so its top-left is at cursor - cursor_offset_in_element.
                // This makes the cursor appear at the same position inside the chip as
                // it did in the original tab when the drag was initiated.
                let chip_origin = ghost.cursor_in_window - ghost.cursor_offset_in_element;
                stack.add_positioned_overlay_child(
                    render_cross_window_ghost_chip(&ghost, appearance, app),
                    OffsetPositioning::offset_from_parent(
                        chip_origin,
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::TopLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        }

        let window_corner_radius = app.windows().window_corner_radius();
        let workspace = Container::new(stack.finish()).with_corner_radius(window_corner_radius);

        let mut stack = Stack::new();
        let theme = appearance.theme();
        let window_settings = WindowSettings::as_ref(app);
        let background_opacity = window_settings
            .background_opacity
            .effective_opacity(self.window_id, app);

        if let Some(img) = theme.background_image() {
            let opacity_ratio = background_opacity as f32 / 100.;
            stack.add_child(
                Shrinkable::new(
                    1.,
                    Image::new(img.source(), CacheOption::Original)
                        .cover()
                        .with_opacity(opacity_ratio)
                        .with_corner_radius(window_corner_radius)
                        .finish(),
                )
                .finish(),
            );
            stack.add_child(workspace.finish());
        } else {
            stack.add_child(
                workspace
                    .with_background(theme.surface_2().with_opacity(background_opacity))
                    .finish(),
            );
        }

        let input_position_id = self
            .get_active_input_view_handle(app)
            .map(|input| app.view(&input).save_position_id());

        stack.add_positioned_overlay_child(
            ChildView::new(&self.toast_stack).finish(),
            self.global_toast_positioning(),
        );

        // Render agent toast stack (for agent-related notifications) if popup is not open
        if FeatureFlag::HOANotifications.is_enabled()
            && *AISettings::as_ref(app).show_agent_notifications
        {
            if !self.current_workspace_state.is_notification_mailbox_open {
                if let Some(stack_view) = &self.notification_toast_stack {
                    let mailbox_on_left = Self::is_mailbox_on_left(
                        &TabSettings::as_ref(app).header_toolbar_chip_selection,
                    );
                    let (anchor, child_anchor, offset_x) = if mailbox_on_left {
                        (
                            PositionedElementAnchor::BottomLeft,
                            ChildAnchor::TopLeft,
                            WORKSPACE_PADDING,
                        )
                    } else {
                        (
                            PositionedElementAnchor::BottomRight,
                            ChildAnchor::TopRight,
                            -WORKSPACE_PADDING,
                        )
                    };
                    stack.add_positioned_overlay_child(
                        ChildView::new(stack_view).finish(),
                        OffsetPositioning::offset_from_save_position_element(
                            TAB_BAR_POSITION_ID,
                            vec2f(offset_x, 4.),
                            PositionedElementOffsetBounds::WindowByPosition,
                            anchor,
                            child_anchor,
                        ),
                    );
                }
            }
        } else if !self.current_workspace_state.is_agent_management_popup_open {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.agent_toast_stack).finish(),
                self.agent_toast_positioning(),
            );
        }

        if let Some(input_position_id) = input_position_id {
            if FeatureFlag::AvatarInTabBar.is_enabled() && self.is_input_box_visible(app) {
                stack.add_positioned_overlay_child(
                    ChildView::new(&self.update_toast_stack).finish(),
                    self.update_toast_positioning(input_position_id, app),
                );
            }
        }

        #[cfg(target_family = "wasm")]
        if self.show_wasm_nux_dialog {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.wasm_nux_dialog).finish(),
                OffsetPositioning::offset_from_parent(
                    vec2f(-10., 67.),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        // Add workspace-wide UI event handling.
        let stack = if FeatureFlag::VerticalTabs.is_enabled()
            && *TabSettings::as_ref(app).use_vertical_tabs
            && self.vertical_tabs_panel_open
            // The vertical-tabs detail sidecar can become stale if the pointer moves through a
            // covered region (for example, its scrollbar gutter) and the row/sidecar hoverables
            // do not observe the expected hover-out transition. Install a workspace-root
            // mouse-move observer only while a detail sidecar is active so we can clear that
            // stale visibility without paying this cost during ordinary vertical-tabs usage.
            && self.vertical_tabs_panel.has_active_detail_target()
        {
            // The workspace root uses this handle bundle to compare the live mouse position
            // against the source row rect, sidecar rect, and safe triangle, then hide the
            // sidecar when the pointer has genuinely left all valid keep-open regions.
            let detail_hover_state = self.vertical_tabs_panel.detail_hover_state(self.window_id);
            EventHandler::new(stack.finish())
                .with_always_handle()
                .on_mouse_in(
                    move |ctx, app, position| {
                        if detail_hover_state.reconcile_visibility_for_mouse_position(position, app)
                        {
                            ctx.notify();
                        }
                        DispatchEventResult::PropagateToParent
                    },
                    Some(MouseInBehavior {
                        fire_on_synthetic_events: false,
                        fire_when_covered: true,
                    }),
                )
                .finish()
        } else {
            stack.finish()
        };

        #[cfg_attr(not(any(windows, target_os = "linux")), allow(unused_mut))]
        let mut event_handler = EventHandler::new(stack);

        #[cfg(any(windows, target_os = "linux"))]
        {
            event_handler =
                event_handler.on_scroll_wheel(move |ctx, _app, delta, modifiers_state| {
                    if !modifiers_state.ctrl {
                        return DispatchEventResult::PropagateToParent;
                    }

                    // If the control key is being held, scrolling should scale the zoom level or font size
                    if FeatureFlag::UIZoom.is_enabled() {
                        if delta.y() > 0.0 {
                            ctx.dispatch_typed_action(WorkspaceAction::IncreaseZoom);
                        } else if delta.y() < 0.0 {
                            ctx.dispatch_typed_action(WorkspaceAction::DecreaseZoom);
                        }
                    } else if delta.y() > 0.0 {
                        ctx.dispatch_typed_action(WorkspaceAction::IncreaseFontSize);
                    } else if delta.y() < 0.0 {
                        ctx.dispatch_typed_action(WorkspaceAction::DecreaseFontSize);
                    }
                    DispatchEventResult::StopPropagation
                });
        }

        event_handler.finish()
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            self.focus_active_tab(ctx);
        }
    }

    /// Update this workspace when it has been closed, but may still be restored.
    fn on_window_closed(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.suppress_detach_panes_on_window_close {
            for pane_group in self.tab_views() {
                pane_group.update(ctx, |pane_group, ctx| {
                    pane_group.detach_panes(ctx);
                });
            }
        }

        let window_id = ctx.window_id();

        WorkspaceRegistry::handle(ctx).update(ctx, |registry, _| {
            registry.unregister(window_id);
        });

        // If this workspace's close was registered as part of a tab-drag
        // handoff, clear the entry now that the workspace is gone from the
        // registry. Safe no-op if this window wasn't registered. See
        // `CrossWindowTabDrag::pending_source_window_closes` for the
        // `terminal_panes.uuid` race this guards.
        CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _| {
            drag.finish_pending_source_close(window_id);
        });

        ActiveSession::handle(ctx).update(ctx, |active_session, _| {
            active_session.close_workspace(window_id);
        })
    }
}

// ---- Tab drag: local reordering, cross-window initiation, drop handling ----
impl Workspace {
    /// Builds a `TransferredTab` snapshot for the tab at `index`, or `None`
    /// if the index is out of bounds.
    fn tab_transfer_info_at_index(&self, index: usize, ctx: &AppContext) -> Option<TransferredTab> {
        let tab = self.tabs.get(index)?;
        let pane_group = tab.pane_group.clone();
        let color = tab.color();
        let draggable_state = tab.draggable_state.clone();
        let custom_title = pane_group.read(ctx, |pg, ctx| pg.custom_title(ctx));
        let left_panel_open = pane_group.read(ctx, |pg, _| pg.left_panel_open);
        let right_panel_open = pane_group.read(ctx, |pg, _| pg.right_panel_open);
        let is_right_panel_maximized = pane_group.read(ctx, |pg, _| pg.is_right_panel_maximized);
        let vertical_tabs_panel_open = self.vertical_tabs_panel_open;

        Some(TransferredTab {
            pane_group,
            color,
            custom_title,
            left_panel_open,
            right_panel_open,
            is_right_panel_maximized,
            draggable_state,
            vertical_tabs_panel_open,
        })
    }

    pub fn get_tab_transfer_info(&self, index: usize, ctx: &AppContext) -> Option<TransferredTab> {
        if self.tabs.len() <= 1 {
            return None;
        }
        self.tab_transfer_info_at_index(index, ctx)
    }

    pub fn get_tab_transfer_info_for_attach(
        &self,
        index: usize,
        ctx: &AppContext,
    ) -> Option<TransferredTab> {
        self.tab_transfer_info_at_index(index, ctx)
    }

    /// Prepares this workspace for having a pane group transferred out by
    /// suppressing pane-detach on close and unsubscribing from the view.
    /// The suppress flag is **not** auto-restored; callers that keep the
    /// window alive must clear it via
    /// `set_suppress_detach_panes_on_window_close(false)`.
    pub fn prepare_for_transferred_tab_attach(
        &mut self,
        pane_group: &ViewHandle<PaneGroup>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.set_suppress_detach_panes_on_window_close(true);
        ctx.unsubscribe_to_view(pane_group);
    }

    /// Suppresses pane-detach and closes this window with
    /// `ContentTransferred`. Called when the source window's last tab has
    /// been transferred elsewhere.
    pub(crate) fn close_window_for_content_transfer(&mut self, ctx: &mut ViewContext<Self>) {
        self.set_suppress_detach_panes_on_window_close(true);
        ctx.windows()
            .close_window(ctx.window_id(), TerminationMode::ContentTransferred);
    }

    pub(crate) fn insert_transferred_tab_at_index(
        &mut self,
        transferred_tab: TransferredTab,
        insertion_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        let TransferredTab {
            pane_group,
            color,
            draggable_state,
            ..
        } = transferred_tab;
        ctx.subscribe_to_view(&pane_group, move |me, pane_group, event, ctx| {
            me.handle_file_tree_event(pane_group, event, ctx)
        });

        let index = insertion_index.min(self.tabs.len());
        let mut tab_data = TabData::new(pane_group);
        tab_data.selected_color = color.map_or(SelectedTabColor::Unset, SelectedTabColor::Color);
        tab_data.draggable_state = draggable_state;
        self.tabs.insert(index, tab_data);
        self.activate_tab_internal(index, ctx);
        ctx.notify();
    }

    /// Returns the tab-bar index where a dragged tab would be inserted for
    /// the given cursor position. Skips tabs clipped by the overflow area
    /// (width below `MIN_VISIBLE_TAB_WIDTH`), and picks between horizontal
    /// and vertical layout by comparing the spread of tab centers on each
    /// axis.
    pub(crate) fn tab_insertion_index_for_cursor(
        &self,
        window_id: WindowId,
        cursor_position_on_screen: Vector2F,
        ctx: &AppContext,
    ) -> usize {
        const MIN_VISIBLE_TAB_WIDTH: f32 = 1.0;

        let Some(window_bounds) = ctx.window_bounds(&window_id) else {
            return self.tabs.len();
        };

        // Pre-compute the bounding rects of the tab bar / vertical tabs panel
        // so we can defensively reject `tab_position_<index>` cache entries
        // that don't lie within either of them. This guards
        // `tab_insertion_index_for_cursor` against any future overlay /
        // chip / preview that accidentally shares a SavePosition key with
        // a real tab — see the `for_drag_ghost` flag on `TabComponent` and
        // `vertical_tabs::render_tab_group_internal` for the original
        // offender (the cross-window drag ghost chip).
        let tab_bar_rects = tab_bar_rects_for_window(window_id, ctx);

        let cursor_in_window = cursor_position_on_screen - window_bounds.origin();
        let mut visible_tabs = Vec::new();
        for index in 0..self.tabs.len() {
            if let Some(tab_position) =
                ctx.element_position_by_id_at_last_frame(window_id, tab_position_id(index))
            {
                if tab_position.width() <= MIN_VISIBLE_TAB_WIDTH {
                    continue;
                }
                // If we have at least one tab-bar-equivalent rect, require
                // that the candidate tab position be (mostly) inside one of
                // them. Use the rect center as the membership test — a tab
                // that's been rendered partway off-screen due to overflow
                // is fine to keep, but a rect that has nothing to do with
                // the tab bar (e.g. the floating chip that follows the
                // cursor anywhere in the window) gets rejected.
                if !tab_bar_rects.is_empty()
                    && !tab_bar_rects
                        .iter()
                        .any(|tb| tb.contains_point(tab_position.center()))
                {
                    continue;
                }
                visible_tabs.push((index, tab_position));
            }
        }

        if visible_tabs.is_empty() {
            return self.tabs.len();
        }

        // Detect orientation from the axis with the larger spread between
        // first and last tab centers (vertical panels stack along Y).
        // With only one tab there is no spread to compare, so fall back to
        // whether the vertical-tabs panel is open.
        let is_vertical = if visible_tabs.len() >= 2 {
            let first = visible_tabs[0].1.center();
            let last = visible_tabs.last().expect("non-empty").1.center();
            (last.y() - first.y()).abs() > (last.x() - first.x()).abs()
        } else {
            self.vertical_tabs_panel_open
        };

        if is_vertical {
            for (index, tab_position) in &visible_tabs {
                if cursor_in_window.y() < tab_position.center().y() {
                    return *index;
                }
            }
        } else {
            for (index, tab_position) in &visible_tabs {
                if cursor_in_window.x() < tab_position.center().x() {
                    return *index;
                }
            }
        }

        visible_tabs
            .last()
            .map(|(index, _)| index + 1)
            .unwrap_or(self.tabs.len())
    }

    pub fn remove_tab_without_undo(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        self.remove_tab(index, false, false, ctx);
    }

    /// Replaces the placeholder pane group (created by
    /// `create_transferred_window`) with the real pane group transferred from
    /// the source window, detaching and dropping the placeholder.
    pub fn adopt_transferred_pane_group(
        &mut self,
        new_pane_group: ViewHandle<PaneGroup>,
        ctx: &mut ViewContext<Self>,
    ) {
        if !self.pending_pane_group_transfer {
            debug_assert!(
                false,
                "adopt_transferred_pane_group called without pending transfer"
            );
            return;
        }

        if self.tabs.is_empty() {
            debug_assert!(false, "adopt_transferred_pane_group called with no tabs");
            return;
        }
        let Some(placeholder_tab) = self.tabs.last_mut() else {
            debug_assert!(
                false,
                "adopt_transferred_pane_group missing placeholder tab"
            );
            return;
        };

        // Swap the placeholder's pane group with the real one, then tear down
        // the placeholder so its terminals are properly detached.
        let placeholder_pane_group =
            std::mem::replace(&mut placeholder_tab.pane_group, new_pane_group.clone());
        let old_id = placeholder_pane_group.id();
        let new_id = placeholder_tab.pane_group.id();
        if let Some(pos) = self.tab_mru_order.iter().position(|&id| id == old_id) {
            self.tab_mru_order[pos] = new_id;
        }

        // Re-route pane-group event subscriptions from the placeholder onto
        // the transferred pane group. The workspace was subscribed to the
        // placeholder in `add_tab_with_pane_layout` (via
        // `NewWorkspaceSource::TransferredTab`), but after this swap the
        // placeholder is dropped and any events we need to react to — most
        // notably `PaneGroup::Event::Exited` fired by cmd-W — come from
        // `new_pane_group` instead. Without this, `close_pane` emits
        // `Exited` but `handle_file_tree_event` is never invoked, so the
        // workspace never calls `close_tab` and cmd-W appears to do nothing.
        ctx.unsubscribe_to_view(&placeholder_pane_group);
        ctx.subscribe_to_view(&new_pane_group, move |me, pane_group, event, ctx| {
            me.handle_file_tree_event(pane_group, event, ctx)
        });

        let working_directories_model = self.working_directories_model.clone();
        placeholder_pane_group.update(ctx, |pg, ctx| {
            pg.detach_panes_for_close(&working_directories_model, ctx);
        });
        self.pending_pane_group_transfer = false;
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    /// Transfers a dragged tab into the attach target's window by delegating
    /// to the appropriate `CrossWindowTabDrag::execute_handoff_*` variant.
    fn perform_handoff(&mut self, target: AttachTarget, ctx: &mut ViewContext<Self>) {
        let caller_window_id = ctx.window_id();

        let has_dedicated_preview = CrossWindowTabDrag::as_ref(ctx).has_dedicated_preview_window();
        let source_tab_index = CrossWindowTabDrag::as_ref(ctx)
            .transferred_tab_index()
            .unwrap_or(0);
        let source_was_single_tab = CrossWindowTabDrag::as_ref(ctx).source_was_single_tab();

        log::info!(
            "tab_drag: perform_handoff caller_wid={caller_window_id} target_wid={} insertion_index={} has_dedicated_preview={has_dedicated_preview} source_tab_index={source_tab_index} source_was_single_tab={source_was_single_tab}",
            target.window_id,
            target.insertion_index
        );

        // Put-back: multi-tab drag whose target is the original source. The
        // pane group is transferred back from the preview into the caller
        // and the preview window is closed.
        if target.window_id == caller_window_id {
            log::info!(
                "tab_drag: perform_handoff branch=target==caller (put-back) caller_wid={caller_window_id}"
            );
            if !has_dedicated_preview {
                log::warn!(
                    "tab_drag: perform_handoff target==caller without dedicated preview -> reset_to_floating (no-op)"
                );
                CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _| {
                    drag.reset_to_floating();
                });
                return;
            }

            // A prior put-back already committed the tab back into the
            // source. `source_tab_index` now points at an unrelated tab, so
            // running `execute_handoff_back_to_caller` + `remove_tab` here
            // would corrupt that bystander and leave the pane group attached
            // to two windows. This is the Overlap A case from the TECH.md
            // at `pei/tab-dragging/put-back-plus-new-window-overlap`.
            // `on_drop` is supposed to filter these drops out before they
            // reach `DropInto`; bail out defensively if something slips
            // through.
            if CrossWindowTabDrag::as_ref(ctx).source_placeholder_consumed() {
                log::warn!(
                    "tab_drag: perform_handoff target==caller called with source_placeholder_consumed=true -> reset_to_floating"
                );
                CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _| {
                    drag.reset_to_floating();
                });
                return;
            }

            let caller_draggable_state = self
                .tabs
                .get(source_tab_index)
                .map(|tab| tab.draggable_state.clone());

            let Some(caller_draggable_state) = caller_draggable_state else {
                CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _| {
                    drag.reset_to_floating();
                });
                return;
            };

            let result = CrossWindowTabDrag::handle(ctx).update(ctx, |drag, ctx| {
                drag.execute_handoff_back_to_caller(
                    target,
                    caller_draggable_state,
                    caller_window_id,
                    ctx,
                )
            });

            if let Some(info) = result {
                if let Some(tab) = self.tabs.get(source_tab_index) {
                    ctx.unsubscribe_to_view(&tab.pane_group);
                }
                if source_was_single_tab {
                    self.close_window_for_content_transfer(ctx);
                } else {
                    self.remove_tab_without_undo(source_tab_index, ctx);
                }
                // The source placeholder is now removed, so `source_tab_index`
                // is stale. Mark it consumed so a later reverse_handoff +
                // empty-space drop falls into `NoOp` instead of trying to
                // remove a non-existent tab. See the field doc on
                // `ActiveDrag::source_placeholder_consumed`.
                CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _| {
                    drag.mark_source_placeholder_consumed();
                });
                self.insert_transferred_tab_at_index(
                    info.transferred_tab,
                    info.insertion_index,
                    ctx,
                );
                self.current_workspace_state.is_tab_being_dragged = true;
                self.focus_active_tab(ctx);
            }
            return;
        }

        if !has_dedicated_preview {
            log::info!(
                "tab_drag: perform_handoff branch=single_tab_source->other target_wid={} caller_wid={caller_window_id}",
                target.window_id
            );
            let Some(mut transfer_info) =
                self.get_tab_transfer_info_for_attach(source_tab_index, ctx)
            else {
                log::warn!(
                    "tab_drag: perform_handoff single_tab could not get transfer info source_tab_index={source_tab_index} -> reset_to_floating"
                );
                CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _| {
                    drag.reset_to_floating();
                });
                return;
            };
            transfer_info.draggable_state = DraggableState::default();
            self.prepare_for_transferred_tab_attach(&transfer_info.pane_group, ctx);
            CrossWindowTabDrag::handle(ctx).update(ctx, |drag, ctx| {
                drag.execute_handoff_single_tab_to_other(
                    target,
                    transfer_info,
                    caller_window_id,
                    ctx,
                );
            });
            return;
        }

        log::info!(
            "tab_drag: perform_handoff branch=multi_tab_source->other target_wid={} caller_wid={caller_window_id}",
            target.window_id
        );
        CrossWindowTabDrag::handle(ctx).update(ctx, |drag, ctx| {
            drag.execute_handoff_multi_tab_to_other(target, ctx);
        });
    }

    /// Handles a tab drag event from the `Draggable` element. Dispatches to
    /// one of three modes: forward to an in-progress cross-window drag,
    /// initiate a new cross-window drag when the drag leaves the tab bar
    /// (or from a single-tab window), or reorder within the current window.
    pub(crate) fn on_tab_drag(
        &mut self,
        current_index: usize,
        position: RectF,
        ctx: &mut ViewContext<Self>,
    ) {
        const DETACH_SENSITIVITY: f32 = 10.0;
        // Only detach when the drag leaves every tab-bar presentation on its
        // perpendicular axis. Windows with vertical tabs still render the
        // horizontal bar, so checking only the horizontal rect would make
        // vertical reorders (which move along Y) spuriously trip the detach.
        let drag_center = position.center();
        let rects = tab_bar_rects_for_window(ctx.window_id(), ctx);
        let is_drag_outside_tab_bar = if rects.is_empty() {
            // No rect laid out yet (first frame); fall back to the horizontal
            // bar's hardcoded height.
            let drag_y = position.min_y();
            !(-DETACH_SENSITIVITY..=TAB_BAR_HEIGHT + DETACH_SENSITIVITY).contains(&drag_y)
        } else {
            rects.into_iter().all(|rect| {
                let is_vertical = rect.height() > rect.width();
                if is_vertical {
                    drag_center.x() < rect.min_x() - DETACH_SENSITIVITY
                        || drag_center.x() > rect.max_x() + DETACH_SENSITIVITY
                } else {
                    drag_center.y() < rect.min_y() - DETACH_SENSITIVITY
                        || drag_center.y() > rect.max_y() + DETACH_SENSITIVITY
                }
            })
        };

        if CrossWindowTabDrag::as_ref(ctx).is_active() {
            let window_id = ctx.window_id();
            let drag_result = CrossWindowTabDrag::handle(ctx)
                .update(ctx, |drag, ctx| drag.on_drag(window_id, position, ctx));
            match drag_result {
                DragResult::Handled => {}
                DragResult::AdjustDraggable { adjustment } => {
                    if let Some(tab) = self.tabs.get(current_index) {
                        tab.draggable_state.adjust_mouse_position(adjustment);
                    }
                }
                DragResult::HandoffNeeded { target } => {
                    self.perform_handoff(target, ctx);
                }
            }
            return;
        }

        if let Some(tab_data) = self.tabs.get(current_index) {
            if tab_data.detached {
                return;
            }
        }

        let source_is_single_tab = self.tabs.len() == 1;
        if (is_drag_outside_tab_bar || source_is_single_tab)
            && FeatureFlag::DragTabsToWindows.is_enabled()
        {
            let source_was_single_tab = source_is_single_tab;
            if !source_was_single_tab {
                if let Some(tab_data) = self.tabs.get_mut(current_index) {
                    tab_data.detached = true;
                }
            }

            let window_bounds = match ctx.window_bounds(&ctx.window_id()) {
                Some(bounds) => bounds,
                None => return,
            };
            let source_window_origin = window_bounds.origin();
            let drag_origin_in_window = vec2f(position.min_x(), position.min_y());
            let drag_origin_on_screen = vec2f(
                source_window_origin.x() + drag_origin_in_window.x(),
                source_window_origin.y() + drag_origin_in_window.y(),
            );
            let last_known_target_tab_origin_in_window = ctx
                .element_position_by_id(tab_position_id(0))
                .map(|rect| vec2f(rect.min_x(), rect.min_y()))
                .unwrap_or_else(|| vec2f(0.0, 0.0));
            let window_position = drag_origin_on_screen - last_known_target_tab_origin_in_window;
            let window_size = window_bounds.size();
            let initial_drag_center_offset =
                position.center() - vec2f(position.min_x(), position.min_y());
            let source_window_id = ctx.window_id();

            // Capture the source layout (vertical tabs panel vs horizontal
            // tab bar) and the rendered tab's element size at drag-start.
            // Both are frozen for the duration of the drag so the floating
            // ghost chip mirrors what was on screen when the drag began,
            // even if the user toggles their layout mid-drag.
            let was_vertical_layout = uses_vertical_tabs(ctx);
            let source_element_size = ctx
                .element_position_by_id(tab_position_id(current_index))
                .map(|rect| rect.size())
                .unwrap_or_else(|| vec2f(120., 34.));

            if source_was_single_tab {
                let new_bounds = RectF::new(window_position, window_size);
                ctx.set_and_cache_window_bounds(source_window_id, new_bounds);
                ctx.windows().cancel_synthetic_drag(source_window_id);
                if let Some(tab) = self.tabs.get(current_index) {
                    tab.draggable_state.set_suppress_overlay_paint(true);
                    tab.draggable_state
                        .adjust_mouse_position(source_window_origin - window_position);
                }

                CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _ctx| {
                    drag.begin_single_tab_drag(
                        source_window_id,
                        initial_drag_center_offset,
                        window_size,
                        last_known_target_tab_origin_in_window,
                        was_vertical_layout,
                        source_element_size,
                    );
                });
            } else {
                let Some(transferred_tab) =
                    self.get_tab_transfer_info_for_attach(current_index, ctx)
                else {
                    return;
                };

                let preview_window_id = crate::root_view::create_transferred_window(
                    transferred_tab,
                    source_window_id,
                    window_size,
                    window_position,
                    true,
                    ctx,
                );
                ctx.set_suppress_focus_for_window(Some(preview_window_id));

                CrossWindowTabDrag::handle(ctx).update(ctx, |drag, _ctx| {
                    drag.begin_multi_tab_drag(
                        source_window_id,
                        current_index,
                        initial_drag_center_offset,
                        window_size,
                        last_known_target_tab_origin_in_window,
                        preview_window_id,
                        was_vertical_layout,
                        source_element_size,
                    );
                });
            }

            if !source_was_single_tab && current_index == self.active_tab_index {
                let adjacent = if current_index + 1 < self.tabs.len() {
                    current_index + 1
                } else {
                    current_index.saturating_sub(1)
                };
                self.set_active_tab_index(adjacent, ctx);
            }

            ctx.notify();
            return;
        }

        let use_vertical_tabs =
            FeatureFlag::VerticalTabs.is_enabled() && *TabSettings::as_ref(ctx).use_vertical_tabs;
        let groups_enabled = FeatureFlag::GroupedTabs.is_enabled();

        if use_vertical_tabs && groups_enabled {
            // Reassign membership when the dragged tab's midpoint enters a
            // different expanded group. Collapsed groups are handled by the
            // safety-net hop below so we don't drop into it.
            let midpoint_drag_y = (position.min_y() + position.max_y()) / 2.;
            let hovered_group = self.target_group_at_y(midpoint_drag_y, ctx);
            let source_group = self.tabs[current_index].group_id;
            let expanded_target =
                hovered_group.filter(|gid| !self.tab_groups.get(gid).is_some_and(|g| g.collapsed));
            if expanded_target != source_group {
                self.assign_tab_to_group(current_index, expanded_target, ctx);
                // Hop into the target group's contiguous block so the group
                // stays one rendered container. Vertical tab rendering only
                // groups consecutive tabs, so leaving `current_index` outside
                // the block would split the group across the panel.
                if let Some(target_gid) = expanded_target {
                    if let Some((first, last)) = group_member_index_range(&self.tabs, target_gid) {
                        let insert_at = if current_index < first { first } else { last };
                        if insert_at != current_index {
                            self.hop_tab_to_index(current_index, insert_at, ctx);
                        }
                    }
                }
                return;
            }
        }

        let new_index = if use_vertical_tabs {
            self.calculate_updated_tab_index_vertical(current_index, position, ctx)
        } else {
            self.calculate_updated_tab_index(current_index, position, ctx)
        };

        if new_index != current_index {
            // Prevent dropping into a collapsed group: if the swap target is a
            // collapsed-group member the dragged tab doesn't belong to, hop
            // past the whole block instead of swapping into it.
            let dragged_group = self.tabs[current_index].group_id;
            let neighbor_collapsed_group = self
                .tabs
                .get(new_index)
                .and_then(|t| t.group_id)
                .filter(|gid| {
                    Some(*gid) != dragged_group
                        && self.tab_groups.get(gid).is_some_and(|g| g.collapsed)
                });
            if let Some(group_id) = neighbor_collapsed_group {
                if let Some((first, last)) = group_member_index_range(&self.tabs, group_id) {
                    let insert_at = if current_index < first { last } else { first };
                    self.hop_tab_to_index(current_index, insert_at, ctx);
                    return;
                }
            }

            self.tabs.swap(new_index, current_index);

            if current_index == self.active_tab_index {
                self.set_active_tab_index(new_index, ctx);
            } else if new_index == self.active_tab_index {
                self.set_active_tab_index(current_index, ctx);
            }

            ctx.notify();
        }
    }

    /// Performs the source-workspace cleanup indicated by `DropResult`.
    /// Cross-workspace mutations (preview/target updates, focus) happen inside
    /// `CrossWindowTabDrag::on_drop`; this method only touches `self`.
    pub(crate) fn handle_drop_result(&mut self, result: DropResult, ctx: &mut ViewContext<Self>) {
        match result {
            DropResult::NoOp => {}
            DropResult::FocusSelf => {
                if let Some(tab) = self.tabs.first() {
                    tab.draggable_state.set_suppress_overlay_paint(false);
                }
                self.focus_active_tab(ctx);
            }
            DropResult::CloseSourceWindow {
                transferred_tab_index,
            } => {
                if let Some(tab) = self.tabs.get(transferred_tab_index) {
                    ctx.unsubscribe_to_view(&tab.pane_group);
                }
                self.close_window_for_content_transfer(ctx);
            }
            DropResult::RemoveSourceTab {
                transferred_tab_index,
            } => {
                if let Some(tab) = self.tabs.get(transferred_tab_index) {
                    ctx.unsubscribe_to_view(&tab.pane_group);
                }
                self.remove_tab_without_undo(transferred_tab_index, ctx);
            }
            DropResult::RemoveSourceTabAndClosePreview {
                transferred_tab_index,
                preview_window_id,
            } => {
                if let Some(tab) = self.tabs.get(transferred_tab_index) {
                    ctx.unsubscribe_to_view(&tab.pane_group);
                }
                self.remove_tab_without_undo(transferred_tab_index, ctx);
                ctx.windows()
                    .close_window(preview_window_id, TerminationMode::ContentTransferred);
            }
            DropResult::ClosePreviewOnly { preview_window_id } => {
                // `Floating` drop after a prior put-back: the source already
                // owns the tab, but the preview still carries a `TabData`
                // pointing at the same pane group. Close the preview
                // asynchronously; `finalize` has already registered the
                // pending close so `is_active()` keeps persistence paused
                // until `on_window_closed` fires.
                ctx.windows()
                    .close_window(preview_window_id, TerminationMode::ContentTransferred);
            }
            DropResult::DropInto { target } => {
                // Drop landed on a tab bar that hadn't yet triggered a
                // handoff in-flight. Commit the handoff now, then finalize
                // to close the preview and clean up the source. Fixes the
                // "empty ghost window" bug when the mouse is released back
                // over the source (or any other) tab bar at drop time.
                self.perform_handoff(target, ctx);
                let final_result =
                    CrossWindowTabDrag::handle(ctx).update(ctx, |drag, ctx| drag.finalize(ctx));
                self.handle_drop_result(final_result, ctx);
            }
        }
    }

    /// Determines the appropriate index for a tab that is being dragged, based on its current
    /// index and drag position
    ///
    /// We check if the midpoint of the dragged tab has crossed into the boundary of either
    /// surrounding tab. For the tab immediately to the left, this means checking against the
    /// rightmost boundary, while for the tab immediately to the right, we check against the
    /// leftmost boundary.
    ///
    /// If the midpoint is not in either location, then we return the current index, as the tab has
    /// not moved out of its position
    fn calculate_updated_tab_index(
        &self,
        current_index: usize,
        drag_position: RectF,
        ctx: &mut ViewContext<Self>,
    ) -> usize {
        let midpoint_drag_x = (drag_position.min_x() + drag_position.max_x()) / 2.;

        let maybe_left_tab = if current_index > 0 {
            ctx.element_position_by_id(tab_position_id(current_index - 1))
        } else {
            None
        };
        if let Some(tab_position) = maybe_left_tab {
            if midpoint_drag_x < tab_position.max_x() {
                return current_index - 1;
            }
        }

        let maybe_right_tab = if current_index < self.tabs.len() - 1 {
            ctx.element_position_by_id(tab_position_id(current_index + 1))
        } else {
            None
        };
        if let Some(tab_position) = maybe_right_tab {
            if midpoint_drag_x > tab_position.min_x() {
                return current_index + 1;
            }
        }

        current_index
    }

    /// Y-axis variant of `calculate_updated_tab_index` for vertical tab layout.
    ///
    /// Uses midpoint-of-neighbor thresholds rather than edge thresholds to prevent
    /// oscillation when groups have different heights.
    fn calculate_updated_tab_index_vertical(
        &self,
        current_index: usize,
        drag_position: RectF,
        ctx: &mut ViewContext<Self>,
    ) -> usize {
        let midpoint_drag_y = (drag_position.min_y() + drag_position.max_y()) / 2.;

        let maybe_above_tab = if current_index > 0 {
            self.neighbor_drag_rect(current_index - 1, ctx)
        } else {
            None
        };
        if let Some(tab_position) = maybe_above_tab {
            let neighbor_midpoint_y = (tab_position.min_y() + tab_position.max_y()) / 2.;
            if midpoint_drag_y < neighbor_midpoint_y {
                return current_index - 1;
            }
        }

        let maybe_below_tab = if current_index < self.tabs.len() - 1 {
            self.neighbor_drag_rect(current_index + 1, ctx)
        } else {
            None
        };
        if let Some(tab_position) = maybe_below_tab {
            let neighbor_midpoint_y = (tab_position.min_y() + tab_position.max_y()) / 2.;
            if midpoint_drag_y > neighbor_midpoint_y {
                return current_index + 1;
            }
        }

        current_index
    }

    /// Returns the group whose saved container rect contains `cursor_y`, if any.
    /// A small edge margin at each end of the rect is treated as "between groups"
    /// so the cursor can land in the ungrouped zone between adjacent groups.
    fn target_group_at_y(&self, cursor_y: f32, ctx: &mut ViewContext<Self>) -> Option<TabGroupId> {
        const EDGE_MARGIN: f32 = 6.0;
        self.tab_groups.keys().copied().find(|group_id| {
            ctx.element_position_by_id(vtab_group_position_id(*group_id))
                .is_some_and(|rect| {
                    rect.min_y() + EDGE_MARGIN <= cursor_y && cursor_y <= rect.max_y() - EDGE_MARGIN
                })
        })
    }

    /// Returns the drag comparison rect for `neighbor_index`.
    ///
    /// For members of a collapsed group the per-tab `tab_position_id` rect
    /// is stale (the tab is no longer painted, but `PositionCache` keeps the
    /// last painted rect). Use the group container's rect instead so
    /// midpoint comparisons fire at the visible header.
    fn neighbor_drag_rect(
        &self,
        neighbor_index: usize,
        ctx: &mut ViewContext<Self>,
    ) -> Option<RectF> {
        let neighbor_group_id = self.tabs.get(neighbor_index).and_then(|t| t.group_id);
        let neighbor_in_collapsed_group = neighbor_group_id
            .and_then(|gid| self.tab_groups.get(&gid))
            .is_some_and(|g| g.collapsed);

        if neighbor_in_collapsed_group {
            return ctx.element_position_by_id(vtab_group_position_id(neighbor_group_id.unwrap()));
        }

        ctx.element_position_by_id(tab_position_id(neighbor_index))
    }

    /// Swaps the group's entire member block with its above/below neighbor
    /// when the dragged header's Y midpoint crosses the neighbor's midpoint.
    pub(crate) fn on_group_drag(
        &mut self,
        group_id: TabGroupId,
        position: RectF,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some((first, last)) = group_member_index_range(&self.tabs, group_id) else {
            return;
        };
        let midpoint_drag_y = (position.min_y() + position.max_y()) / 2.;

        // Swap up: check the neighbor directly above the group's first member.
        if first > 0 {
            let above_index = first - 1;
            if let Some(rect) = self.neighbor_drag_rect(above_index, ctx) {
                let neighbor_midpoint = (rect.min_y() + rect.max_y()) / 2.;
                if midpoint_drag_y < neighbor_midpoint {
                    let target = if let Some(other_gid) = self.tabs[above_index].group_id {
                        group_member_index_range(&self.tabs, other_gid)
                            .map(|(f, _)| f)
                            .unwrap_or(above_index)
                    } else {
                        above_index
                    };
                    self.move_group_block(group_id, target, ctx);
                    return;
                }
            }
        }

        // Swap down: check the neighbor directly below the group's last member.
        // Pass `below_block_last + 1` (the pre-drain target index); `move_group_block`
        // accounts for the drain internally when `target > last`.
        if last + 1 < self.tabs.len() {
            let below_index = last + 1;
            if let Some(rect) = self.neighbor_drag_rect(below_index, ctx) {
                let neighbor_midpoint = (rect.min_y() + rect.max_y()) / 2.;
                if midpoint_drag_y > neighbor_midpoint {
                    let below_block_last = if let Some(other_gid) = self.tabs[below_index].group_id
                    {
                        group_member_index_range(&self.tabs, other_gid)
                            .map(|(_, l)| l)
                            .unwrap_or(below_index)
                    } else {
                        below_index
                    };
                    self.move_group_block(group_id, below_block_last + 1, ctx);
                }
            }
        }
    }
}

fn should_reserve_traffic_light_space_in_tab_bar(side: TrafficLightSide) -> bool {
    side == TrafficLightSide::Right
}

/// Returns the indices of every tab in `tabs` that belongs to `group_id`,
/// in ascending order.
fn group_member_indices(
    tabs: &[TabData],
    group_id: TabGroupId,
) -> impl Iterator<Item = usize> + '_ {
    tabs.iter()
        .enumerate()
        .filter(move |(_, tab)| tab.group_id == Some(group_id))
        .map(|(idx, _)| idx)
}

/// Returns the `(first, last)` index range for the contiguous run of tabs
/// in `tabs` that belong to `group_id`, or `None` if the group has no members.
/// The run is assumed to be contiguous (the workspace enforces this invariant);
/// only the earliest and latest matching indices are returned.
fn group_member_index_range(tabs: &[TabData], group_id: TabGroupId) -> Option<(usize, usize)> {
    let mut members = group_member_indices(tabs, group_id);
    let first = members.next()?;
    let last = members.last().unwrap_or(first);
    Some((first, last))
}

/// Returns every tab-bar-equivalent rect laid out in `window_id` (horizontal
/// tab bar and/or vertical tabs panel). Both must be considered because a
/// window with vertical tabs still renders the horizontal bar at the top.
pub(crate) fn tab_bar_rects_for_window(window_id: WindowId, app: &AppContext) -> Vec<RectF> {
    let mut rects = Vec::with_capacity(2);
    if let Some(rect) = app.element_position_by_id_at_last_frame(window_id, TAB_BAR_POSITION_ID) {
        rects.push(rect);
    }
    if let Some(rect) =
        app.element_position_by_id_at_last_frame(window_id, VERTICAL_TABS_PANEL_POSITION_ID)
    {
        rects.push(rect);
    }
    rects
}

/// Renders the floating chip shown in the target window during a cross-window
/// ghost drag. The chip's contents come from the same render code paths used
/// by the source layout (`TabComponent` for horizontal, `render_tab_group`
/// for vertical) by reading the dragged tab from the source/preview
/// workspace, so the chip looks identical to the source tab. The chip is
/// constrained to `ghost.source_element_size` (the source tab's rendered
/// dimensions). Its top-left is placed at
/// `cursor_in_window - cursor_offset_in_element` by the caller so the cursor
/// sits at the same relative position inside the chip as it did in the
/// original tab when the drag was initiated.
fn render_cross_window_ghost_chip(
    ghost: &GhostState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    use riftui::elements::DropShadow;

    let theme = appearance.theme();

    // Render the dragged tab using the same code path the source layout
    // uses. The dragged tab is always at index 0 in the preview workspace
    // (single-tab drags use the source window itself as the preview, which
    // by definition has only one tab; multi-tab drags move the dragged tab
    // to a dedicated preview window's index 0).
    let inner = WorkspaceRegistry::as_ref(app)
        .get(ghost.preview_window_id, app)
        .map(|ws| {
            ws.as_ref(app)
                .render_tab_for_drag_ghost(0, ghost.was_vertical_layout, app)
        })
        .unwrap_or_else(|| Empty::new().finish());

    // Wrap in a container with the source tab's background fill + a drop
    // shadow so the chip reads as a floating, detached element. The inner
    // tab/group renderer already paints its own background where applicable;
    // the drop shadow is applied at this outer layer.
    let chip = Container::new(inner)
        .with_background(internal_colors::fg_overlay_1(theme))
        .with_drop_shadow(DropShadow::default())
        .finish();

    // Constrain to the source tab's rendered dimensions so the chip matches
    // the size of the tab the user grabbed.
    let size = ghost.source_element_size;
    if size.x() > 0. && size.y() > 0. {
        ConstrainedBox::new(chip)
            .with_width(size.x())
            .with_height(size.y())
            .finish()
    } else {
        ConstrainedBox::new(chip).with_max_width(200.).finish()
    }
}

fn compute_default_panel_widths(
    app: &AppContext,
    window_id: WindowId,
    has_horizontal_split: bool,
) -> (f32, f32) {
    if let Some(bounds) = app.window_bounds(&window_id) {
        let window_width = bounds.width();
        let left_ratio = 0.15;
        let right_ratio = if has_horizontal_split { 0.3 } else { 0.5 };
        let left = window_width * left_ratio;
        let right = window_width * right_ratio;
        (left, right)
    } else {
        (DEFAULT_LEFT_PANEL_WIDTH, DEFAULT_RIGHT_PANEL_WIDTH)
    }
}

/// Idempotently sets the opencode-warp plugin entry in `~/.config/opencode/opencode.json`.
/// Removes any existing opencode-warp plugin entries (both local file:// and github:) and adds
/// the given `new_entry`. Creates the config file with a default structure if it doesn't exist.
#[cfg(debug_assertions)]
fn set_opencode_warp_plugin(new_entry: &str) -> String {
    let Some(home) = dirs::home_dir() else {
        return "Failed to determine home directory".to_string();
    };

    let config_dir = home.join(".config/opencode");
    let config_path = config_dir.join("opencode.json");

    let mut config: serde_json::Value = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(val) => val,
                Err(e) => return format!("Failed to parse opencode.json: {e}"),
            },
            Err(e) => return format!("Failed to read opencode.json: {e}"),
        }
    } else {
        serde_json::json!({
            "$schema": "https://opencode.ai/config.json"
        })
    };

    let plugins = config.as_object_mut().and_then(|obj| {
        obj.entry("plugin")
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
    });

    let Some(plugins) = plugins else {
        return "opencode.json has unexpected structure (plugin is not an array)".to_string();
    };

    // Remove any existing opencode-warp entries
    plugins.retain(|entry| {
        let s = entry.as_str().unwrap_or("");
        !s.contains("opencode-warp")
    });

    plugins.push(serde_json::Value::String(new_entry.to_string()));

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        return format!("Failed to create config directory: {e}");
    }

    match serde_json::to_string_pretty(&config) {
        Ok(json_str) => match std::fs::write(&config_path, format!("{json_str}\n")) {
            Ok(()) => format!("OpenCode plugin set to: {new_entry}"),
            Err(e) => format!("Failed to write opencode.json: {e}"),
        },
        Err(e) => format!("Failed to serialize opencode.json: {e}"),
    }
}
