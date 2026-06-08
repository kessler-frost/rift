use std::path::PathBuf;

use riftui::accessibility::AccessibilityVerbosity;
use riftui::geometry::rect::RectF;
use riftui::geometry::vector::Vector2F;
use riftui::platform::Cursor;
use ui_components::lightbox;

use super::tab_settings::{
    VerticalTabsCompactSubtitle, VerticalTabsDisplayGranularity, VerticalTabsPrimaryInfo,
    VerticalTabsTabItemMode, VerticalTabsViewMode,
};
use super::view::WorkspaceBanner;
use crate::auth::auth_manager::LoginGatedFeature;
use crate::palette::PaletteMode;
use crate::search;
use crate::server::telemetry::{
    AddTabWithShellSource, PaletteSource,
};
use crate::settings_view::{SettingsAction as SettingsTabAction, SettingsSection};
use crate::tab::{NewSessionMenuItem, SelectedTabColor};
use crate::tab_configs::TabConfig;
use crate::terminal::available_shells::AvailableShell;
use crate::themes::theme::AnsiColorIdentifier;
use crate::themes::theme_chooser::ThemeChooserMode;
use crate::workspace::tab_group::TabGroupId;
use crate::workspace::PaneViewLocator;

/// This enum determines how the search query is initialized when opening command search.
#[derive(Clone, Default, Debug)]
pub enum InitContent {
    /// Read the content of the active terminal input, and make that the initial search query.
    #[default]
    FromInputBuffer,
    /// Specify an exact string to initialize the query to.
    Custom(String),
}

/// To initialize command search, we may want to specify a search filter, or the content of the
/// query itself.
#[derive(Clone, Default, Debug)]
pub struct CommandSearchOptions {
    pub filter: Option<search::QueryFilter>,
    pub init_content: InitContent,
}

/// Specifies how to restore a conversation when it's not already open in a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum RestoreConversationLayout {
    /// Restore the conversation into the currently active pane.
    ActivePane,
    /// Restore the conversation in a new split pane.
    SplitPane,
    /// Restore the conversation in a new tab.
    #[default]
    NewTab,
}

#[derive(Debug, Clone, Copy)]
pub enum TabContextMenuAnchor {
    Pointer(Vector2F),
    VerticalTabsKebab,
}

/// Describes how the new-session dropdown menu was opened so the renderer
/// can pick the right anchor strategy.
#[derive(Debug, Clone, Copy)]
pub enum NewSessionMenuAnchor {
    /// Menu was opened from the `+` add-tab button. When vertical tabs are
    /// active, the renderer anchors below the button's save position;
    /// otherwise the contained position is used directly.
    AddTabButton(Vector2F),
    /// Menu was opened by right-clicking the vertical tabs panel.
    /// Always anchored at the contained pointer position.
    Pointer(Vector2F),
}

impl NewSessionMenuAnchor {
    pub fn position(&self) -> Vector2F {
        match self {
            Self::AddTabButton(position) | Self::Pointer(position) => *position,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum VerticalTabsPaneContextMenuTarget {
    ClickedPane(PaneViewLocator),
    ActivePane(PaneViewLocator),
}

impl VerticalTabsPaneContextMenuTarget {
    pub fn locator(self) -> PaneViewLocator {
        match self {
            Self::ClickedPane(locator) | Self::ActivePane(locator) => locator,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoCloudHandoffTrigger {
    MacOsSleep,
    Uri,
}

#[derive(Debug, Clone)]
pub enum WorkspaceAction {
    ActivateTab(usize),
    ActivatePrevTab,
    ActivateNextTab,
    ActivateLastTab,
    CyclePrevSession,
    CycleNextSession,
    MoveActiveTabLeft,
    MoveActiveTabRight,
    MoveTabLeft(usize),
    MoveTabRight(usize),
    RenameTab(usize),
    ResetTabName(usize),
    RenamePane(PaneViewLocator),
    ResetPaneName(PaneViewLocator),
    RenameActiveTab,
    /// Renames the focused pane in the active tab. Mirrors `RenameActiveTab`
    /// so the action is reachable from the binding registry / Command Palette
    /// (see #9351). The context-menu path keeps using `RenamePane(locator)`.
    RenameActivePane,
    SetActiveTabName(String),
    /// Sets the manual color override for the active tab.
    ///
    /// - `Color(_)` — apply that color.
    /// - `Cleared` — explicitly clear (suppresses any directory default).
    /// - `Unset` — remove the manual override (lets the directory default apply, if any).
    SetActiveTabColor(SelectedTabColor),
    ToggleTabRightClickMenu {
        tab_index: usize,
        anchor: TabContextMenuAnchor,
    },
    ToggleVerticalTabsPaneContextMenu {
        tab_index: usize,
        target: VerticalTabsPaneContextMenuTarget,
        position: Vector2F,
    },
    TabHoverWidthStart {
        width: f32,
    },
    TabHoverWidthEnd,
    ToggleTabBarOverflowMenu,
    ToggleWelcomeTips,
    CloseTab(usize),
    CloseActiveTab,
    CloseOtherTabs(usize),
    CloseNonActiveTabs,
    CloseTabsRight(usize),
    CloseTabsRightActiveTab,
    /// Close every tab that belongs to the given tab group.
    CloseTabGroup(TabGroupId),
    /// Toggle collapsed state for the given tab group.
    ToggleTabGroupCollapsed(TabGroupId),
    /// Opens an inline editor over the given group's header for renaming.
    RenameTabGroup(TabGroupId),
    /// Creates a new tab group containing the tab at the given index.
    NewTabGroupFromTab(usize),
    /// Moves the tab at `tab_index` into `group_id`, appending it to the
    /// end of the group's contiguous run.
    MoveTabToGroup {
        tab_index: usize,
        group_id: TabGroupId,
    },
    /// Removes the tab at the given index from its current group.
    RemoveTabFromGroup(usize),
    ToggleTabGroupRightClickMenu {
        group_id: TabGroupId,
        anchor: TabContextMenuAnchor,
    },
    UngroupTabs(TabGroupId),
    NewTabInGroup(TabGroupId),
    MoveTabGroupUp(TabGroupId),
    MoveTabGroupDown(TabGroupId),
    CloseTabsOutsideGroup(TabGroupId),
    CloseTabsAboveGroup(TabGroupId),
    CloseTabsBelowGroup(TabGroupId),
    AddDefaultTab,
    AddTerminalTab {
        hide_homepage: bool,
    },
    AddTabWithShell {
        shell: AvailableShell,
        source: AddTabWithShellSource,
    },
    AddGetStartedTab,
    OpenNewSessionMenu {
        anchor: NewSessionMenuAnchor,
    },
    ToggleTabConfigsMenu,
    ToggleNewSessionMenu {
        anchor: NewSessionMenuAnchor,
    },
    SelectNewSessionMenuItem(NewSessionMenuItem),
    LogOut,
    CopyVersion(&'static str),
    ConfigureKeybindingSettings {
        keybinding_name: Option<String>,
    },
    ShowSettings,
    ShowSettingsPage(SettingsSection),
    ShowSettingsPageWithSearch {
        search_query: String,
        section: Option<SettingsSection>,
    },
    ShowThemeChooser(ThemeChooserMode),
    ShowThemeChooserForActiveTheme,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    IncreaseZoom,
    DecreaseZoom,
    ResetZoom,
    ActivateTabByNumber(usize),
    OpenPalette {
        mode: PaletteMode,
        source: PaletteSource,
        query: Option<String>,
    },
    TogglePalette {
        mode: PaletteMode,
        source: PaletteSource,
    },
    JoinSlack,
    ViewUserDocs,
    ViewPrivacyPolicy,
    SendFeedback,
    /// Open the log directory in the system file explorer with the current log file selected.
    #[cfg(not(target_family = "wasm"))]
    ViewLogs,
    ChangeCursor(Cursor),
    ToggleBlockSnackbar,
    ToggleErrorUnderlining,
    ToggleSyntaxHighlighting,
    SetA11yVerbosityLevel(AccessibilityVerbosity),
    ToggleNotifications,
    ToggleTabColor {
        color: AnsiColorIdentifier,
        tab_index: usize,
    },
    OpenLaunchConfigSaveModal,
    SelectTabConfig(TabConfig),
    DispatchToSettingsTab(SettingsTabAction),
    ToggleResourceCenter,
    ToggleUserMenu,
    ToggleKeybindingsPage,
    ShowCommandSearch(CommandSearchOptions),
    ToggleMouseReporting,
    ToggleScrollReporting,
    ToggleFocusReporting,
    StartTabDrag,
    DragTab {
        tab_index: usize,
        tab_position: RectF,
    },
    DropTab,
    StartGroupDrag(TabGroupId),
    DragGroup {
        group_id: TabGroupId,
        position: RectF,
    },
    DropGroup,
    /// Toggles the left panel. In Code Mode V1 this toggles Warp Drive.
    /// In Code Mode V2 this toggles the left panel which contains both the project explorer and
    /// Warp Drive. This happens as explicit action from the user.
    ToggleLeftPanel,
    /// Toggles the right panel. This happens as an explicit action from the user.
    ToggleRightPanel,
    /// Toggles the vertical tabs panel. This happens as an explicit action from the user.
    ToggleVerticalTabsPanel,
    ToggleVerticalTabsSettingsPopup,
    SetVerticalTabsDisplayGranularity(VerticalTabsDisplayGranularity),
    SetVerticalTabsTabItemMode(VerticalTabsTabItemMode),
    SetVerticalTabsViewMode(VerticalTabsViewMode),
    SetVerticalTabsPrimaryInfo(VerticalTabsPrimaryInfo),
    SetVerticalTabsCompactSubtitle(VerticalTabsCompactSubtitle),
    ToggleVerticalTabsShowPrLink,
    ToggleVerticalTabsShowDiffStats,
    ToggleVerticalTabsShowDetailsOnHover,
    /// Closes the focused panel. This happens as an explicit action from the user.
    ClosePanel,
    CopyTextToClipboard(String),
    /// An action only registered in dev and local builds, which writes the user's current access
    /// token to the system clipboard to aid debugging and development.
    CopyAccessTokenToClipboard,
    DismissWorkspaceBanner(WorkspaceBanner),
    /// An action only registered in dev and local builds, which crashes the
    /// app (via a Sentry helper method) immediately when called.
    Crash,
    /// An action only registered in dev and local builds, which triggers a
    /// panic immediately when called.
    Panic,
    /// Stops the heap profiler (if one is running) and writes the profiling
    /// data to disk.
    DumpHeapProfile,
    /// An action to open a new window with a view hierarchy debugger.
    OpenViewTreeDebugWindow,
    /// An action to either upgrade syncing status from none or just in one tab
    /// to syncing all tabs, or downgrade from syncing all tabs to no syncing
    ToggleSyncAllTerminalInputsInAllTabs,
    /// An action to either cancel syncing
    /// or switch from no syncing/syncing all tabs to syncing within one tab
    ToggleSyncTerminalInputsInTab,
    /// An action to force terminal input syncing off
    DisableTerminalInputSync,
    OpenHeaderToolbarEditor,
    ShowHeaderToolbarContextMenu {
        position: Vector2F,
    },
    OpenLink(String),
    /// On WASM, opens a given URL in the desktop Warp app (if installed) or redirects to download page.
    #[cfg(target_family = "wasm")]
    OpenLinkOnDesktop(url::Url),
    ReopenClosedSession,
    AddWindow,
    AddWindowWithShell {
        shell: AvailableShell,
    },
    /// Moves focus to the panel on the left
    FocusLeftPanel,
    /// Moves focus to the panel on the right
    FocusRightPanel,
    /// Open a local path in the file explorer.
    OpenInExplorer {
        path: PathBuf,
    },
    /// Open a local file with the system's default application.
    OpenFilePath {
        path: PathBuf,
    },
    TerminateApp,
    CloseWindow,
    /// Help the user call the Warp executable with the [`crate::args::DEBUG_DUMP_FLAG`].
    DumpDebugInfo,
    ToggleRecordingMode,
    ToggleInBandGenerators,
    ToggleDebugNetworkStatus,
    ToggleShowMemoryStats,
    RunCommand(String),
    InsertInInput {
        content: String,
        replace_buffer: bool,
        /// Whether to ensure agent mode is enabled when inserting content
        ensure_agent_mode: bool,
    },
    /// Dismisses the Wayland crash recovery banner and opens a link to our docs page with more
    /// information.
    #[cfg(target_os = "linux")]
    DismissWaylandCrashRecoveryBannerAndOpenLink,
    /// Focus a specific pane by its locator (pane_group_id and pane_id).
    FocusPane(PaneViewLocator),
    ScrollToSettingsWidget {
        page: SettingsSection,
        widget_id: &'static str,
    },
    /// Install the Warp CLI command to /usr/local/bin
    #[cfg(target_os = "macos")]
    InstallCLI,
    /// Uninstall the Warp CLI command from /usr/local/bin
    #[cfg(target_os = "macos")]
    UninstallCLI,
    /// Open a repository directory via file picker. The `path` is an `Option` because some
    /// dispatchers don't know the path to open yet (so the Workspace must open the file picker)
    /// and some do, e.g. the GetStartedView. The GetStartedView needs to handle the file picker
    /// because it needs to determine whether or not to close itself based on whether the user
    /// actually selects a file in the file picker or cancels it.
    OpenRepository {
        path: Option<String>,
    },
    /// Open the native folder picker for a repo param in the tab-config modal after the
    /// current interaction cycle finishes.
    OpenTabConfigRepoPicker {
        param_index: usize,
    },
    NavigatePrevPaneOrPanel,
    NavigateNextPaneOrPanel,
    /// Take a process sample of the app (equivalent to Activity Monitor > Sample Process).
    #[cfg(target_os = "macos")]
    SampleProcess,
    ToggleNotificationMailbox {
        select_first: bool,
    },
    /// Open a full-window lightbox displaying the given images.
    OpenLightbox {
        images: Vec<lightbox::LightboxImage>,
        /// The index of the image to display initially.
        initial_index: usize,
    },
    /// Update a single image in the currently open lightbox.
    UpdateLightboxImage {
        index: usize,
        image: lightbox::LightboxImage,
    },
    ShowSessionConfigModal,
    DismissSessionConfigTabConfigChip,
    /// Open the "New worktree" modal for creating a reusable worktree tab config.
    OpenNewWorktreeModal,
    /// Open the native folder picker for the repo field in the new-worktree modal.
    OpenNewWorktreeRepoPicker,
    /// Create a new worktree in the given repo using the default worktree tab config.
    /// The branch name is auto-generated.
    OpenWorktreeInRepo {
        repo_path: String,
    },
    /// Open a folder picker to add a new repo to PersistedWorkspace (from the
    /// "New worktree config" submenu's "+ Add new repo..." item).
    OpenWorktreeAddRepoPicker,
    SaveCurrentTabAsNewConfig(usize),
    SyncTrafficLights,
    /// Opens a tab config file in the editor and dismisses the associated error toast.
    OpenTabConfigErrorFile {
        path: PathBuf,
        toast_object_id: String,
    },
    /// Sidecar action: set the hovered item as the Cmd+T default.
    TabConfigSidecarMakeDefault {
        mode: crate::settings::ai::DefaultSessionMode,
        tab_config_path: Option<PathBuf>,
        shell: Option<AvailableShell>,
    },
    /// Sidecar action: open the tab config TOML in the user's editor.
    TabConfigSidecarEditConfig {
        path: PathBuf,
    },
    /// Sidecar action: show the remove confirmation dialog for a tab config.
    TabConfigSidecarRemoveConfig {
        name: String,
        path: PathBuf,
    },
    /// Opens the settings.toml file in a code editor pane.
    OpenSettingsFile,
    /// Opens (or focuses) the in-app network log pane as a right-split of the
    /// active pane group. Gated on `ContextFlag::NetworkLogConsole`.
    OpenNetworkLogPane,
}

impl From<&WorkspaceAction> for LoginGatedFeature {
    fn from(_val: &WorkspaceAction) -> LoginGatedFeature {
        "Unknown reason"
    }
}

impl WorkspaceAction {
    pub fn blocked_for_anonymous_user(&self) -> bool {
        false
    }

    /// Matches what actions require the app state to be saved, and which don't. We match all
    /// actions directly, rather than using _, so we're forced to make a conscious decision for each
    /// of them, rather than following some default.
    pub fn should_save_app_state_on_action(&self) -> bool {
        use WorkspaceAction::*;
        // State-changing tab/window/panel actions should trigger a save so the
        // workspace can be restored after a restart; everything else is transient.
        matches!(
            self,
            ActivateTab(_)
                | ActivateTabByNumber(_)
                | ActivatePrevTab
                | ActivateNextTab
                | ActivateLastTab
                | CyclePrevSession
                | CycleNextSession
                | MoveActiveTabLeft
                | MoveActiveTabRight
                | MoveTabLeft(_)
                | MoveTabRight(_)
                | DropTab
                | DropGroup
                | RenameTab(_)
                | ResetTabName(_)
                | RenamePane(_)
                | ResetPaneName(_)
                | RenameActiveTab
                | RenameActivePane
                | SetActiveTabName(_)
                | SetActiveTabColor(_)
                | CloseTab(_)
                | CloseActiveTab
                | CloseOtherTabs(_)
                | CloseNonActiveTabs
                | CloseTabsRight(_)
                | CloseTabsRightActiveTab
                | CloseTabGroup(_)
                | ToggleTabGroupCollapsed(_)
                | RenameTabGroup(_)
                | NewTabGroupFromTab(_)
                | MoveTabToGroup { .. }
                | RemoveTabFromGroup(_)
                | UngroupTabs(_)
                | NewTabInGroup(_)
                | MoveTabGroupUp(_)
                | MoveTabGroupDown(_)
                | CloseTabsOutsideGroup(_)
                | CloseTabsAboveGroup(_)
                | CloseTabsBelowGroup(_)
                | ToggleTabColor { .. }
                | AddDefaultTab
                | AddTerminalTab { .. }
                | AddTabWithShell { .. }
                | AddGetStartedTab
                | AddWindow
                | AddWindowWithShell { .. }
                | CloseWindow
                | ScrollToSettingsWidget { .. }
                | OpenRepository { .. }
                | SelectTabConfig(_)
                | ToggleVerticalTabsPanel
        )
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
