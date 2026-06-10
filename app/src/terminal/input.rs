pub mod buffer_model;
mod classic;
mod common;
pub mod decorations;
pub mod inline_history;
pub mod inline_menu;
pub mod message_bar;
pub mod repos;
mod suggestions_mode_menu;
pub mod suggestions_mode_model;
mod terminal;
mod terminal_message_bar;

use std::any::Any;
use std::borrow::Cow;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use async_channel::Sender;
#[cfg(feature = "local_fs")]
use diesel::SqliteConnection;
use futures::stream::AbortHandle;
use futures::FutureExt as _;
use itertools::Itertools;
use ordered_float::Float;
use parking_lot::FairMutex;
#[cfg(feature = "local_fs")]
use parking_lot::Mutex;
use rift_completer::completer::{
    self, CompleterOptions, CompletionContext, CompletionsFallbackStrategy, Description, Match,
    MatchStrategy, MatchType, PathSeparators, SuggestionResults,
};
use rift_completer::meta::{HasSpan, Spanned};
use rift_completer::parsers::simple::command_at_cursor_position;
use rift_completer::parsers::LiteCommand;
use rift_completer::signatures::CommandRegistry;
use rift_core::r#async::debounce;
use rift_core::user_preferences::GetUserPreferences as _;
use rift_editor::editor::NavigationKey;
use rift_util::path::ShellFamily;
use riftui::accessibility::{AccessibilityContent, ActionAccessibilityContent, RiftA11yRole};
use riftui::clipboard::ClipboardContent;
use riftui::elements::{
    resizable_state_handle, AnchorPair, Clipped, ConstrainedBox, Container, DispatchEventResult, DropTargetData, Element, EventHandler, MouseStateHandle, OffsetType,
    ResizableStateHandle, SavePosition, SelectionHandle, YAxisAnchor,
};
pub use riftui::elements::{ParentElement as _, Stack};
pub use riftui::geometry::vector::{vec2f, Vector2F};
use riftui::keymap::{EditableBinding, FixedBinding, Keystroke};
use riftui::presenter::ChildView;
use riftui::r#async::SpawnedFutureHandle;
use riftui::units::IntoPixels;
pub use riftui::WindowId;
use riftui::{
    end_trace, start_trace, AppContext, Entity, EntityId, FocusContext, ModelAsRef, ModelHandle,
    SingletonEntity, TypedActionView, View, ViewContext, ViewHandle, WeakViewHandle,
};
use serde::Serialize;
use settings::{Setting as _, ToggleableSetting};
use string_offset::{ByteOffset, CharOffset};
use vim::vim::VimMode;

use self::decorations::InputBackgroundJobOptions;
use super::alias::is_expandable_alias;
use super::event::{BlockCompletedEvent, BlockType};
use super::ligature_settings::LigatureSettings;
use super::model::block::{BlockMetadata, BlocklistEnvVarMetadata};
use super::model::session::{Session, SessionId, Sessions};
use super::prompt_render_helper::{
    should_render_prompt_on_same_line, should_render_prompt_using_editor_decorator_elements,
    PromptRenderHelper, SameLinePromptElements,
};
use super::safe_mode_settings::{
    get_secret_obfuscation_mode, SafeModeSettings, SafeModeSettingsChangedEvent,
};
use super::session_settings::{SessionSettings, SessionSettingsChangedEvent};
use super::settings::{TerminalSettings, TerminalSettingsChangedEvent};
use super::view::{
    ExecuteCommandEvent, SyncInputType, TerminalAction, PADDING_LEFT as TERMINAL_VIEW_PADDING_LEFT,
};
use super::riftify::SubshellSource;
use super::{prompt, History, HistoryEntry, SizeInfo, TerminalModel, UpArrowHistoryConfig};
use crate::appearance::{Appearance, AppearanceEvent};
use crate::completer::SessionContext;
use crate::context_chips::display::PromptDisplay;
use crate::context_chips::prompt_type::PromptType;
use crate::editor::{
    default_cursor_colors, position_id_for_cached_point, position_id_for_cursor,
    position_id_for_first_cursor, AutosuggestionLocation,
    AutosuggestionType, BaselinePositionComputationMethod, CommandXRayAnchor, DisplayPoint, EditOrigin, EditorAction, EditorDecoratorElements, EditorOptions,
    EditorSnapshot, EditorView, Event as EditorEvent, InteractionState,
    PathTransformerFn, PlainTextEditorViewAction, Point as BufferPoint, PropagateAndNoOpEscapeKey,
    PropagateAndNoOpNavigationKeys, PropagateHorizontalNavigationKeys, TextColors,
};
use crate::features::FeatureFlag;
use crate::input_suggestions::{
    HistoryInputSuggestion, InputSuggestions,
    TabCompletionsPreselectOption,
};
use crate::pane_group::focus_state::PaneFocusHandle;
use crate::pane_group::PaneGroupAction;
#[cfg(feature = "local_fs")]
use crate::persistence::{database_file_path_for_scope, establish_ro_connection, PersistenceScope};
use crate::prefix::longest_common_prefix;
use crate::resource_center::{
    mark_feature_used_and_write_to_user_defaults, Tip, TipHint, TipsCompleted,
};
use crate::search::QueryFilter;
use crate::server::telemetry::{
    CommandXRayTrigger,
    TelemetryEvent, 
};
use crate::session_management::SessionNavigationPromptElements;
use crate::settings::{
    AliasExpansionSettings, AppEditorSettings,
    AppEditorSettingsChangedEvent, InputModeSettings, InputSettings, InputSettingsChangedEvent, MAX_TIMES_TO_SHOW_AUTOSUGGESTION_HINT,
};
use crate::settings_view::{flags, SettingsSection};
use crate::suggestions::ignored_suggestions_model::{
    IgnoredSuggestionsModel, IgnoredSuggestionsModelEvent, SuggestionType,
};
use crate::terminal::input::buffer_model::InputBufferModel;
use crate::terminal::input::inline_menu::InlineMenuPositioner;
use crate::terminal::input::suggestions_mode_model::{
    InputSuggestionsModeEvent, InputSuggestionsModeModel,
};
use crate::terminal::input::terminal_message_bar::TerminalInputMessageBar;
use crate::terminal::model::session::active_session::ActiveSession;
use crate::util::bindings::{self, CustomAction};
use crate::util::truncation::truncate_from_end;
use crate::view_components::{DismissibleToast, ToastFlavor};
use crate::workspace::sync_inputs::SyncedInputState;
use crate::workspace::{
    CommandSearchOptions, InitContent,
    ToastStack, WorkspaceAction,
};
#[allow(unused_imports)]
use crate::ASSETS;
#[allow(unused_imports)]
use crate::{
    cmd_or_ctrl_shift, report_if_error, send_telemetry_from_ctx,
};

/// Drop target data for dropping content on the [`Input`].
#[derive(Debug, Clone)]
pub struct InputDropTargetData {
    pub input_view: WeakViewHandle<Input>,
}

impl InputDropTargetData {
    fn new(input_view: WeakViewHandle<Input>) -> Self {
        Self { input_view }
    }

    pub fn weak_view_handle(&self) -> WeakViewHandle<Input> {
        self.input_view.clone()
    }
}

impl DropTargetData for InputDropTargetData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub const DEBOUNCE_INPUT_DECORATION_PERIOD: Duration = Duration::from_millis(10);
pub const DEBOUNCE_AI_QUERY_PREDICTION_PERIOD: Duration = Duration::from_millis(250);
const SHORT_CIRCUIT_HIGHLIGHTING_ACTIONS: [Option<PlainTextEditorViewAction>; 7] = [
    Some(PlainTextEditorViewAction::Space),
    Some(PlainTextEditorViewAction::NonExpandingSpace),
    Some(PlainTextEditorViewAction::Paste),
    Some(PlainTextEditorViewAction::Tab),
    Some(PlainTextEditorViewAction::AcceptCompletionSuggestion),
    Some(PlainTextEditorViewAction::CursorChanged),
    Some(PlainTextEditorViewAction::NewLine),
];

/// Border width for the line at the top of the input box in pixels
pub fn get_input_box_top_border_width() -> f32 {
    if FeatureFlag::MinimalistUI.is_enabled() {
        0.0
    } else {
        1.0
    }
}

pub const COMPLETIONS_MENU_WIDTH: f32 = 330.;
pub const OPEN_COMPLETIONS_KEYBINDING_NAME: &str = "input:open_completion_suggestions";
pub const INPUT_A11Y_LABEL: &str = "Command Input.";
pub const INPUT_A11Y_HELPER: &str = "Input your shell command, press enter to execute. Press cmd-up to navigate to output of previously executed commands. Press cmd-l to re-focus command input.";

/// The position ID used to identify the start of the replacement span for completions.
const COMPLETIONS_START_OF_REPLACEMENT_SPAN_POSITION_ID: &str =
    "start_of_completions_replacement_span";

const HISTORY_DETAILS_VIEW_WIDTH_REQUIREMENT: f32 = 1100.;

const MIN_BUFFER_LEN_TO_SHOW_COMPLETIONS_WHILE_TYPING: usize = 2;

const AI_COMMAND_SEARCH_TRIGGER: &str = "#";

#[derive(PartialEq, Eq, Copy, Clone, Serialize)]
pub enum TelemetryInputSuggestionsMode {
    HistoryFuzzySearch,
    CompletionSuggestions,
    HistoryUp,
    NaturalLanguageCommandSearch,
    StaticWorkflowEnumSuggestions,
    DynamicWorkflowEnumSuggestions,
    AIContextMenu,
    SlashCommands,
    ConversationMenu,
    ModelSelector,
    ProfileSelector,
    PromptsMenu,
    SkillMenu,
    InlineHistoryMenu,
    IndexedReposMenu,
    PlanMenu,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HistorySearchMode {
    /// Prefix match commands.
    Prefix,
    /// Fuzzy match commands.
    Fuzzy,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum TabCompletionsMenuPosition {
    /// The menu should be positioned at the last cursor.
    AtLastCursor,
    /// The menu should be positioned at the first cursor.
    AtFirstCursor,
    /// The menu should be positioned at the given position.
    AtStartOfReplacementSpan,
}

impl TabCompletionsMenuPosition {
    fn to_position_id(self, editor_view_id: EntityId) -> String {
        match self {
            Self::AtLastCursor => position_id_for_cursor(editor_view_id),
            Self::AtFirstCursor => position_id_for_first_cursor(editor_view_id),
            Self::AtStartOfReplacementSpan => position_id_for_cached_point(
                editor_view_id,
                COMPLETIONS_START_OF_REPLACEMENT_SPAN_POSITION_ID,
            ),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct BufferState {
    buffer: String,
    cursor_point: Option<BufferPoint>,
}

impl BufferState {
    pub fn new(buffer: String, cursor_point: Option<BufferPoint>) -> Self {
        Self {
            buffer,
            cursor_point,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum InputSuggestionsMode {
    /// Mode used when arrow-up is pressed.
    HistoryUp {
        /// Text in the buffer when arrow-up is pressed (possibly empty).
        original_buffer: String,
        /// Cursor point when arrow-up is pressed.
        /// This is None when there are > 1 active selections when HistoryUp is invoked.
        /// TODO: eventually, we should support saving/resetting _many_ cursors rather than a single one.
        original_cursor_point: Option<BufferPoint>,
        search_mode: HistorySearchMode,
    },
    CompletionSuggestions {
        /// Stores the byte index of the beginning of the text we are replacing
        replacement_start: usize,

        /// Stores the original buffer text before the user pressed TAB.
        /// Used to close the suggestions menu if the buffer_text_original is no longer in the input buffer.
        buffer_text_original: String,

        /// Stores the suggestions for the original buffer_text_original.
        /// Used to filter down results during prefix search.
        completion_results: SuggestionResults,

        /// Stores the original trigger of the completions, so that we can track whether the menu
        /// was opened automatically (AsYouType) or manually (with Tab)
        trigger: CompletionsTrigger,

        /// Where the menu should be positioned.
        menu_position: TabCompletionsMenuPosition,
    },

    /// Mode indicating that no suggestion UI is being shown.
    Closed,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UserQueryMenuAction {
    ForkFrom,
    Rewind,
}

impl InputSuggestionsMode {
    pub fn is_visible(&self) -> bool {
        *self != InputSuggestionsMode::Closed
    }

    pub fn is_inline_menu(&self) -> bool {
        false
    }

    /// Whether this mode should snapshot the input buffer on open and restore it on dismiss.
    fn should_snapshot_and_restore_buffer(&self) -> bool {
        // For now this just delegates to whether the current mode is an inline menu,
        // but in the future we might build this out/add more detail here.
        self.is_inline_menu()
    }


    /// Returns the placeholder text for this mode, if it has a custom one.
    pub fn placeholder_text(&self) -> Option<&'static str> {
        None
    }

}

/// Where a command execution request originates from.
#[derive(Clone)]
pub enum CommandExecutionSource {
    /// A normal command execution request.
    User,

    EnvVarCollection {
        metadata: BlocklistEnvVarMetadata,
    },
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum HistoryUpMode {
    // Show prefixed results.
    Prefixed,
    // Show all results with no query.
    RegularNoQuery,
    // Show all results with query.
    RegularWithQuery,
    // Used for ConfirmSuggestion event.
    NotApplicable,
}

impl HistoryUpMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            HistoryUpMode::Prefixed => "prefixed history up",
            HistoryUpMode::RegularNoQuery => "regular history up (no query)",
            HistoryUpMode::RegularWithQuery => "regular history up (with query)",
            HistoryUpMode::NotApplicable => "history up",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEmptyStateChangeReason {
    /// The buffer transitioned between empty and non-empty due to a regular edit.
    Edited,
    /// The buffer was cleared because a user-executed command completed and we reinitialized the
    /// buffer for the next command.
    UserCommandCompleted,
}

pub enum Event {
    AutosuggestionAccepted,
    ClearSelectedBlock,
    PageUp,
    PageDown,
    SelectRecentBlocks {
        /// Select the `count` most recent blocks.
        count: usize,
    },
    Copy,
    UnhandledModifierKeyOnEditor(Arc<String>),
    ClearSelectionsWhenShellMode,
    InputStateChanged(InputState),
    /// Emitted when the input text transitions between empty and non-empty states
    InputEmptyStateChanged {
        is_empty: bool,
        reason: InputEmptyStateChangeReason,
    },
    Escape,
    /// note: Terminal Inputs should only emit the variant
    /// SyncInputType::InputEditorContentsChanged.
    SyncInput(SyncInputType),
    ShowCommandSearch(CommandSearchOptions),
    CtrlD,
    CtrlC {
        // The number of chars cleared from the buffer, if the ctrl-c triggered a buffer clear.
        cleared_buffer_len: usize,
    },
    Enter,
    ExecuteCommand(Box<ExecuteCommandEvent>),
    EmacsBindingUsed,
    InputFocusedFromMiddleClick,
    EditorFocused,
    OpenSettings(SettingsSection),
    ShowToast {
        message: String,
        flavor: ToastFlavor,
    },
}

pub enum InputState {
    Enabled,
    Disabled,
}

#[derive(Clone, Debug)]
pub enum InputAction {
    FocusInputBox,
    CtrlR,
    CtrlD,
    Up,
    PageUp,
    PageDown,
    ClearScreen,
    /// Open the completions menu if the cursor is in a valid position to generate completion
    /// suggestions.
    MaybeOpenCompletionSuggestions,

    ToggleClassicCompletionsMode,

    /// Persist the completions menu width when the user resizes it.
    UpdateCompletionsMenuWidth(f32),

    /// Persist the completions menu height when the user resizes it.
    UpdateCompletionsMenuHeight(f32),


    /// Opens the inline history menu for cycling through past commands and conversations.
    OpenInlineHistoryMenu,

}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum MenuPositioning {
    /// Position floating input menus above the input box -- corresponds
    /// to the regular blocklist.
    #[default]
    AboveInputBox,

    /// Position floating input menus below the input box -- corresponds
    /// to the inverted blocklist.
    BelowInputBox,
}

impl MenuPositioning {
    fn completion_suggestions_y_anchor(&self) -> AnchorPair<YAxisAnchor> {
        self.y_anchor()
    }

    fn history_y_anchor(&self) -> AnchorPair<YAxisAnchor> {
        self.y_anchor()
    }

    fn history_y_offset(&self) -> OffsetType {
        match *self {
            MenuPositioning::AboveInputBox => OffsetType::Pixel(0.),
            MenuPositioning::BelowInputBox => OffsetType::Pixel(-11.),
        }
    }

    fn command_xray_y_anchor(&self) -> AnchorPair<YAxisAnchor> {
        self.y_anchor()
    }


    fn y_anchor(&self) -> AnchorPair<YAxisAnchor> {
        match *self {
            MenuPositioning::AboveInputBox => {
                AnchorPair::new(YAxisAnchor::Top, YAxisAnchor::Bottom)
            }
            MenuPositioning::BelowInputBox => {
                AnchorPair::new(YAxisAnchor::Bottom, YAxisAnchor::Top)
            }
        }
    }
}

impl MenuPositioningProvider for MenuPositioning {
    fn menu_position(&self, _app: &AppContext) -> MenuPositioning {
        *self
    }
}

/// Helper struct for performing alias expansion.
struct ExpansionInfo {
    /// The expanded text to replace the alias with.
    alias_value: String,
    /// The buffer text to replace the alias in.
    buffer_text: String,
    /// The byte indices that should be replaced with the alias_value.
    byte_range: Range<usize>,
}

/// For inserting last word of last command in history - by default, this is the last command but consecutive
/// inserts fetch further in history. Represents reverse index of history command to reference.
/// (insert_command_from_history_index=0 for most recent, 1 for command before it, etc.) See self.update_last_word_insertion_state()
struct LastWordInsertion {
    insert_command_from_history_index: usize,
    is_latest_editor_event: bool,
}

/// Data pertaining to the session state and history is bundled together, making
/// it accessible to other objects coupled with the same terminal session, such as a notebook.
#[derive(Clone)]
pub struct CompleterData {
    pub sessions: ModelHandle<Sessions>,
    pub active_block_metadata: Option<BlockMetadata>,
    command_registry: Arc<CommandRegistry>,
}

impl CompleterData {
    pub fn new(
        sessions: ModelHandle<Sessions>,
        active_block_metadata: Option<BlockMetadata>,
        command_registry: Arc<CommandRegistry>,
    ) -> Self {
        Self {
            sessions,
            active_block_metadata,
            command_registry,
        }
    }

    pub fn active_block_session_id(&self) -> Option<SessionId> {
        self.active_block_metadata
            .as_ref()
            .and_then(BlockMetadata::session_id)
    }

    pub fn completion_session_context(&self, app: &AppContext) -> Option<SessionContext> {
        let active_block_session_id = self.active_block_session_id()?;
        let current_session = self.sessions.as_ref(app).get(active_block_session_id);
        let pwd = self
            .active_block_metadata
            .as_ref()
            .and_then(BlockMetadata::current_working_directory)
            .map(str::to_owned);

        current_session.zip(pwd).map(|(current_session, pwd)| {
            // TODO(abhishek): Ideally, BlockMetadata::current_working_directory should directly
            // return a TypedPathBuf. This shouldn't happen here in the view.
            let current_working_directory =
                current_session.convert_directory_to_typed_path_buf(pwd);

            SessionContext::new(
                current_session,
                self.command_registry.clone(),
                current_working_directory,
                app,
            )
        })
    }
}

/// Autosuggestion result returned by the generator.
pub struct AutoSuggestionResult {
    /// Text in the editor buffer.
    pub buffer_text: String,
    /// Generated autosuggestion result.
    pub autosuggestion_result: Option<String>,
}

/// Views that call into the autosuggestion generation logic must implement the Autosuggester
/// trait. This requires a callback on_autosuggestion_result and functions to set and abort
/// the latest future that's been spawned for autosuggestions.
pub trait Autosuggester {
    fn on_autosuggestion_result(
        &mut self,
        _result: AutoSuggestionResult,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    fn abort_latest_autosuggestion_future(&mut self);

    fn set_autosuggestion_future(&mut self, abort_handle: AbortHandle);
}

/// Implement this trait to provide whether menus like autocomplete, voltron, etc
/// should be positionined above or below the input.
pub trait MenuPositioningProvider {
    fn menu_position(&self, app: &AppContext) -> MenuPositioning;

    fn inline_menu_position(&self, _inline_menu_height: f32, _app: &AppContext) -> MenuPositioning {
        MenuPositioning::AboveInputBox
    }
}

/// Stores state referenced by the Input view and PromptRenderHelper.
/// Note that this is largely a workaround to avoid having to pass/upgrade
/// a weak view handle from `Input` to `PromptRenderHelper` for this state.
pub struct InputRenderStateModel {
    editor_modified_since_block_finished: bool,
    // For future: we should explore reading this directly off TerminalModel.
    size_info: SizeInfo,
}

impl InputRenderStateModel {
    pub fn new(editor_modified_since_block_finished: bool, size_info: SizeInfo) -> Self {
        Self {
            editor_modified_since_block_finished,
            size_info,
        }
    }

    pub fn editor_modified_since_block_finished(&self) -> bool {
        self.editor_modified_since_block_finished
    }

    pub fn size_info(&self) -> SizeInfo {
        self.size_info
    }

    pub fn set_editor_modified_since_block_finished(
        &mut self,
        editor_modified_since_block_finished: bool,
    ) {
        self.editor_modified_since_block_finished = editor_modified_since_block_finished;
    }

    pub fn set_size_info(&mut self, size_info: SizeInfo) {
        self.size_info = size_info;
    }
}

impl Entity for InputRenderStateModel {
    type Event = ();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DenyExecutionReason {
    /// Can't execute command because shell bootstrapping is still underway; shell isn't ready to
    /// execute user-supplied commands yet.
    NotBootstrapped,

    /// Can't execute command because there's an active command in control of the pty.
    ExistingActiveCommand,

    /// With the exception of shared sessions, we should only execute commands if they can be
    /// recorded in history.
    ///
    /// Gonna be honest, I (zach b) have the least amount of context on this one, don't really know
    /// why this is the case.
    ///
    /// This is not returned as a `CancellationReason::No` for shared sessions even if it may be
    /// true; we do not record shared sessions in the History model thus they are default not-
    /// appendable.
    HistoryNotAppendable,
}

impl DenyExecutionReason {
    pub fn is_existing_active_command(&self) -> bool {
        matches!(self, DenyExecutionReason::ExistingActiveCommand)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanExecuteCommand {
    Yes,
    No(DenyExecutionReason),
}

impl CanExecuteCommand {
    pub fn is_no(&self) -> bool {
        matches!(self, CanExecuteCommand::No(_))
    }
}

pub struct Input {
    model: Arc<FairMutex<TerminalModel>>,
    menu_positioning_provider: Arc<dyn MenuPositioningProvider>,
    tips_completed: ModelHandle<TipsCompleted>,
    editor: ViewHandle<EditorView>,
    input_suggestions: ViewHandle<InputSuggestions>,
    suggestions_mode_model: ModelHandle<InputSuggestionsModeModel>,
    completions_menu_resizable_width: ResizableStateHandle,
    completions_menu_resizable_height: ResizableStateHandle,
    sessions: ModelHandle<Sessions>,
    focus_handle: Option<PaneFocusHandle>,
    active_block_metadata: Option<BlockMetadata>,
    /// The [`EntityId`] of the terminal view that this input view is inside.
    terminal_view_id: EntityId,
    view_id: EntityId,
    input_render_state_model_handle: ModelHandle<InputRenderStateModel>,
    command_x_ray_description: Option<Arc<Description>>,
    last_parsed_tokens: Option<decorations::ParsedTokensSnapshot>,
    debounce_input_background_tx: Sender<InputBackgroundJobOptions>,
    /// If true, will submit the command in the editor to the shell upon receiving the
    /// precmd message.
    has_pending_command: bool,
    last_word_insertion: LastWordInsertion,

    /// To ensure we only have one run of completions-as-you-type at any given time,
    /// we keep an abort handle of the current run. If we have reason to start a new run
    /// (e.g. new input), we simply abort the existing run. The same applies to the
    /// syntax highlighting and autosuggestions features (all which use the completer).
    completions_abort_handle: Option<AbortHandle>,
    decorations_future_handle: Option<SpawnedFutureHandle>,
    autosuggestions_abort_handle: Option<AbortHandle>,

    pub prompt_render_helper: PromptRenderHelper,
    prompt_type: ModelHandle<PromptType>,
    // A cached copy of enable_autosuggestions from settings (to avoid
    // a settings read on every typed character).
    enable_autosuggestions_setting: bool,

    hoverable_handle: MouseStateHandle,

    #[cfg(feature = "local_fs")]
    conn: Option<Arc<Mutex<SqliteConnection>>>,


    is_processing_attached_images: bool,

    terminal_input_message_bar: ViewHandle<TerminalInputMessageBar>,

    inline_terminal_menu_positioner: ModelHandle<InlineMenuPositioner>,

    /// Cached flag indicating whether the editor buffer is empty, used to track changes between
    /// empty and non-empty states.
    ///
    /// If simply looking for if the editor contents empty, check the editor view directly instead
    /// of using this flag.
    is_editor_empty_on_last_edit: bool,

    /// Weak handle to this input view for drop target data
    weak_view_handle: WeakViewHandle<Input>,

    /// When a command is executed from a prompt chip (e.g. `cd` from the directory dropdown),
    /// we snapshot the current input contents here so we can restore them after the command
    /// completes and the buffer would normally be cleared.
    input_contents_before_prompt_chip_command: Option<String>,
}

pub fn init(app: &mut AppContext) {
    use riftui::keymap::macros::*;

    if cfg!(feature = "integration_tests") {
        app.register_fixed_bindings([
            // Hack: Add explicit ctrl-r binding for integration tests, since the tests' injected
            // keypresses won't trigger Mac menu items. Unfortunately we can't use
            // cfg[test] because we are a separate process!
            FixedBinding::new(
                "ctrl-r",
                WorkspaceAction::ShowCommandSearch(Default::default()),
                id!("Input") & !id!("VoltronActive"),
            ),
        ]);
    }

    app.register_fixed_bindings(vec![
        FixedBinding::new("ctrl-d", InputAction::CtrlD, id!("Input")),
        FixedBinding::custom(
            CustomAction::History,
            InputAction::Up,
            "Show History",
            // We need to ensure the workflow info box is not open as the "up" arrow
            // key is used to navigate the environment variables dropdown.
            // Same goes with the LLM menu.
            id!("Input")
                & !id!("IMEOpen")
                & !id!("VoltronActive")
                & !id!("ProfileModelSelectorOpen")
                & !id!("PromptChipMenuOpen")
                & !id!("AIContextMenuOpen")
                & !id!("BuyCreditsBannerOpen"),
        ),
    ]);

    app.register_editable_bindings([EditableBinding::new(
        "input:clear_screen",
        "Clear screen",
        InputAction::ClearScreen,
    )
    .with_context_predicate(id!("Input"))
    .with_key_binding("ctrl-l")]);

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:scroll_up_one_page",
            "Scroll terminal output up one page",
            InputAction::PageUp,
        )
        .with_context_predicate(id!("Input") & !id!("IMEOpen"))
        .with_key_binding("pageup"),
        EditableBinding::new(
            "terminal:scroll_down_one_page",
            "Scroll terminal output down one page",
            InputAction::PageDown,
        )
        .with_context_predicate(id!("Input") & !id!("IMEOpen"))
        .with_key_binding("pagedown"),
    ]);


    if FeatureFlag::ClassicCompletions.is_enabled()
        && !FeatureFlag::ForceClassicCompletions.is_enabled()
    {
        app.register_editable_bindings([EditableBinding::new(
            "input:toggle_classic_completions_mode",
            "(Experimental) Toggle classic completions mode",
            InputAction::ToggleClassicCompletionsMode,
        )
        .with_context_predicate(id!("Input"))]);
    }

    // Register editable bindings relating to Command Search.
    app.register_editable_bindings([
        EditableBinding::new(
            "workspace:show_command_search",
            "Command Search",
            WorkspaceAction::ShowCommandSearch(Default::default()),
        )
        // Only show command search if none of the input-related panels are open, and if we aren't
        // in Vim normal mode. Command Search is ctrl-r by default, and so is Redo in Vim (in
        // normal mode). So, the child should be allowed to handle this action first. Child views
        // normally do get first precedence to handle keybindings, but this is _not_ the case when
        // a parent view binds a CustomAction, which is what is happening here in the Input view.
        // Therefore, this binding is guarded with !id!("VimNormalMode"). Note that although there
        // is usually a conflict between these, that isn't always the case if the user has
        // re-mapped CommandSearch to something else. However, we don't account for that here.
        .with_context_predicate(id!("Input") & !id!("VoltronActive") & !id!("VimNormalMode"))
        .with_custom_action(CustomAction::CommandSearch),
        EditableBinding::new(
            "input:search_command_history",
            "History Search",
            WorkspaceAction::ShowCommandSearch(CommandSearchOptions {
                filter: Some(QueryFilter::History),
                init_content: Default::default(),
            }),
        )
        .with_context_predicate(id!("Input") & !id!("VoltronActive"))
        .with_custom_action(CustomAction::HistorySearch),
        EditableBinding::new(
            OPEN_COMPLETIONS_KEYBINDING_NAME,
            "Open completions menu",
            InputAction::MaybeOpenCompletionSuggestions,
        )
        .with_context_predicate(id!("Input"))
        .with_key_binding("tab"),
    ]);




}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CompletionsTrigger {
    Keybinding,
    AsYouType,
}

/// Represents whether the input editor should render the subshell flag.
#[derive(Clone, Debug)]
enum SubshellRenderState {
    /// Contains the subshell-spawning command for the flag. Render the flag
    /// and extend the flag into the input editor.
    Flag(SubshellSource),
    /// The input is inside a subshell, extend the flag into the input editor,
    /// but do not render the actual flag.
    Flagpole,
}

/// Represents whether a command is currently being executed.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Executing {
    Yes,
    No,
}

impl Input {
    pub fn send_input_buffer_to_terminal_editor(
        &mut self,
        buffer_contents: Arc<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.editor.update(ctx, |editor, ctx| {
            editor.set_buffer_text_for_syncing_inputs(buffer_contents, ctx);
        });
    }

    pub fn run_command_in_synced_terminal_input(&mut self, ctx: &mut ViewContext<Self>) {
        self.has_pending_command = true;
        self.execute_pending_command(ctx);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        model: Arc<FairMutex<TerminalModel>>,
        tips_completed: ModelHandle<TipsCompleted>,
        sessions: ModelHandle<Sessions>,
        size_info: SizeInfo,
        menu_positioning_provider: Arc<dyn MenuPositioningProvider>,
        current_prompt: ModelHandle<PromptType>,
        terminal_view_id: EntityId,
        current_repo_path: Option<PathBuf>,
        model_events: ModelHandle<crate::terminal::model_events::ModelEventDispatcher>,
        active_session: ModelHandle<ActiveSession>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let initial_session_context = {
            let completer_data = CompleterData::new(
                sessions.clone(),
                None, // active_block_metadata will be set later when blocks are available
                CommandRegistry::global_instance(),
            );
            completer_data.completion_session_context(ctx)
        };

        let prompt_view = ctx.add_typed_action_view(|ctx| {
            PromptDisplay::new(
                current_prompt.clone(),
                terminal_view_id,
                menu_positioning_provider.clone(),
                initial_session_context.clone(),
                current_repo_path.clone(),
                model_events.clone(),
                ctx,
            )
        });
        ctx.subscribe_to_model(&Appearance::handle(ctx), move |me, _, event, ctx| {
            if let AppearanceEvent::ThemeChanged = event {
                me.handle_theme_change(ctx);
            }
        });
        ctx.subscribe_to_model(&TerminalSettings::handle(ctx), move |_, _, event, ctx| {
            if let TerminalSettingsChangedEvent::Spacing { .. } = event {
                ctx.notify();
            }
        });

        let prompt_selection_state_handle = SelectionHandle::default();
        let view_id = ctx.view_id();
        let input_render_state_model_handle: ModelHandle<InputRenderStateModel> =
            ctx.add_model(|_| InputRenderStateModel::new(false, size_info));

        let prompt_render_helper = PromptRenderHelper::new(
            sessions.clone(),
            prompt_view,
            prompt_selection_state_handle,
            view_id,
            input_render_state_model_handle.clone(),
        );

        let has_prompt_suggestion_banner = Arc::new(AtomicBool::new(false));
        let editor = {
            // Clones used in render_decorator_elements closure below.
            let prompt_render_helper_clone = prompt_render_helper.clone();
            let model_clone = model.clone();
            // Clone used in keymap_context_modifier closure below.
            let _terminal_model_for_keymap_context = model.clone();
            let _has_prompt_suggestion_banner_for_keymap = has_prompt_suggestion_banner.clone();
            let input_render_state_model_handle_clone = input_render_state_model_handle.clone();

            ctx.add_typed_action_view(|ctx| {
                let options = EditorOptions {
                    autogrow: true,
                    autocomplete_symbols: true,
                    propagate_and_no_op_vertical_navigation_keys:
                        PropagateAndNoOpNavigationKeys::Always,
                    propagate_horizontal_navigation_keys:
                        PropagateHorizontalNavigationKeys::AtBoundary,
                    propagate_and_no_op_escape_key: PropagateAndNoOpEscapeKey::PropagateFirst,
                    soft_wrap: true,
                    supports_vim_mode: true,
                    use_settings_line_height_ratio: true,
                    render_decorator_elements: Some(Box::new(
                        move |app| -> EditorDecoratorElements {
                            let terminal_model = model_clone.lock();
                            let active_block = terminal_model.block_list().active_block();

                            let mut editor_decorator_elements = EditorDecoratorElements::default();

                            let is_universal_developer_input_enabled = InputSettings::as_ref(app)
                                .is_universal_developer_input_enabled(app);

                            if should_render_prompt_using_editor_decorator_elements(
                                is_universal_developer_input_enabled,
                                &terminal_model,
                                app,
                            ) {
                                let SameLinePromptElements {
                                    lprompt_top,
                                    lprompt_bottom,
                                    rprompt,
                                } = prompt_render_helper_clone.render_same_line_prompt_areas(
                                    &terminal_model,
                                    Appearance::as_ref(app),
                                    app,
                                );

                                editor_decorator_elements.top_section = lprompt_top;
                                editor_decorator_elements.left_notch = lprompt_bottom;
                                editor_decorator_elements.right_notch = rprompt;
                                editor_decorator_elements.right_notch_offset_px = Some(
                                    active_block.rprompt_render_offset(
                                        &input_render_state_model_handle_clone
                                            .as_ref(app)
                                            .size_info,
                                    ),
                                )
                            }

                            editor_decorator_elements
                        },
                    )),
                    cursor_colors_fn: Box::new(move |app| default_cursor_colors(app)),
                    baseline_position_computation_method: BaselinePositionComputationMethod::Grid,
                    // We implement middle-click paste at the [`TerminalView`] level,
                    // and we don't want to double-paste.
                    middle_click_paste: false,
                    allow_user_cursor_preference: true,
                    delegate_paste_handling: true,
                    keymap_context_modifier: Some(Box::new(move |context, _app| {
                        context
                            .set
                            .insert(flags::TERMINAL_INPUT_PAGE_KEYS_HANDLED_BY_INPUT);
                    })),
                    ..Default::default()
                };
                EditorView::new(options, ctx)
            })
        };

        let buffer_model = ctx.add_model(|ctx| InputBufferModel::new(&editor, ctx));
        let suggestions_mode_model =
            ctx.add_model(|_| InputSuggestionsModeModel::new(buffer_model.clone()));

        let terminal_content_element_position_id =
            format!("terminal_content_element_{terminal_view_id}");
        let input_save_position_id = format!("status_free_input_{}", ctx.view_id());
        let window_id = ctx.window_id();
        let inline_terminal_menu_positioner = ctx.add_model(|ctx| {
            InlineMenuPositioner::new(
                &suggestions_mode_model,
                terminal_content_element_position_id,
                input_save_position_id,
                size_info,
                window_id,
                ctx,
            )
        });

        let inline_history_menu_view = ctx.add_view({
            let active_session = active_session.clone();
            let buffer_model = buffer_model.clone();
            |ctx| {
                inline_history::InlineHistoryMenuView::new(
                    terminal_view_id,
                    active_session,
                    &suggestions_mode_model,
                    &inline_terminal_menu_positioner,
                    buffer_model,
                    ctx,
                )
            }
        });
        let inline_history_model = inline_history_menu_view.as_ref(ctx).model().clone();

        let terminal_input_message_bar = ctx.add_view(|ctx| {
            TerminalInputMessageBar::new(
                model.clone(),
                buffer_model.clone(),
                suggestions_mode_model.clone(),
                inline_history_model,
                ctx,
            )
        });

        current_prompt.update(ctx, |prompt_type, ctx| {
            if let PromptType::Dynamic { prompt } = prompt_type {
                prompt.update(ctx, |current_prompt, ctx| {
                    current_prompt.subscribe_to_input_editor(editor.clone(), ctx);
                });
            }
        });

        ctx.subscribe_to_view(&editor, move |me, _, event, ctx| {
            me.handle_editor_event(event, ctx);
        });

        let input_suggestions = ctx.add_typed_action_view(InputSuggestions::new);

        let safe_mode_settings = SafeModeSettings::handle(ctx);
        ctx.subscribe_to_model(&safe_mode_settings, |me, _, event, ctx| {
            me.handle_safe_mode_settings_changed_event(event, ctx)
        });

        ctx.subscribe_to_model(&InputModeSettings::handle(ctx), |_, _, _, ctx| {
            ctx.notify();
        });

        let (debounce_input_background_tx, debounce_input_background_rx) =
            async_channel::unbounded();
        let _ = ctx.spawn_stream_local(
            debounce(
                DEBOUNCE_INPUT_DECORATION_PERIOD,
                debounce_input_background_rx,
            ),
            |me, mode, ctx| me.run_input_background_jobs(mode, ctx),
            |_me, _ctx| {},
        );

        ctx.subscribe_to_model(&SessionSettings::handle(ctx), move |me, _, evt, ctx| {
            me.handle_session_settings_event(evt, ctx);
        });

        let editor_settings_handle = &AppEditorSettings::handle(ctx);
        ctx.subscribe_to_model(
            editor_settings_handle,
            Self::handle_app_editor_settings_event,
        );

        ctx.subscribe_to_model(&LigatureSettings::handle(ctx), |_, _, _, ctx| ctx.notify());

        let last_word_insertion = LastWordInsertion {
            insert_command_from_history_index: 0,
            is_latest_editor_event: false,
        };

        ctx.subscribe_to_model(
            &InputSettings::handle(ctx),
            Self::handle_input_settings_event,
        );

        ctx.subscribe_to_model(&suggestions_mode_model, |me, _, event, ctx| {
            let InputSuggestionsModeEvent::ModeChanged {
                buffer_to_restore,
                ..
            } = event;
            if let Some(buffer_state) = buffer_to_restore {
                me.restore_buffer_state(buffer_state, ctx);
            }

            ctx.notify();
        });

        ctx.subscribe_to_model(
            &IgnoredSuggestionsModel::handle(ctx),
            |me, _, event, ctx| {
                me.handle_ignored_suggestions_event(event, ctx);
            },
        );

        // Use persisted menu sizes from settings, or fall back to defaults
        let input_settings = InputSettings::as_ref(ctx);
        let completions_menu_width = *input_settings.completions_menu_width.value();
        let completions_menu_height = *input_settings.completions_menu_height.value();

        let is_editor_empty = editor.as_ref(ctx).is_empty(ctx);
        let mut input = Self {
            input_suggestions,
            suggestions_mode_model,
            completions_menu_resizable_width: resizable_state_handle(completions_menu_width),
            completions_menu_resizable_height: resizable_state_handle(completions_menu_height),
            tips_completed,
            editor,
            model,
            sessions,
            focus_handle: None,
            active_block_metadata: None,
            view_id,
            input_render_state_model_handle,
            command_x_ray_description: None,
            last_parsed_tokens: None,
            debounce_input_background_tx,
            has_pending_command: false,
            last_word_insertion,
            decorations_future_handle: None,
            autosuggestions_abort_handle: None,
            completions_abort_handle: None,
            menu_positioning_provider,
            terminal_input_message_bar,
            prompt_render_helper,
            prompt_type: current_prompt,
            enable_autosuggestions_setting: *editor_settings_handle
                .as_ref(ctx)
                .enable_autosuggestions,
            hoverable_handle: Default::default(),
            terminal_view_id,
            #[cfg(feature = "local_fs")]
            conn: None,
            is_processing_attached_images: false,
            inline_terminal_menu_positioner,
            is_editor_empty_on_last_edit: is_editor_empty,
            weak_view_handle: ctx.handle(),
            input_contents_before_prompt_chip_command: None,
        };

        #[cfg(feature = "local_fs")]
        if let Some(db_url) = database_file_path_for_scope(&PersistenceScope::App).to_str() {
            if let Ok(conn) = establish_ro_connection(db_url) {
                input.conn = Some(Arc::new(Mutex::new(conn)));
            }
        }

        input
    }





















































    fn restore_buffer_state(&mut self, buffer_state: &BufferState, ctx: &mut ViewContext<Self>) {
        self.editor.update(ctx, |editor, ctx| {
            editor.set_buffer_text_ignoring_undo(&buffer_state.buffer, ctx);
            if let Some(original_cursor_point) = &buffer_state.cursor_point {
                editor.reset_selections_to_point(original_cursor_point, ctx);
            }
        });
        ctx.notify();
    }











    // Auto-attach the last block for this query.







    fn handle_theme_change(&mut self, ctx: &mut ViewContext<Self>) {
        if self.should_apply_decorations(ctx) {
            self.run_input_background_jobs(
                InputBackgroundJobOptions::default().with_command_decoration(),
                ctx,
            );
        }
    }

    pub fn sessions<'a, A: ModelAsRef>(&self, ctx: &'a A) -> &'a Sessions {
        self.sessions.as_ref(ctx)
    }

    pub fn set_focus_handle(&mut self, focus_handle: PaneFocusHandle, ctx: &mut ViewContext<Self>) {
        self.focus_handle = Some(focus_handle.clone());
        let focus_model = focus_handle.focus_state_handle().clone();
        ctx.subscribe_to_model(&focus_model, move |me, _, event, ctx| {
            if !focus_handle.is_affected(event) {
                return;
            }

            let is_focused = focus_handle.is_focused(ctx);

            me.prompt_render_helper
                .prompt_view()
                .update(ctx, |prompt_view, ctx| {
                    prompt_view.on_pane_focus_changed(is_focused, ctx);
                });
        });
    }

    fn is_pane_focused(&self, app: &AppContext) -> bool {
        // If the focus handle hasn't been set yet, assume we're not in a split pane and therefore focused.
        self.focus_handle.as_ref().is_none_or(|h| h.is_focused(app))
    }

    fn is_active_session(&self, app: &AppContext) -> bool {
        self.focus_handle
            .as_ref()
            .is_some_and(|h| h.is_active_session(app))
    }

    pub fn menu_positioning(&self, app: &AppContext) -> MenuPositioning {
        self.menu_positioning_provider.menu_position(app)
    }

    fn size_info(&self, ctx: &AppContext) -> SizeInfo {
        ctx.model(&self.input_render_state_model_handle).size_info()
    }

    pub fn set_size_info(&mut self, size_info: SizeInfo, ctx: &mut AppContext) {
        self.input_render_state_model_handle
            .update(ctx, |input_render_state_model, _| {
                input_render_state_model.set_size_info(size_info);
            });
    }

    pub fn editor(&self) -> &ViewHandle<EditorView> {
        &self.editor
    }


    pub fn buffer_text(&self, ctx: &AppContext) -> String {
        self.editor.as_ref(ctx).buffer_text(ctx)
    }

    pub fn buffer_text_number_of_lines(&self, ctx: &AppContext) -> usize {
        self.buffer_text(ctx).lines().count()
    }

    #[cfg(feature = "integration_tests")]
    pub fn input_suggestions(&self) -> &ViewHandle<InputSuggestions> {
        &self.input_suggestions
    }

    pub fn suggestions_mode_model(&self) -> &ModelHandle<InputSuggestionsModeModel> {
        &self.suggestions_mode_model
    }

    pub fn inline_terminal_menu_positioner(&self) -> &ModelHandle<InlineMenuPositioner> {
        &self.inline_terminal_menu_positioner
    }

    pub fn completer_data(&self) -> CompleterData {
        CompleterData::new(
            self.sessions.clone(),
            self.active_block_metadata.clone(),
            CommandRegistry::global_instance(),
        )
    }

    fn start_byte_index_of_first_selection(&self, ctx: &ViewContext<Self>) -> ByteOffset {
        self.editor
            .as_ref(ctx)
            .start_byte_index_of_first_selection(ctx)
    }


    fn handle_input_settings_event(
        &mut self,
        input_settings: ModelHandle<InputSettings>,
        event: &InputSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            InputSettingsChangedEvent::ShowHintText { .. } => {
                ctx.notify();
            }
            InputSettingsChangedEvent::SyntaxHighlighting { .. } => {
                if !*input_settings.as_ref(ctx).syntax_highlighting.value() {
                    self.clear_decorations(ctx);
                }
                self.run_input_background_jobs(
                    InputBackgroundJobOptions::default().with_command_decoration(),
                    ctx,
                );
            }
            InputSettingsChangedEvent::ErrorUnderliningEnabled { .. } => {
                if !*input_settings.as_ref(ctx).error_underlining.value() {
                    self.clear_decorations(ctx);
                }
                self.run_input_background_jobs(
                    InputBackgroundJobOptions::default().with_command_decoration(),
                    ctx,
                );
            }
            InputSettingsChangedEvent::InputBoxTypeSetting { .. } => {
                // Force a re-render when switching between Universal and Classic input modes
                // to ensure all UI elements update in real-time
                ctx.notify();
            }
            InputSettingsChangedEvent::AtContextMenuInTerminalMode { .. } => {
                ctx.notify();
            }
            InputSettingsChangedEvent::CompletionsMenuWidth { .. } => {
                let new_value = *input_settings.as_ref(ctx).completions_menu_width.value();
                if let Ok(mut guard) = self.completions_menu_resizable_width.lock() {
                    guard.set_size(new_value);
                }
                ctx.notify();
            }
            InputSettingsChangedEvent::CompletionsMenuHeight { .. } => {
                let new_value = *input_settings.as_ref(ctx).completions_menu_height.value();
                if let Ok(mut guard) = self.completions_menu_resizable_height.lock() {
                    guard.set_size(new_value);
                }
                ctx.notify();
            }
            _ => {}
        }
    }













    /// Finds the start byte of the token under the given hovered point
    fn start_byte_index_at_point(
        &self,
        point: &DisplayPoint,
        ctx: &AppContext,
    ) -> Option<ByteOffset> {
        self.editor.read(ctx, |editor, ctx| {
            editor.start_byte_offset_at_point(point, ctx)
        })
    }

    fn handle_safe_mode_settings_changed_event(
        &mut self,
        event: &SafeModeSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            SafeModeSettingsChangedEvent::SafeModeEnabled { .. }
            | SafeModeSettingsChangedEvent::HideSecretsInBlockList { .. }
            | SafeModeSettingsChangedEvent::SecretDisplayModeSetting { .. } => {
                self.model
                    .lock()
                    .set_obfuscate_secrets(get_secret_obfuscation_mode(ctx));
            }
        }
    }


    fn handle_ignored_suggestions_event(
        &mut self,
        event: &IgnoredSuggestionsModelEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            IgnoredSuggestionsModelEvent::SuggestionIgnored => {
                // We may need to regenerate the autosuggestion if the suggestion just ignored
                // was the one suggested in the input.
                self.editor.update(ctx, |editor, ctx| {
                    editor.clear_autosuggestion(ctx);
                });
                self.maybe_generate_autosuggestion(ctx);
            }
        }
    }

    /// Returns `true` if we can query the [`History`] model for the active session.
    fn can_query_history(&self, ctx: &AppContext) -> bool {
        let model = self.model.lock();
        let Some(session_id) = model.block_list().active_block().session_id() else {
            return false;
        };

        let is_bootstrapped = model.block_list().is_bootstrapped();
        let is_history_queryable = History::as_ref(ctx).is_queryable(&session_id);

        // TODO: we should investigate why we need to check for bootstrapped here.
        // It's confusing and might actually be implied
        // (session history is only queryable if the session is bootstrapped).

        is_bootstrapped && is_history_queryable
    }

    /// Returns enum indicating if we can execute a command in the active session.
    ///
    /// We can only execute a command if:
    /// 1. the session is bootstrapped, because we don't want to interfere
    ///    with the PTY while bootstrapping is in progress
    /// 2. there isn't an active, long-running command (in-band commands are okay)
    /// 3. if the history for the session is appendable, because we want to
    ///    acknowledge the command in the session's history. Except when viewing
    ///    a shared session, since those sessions aren't registered in the [`History`]
    ///    model.
    fn can_execute_command(&self, ctx: &AppContext) -> CanExecuteCommand {
        let model = self.model.lock();
        let active_block = model.block_list().active_block();

        if !model.block_list().is_bootstrapped() {
            CanExecuteCommand::No(DenyExecutionReason::NotBootstrapped)
        } else if active_block.is_active_and_long_running()
            && !active_block.is_in_band_command_block()
        {
            CanExecuteCommand::No(DenyExecutionReason::ExistingActiveCommand)
        } else if active_block
            .session_id()
            .is_none_or(|session_id| !History::as_ref(ctx).is_appendable(&session_id))
        {
            CanExecuteCommand::No(DenyExecutionReason::HistoryNotAppendable)
        } else {
            CanExecuteCommand::Yes
        }
    }

    pub fn execute_pending_command(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.has_pending_command {
            return;
        }

        let command = self.get_command(ctx);
        if self.can_execute_command(ctx).is_no() {
            return;
        }

        self.try_execute_command(&command, ctx);
        self.has_pending_command = false;

        self.editor.update(ctx, |editor, ctx| {
            editor.set_interaction_state(InteractionState::Editable, ctx);
        });
    }

    /// Freeze the editor and put it in a loading state.
    pub fn freeze_input_in_loading_state(&mut self, ctx: &mut ViewContext<Self>) -> String {
        let buffer_text = self.editor.as_ref(ctx).buffer_text(ctx);
        self.freeze_input_in_loading_state_with_text(&buffer_text, ctx);
        buffer_text
    }

    /// Freeze the editor and render `"{display_text} ◌"` as the loading indicator.
    /// Shared between the user-initiated viewer submission path (which passes the
    /// editor's current buffer text) and the queued-prompt drain path (which passes
    /// the popped prompt text without ever reading from / writing to the user's
    /// in-progress buffer).
    fn freeze_input_in_loading_state_with_text(
        &mut self,
        buffer_text: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        self.editor.update(ctx, |editor, ctx| {
            // Use an ephemeral edit to show the loading state
            // and disallow edits.
            // TODO: the ◌ treatment is a stop-gap to rendering an svg
            // to the right of the buffer text.
            editor.set_buffer_text_ignoring_undo(&format!("{buffer_text} ◌"), ctx);
            editor.set_interaction_state(InteractionState::Selectable, ctx);

            // We manually set the text color to appear disabled.
            // We could use the [`InteractionState::Disabled`] interaction state
            // but that disallows text selection.
            let appearance = Appearance::as_ref(ctx);
            editor.set_text_colors(TextColors::all_hint_color(appearance), ctx);
        });
    }

    pub fn try_execute_command(&mut self, command: &str, ctx: &mut ViewContext<Self>) -> bool {
        self.try_execute_command_from_source(command, CommandExecutionSource::User, ctx)
    }

    /// Executes the given command if the terminal session is in a valid state to accept and
    /// execute a command. Afterwards, ensures the workflows info menu and input suggestions menu
    /// are both closed.
    ///
    /// This will _not_ execute a command if any of the following are true:
    ///     1. The history list and/or blocklist are not yet bootstrapped.
    ///     2. The active blocklist has not yet received the precmd payload.
    ///     3. There is an active, long-running command.
    ///
    /// Returns `true` if the command was executed, `false` otherwise.
    fn try_execute_command_from_source(
        &mut self,
        command: &str,
        source: CommandExecutionSource,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        if let CanExecuteCommand::No(reason) = self.can_execute_command(ctx) {
            if reason.is_existing_active_command() {
                const MAX_COMMAND_LENGTH: usize = 43;
                let truncated_command = truncate_from_end(command, MAX_COMMAND_LENGTH);

                // Block user submissions while a requested command is actively running
                let window_id = ctx.window_id();
                ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                    toast_stack.add_ephemeral_toast(
                        DismissibleToast::error(format!(
                            "Cannot run `{truncated_command}` (command already running)."
                        )),
                        window_id,
                        ctx,
                    );
                });
            }

            log::warn!("Tried to execute command but can_execute_command was false: {reason:?}");
            return false;
        }

        // Clear the auto-suggestion in the editor, so the height of
        // the input box is not inaccurate for its contents. Since we
        // we adjust the height of the long running block to be the same
        // as the height of the input box, we don't want the long
        // running block to have a lot of extra space for the frames
        // before it has any output or if it's a command that doesn't
        // have any output.
        //
        // Note that we do not clear the input box here (we do it in
        // `TerminalView` when we handle the `BlockCompleted` message
        // instead) for a similar reason. Specifically, we don't want
        // multi-line commands to have the height of the empty input
        // box because we don't want its contents to be cut off.
        //
        // If we had a zero-state autosuggestion and the user created an empty block,
        // keep the zero-state autosuggestion.
        if !command.is_empty() {
            self.editor.update(ctx, |editor, ctx| {
                editor.clear_autosuggestion(ctx);
                editor.clear_all_placeholder_text();
                ctx.notify();
            });
        }

        let home_dir = prompt::home_dir_for_block(
            self.model.lock().block_list().active_block(),
            self.sessions.as_ref(ctx),
        );
        self.model
            .lock()
            .block_list_mut()
            .active_block_mut()
            .set_home_dir(home_dir);

        let did_execute: bool;
        if self
            .model
            .lock()
            .block_list()
            .active_block()
            .has_received_precmd()
        {
            self.tips_completed.update(ctx, |tips, ctx| {
                mark_feature_used_and_write_to_user_defaults(
                    Tip::Hint(TipHint::CreateBlock),
                    tips,
                    ctx,
                );
                ctx.notify();
            });

            if !command.is_empty() {
                IgnoredSuggestionsModel::handle(ctx).update(ctx, |model, ctx| {
                    model.remove_ignored_suggestion(
                        command.to_string(),
                        SuggestionType::ShellCommand,
                        ctx,
                    );
                });
            }

            self.start_block_and_write_command_to_pty(command, source, ctx);
            did_execute = true;
        } else {
            // We don't want to submit the command if precmd has not
            // been received. Instead, we want the user to be aware
            // that the prompt might not be up to date.
            send_telemetry_from_ctx!(TelemetryEvent::TriedToExecuteBeforePrecmd, ctx);
            did_execute = false;
        }


        // Close the input suggestions menu if it was open.
        self.close_input_suggestions(/*should_focus_input=*/ false, ctx);
        did_execute
    }



    pub fn reset_after_cloud_followup_submission(&mut self, ctx: &mut ViewContext<Self>) {
        self.editor.update(ctx, |editor, ctx| {
            editor.set_interaction_state(InteractionState::Editable, ctx);
            editor.clear_buffer_and_reset_undo_stack(ctx);

            let appearance: &Appearance = Appearance::as_ref(ctx);
            editor.set_text_colors(TextColors::from_appearance(appearance), ctx);
        });
    }





    /// Returns the starting byte index position of the last selection.
    fn start_byte_index_of_last_selection(&self, ctx: &ViewContext<Self>) -> ByteOffset {
        self.editor
            .as_ref(ctx)
            .start_byte_index_of_last_selection(ctx)
    }

    fn handle_session_settings_event(
        &mut self,
        evt: &SessionSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match evt {
            SessionSettingsChangedEvent::HonorPS1 { .. } => {
                let mut model = self.model.lock();
                model.set_honor_ps1(*SessionSettings::as_ref(ctx).honor_ps1);
                ctx.notify();
            }
            SessionSettingsChangedEvent::SavedPrompt { .. } => {
                self.notify_and_notify_children(ctx);
            }
            _ => {}
        }
    }

    fn handle_app_editor_settings_event(
        &mut self,
        settings: ModelHandle<AppEditorSettings>,
        evt: &AppEditorSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if let AppEditorSettingsChangedEvent::EnableAutosuggestions { .. } = evt {
            let next_enable_autosuggestions_setting =
                *AppEditorSettings::as_ref(ctx).enable_autosuggestions;
            if self.enable_autosuggestions_setting && !next_enable_autosuggestions_setting {
                // Clear the active autosuggestion if autosuggestions was turned off.
                self.editor.update(ctx, |view, ctx| {
                    view.clear_autosuggestion(ctx);
                });
                ctx.notify();
            }
            // Ensure our cached copy of the enabled_autosuggestions setting
            // is up-to-date.
            self.enable_autosuggestions_setting = next_enable_autosuggestions_setting;
        }

        // The cursor and status bar may change appearance when vim mode is enabled or disabled.
        if let AppEditorSettingsChangedEvent::VimModeEnabled { .. } = evt {
            ctx.notify();
        }

        if let AppEditorSettingsChangedEvent::CursorDisplayState { .. } = evt {
            ctx.notify();
        }

        // The vim status bar should be shown and hidden immediately upon toggling.
        if settings.as_ref(ctx).vim_mode_enabled() {
            if let AppEditorSettingsChangedEvent::VimStatusBar { .. } = evt {
                ctx.notify();
            }
        }
    }

    pub fn set_autosuggestion(
        &mut self,
        autosuggestion: impl Into<String>,
        autosuggestion_type: AutosuggestionType,
        ctx: &mut ViewContext<Self>,
    ) {
        self.editor.update(ctx, |editor, ctx| {
            editor.set_autosuggestion(
                autosuggestion,
                AutosuggestionLocation::EndOfBuffer,
                autosuggestion_type,
                ctx,
            );
        })
    }
















    pub fn close_input_suggestions_and_restore_buffer(
        &mut self,
        should_focus_input: bool,
        should_restore_buffer_before_history_up: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if should_restore_buffer_before_history_up {
            if let InputSuggestionsMode::HistoryUp {
                original_buffer,
                original_cursor_point,
                ..
            } = self.suggestions_mode_model.as_ref(ctx).mode()
            {
                let original_buffer = original_buffer.clone();
                let original_cursor_point = *original_cursor_point;
                self.editor.update(ctx, |editor, ctx| {
                    editor.set_buffer_text_ignoring_undo(&original_buffer, ctx);
                    if let Some(original_cursor_point) = original_cursor_point {
                        editor.reset_selections_to_point(&original_cursor_point, ctx);
                    }
                });
            }
        }
        self.close_input_suggestions(/*should_focus_input=*/ should_focus_input, ctx);
    }

    pub fn close_input_suggestions(
        &mut self,
        should_focus_input: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        // If the input suggestions view is already closed, don't refocus the input box.
        if !self.suggestions_mode_model.as_ref(ctx).is_closed() {
            let was_inline_menu_open = self
                .suggestions_mode_model
                .as_ref(ctx)
                .is_inline_menu_open();

            self.suggestions_mode_model.update(ctx, |m, ctx| {
                m.set_mode(InputSuggestionsMode::Closed, ctx);
            });

            // If we're closing an inline menu, trigger autodetection on the buffer contents
            if was_inline_menu_open {
                self.run_input_background_jobs(
                    InputBackgroundJobOptions::default().with_ai_input_detection(),
                    ctx,
                );
            }

            if should_focus_input {
                self.focus_input_box(ctx);
                self.maybe_generate_autosuggestion(ctx);
            } else {
                ctx.notify();
            }
        }
    }

    pub fn clear_buffer_and_reset_undo_stack(&mut self, ctx: &mut ViewContext<Self>) {
        self.editor.update(ctx, |view, ctx| {
            view.clear_buffer_and_reset_undo_stack(ctx);
        });
    }

    pub fn replace_buffer_content(&mut self, content: &str, ctx: &mut ViewContext<Self>) {
        self.editor.update(ctx, |view, ctx| {
            view.set_buffer_text(content, ctx);
        });
    }

    // Fill the input buffer with the provided text and auto-select all of the text
    // (so that it's easy to delete).
    pub fn prefill_buffer_and_select_all(&mut self, content: &str, ctx: &mut ViewContext<Self>) {
        let content = content.trim();
        if content.is_empty() {
            return;
        }

        self.editor.update(ctx, |editor, ctx| {
            editor.clear_autosuggestion(ctx);
            editor.set_buffer_text_ignoring_undo(content, ctx);
            editor.handle_action(&EditorAction::SelectAll, ctx);
        });
    }

    /// Appends text to the current buffer at the cursor position, preserving existing buffer content.
    pub fn append_to_buffer(&mut self, content: &str, ctx: &mut ViewContext<Self>) {
        self.system_insert(content, ctx);
    }

    pub fn insert_typeahead_text(
        &mut self,
        num_typeahead_chars_inserted: CharOffset,
        typeahead: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        self.editor.update(ctx, |view, ctx| {
            view.replace_first_n_characters(num_typeahead_chars_inserted, typeahead, ctx);
            view.move_to_buffer_end(ctx);
        });
    }

    pub fn focus_input_box(&self, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }


    pub fn handle_command_search_closed(
        &mut self,
        query_when_closed: &str,
        filter_when_closed: &Option<QueryFilter>,
        ctx: &mut ViewContext<Self>,
    ) {
        // We want to restore / preserve the buffer as follows when the buffer text is "#":
        // - if command search was "#" when closed, keep the "#" in the buffer
        //   because the user probably wanted "#" without command search.
        // - if command search was "#: some_query" when closed, clear the buffer
        //   because the user probably got their answer from ai command search.
        // - if command search was empty when closed, clear the buffer
        //   because the user probably backspace'd out of "#" and then hit escape.
        let is_command_search_empty =
            filter_when_closed.is_none() && query_when_closed.trim().is_empty();
        let was_non_empty_ai_command_search =
            matches!(filter_when_closed, Some(QueryFilter::NaturalLanguage))
                && !query_when_closed.trim().is_empty();
        let was_triggered_by_hashtag = self.buffer_text(ctx).trim() == AI_COMMAND_SEARCH_TRIGGER;

        if (is_command_search_empty || was_non_empty_ai_command_search) && was_triggered_by_hashtag
        {
            self.editor().update(ctx, |editor, ctx| {
                editor.clear_buffer(ctx);
            });
        }
        self.focus_input_box(ctx);
    }

    /// Close all overlays managed by the input view. Does not change what is focused.
    /// If should_restore_buffer_before_history_up is true, the buffer will be restored to the state it was in before the history up menu was opened.
    pub fn close_overlays(
        &mut self,
        should_restore_buffer_before_history_up: bool,
        ctx: &mut ViewContext<Input>,
    ) {
        self.close_input_suggestions_and_restore_buffer(
            false,
            should_restore_buffer_before_history_up,
            ctx,
        );
    }

    fn editor_up(&mut self, ctx: &mut ViewContext<Self>) {
        // For some input suggestion modes, the menu handles its own actions.
        let handled = match self.suggestions_mode_model.as_ref(ctx).mode() {
            InputSuggestionsMode::HistoryUp { .. }
            | InputSuggestionsMode::CompletionSuggestions { .. }
            | InputSuggestionsMode::Closed => false,
        };

        if handled {
            return;
        }

        // If the input suggestions menu is open, always cycle to the next option.
        if self.suggestions_mode_model.as_ref(ctx).is_visible() && self.can_query_history(ctx) {
            self.input_suggestions.update(ctx, |suggestions, ctx| {
                suggestions.select_prev(ctx);
            });
            return;
        }

        // Otherwise, check if the cursor is on the first row and open the
        // history up menu.
        let editor = self.editor.as_ref(ctx);
        if editor.single_cursor_on_first_row(ctx) {
            let history = self.collate_ai_and_command_history(ctx);
            let original_buffer = self.editor.as_ref(ctx).buffer_text(ctx);

            let matches = InputSuggestions::history_prefix_search(&original_buffer, history);
            self.input_suggestions
                .update(ctx, move |input_suggestions, ctx| {
                    input_suggestions.set_history_matches(matches, ctx);
                });

            let original_cursor_point = self.editor.as_ref(ctx).single_cursor_to_point(ctx);
            self.suggestions_mode_model.update(ctx, |m, ctx| {
                m.set_mode(
                    InputSuggestionsMode::HistoryUp {
                        original_buffer,
                        original_cursor_point,
                        search_mode: HistorySearchMode::Prefix,
                    },
                    ctx,
                );
            });

            send_telemetry_from_ctx!(
                TelemetryEvent::OpenSuggestionsMenu(
                    self.suggestions_mode_model
                        .as_ref(ctx)
                        .mode()
                        .to_telemetry_mode(),
                ),
                ctx
            );
            ctx.notify();
            return;
        }
        // Finally, if we're neither scrolling through an existing suggestion
        // list nor entering the history mode, we move the cursor up.
        self.editor.update(ctx, |input, ctx| input.move_up(ctx));
    }

    // TODO - Implement PageUp functionality for input suggestions menu
    fn editor_page_up(&mut self, ctx: &mut ViewContext<Self>) {
        let _event = self.editor.read(ctx, |editor, ctx| {
            TelemetryEvent::PageUpDownInEditorPressed {
                is_empty_editor: editor.is_empty(ctx),
                is_down: false,
            }
        });
        send_telemetry_from_ctx!(event, ctx);
        if self.suggestions_mode_model.as_ref(ctx).is_visible() {
            self.editor
                .update(ctx, |input, ctx| input.move_page_up(ctx));
        } else {
            ctx.emit(Event::PageUp);
        }
    }

    /// Asks the currently active inline menu whether the buffer should be restored on dismiss
    /// (defaulting to true for any inline menus that don't have specific behavior requirements for this decision).
    fn should_restore_buffer_on_inline_menu_dismiss(&self, ctx: &ViewContext<Self>) -> bool {
        match self.suggestions_mode_model.as_ref(ctx).mode() {
            // If the input is not being used as a search on the model menu
            // we should not restore/revert the changes to the input on-dismiss,
            // unless we parked a prompt to search (then we restore that prompt).
            _ => true,
        }
    }

    fn editor_escape(&mut self, ctx: &mut ViewContext<Self>) {
        let vim_mode = self.editor.as_ref(ctx).vim_mode(ctx);
        let should_escape_vim_before_dismissing = vim_mode == Some(VimMode::Insert)
            && (self.suggestions_mode_model.as_ref(ctx).is_history_up()
                || self
                    .suggestions_mode_model
                    .as_ref(ctx)
                    .is_inline_history_menu());

        if should_escape_vim_before_dismissing {
            self.editor.update(ctx, |editor, editor_ctx| {
                editor.handle_action(&EditorAction::VimEscape, editor_ctx);
            });
        } else if self
            .suggestions_mode_model
            .as_ref(ctx)
            .is_inline_menu_open()
        {
            if self.should_restore_buffer_on_inline_menu_dismiss(ctx) {
                self.suggestions_mode_model.update(ctx, |model, ctx| {
                    model.close_and_restore_buffer(ctx);
                });
            } else {
                self.suggestions_mode_model.update(ctx, |model, ctx| {
                    model.set_mode(InputSuggestionsMode::Closed, ctx);
                });
            }
            ctx.notify();
        } else if self.suggestions_mode_model.as_ref(ctx).is_visible() {
            self.input_suggestions
                .update(ctx, |input_suggestions, ctx| {
                    input_suggestions.exit(true, ctx);
                });
        } else if !matches!(vim_mode, None | Some(VimMode::Normal)) {
            self.editor.update(ctx, |editor, editor_ctx| {
                editor.handle_action(&EditorAction::VimEscape, editor_ctx);
            });
        } else {
            ctx.emit(Event::Escape);
        }
    }




    fn editor_down(&mut self, ctx: &mut ViewContext<Self>) {
        // For some input suggestion modes, the menu handles its own actions.
        let handled = match self.suggestions_mode_model.as_ref(ctx).mode() {
            InputSuggestionsMode::HistoryUp { .. }
            | InputSuggestionsMode::CompletionSuggestions { .. }
            | InputSuggestionsMode::Closed => false,
        };

        if handled {
            return;
        }

        if self.suggestions_mode_model.as_ref(ctx).is_visible() {
            if self.input_suggestions.as_ref(ctx).is_empty() {
                // arrow down on an empty suggestions means we should close it.
                self.close_input_suggestions_and_restore_buffer(true, true, ctx);
            } else {
                self.input_suggestions.update(ctx, |suggestions, ctx| {
                    suggestions.select_next(ctx);
                });
            }
        } else {
            self.editor.update(ctx, |editor, ctx| editor.move_down(ctx));
        }
    }

    // TODO - Implement PageDown functionality for input suggestions menu
    fn editor_page_down(&mut self, ctx: &mut ViewContext<Self>) {
        let _event = self.editor.read(ctx, |editor, ctx| {
            TelemetryEvent::PageUpDownInEditorPressed {
                is_empty_editor: editor.is_empty(ctx),
                is_down: true,
            }
        });
        send_telemetry_from_ctx!(event, ctx);
        if self.suggestions_mode_model.as_ref(ctx).is_visible() {
            self.editor
                .update(ctx, |input, ctx| input.move_page_down(ctx));
        } else {
            ctx.emit(Event::PageDown);
        }
    }

    fn maybe_generate_autosuggestion(&mut self, ctx: &mut ViewContext<Self>) {
        let editor = self.editor.as_ref(ctx);

        let should_generate_autosuggestion =
            !editor.active_autosuggestion() && self.enable_autosuggestions_setting;

        if should_generate_autosuggestion {
            let buffer_text = editor.buffer_text(ctx);
            self.generate_autosuggestion_async(buffer_text, self.completer_data(), ctx)
        }
    }

    /// Asynchronously generate an autosuggestion to be inserted into the editor. First, reverse
    /// search the user's history to find a possible command that starts with the buffer text. If
    /// no commands are found, run the completer in a background thread to generate a result.
    pub fn generate_autosuggestion_async(
        &mut self,
        buffer_text: String,
        completer_data: CompleterData,
        ctx: &mut ViewContext<Self>,
    ) {
        if buffer_text.is_empty() {
            return;
        }

        let Some(session_id) = completer_data.active_block_session_id() else {
            return;
        };
        self.abort_latest_autosuggestion_future();

        let completion_context = completer_data.completion_session_context(ctx);
        let completion_session = completion_context
            .as_ref()
            .map(|completion_context| completion_context.session.clone());

        let session_env_vars = self.sessions.read(ctx, |sessions, _| {
            sessions.get_env_vars_for_session(session_id)
        });
        // Get current ignored shell commands to filter during generation
        let ignored_suggestions = IgnoredSuggestionsModel::as_ref(ctx)
            .get_ignored_suggestions_for_type(SuggestionType::ShellCommand);
        #[cfg(feature = "local_fs")]
        let _conn = self.conn.clone();
        let abort_handle = ctx
            .spawn_abortable(
                async move {
                    // Fallback to the first completer result for the matching prefix.
                    let Some(completion_context) = completion_context else {
                        return AutoSuggestionResult {
                            buffer_text,
                            autosuggestion_result: None,
                        };
                    };
                    let completion_result = completer::suggestions(
                        buffer_text.as_str(),
                        buffer_text.len(),
                        session_env_vars.as_ref(),
                        CompleterOptions {
                            match_strategy: MatchStrategy::CaseSensitive,
                            fallback_strategy: CompletionsFallbackStrategy::FilePaths,
                            suggest_file_path_completions_only: false,
                            parse_quotes_as_literals: false,
                        },
                        &completion_context,
                    )
                    .await;

                    let autosuggestion = completion_result.and_then(|result| {
                        let replacement_span = result.replacement_span;
                        result
                            .suggestions
                            .into_iter()
                            .map(|s| {
                                // Reproduce the final buffer text with the autosuggestion since the
                                // completer only gives the replacement span of the suggestion.
                                format!(
                                    "{}{}",
                                    &buffer_text[..replacement_span.start()],
                                    s.replacement()
                                )
                            })
                            .find(|suggestion| !ignored_suggestions.contains(suggestion))
                    });

                    AutoSuggestionResult {
                        buffer_text,
                        autosuggestion_result: autosuggestion,
                    }
                },
                Self::on_autosuggestion_result,
                move |_, _| {
                    if let Some(session) = completion_session {
                        session.cancel_active_commands();
                    }
                },
            )
            .abort_handle();

        self.set_autosuggestion_future(abort_handle);
    }

    fn is_potential_expansion(
        token: &Spanned<String>,
        cursor_pos: usize,
        executing: Executing,
    ) -> bool {
        match executing {
            // Expansion was triggered by user entering the command to be executed.
            // To expand, cursor must be exactly at the end of the token.
            Executing::Yes => token.span().end() == cursor_pos,
            // Expansion was triggered by user pressing Space at the end of a token.
            // To expand, cursor must be one index after the end of the token.
            Executing::No => token.span().end() + 1 == cursor_pos,
        }
    }

    /// Gets the abbreviation and abbreviation value, or alias and alias value, given
    /// a command, if they exist. Will return None if the conditions for alias
    /// expansion are not met.
    fn get_valid_abbreviation_or_alias_for_expansion<'a>(
        &self,
        command: Option<&'a LiteCommand>,
        cursor_pos: usize,
        executing: Executing,
        session_context: &'a SessionContext,
        ctx: &mut ViewContext<Self>,
    ) -> Option<(&'a Spanned<String>, &'a str)> {
        // An alias must be the first token of a command
        let first_token = command?.parts.first()?;

        if !Self::is_potential_expansion(first_token, cursor_pos, executing) {
            return None;
        }

        // If there is an abbreviation, we expand it as long as we aren't executing.
        // In fish, an alias formatted like `ls=echo Hello && ls` would get expanded
        // twice if we also performed expansion on enter.
        if matches!(executing, Executing::No) {
            if let Some(abbr_value) = session_context
                .session
                .abbreviation_value(&first_token.item)
            {
                return Some((first_token, abbr_value));
            }
        }

        // We only expand aliases if the user has turned the setting on.
        if self.should_expand_aliases(ctx) {
            let alias_value = session_context.session.alias_value(&first_token.item)?;
            if !is_expandable_alias(&first_token.item, alias_value) {
                return None;
            }

            return Some((first_token, alias_value));
        }
        None
    }

    /// Function to check whether the previous token was a valid command abbreviation
    /// or alias and handle expansion. This should only be called after the user has
    /// entered a space into the input editor.
    fn run_expansion_on_space(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(expansion_info) = self.run_expansion_internal(Executing::No, ctx) {
            self.expand_alias(expansion_info.byte_range, &expansion_info.alias_value, ctx);
        }
    }

    /// Function that checks whether the current token was a valid command abbreviation
    /// or alias, and returns a String representing the input buffer with the expanded
    /// text. This should be called after the user has pressed Enter to execute the
    /// command.
    fn get_expanded_command_on_execute(&mut self, ctx: &mut ViewContext<Self>) -> Option<String> {
        self.run_expansion_internal(Executing::Yes, ctx)
            .and_then(|expansion_info| {
                let mut text = expansion_info.buffer_text;
                let is_valid_byte_range = text.is_char_boundary(expansion_info.byte_range.start)
                    && text.is_char_boundary(expansion_info.byte_range.end);
                is_valid_byte_range.then(|| {
                    text.replace_range(expansion_info.byte_range, &expansion_info.alias_value);
                    text
                })
            })
    }

    /// Helper function that handles whether there is a valid expansion based on
    /// the current input buffer and cursor position. Returns info needed to
    /// perform the expansion.
    fn run_expansion_internal(
        &mut self,
        executing: Executing,
        ctx: &mut ViewContext<Self>,
    ) -> Option<ExpansionInfo> {
        let session_context = self.completion_session_context(ctx)?;
        let editor = self.editor.as_ref(ctx);
        editor.single_cursor_to_point(ctx)?;
        let buffer_text = editor.buffer_text(ctx);
        let cursor_pos = editor.end_byte_index_of_last_selection(ctx);
        let command = command_at_cursor_position(
            buffer_text.as_str(),
            session_context.escape_char(),
            cursor_pos,
        );

        self.get_valid_abbreviation_or_alias_for_expansion(
            command.as_ref(),
            cursor_pos.as_usize(),
            executing,
            &session_context,
            ctx,
        )
        .map(|(alias, alias_value)| ExpansionInfo {
            alias_value: alias_value.into(),
            buffer_text,
            byte_range: alias.span().start()..cursor_pos.as_usize(),
        })
    }

    fn expand_alias(
        &mut self,
        replacement_range: Range<usize>,
        alias_value: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        let alias_value_with_space = format!("{alias_value} ");
        self.editor.update(ctx, |input, ctx| {
            input.select_and_replace(
                &alias_value_with_space,
                [ByteOffset::from(replacement_range.start)
                    ..ByteOffset::from(replacement_range.end)],
                PlainTextEditorViewAction::ExpandAlias,
                ctx,
            );
        });
    }

    /// If at least one input is being synced, emit an event that other
    /// terminal views can decide to process based on their sync state.
    fn send_input_sync_event(&self, edit_origin: &EditOrigin, ctx: &mut ViewContext<Self>) {
        let is_syncing_inputs =
            SyncedInputState::as_ref(ctx).is_syncing_any_inputs(ctx.window_id());

        if is_syncing_inputs
                    // If the edit we're applying in `handle_editor_event`
                    //came from another synced terminal,
                    // don't emit a new event which would create a cycle
                    && *edit_origin != EditOrigin::SyncedTerminalInput
                    // Similarly, only emit an event from the session the user is typing in
                    && self.focus_handle.as_ref().is_none_or(|h| h.is_focused(ctx))
        {
            let buffer = self.editor.as_ref(ctx).buffer_text(ctx);
            ctx.emit(Event::SyncInput(
                SyncInputType::InputEditorContentsChanged {
                    contents: Arc::new(buffer),
                },
            ));
        }
    }

    fn handle_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        // We want to clear the token description hover on any editor action
        self.hide_x_ray(ctx);

        if !matches!(event, EditorEvent::InsertLastWordPrevCommand) {
            self.update_last_word_insertion_state();
        }

        match event {
            EditorEvent::Edited(edit_origin) => {
                // We should ideally be handling all `Edited` events, not just those that are
                // marked EditOrigin. However, we receive the notification that the block has
                // completed, in the same event we clear the input box per-command. Due to how
                // events are dispatched in the UI framework, we would receive an Edited event
                // immediately from clearing the input box. But we don't want that.
                // Only processing the user typed events should be good enough here.

                if matches!(
                    edit_origin,
                    EditOrigin::UserTyped | EditOrigin::UserInitiated
                ) {
                    self.model.lock().set_is_input_dirty(true);
                }

                if *edit_origin == EditOrigin::UserTyped
                    && !ctx
                        .model(&self.input_render_state_model_handle)
                        .editor_modified_since_block_finished()
                {
                    self.input_render_state_model_handle.update(
                        ctx,
                        |input_render_state_model, _| {
                            input_render_state_model.set_editor_modified_since_block_finished(true);
                        },
                    );

                    if !self
                        .model
                        .lock()
                        .block_list()
                        .active_block()
                        .has_received_precmd()
                    {
                        send_telemetry_from_ctx!(TelemetryEvent::EditedInputBeforePrecmd, ctx);
                        ctx.notify();
                    }
                }

                let is_editor_empty = self.editor.as_ref(ctx).is_empty(ctx);
                if is_editor_empty != self.is_editor_empty_on_last_edit {
                    self.is_editor_empty_on_last_edit = is_editor_empty;
                    ctx.emit(Event::InputEmptyStateChanged {
                        is_empty: is_editor_empty,
                        reason: InputEmptyStateChangeReason::Edited,
                    });
                }

                let mut short_circuit_highlighting = false;
                let mut check_alias_expansion = false;

                self.editor.read(ctx, |editor, editor_ctx| {
                    let last_action = editor.get_last_action(editor_ctx);
                    if Some(PlainTextEditorViewAction::Space) == last_action
                        && *edit_origin == EditOrigin::UserTyped
                    {
                        check_alias_expansion = true;
                    }

                    if SHORT_CIRCUIT_HIGHLIGHTING_ACTIONS.contains(&last_action) {
                        short_circuit_highlighting = true;
                    }
                });

                if check_alias_expansion {
                    self.run_expansion_on_space(ctx);
                }

                // Don't run NLD autodetection when an inline menu is open (slash commands,
                // conversation menu, model selector), as the buffer contents are being used as
                // a search query for the menu rather than as a command/prompt.
                let is_inline_menu_open = self
                    .suggestions_mode_model
                    .as_ref(ctx)
                    .is_inline_menu_open();

                let _ = is_inline_menu_open;

                if self.should_apply_decorations(ctx) {
                    let mut mode = InputBackgroundJobOptions::default();
                    mode = mode.with_command_decoration();

                    if short_circuit_highlighting {
                        self.run_input_background_jobs(mode, ctx);
                    } else {
                        let _ = self.debounce_input_background_tx.try_send(mode);
                    }
                }

                // We only sync on EditorEvent::Edited events because we're only
                // syncing terminal input editor contents, not the full
                // functionality of the terminal input in each blocklist
                // e.g., we don't want to sync EditorEvent::CmdUpOnFirstRow.
                self.send_input_sync_event(edit_origin, ctx);

                let mode = self.suggestions_mode_model.as_ref(ctx).mode().clone();
                match &mode {
                    InputSuggestionsMode::CompletionSuggestions {
                        replacement_start,
                        buffer_text_original,
                        completion_results,
                        trigger,
                        ..
                    } => {
                        let replacement_start = *replacement_start;
                        let editor_text = self.buffer_text(ctx);
                        let cursor_position = self.start_byte_index_of_last_selection(ctx);
                        let current_word =
                            editor_text.get(replacement_start..cursor_position.as_usize());
                        let current_selected_item =
                            self.input_suggestions.as_ref(ctx).get_selected_item_text();
                        let selected_item_differs_from_current_word = current_selected_item
                            .zip(current_word)
                            .map(|(selected_item, current_word)| selected_item != current_word)
                            .unwrap_or(true);

                        // To support completions-as-you-type x classic completions,
                        // we need to make sure we don't recompute the completion results
                        // as the user cycles (which inserts into buffer and thus is treated
                        // as an edit). Thus, when using the two features together, we only
                        // recompute the result set if the selected item doesn't match the
                        // current word span.
                        let old_buffer_text_original = buffer_text_original.clone();
                        if *trigger == CompletionsTrigger::AsYouType
                            && (!self.is_classic_completions_enabled(ctx)
                                || (self.is_classic_completions_enabled(ctx)
                                    && selected_item_differs_from_current_word))
                        {
                            // For as-you-type completions, we recalculate suggestions rather than
                            // filtering, since typing could involve moving to a new parameter
                            // within a given command, rather than being a strict subset as is the
                            // case with manual tab completions.
                            self.open_completion_suggestions(CompletionsTrigger::AsYouType, ctx);
                            self.maybe_generate_autosuggestion(ctx);

                            // Since tab completions are async, we should close the
                            // menu if it's been some time and the menu still hasn't updated,
                            // otherwise the user will see an old completions menu even while
                            // the buffer text has changed. We wait with a delay so that way
                            // the menu doesn't close right away and open away right after if
                            // the completions finish quickly, since that causes a jittery UX.
                            let _ = ctx.spawn(
                                async move {
                                    riftui::r#async::Timer::after(Duration::from_millis(750)).await;
                                    old_buffer_text_original
                                },
                                move |input, old_buffer_text_original, ctx| {
                                    if let InputSuggestionsMode::CompletionSuggestions {
                                        buffer_text_original,
                                        ..
                                    } = input.suggestions_mode_model.as_ref(ctx).mode()
                                    {
                                        // The menu hasn't changed since last time so
                                        // close it for now. If the menu is truly delayed,
                                        // the completions callback will eventually open it.
                                        if old_buffer_text_original == *buffer_text_original {
                                            input.close_input_suggestions(true, ctx);
                                        }
                                    }
                                },
                            );
                        } else {
                            let buffer_text_original = buffer_text_original.clone();
                            let completion_results = completion_results.clone();
                            let should_close = self.update_tab_completion_menu(
                                replacement_start,
                                buffer_text_original.as_str(),
                                &completion_results,
                                ctx,
                            );
                            if should_close {
                                self.close_input_suggestions(
                                    /*should_focus_input=*/ true, ctx,
                                );
                            }
                        }
                    }
                    InputSuggestionsMode::HistoryUp { .. } => {
                        // In HistoryUp mode, we replace the buffer as options
                        // are selected.
                        // We also dismiss the suggestion menu if the buffer
                        // is edited such that it doesn't exactly match
                        // the selected suggestion.

                        if let Some(selected_text) =
                            self.input_suggestions.as_ref(ctx).get_selected_item_text()
                        {
                            if *selected_text.to_string()
                                == self.editor.as_ref(ctx).buffer_text(ctx)
                            {
                                return;
                            }

                            self.close_input_suggestions(/*should_focus_input=*/ true, ctx);
                        }
                    }
                    InputSuggestionsMode::Closed => {
                        if !self.can_query_history(ctx) {
                            return;
                        }

                        let editor = self.editor.as_ref(ctx);
                        let _buffer_text = editor.buffer_text(ctx);

                        self.maybe_generate_autosuggestion(ctx);

                        if self.should_show_completions_while_typing(ctx)
                            && matches!(edit_origin, EditOrigin::UserTyped)
                        {
                            self.open_completion_suggestions(CompletionsTrigger::AsYouType, ctx);
                        }
                    }
                }
            }
            EditorEvent::BufferReplaced => {}
            EditorEvent::SelectionChanged => {
                let mode = self.suggestions_mode_model.as_ref(ctx).mode().clone();
                let is_completion_suggestions =
                    matches!(mode, InputSuggestionsMode::CompletionSuggestions { .. });
                if is_completion_suggestions && !self.cursor_positioned_for_completion(ctx) {
                    self.close_input_suggestions(/*should_focus_input=*/ true, ctx);
                } else {
                    match &mode {
                        InputSuggestionsMode::HistoryUp { .. } | InputSuggestionsMode::Closed => {}
                        InputSuggestionsMode::CompletionSuggestions {
                            replacement_start,
                            buffer_text_original,
                            completion_results,
                            ..
                        } => {
                            let replacement_start = *replacement_start;
                            let buffer_text_original = buffer_text_original.clone();
                            let completion_results = completion_results.clone();
                            let should_close = self.update_tab_completion_menu(
                                replacement_start,
                                buffer_text_original.as_str(),
                                &completion_results,
                                ctx,
                            );

                            if should_close {
                                self.close_input_suggestions(
                                    /*should_focus_input=*/ true, ctx,
                                );
                            }
                        }
                    }
                }
            }
            EditorEvent::AutosuggestionAccepted {
                insertion_length: _,
                buffer_char_length: _,
                autosuggestion_type,
            } => {
                send_telemetry_from_ctx!(
                    TelemetryEvent::AutosuggestionInserted {
                        insertion_length: *insertion_length,
                        buffer_length: *buffer_char_length
                    },
                    ctx
                );
                ctx.emit(Event::AutosuggestionAccepted);

                self.input_suggestions
                    .update(ctx, |input_suggestions, ctx| {
                        // We should not restore the buffer to the old state since we're accepting an autosuggestion from the new state.
                        input_suggestions.exit(false, ctx);
                    });
                match autosuggestion_type {
                    AutosuggestionType::Command {
                        was_intelligent_autosuggestion,
                    } => {
                        if !*was_intelligent_autosuggestion {
                            // This accepted autosuggestion count is used to determine whether to show the right arrow to accept icon
                            // when there's an autosuggestion while the input buffer is not empty.
                            // So it should only be incremented when an autosuggestion is accepted while the buffer is not empty (is NOT intelligent/zero-state).
                            InputSettings::handle(ctx).update(ctx, |input_settings, ctx| {
                                let current_count =
                                    *input_settings.autosuggestion_accepted_count.value();
                                if current_count < MAX_TIMES_TO_SHOW_AUTOSUGGESTION_HINT {
                                    let new_count = if current_count < 0 {
                                        // Note: there was a bug in the previous implementation of this method which would
                                        // cause it to overflow the i8 value to a negative value. In that case, we know
                                        // that the user has definitely accepted at _least_ 128 autosuggestions, so we can
                                        // set it to the maximum relevant value: MAX_TIMES_TO_SHOW_AUTOSUGGESTION_HINT
                                        MAX_TIMES_TO_SHOW_AUTOSUGGESTION_HINT
                                    } else {
                                        current_count + 1
                                    };

                                    report_if_error!(input_settings
                                        .autosuggestion_accepted_count
                                        .set_value(new_count, ctx))
                                }
                            })
                        }
                    }
                };
            }
            EditorEvent::Navigate(NavigationKey::Up) => {
                self.editor_up(ctx);
            }
            EditorEvent::Navigate(NavigationKey::Down) => {
                self.editor_down(ctx);
            }
            EditorEvent::Navigate(NavigationKey::PageUp) => {
                self.editor_page_up(ctx);
            }
            EditorEvent::Navigate(NavigationKey::PageDown) => {
                self.editor_page_down(ctx);
            }
            EditorEvent::Navigate(NavigationKey::Tab) => {
                self.input_tab(ctx);
            }
            EditorEvent::Navigate(NavigationKey::ShiftTab) => {
                self.input_shift_tab(ctx);
            }
            EditorEvent::Navigate(NavigationKey::Right) => {}
            EditorEvent::Enter => self.input_enter(ctx),
            EditorEvent::CmdEnter => self.input_cmd_enter(ctx),
            EditorEvent::CtrlEnter => self.input_ctrl_enter(ctx),
            EditorEvent::Escape => self.editor_escape(ctx),
            EditorEvent::CtrlC { cleared_buffer_len } => {
                self.close_input_suggestions(/*should_focus_input=*/ true, ctx);
                ctx.emit(Event::CtrlC {
                    cleared_buffer_len: *cleared_buffer_len,
                });
            }
            EditorEvent::DeleteAllLeft => {}
            EditorEvent::CmdUpOnFirstRow => ctx.emit(Event::SelectRecentBlocks { count: 1 }),
            EditorEvent::Copy => ctx.emit(Event::Copy),
            EditorEvent::UnhandledModifierKeyOnEditor(keystroke) => {
                ctx.emit(Event::UnhandledModifierKeyOnEditor(keystroke.clone()))
            }
            EditorEvent::ClearParentSelections => {
                ctx.emit(Event::ClearSelectionsWhenShellMode);
            }
            EditorEvent::HideXRay => {
                self.hide_x_ray(ctx);
            }
            EditorEvent::TryToShowXRay(token_at) => {
                match token_at {
                    CommandXRayAnchor::Cursor => {
                        send_telemetry_from_ctx!(
                            TelemetryEvent::CommandXRayTriggered {
                                trigger: CommandXRayTrigger::Keystroke
                            },
                            ctx
                        );
                        let pos = self.start_byte_index_of_first_selection(ctx);
                        self.start_xray_at_offset(pos, CommandXRayTrigger::Keystroke, ctx);
                    }
                    CommandXRayAnchor::Hover(mouse_position) => {
                        if let Some(offset) = self.start_byte_index_at_point(mouse_position, ctx) {
                            if !self.suggestions_mode_model.as_ref(ctx).is_visible() {
                                send_telemetry_from_ctx!(
                                    TelemetryEvent::CommandXRayTriggered {
                                        trigger: CommandXRayTrigger::Hover
                                    },
                                    ctx
                                );
                                self.start_xray_at_offset(offset, CommandXRayTrigger::Hover, ctx);
                            }
                        }
                    }
                }
            }
            EditorEvent::InsertLastWordPrevCommand => self.insert_last_word_previous_command(ctx),
            // For this particular view, the terminal Input, we ignore search direction because in
            // this context, search means search through History which isn't actually sensitive to
            // left/right direction.
            EditorEvent::Search { term, .. } => {
                ctx.emit(Event::ShowCommandSearch(CommandSearchOptions {
                    filter: Some(QueryFilter::History),
                    init_content: InitContent::Custom(term.clone().unwrap_or("".to_owned())),
                }));
            }
            // For this view, the terminal Input, we do not support ex-commands. The closest
            // analogy we have in this view would be workflows. So, open command search with the
            // workflows filter to handle this event.
            EditorEvent::ExCommand => ctx.emit(Event::ShowCommandSearch(CommandSearchOptions {
                filter: Some(QueryFilter::History),
                init_content: InitContent::Custom("".to_owned()),
            })),
            EditorEvent::VimStatusUpdate => ctx.notify(),
            EditorEvent::BackspaceOnEmptyBuffer | EditorEvent::BackspaceAtBeginningOfBuffer => {}
            EditorEvent::EmacsBindingUsed => {
                ctx.emit(Event::EmacsBindingUsed);
            }
            EditorEvent::UpdatePeers { .. } => {}
            EditorEvent::MiddleClickPaste => {
                ctx.emit(Event::InputFocusedFromMiddleClick);
            }
            EditorEvent::Focused => ctx.emit(Event::EditorFocused),
            EditorEvent::ProcessingAttachedImages(is_processing) => {
                self.set_is_processing_attached_images(*is_processing, ctx);
            }
            EditorEvent::VoiceStateUpdated {
                is_listening,
                is_transcribing,
            } => {
                if *is_listening || *is_transcribing {
                    // Show voice status as placeholder when the buffer is empty.
                    if self.editor.as_ref(ctx).is_empty(ctx) {
                        let placeholder = if *is_listening {
                            "Listening..."
                        } else {
                            "Transcribing..."
                        };
                        self.editor.update(ctx, |editor, ctx| {
                            editor.set_placeholder_text(placeholder, ctx);
                        });
                    }
                }
            }
            EditorEvent::Paste => {
                self.process_paste_event(ctx);
            }
            EditorEvent::DroppedImageFiles(image_filepaths) => {
                // Handle image processing from EditorView drag-and-drop
                let num_attached =
                    self.handle_pasted_or_dragdropped_image_filepaths(image_filepaths.clone(), ctx);

                // If any attachment failed, insert all dropped image paths as text. Apply the
                // same session-aware path transformation that the editor uses for dropped
                // non-image paths so the fallback matches the primary drop flow (e.g.
                // `/mnt/c/...` in a WSL session).
                if num_attached < image_filepaths.len() {
                    let shell_family = self.editor.read(ctx, |editor, _| editor.shell_family());
                    let converter = self
                        .active_session(ctx)
                        .as_deref()
                        .and_then(Session::windows_path_converter);
                    let transformed: Vec<String> = match converter {
                        Some(convert) => image_filepaths.iter().map(|p| convert(p)).collect(),
                        None => image_filepaths.clone(),
                    };
                    let paths_str =
                        riftui::clipboard_utils::escaped_paths_str(&transformed, shell_family);

                    self.editor.update(ctx, |editor, ctx| {
                        editor.user_insert(&paths_str, ctx);
                    });
                }
            }
            EditorEvent::IgnoreAutosuggestion { suggestion } => {
                IgnoredSuggestionsModel::handle(ctx).update(ctx, |model, ctx| {
                    model.add_ignored_suggestion(
                        suggestion.clone(),
                        SuggestionType::ShellCommand,
                        ctx,
                    );
                });

                self.editor.update(ctx, |editor, ctx| {
                    editor.clear_autosuggestion(ctx);
                });
            }
            _ => {}
        }
    }

    /// Process paste event by checking clipboard for images and handling appropriately.
    fn process_paste_event(&mut self, ctx: &mut ViewContext<Self>) {
        // Read from app clipboard
        let content = ctx.clipboard().read();

        // Image attachment was an AI feature; paste is always plain text.
        self.insert_clipboard_text_content(ctx, content);
    }

    /// Insert clipboard text content (paths / plaintext)
    fn insert_clipboard_text_content(
        &self,
        ctx: &mut ViewContext<Self>,
        content: ClipboardContent,
    ) {
        let clipboard_content_str = self
            .editor
            .read(ctx, |editor, _| editor.clipboard_text_content(content));
        self.editor.update(ctx, |editor, ctx| {
            editor.user_initiated_insert(
                &clipboard_content_str,
                PlainTextEditorViewAction::Paste,
                ctx,
            );
        });
    }


    /// Image auto-attachment was an AI feature and has been removed; callers fall
    /// back to inserting the dropped/pasted paths as plain text.
    pub fn handle_pasted_or_dragdropped_image_filepaths(
        &mut self,
        _image_filepaths: Vec<String>,
        _ctx: &mut ViewContext<Self>,
    ) -> usize {
        0
    }

    pub fn set_is_processing_attached_images(
        &mut self,
        is_processing_attached_images: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        self.is_processing_attached_images = is_processing_attached_images;
        ctx.notify();
    }

    /// Updates the tab completion menu given the current text of the editor and location of the
    /// cursor. Returns whether the input suggestions should be closed.
    ///
    /// If the original text is still within the buffer up to where the cursor is, we filter the
    /// suggestions to only show the suggestions that match the current word. If the original text
    /// is _not_ within the buffer up to the cursor, we close the input suggestions.
    fn update_tab_completion_menu(
        &self,
        replacement_start: usize,
        buffer_text_original: &str,
        completion_results: &SuggestionResults,
        ctx: &mut ViewContext<Input>,
    ) -> bool {
        let editor_text = self.editor.as_ref(ctx).buffer_text(ctx);
        let cursor_position = self.start_byte_index_of_last_selection(ctx);
        let text_up_to_cursor = &editor_text[0..cursor_position.as_usize()];

        // If the cursor position is before the start of the replacement span,
        // then we should definitely close the menu.
        if cursor_position.as_usize() < replacement_start {
            return true;
        }

        // If the buffer no longer starts with the original buffer text,
        // then we should close the completion menu because the result set
        // was based on a different query.
        //
        // For classic completions, this is a poor heuristic: when you cycle
        // through fuzzy matches, the text up to the cursor might not start
        // with the original buffer text anymore.
        // TODO: there's a bug here where if you hit tab and backspace,
        // the result set won't go away (stale).
        if !text_up_to_cursor.starts_with(buffer_text_original)
            && !self.is_classic_completions_enabled(ctx)
        {
            // Close the input suggestions since the buffer was edited to no longer
            // contain the text that triggered tab completion.
            true
        } else {
            // The current word is everything from the start of the replacement to the
            // cursor
            let current_word = &editor_text[replacement_start..cursor_position.as_usize()];

            if self.is_classic_completions_enabled(ctx) {
                let current_selected_item =
                    self.input_suggestions.as_ref(ctx).get_selected_item_text();
                if current_selected_item.is_some_and(|selected| selected == current_word) {
                    // If we're in classic completion mode and the selected item is equal
                    // to the current word, then we should keep the menu open; the user is cycling.
                    // We early-return because we don't want to filter the menu based on the
                    // selected item.
                    return false;
                }
            }

            // If the user continues to type with the tab suggestions open, we perform a
            // prefix search on the original results to filter the suggestions.
            let should_close = self.input_suggestions.update(ctx, |suggestions, ctx| {
                suggestions.prefix_search_for_tab_completion(
                    current_word,
                    completion_results,
                    TabCompletionsPreselectOption::Unchanged,
                    ctx,
                );

                // We should close the menu if there aren't any results
                // after filtering.
                suggestions.items().is_empty()
            });

            ctx.notify();
            should_close
        }
    }

    fn clear_screen(&mut self, ctx: &mut ViewContext<Self>) {
        self.model.lock().clear_visible_screen();
        ctx.notify();
    }

    /// Attempts to write the EOT (End-of-Transmission) char to the PTY, which is canonically mapped
    /// to Ctrl-D. If successful, the session is terminated.
    fn ctrl_d(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(Event::CtrlD);
    }

    fn ctrl_r(&mut self, ctx: &mut ViewContext<Self>) {
        if self.suggestions_mode_model.as_ref(ctx).is_history_up() {
            // Iterate through menu if we're already in history substring mode and
            // the user hits ctrl-r.
            self.input_suggestions
                .update(ctx, |input_suggestions, ctx| {
                    input_suggestions.select_prev(ctx);
                });
        } else {
            self.fuzzy_history_search(ctx);
        }
    }

    fn fuzzy_history_search(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.can_query_history(ctx) {
            return;
        }

        self.focus_input_box(ctx);

        let editor = self.editor.as_ref(ctx);

        let original_cursor_point = editor.single_cursor_to_point(ctx);

        // Although we don't use suggestions_mode_model when using Voltron,
        // we still close the input suggestion menu before opening the Voltron modal,
        // which involves resetting the cursor point.
        let original_buffer = editor.buffer_text(ctx);
        self.suggestions_mode_model.update(ctx, |m, ctx| {
            m.set_mode(
                InputSuggestionsMode::HistoryUp {
                    original_buffer,
                    original_cursor_point,
                    search_mode: HistorySearchMode::Fuzzy,
                },
                ctx,
            );
        });
        send_telemetry_from_ctx!(
            TelemetryEvent::OpenSuggestionsMenu(
                self.suggestions_mode_model
                    .as_ref(ctx)
                    .mode()
                    .to_telemetry_mode(),
            ),
            ctx
        );

        ctx.notify();
    }



    /// Returns a collection of history entries that are user AI queries or shell commands in order
    /// from oldest to most recent.
    fn collate_ai_and_command_history<'a>(
        &'a self,
        ctx: &'a ViewContext<Self>,
    ) -> Vec<HistoryInputSuggestion<'a>> {
        let config = UpArrowHistoryConfig {
            include_commands: true,
        };

        History::as_ref(ctx).up_arrow_suggestions_for_terminal_view(
            self.terminal_view_id,
            self.active_block_session_id(),
            config,
            ctx,
        )
    }

    fn update_last_word_insertion_state(&mut self) {
        // If an `InsertLastWordPrevCommand` action is received, its handler method will set
        // `is_latest_editor_event` on `self.last_word_insertion` to true, marking the following
        // EditorEvent (buffer edited) received is from this insertion.
        //
        // Any other editor event means the following "last word" insert is not consecutive, so
        // index is reset - the following insert will insert last word from most recent command
        // in history, index 0 (After that, a consecutive insertion would increment to index 1,
        // last word of second last command in history).
        //
        // If the last event was a last word insertion, we increment the
        // `insert_command_from_history_index` on `self.last_word_insertion` to indicate
        // consecutive inserts may be made (if so, insert from next earlier command in history).
        // We then set `is_latest_editor_event` to false for the following editor event; if another
        // last word insertion occurs, it is responsible for re-setting this boolean to true.
        if self.last_word_insertion.is_latest_editor_event {
            self.last_word_insertion.insert_command_from_history_index += 1;
            self.last_word_insertion.is_latest_editor_event = false;
        } else {
            self.last_word_insertion.insert_command_from_history_index = 0;
        }
    }

    fn history_commands<'b>(&self, ctx: &'b ViewContext<Input>) -> Vec<&'b HistoryEntry> {
        self.active_block_session_id()
            .map_or_else(Vec::new, |session_id| {
                History::as_ref(ctx)
                    .commands(session_id)
                    .unwrap_or_default()
            })
    }

    fn insert_last_word_previous_command(&mut self, ctx: &mut ViewContext<Input>) {
        if let Some(word_to_insert) = self.get_last_word_of_command_in_history(
            self.last_word_insertion.insert_command_from_history_index,
            ctx,
        ) {
            self.editor.update(ctx, |editor, ctx| {
                editor.insert_selected_text_to_buffer_ignoring_undo(&word_to_insert, ctx);
            });

            self.last_word_insertion.is_latest_editor_event = true;
        }
    }

    fn get_last_word_of_command_in_history(
        &mut self,
        command_history_index: usize,
        ctx: &mut ViewContext<Input>,
    ) -> Option<String> {
        let commands = self.history_commands(ctx);
        if commands.is_empty() {
            return None;
        }

        let view_command_idx = commands.len().saturating_sub(1 + command_history_index);
        let view_command = commands[view_command_idx];

        let last_word = view_command
            .command
            .rsplit_once(' ')
            .map(|(_, last_word)| last_word)
            .unwrap_or(&view_command.command);

        Some(last_word.to_string())
    }

    /// We only want to show the completions while typing menu when the cursor is
    /// positioned at the end of the buffer text
    fn is_cursor_in_valid_position_for_completions_while_typing(
        &self,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let editor = self.editor.as_ref(ctx);
        editor.single_cursor_at_buffer_end(false /* respect_line_cap */, ctx)
    }

    fn should_show_completions_while_typing(&self, ctx: &mut ViewContext<Self>) -> bool {
        let editor = self.editor.as_ref(ctx);
        let buffer_text = editor.buffer_text(ctx);

        self.is_completions_while_typing_turned_on(ctx)
            && buffer_text.len() >= MIN_BUFFER_LEN_TO_SHOW_COMPLETIONS_WHILE_TYPING
            && self.is_cursor_in_valid_position_for_completions_while_typing(ctx)
    }

    fn is_completions_while_typing_turned_on(&self, app: &AppContext) -> bool {
        *InputSettings::as_ref(app)
            .completions_open_while_typing
            .value()
    }


    fn is_classic_completions_enabled(&self, ctx: &AppContext) -> bool {
        (FeatureFlag::ClassicCompletions.is_enabled()
            && *InputSettings::as_ref(ctx).classic_completions_mode)
            || FeatureFlag::ForceClassicCompletions.is_enabled()
    }

    fn should_expand_aliases(&self, ctx: &mut ViewContext<Self>) -> bool {
        *AliasExpansionSettings::as_ref(ctx)
            .alias_expansion_enabled
            .value()
    }

    fn open_completion_suggestions(
        &mut self,
        completions_trigger: CompletionsTrigger,
        ctx: &mut ViewContext<Self>,
    ) {
        let editor = self.editor.as_ref(ctx);
        let buffer_text = editor.buffer_text(ctx);

        let is_command_grid_active = {
            let model = self.model.lock();
            !model.is_alt_screen_active()
                && model.block_list().active_block().is_command_grid_active()
        };

        // If the cursor is in a valid completion position, go into CompletionSuggestions mode
        if is_command_grid_active && self.can_query_history(ctx) {
            let matcher = MatchStrategy::Fuzzy;

            if let Some(completion_context) = self.completion_session_context(ctx) {
                let cursor_position = self.start_byte_index_of_last_selection(ctx);
                let before_cursor_text = buffer_text[..cursor_position.as_usize()].to_owned();
                let editor_model = self.editor.read(ctx, |view, ctx| view.snapshot_model(ctx));

                self.run_completions_async(
                    before_cursor_text,
                    matcher,
                    completions_trigger,
                    editor_model,
                    cursor_position,
                    completion_context,
                    ctx,
                );
            }
        }
    }

    /// _Asynchronously_ generates completions by calling into the completer.
    #[allow(clippy::too_many_arguments)]
    fn run_completions_async(
        &mut self,
        before_cursor_text: String,
        matcher: MatchStrategy,
        completions_trigger: CompletionsTrigger,
        editor_snapshot: EditorSnapshot,
        cursor_position: ByteOffset,
        completion_context: SessionContext,
        ctx: &mut ViewContext<'_, Input>,
    ) {
        let buffer_text = self.buffer_text(ctx);

        // The 'ForceNativeShellCompletions' user pref can be used to unconditionally
        // generate and show native shell completion results (i.e. regardless of whether or
        // not we have completion results via completion specs).
        let force_native_shell_completions = ctx
            .private_user_preferences()
            .read_value("ForceNativeShellCompletions")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        let use_native_shell_completions = (FeatureFlag::NativeShellCompletions.is_enabled() || force_native_shell_completions)
            && completion_context
                .session
                .shell()
                .supports_native_shell_completions()
            // For now, don't use native shell completions for multi-line commands.
            && !buffer_text.contains('\n');

        let fallback_strategy = match completions_trigger {
            CompletionsTrigger::Keybinding if !use_native_shell_completions => {
                CompletionsFallbackStrategy::FilePaths
            }
            _ => CompletionsFallbackStrategy::None,
        };

        if self.is_completions_while_typing_turned_on(ctx) {
            if let Some(last_abort_handle) = self.completions_abort_handle.take() {
                last_abort_handle.abort();
            }
        }

        let Some(session_id) = self.completer_data().active_block_session_id() else {
            return;
        };
        let session_env_vars = self.sessions.read(ctx, |sessions, _| {
            sessions.get_env_vars_for_session(session_id)
        });

        let cursor_position = cursor_position.as_usize();
        let native_results_fut = if use_native_shell_completions {
            // If we're using native shell completions, construct a future that
            // will be resolved with any completions data provided by the shell.
            let (results_tx, results_rx) = async_channel::unbounded();
            ctx.dispatch_typed_action(&TerminalAction::RunNativeShellCompletions {
                buffer_text: buffer_text[0..cursor_position].to_owned(),
                results_tx,
            });
            async move { results_rx.recv().await.ok() }.boxed()
        } else {
            // If not, we can immediately say that there are no completion
            // results from the shell.
            futures::future::ready(None).boxed()
        };

        let completion_session = completion_context.session.clone();

        let abort_handle = ctx
            .spawn_abortable(
                async move {
                    let suggestions = completer::suggestions(
                        before_cursor_text.as_str(),
                        cursor_position,
                        session_env_vars.as_ref(),
                        CompleterOptions {
                            match_strategy: matcher,
                            fallback_strategy,
                            suggest_file_path_completions_only: false,
                            parse_quotes_as_literals: false,
                        },
                        &completion_context,
                    )
                    .await;

                    let suggestions = match suggestions {
                        Some(s) if !s.suggestions.is_empty() && !force_native_shell_completions => {
                            Some(s)
                        }
                        _ => native_results_fut.await.map(|results| {
                            let suggestions = results.into_iter().map(Into::into).collect_vec();

                            let token_end = cursor_position;
                            // Within the section of the buffer from the start
                            // to the end of this token...
                            let token_start = buffer_text[0..token_end]
                                // Find the last whitespace char before the token end.
                                .rfind(char::is_whitespace)
                                // If we find one, the token start is the next char.
                                .map(|pos| pos + 1)
                                // Otherwise, the start is the beginning of the buffer.
                                .unwrap_or_default();

                            SuggestionResults {
                                replacement_span: (token_start, token_end).into(),
                                suggestions,
                                match_strategy: MatchStrategy::Fuzzy,
                            }
                        }),
                    };

                    (suggestions, completions_trigger, editor_snapshot)
                },
                |input, (suggestions, completions_trigger, editor_model), ctx| {
                    input.handle_completion_suggestions_results(
                        suggestions,
                        completions_trigger,
                        editor_model,
                        ctx,
                    )
                },
                move |_, _| {
                    completion_session.cancel_active_commands();
                },
            )
            .abort_handle();

        self.completions_abort_handle = Some(abort_handle);
    }

    fn path_separators(&self, ctx: &AppContext) -> PathSeparators {
        self.active_session(ctx)
            .map(|session| session.path_separators())
            .unwrap_or(PathSeparators::for_os())
    }

    /// Returns the buffer point that the tab completion menu should be positioned relative to.
    /// If None, the menu should be positioned relative to the cursor.
    ///
    /// In regular completions mode, we want to dock the completions menu at the cursor.
    ///
    /// In classic completions mode, we want to dock the completions menu at the start of
    /// the replacement span*. This ensures that the menu doesn't jump around as the cursor
    /// moves when the user cycles through items in the menu.
    /// * The one edge case is when we're completing a file path. In this case, the menu
    ///   should be docked at the end of the last directory in the replacement span.
    ///   This is because the replacement span will include the entire file path.
    ///   For example, if the user types "cd app/D" and one of the completion display result is
    ///   "Documents", then the replacement span will be for "app/D" and the replacement will
    ///   be "app/Documents".
    fn tab_completions_menu_position(
        &self,
        results: &SuggestionResults,
        buffer_text_original: &str,
        ctx: &AppContext,
    ) -> Option<BufferPoint> {
        // In regular mode, the menu should be positioned at the cursor.
        if !self.is_classic_completions_enabled(ctx) {
            return None;
        }

        // Note: the replacement span is in terms of byte offsets.
        // But these byte offsets should correspond to valid char offsets.
        let start = results.replacement_span.start();
        let end = results.replacement_span.end();

        let all_results_are_file_completions = results
            .suggestions
            .iter()
            .all(|s| s.suggestion.file_type.is_some());

        let offset = if all_results_are_file_completions {
            // If all the results are file completions, let's find the last slash in the replacement
            // span and dock the completions menu right after it. We do this because the replacement
            // span of file path completions is relative to the beginning of the file path. For
            // example, if the user types "cd app/D" and one of the completion display result is
            // "Documents", then the replacement span will be for "app/D" and the replacement will
            // be "app/Documents".
            buffer_text_original
                .get(0..end)
                .and_then(|s| s.rfind(self.path_separators(ctx).all))
                .map(|i| i + 1)
                .unwrap_or(start)
        } else {
            start
        };

        let point = self
            .editor
            .as_ref(ctx)
            .point_for_offset(ByteOffset::from(offset), ctx);
        point.ok()
    }

    fn handle_completion_suggestions_results(
        &mut self,
        results: Option<SuggestionResults>,
        completions_trigger: CompletionsTrigger,
        editor_snapshot_when_completer_was_ran: EditorSnapshot,
        ctx: &mut ViewContext<Self>,
    ) {
        let current_editor_model = self
            .editor
            .read(ctx, |editor, ctx| editor.snapshot_model(ctx));

        let buffer_text = self.editor.as_ref(ctx).buffer_text(ctx);
        // If the editor has changed since the completions trigger was hit-- noop since the
        // suggestions are no longer valid. Note that we purposely ignore attributes such as text
        // styles for the purposes of this check (we only care about the buffer text content and
        // the cursor selections state).
        if buffer_text != editor_snapshot_when_completer_was_ran.text()
            || current_editor_model.selections()
                != editor_snapshot_when_completer_was_ran.selections()
        {
            return;
        }

        match results {
            None => {
                // It's necessary to specifically set to closed in the case where we first
                // opened the tab menu and then keep typing
                self.suggestions_mode_model.update(ctx, |m, ctx| {
                    m.set_mode(InputSuggestionsMode::Closed, ctx);
                });
            }
            Some(results) if results.suggestions.is_empty() => {
                self.suggestions_mode_model.update(ctx, |m, ctx| {
                    m.set_mode(InputSuggestionsMode::Closed, ctx);
                });
            }
            Some(results) => {
                match (results.single_prefix_suggestion(), completions_trigger) {
                    (Some(only_prefix_suggestion), CompletionsTrigger::Keybinding) => {
                        // If there is exactly one prefix suggestion, just insert into the buffer.
                        self.insert_completion_result_into_editor(
                            only_prefix_suggestion.replacement(),
                            results.replacement_span.start(),
                            Executing::No,
                            ctx,
                        );
                    }
                    (_, completions_trigger) => {
                        let buffer_text_original = buffer_text
                            [0..self.start_byte_index_of_last_selection(ctx).as_usize()]
                            .to_string();

                        if completions_trigger == CompletionsTrigger::Keybinding {
                            if let Some(common_prefix) = longest_common_prefix(
                                results
                                    .suggestions
                                    .iter()
                                    .filter(|suggestion| {
                                        // Ignore fuzzy matches and case-insensitive matches
                                        // when calculating the longest common prefix, so we
                                        // are able to insert a common prefix more often.
                                        matches!(
                                            suggestion.match_type,
                                            Match::Prefix {
                                                is_case_sensitive: true
                                            } | Match::Exact {
                                                is_case_sensitive: true
                                            }
                                        )
                                    })
                                    .map(|suggestion| suggestion.replacement()),
                            ) {
                                // Insert the common prefix if it is longer than what the user has
                                // already typed. This check is necessary because the suggestions
                                // are case-insensitive, while the common prefix is necessarily
                                // case-sensitive. That can lead to the common prefix being shorter
                                // than the input, causing confusing behavior where the input is
                                // truncated. Also, only fill in the common prefix if the
                                // replacement itself is a prefix of the common prefix. If there
                                // are only fuzzy completions, then it's possible this is not the
                                // case, and we don't want to fill in the common prefix in that
                                // case.
                                let replacement_start = results.replacement_span.start();
                                let current_word = &buffer_text_original[replacement_start
                                    ..self.start_byte_index_of_last_selection(ctx).as_usize()];
                                if common_prefix.len() > results.replacement_span.distance()
                                    && common_prefix.starts_with(current_word)
                                {
                                    self.insert_completion_prefix_into_editor(
                                        ctx,
                                        common_prefix,
                                        results.replacement_span.start(),
                                    );
                                }
                            }
                        }

                        // If not using completions as you type, then
                        // clear any autosuggestions when tab completions are open.
                        // The autosuggestion will be repopulated when the menu is closed.
                        // We don't do this for completions as you type because the user would
                        // otherwise hardly see autosuggestons.
                        if FeatureFlag::RemoveAutosuggestionDuringTabCompletions.is_enabled()
                            && !self.is_completions_while_typing_turned_on(ctx)
                        {
                            self.editor.update(ctx, |view, ctx| {
                                view.clear_autosuggestion(ctx);
                            });
                        }

                        // Decide where to render the tab completion menu.
                        // If we're rendering it at a specific position, let's make sure
                        // that position exists in the position cache.
                        let position = self.tab_completions_menu_position(
                            &results,
                            &buffer_text_original,
                            ctx,
                        );
                        let menu_position = if let Some(position) = position {
                            self.editor.update(ctx, |editor, ctx| {
                                editor.cache_buffer_point(
                                    position,
                                    COMPLETIONS_START_OF_REPLACEMENT_SPAN_POSITION_ID,
                                    ctx,
                                );
                            });
                            TabCompletionsMenuPosition::AtStartOfReplacementSpan
                        } else {
                            TabCompletionsMenuPosition::AtLastCursor
                        };

                        self.suggestions_mode_model.update(ctx, |m, ctx| {
                            m.set_mode(
                                InputSuggestionsMode::CompletionSuggestions {
                                    replacement_start: results.replacement_span.start(),
                                    buffer_text_original,
                                    completion_results: results.clone(),
                                    trigger: completions_trigger,
                                    menu_position,
                                },
                                ctx,
                            );
                        });

                        send_telemetry_from_ctx!(
                            TelemetryEvent::OpenSuggestionsMenu(
                                self.suggestions_mode_model
                                    .as_ref(ctx)
                                    .mode()
                                    .to_telemetry_mode(),
                            ),
                            ctx
                        );

                        let preselect_option = if self.is_classic_completions_enabled(ctx) {
                            TabCompletionsPreselectOption::Unselected
                        } else {
                            TabCompletionsPreselectOption::First
                        };

                        self.input_suggestions
                            .update(ctx, |input_suggestions, ctx| {
                                input_suggestions.prefix_search_for_tab_completion(
                                    results.replacement_span.slice(&buffer_text),
                                    &results,
                                    preselect_option,
                                    ctx,
                                );
                            });
                    }
                }
            }
        }
        ctx.notify();
    }

    /// Replace the replacement with the common completion prefix. Note that completion prefix
    /// itself is not the completion result so we don't add a space.
    fn insert_completion_prefix_into_editor(
        &mut self,
        ctx: &mut ViewContext<Input>,
        completion_prefix: &str,
        replacement_start: usize,
    ) {
        self.editor.update(ctx, |input, ctx| {
            let cursor_end_offset = input.end_byte_index_of_last_selection(ctx);
            input.select_and_replace(
                completion_prefix,
                [ByteOffset::from(replacement_start)..cursor_end_offset],
                PlainTextEditorViewAction::AcceptCompletionSuggestion,
                ctx,
            );
        });
    }

    /// Replace the replacement with the completion result and potentially add a space after.
    fn insert_completion_result_into_editor(
        &mut self,
        completion_result: &str,
        replacement_start: usize,
        executing: Executing,
        ctx: &mut ViewContext<Input>,
    ) {
        let is_completions_as_you_type_enabled = self.is_completions_while_typing_turned_on(ctx);
        self.editor.update(ctx, |input, ctx| {
            let cursor_end_offset = input.end_byte_index_of_last_selection(ctx);

            // Add a space to the end if the end of the selection/replacement
            // is at the end of the buffer and the completion result doesn't end with a slash.
            // If completions as you type is turned on and classic completions is off, then
            // _don't_ add a space.
            let is_classic_completions_enabled = self.is_classic_completions_enabled(ctx);
            let replacement: Cow<str> = if (!is_completions_as_you_type_enabled
                || is_classic_completions_enabled)
                && cursor_end_offset.as_usize() == input.buffer_text(ctx).len()
                && !completion_result.ends_with(self.path_separators(ctx).main)
                && executing == Executing::No
            {
                format!("{completion_result} ").into()
            } else {
                completion_result.into()
            };

            input.select_and_replace(
                &replacement,
                [ByteOffset::from(replacement_start)..cursor_end_offset],
                PlainTextEditorViewAction::AcceptCompletionSuggestion,
                ctx,
            );
        });
        send_telemetry_from_ctx!(TelemetryEvent::TabSingleResultAutocompletion, ctx);
    }

    /// Whether the editor is in a state where we should tab complete instead of indenting text
    /// within the editor.
    /// The editor is considered in a state where we should tab complete if:
    ///     1) The buffer text is not empty.
    ///     2) The user is not actively selecting.
    ///     3) There is only a single selection and that selection does not take up the entire
    ///        buffer.
    fn cursor_positioned_for_completion(&self, ctx: &mut ViewContext<Self>) -> bool {
        let input = self.editor.as_ref(ctx);
        let buffer_text = input.buffer_text(ctx);

        // We can show the completion menu when there is a single cursor selection
        // and we aren't actively selecting.
        !buffer_text.trim_start().is_empty()
            && !input.is_selecting(ctx)
            && input.num_selections(ctx) == 1
            && !input.any_selections_span_entire_buffer(ctx)
    }


    fn input_shift_tab(&mut self, ctx: &mut ViewContext<Self>) {
        match self.suggestions_mode_model.as_ref(ctx).mode() {
            // If the model selector is open and has multiple tabs,
            // shift + tab should cycle between them.
            // If the inline history menu is open and has multiple tabs,
            // shift + tab should cycle between them.
            // If the conversation menu is open and has multiple tabs,
            // shift + tab should cycle between them.
            // If we're in CompletionSuggestions mode, shift tab moves to the previous selection.
            InputSuggestionsMode::CompletionSuggestions { .. } => {
                self.input_suggestions.update(ctx, |suggestions, ctx| {
                    suggestions.select_prev(ctx);
                });
                return;
            }
            _ => {}
        }

        self.editor.update(ctx, |input, ctx| input.unindent(ctx));
    }

    pub fn completion_session_context(&self, ctx: &AppContext) -> Option<SessionContext> {
        self.active_block_session_id()
            .and_then(|active_block_session_id| {
                let current_session = self.sessions.as_ref(ctx).get(active_block_session_id);
                let pwd = self
                    .active_block_metadata
                    .as_ref()
                    .and_then(BlockMetadata::current_working_directory)
                    .map(str::to_owned);

                current_session.zip(pwd).map(|(current_session, pwd)| {
                    // TODO(abhishek): Ideally, BlockMetadata::current_working_directory should directly
                    // return a TypedPathBuf. This shouldn't happen here in the view.
                    let current_working_directory =
                        current_session.convert_directory_to_typed_path_buf(pwd);
                    SessionContext::new(
                        current_session,
                        CommandRegistry::global_instance(),
                        current_working_directory,
                        ctx,
                    )
                })
            })
    }

    pub fn active_session(&self, ctx: &AppContext) -> Option<Arc<Session>> {
        self.active_block_session_id()
            .and_then(|active_block_session_id| {
                self.sessions.as_ref(ctx).get(active_block_session_id)
            })
    }

    fn hide_x_ray(&mut self, ctx: &mut ViewContext<Self>) {
        if self.command_x_ray_description.take().is_some() {
            self.editor.update(ctx, |editor, ctx| {
                editor.clear_command_x_ray();
                ctx.notify();
            });
            ctx.notify();
        }
    }

    fn start_xray_at_offset(
        &mut self,
        pos: ByteOffset,
        trigger: CommandXRayTrigger,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(completion_context) = self.completion_session_context(ctx) {
            let buffer_text = self.buffer_text(ctx);
            let _ =
                ctx.spawn(
                    async move {
                        completer::describe(buffer_text.as_str(), pos, &completion_context).await
                    },
                    |input, description, ctx| {
                        input.show_xray(description, trigger, ctx);
                    },
                );
        }
    }

    fn show_xray(
        &mut self,
        description: Option<Description>,
        trigger: CommandXRayTrigger,
        ctx: &mut ViewContext<'_, Self>,
    ) {
        let description = description.map(Arc::new);
        self.command_x_ray_description.clone_from(&description);
        if let Some(description) = description {
            if trigger == CommandXRayTrigger::Keystroke {
                ctx.emit_a11y_content(AccessibilityContent::new_without_help(
                    description.a11y_text(),
                    RiftA11yRole::UserAction,
                ));
            }
            ctx.notify();
            self.editor.update(ctx, move |editor, ctx| {
                editor.set_command_x_ray(description);
                ctx.notify();
            });
        }
        ctx.notify();
    }

    fn active_block_session_id(&self) -> Option<SessionId> {
        self.active_block_metadata
            .as_ref()
            .and_then(BlockMetadata::session_id)
    }

    /// Handles a tab keypress from the editor.
    ///
    /// "Tab" is the default trigger to open the completion suggestions menu, but this may be
    /// overridden in settings. If the completion suggestions menu is already open, tab and
    /// shift-tab are used to select the next and previous suggestion, respectively -- this is not
    /// overridable; note that even if "open completion suggestions menu" is rebound to a non-tab
    /// key, tab and shift-tab are still used to navigate within the menu once it is open.
    ///
    /// If tab is not bound to "open completion suggestions menu" nor is the suggestions menu
    /// already open, inserts a tab char into the input editor.
    fn input_tab(&mut self, ctx: &mut ViewContext<Self>) {
        // We have to manually check if "tab" is bound to
        // `InputAction::MaybeOpenCompletionSuggestions` here because the child `EditorView`
        // handles the actual tab keypress event -- the handler method attached to the
        // `EditableBinding` for `MaybeOpenCompletionSuggestions` is not called when the
        // binding is tab because the UI framework dictates that only one View may receive a
        // keypress event.
        let is_tab_bound_to_open_completions =
            bindings::keybinding_name_to_keystroke(OPEN_COMPLETIONS_KEYBINDING_NAME, ctx)
                .map(|keystroke| keystroke.key == "tab")
                .unwrap_or_default();

        let replacement_start_opt = if let InputSuggestionsMode::CompletionSuggestions {
            replacement_start,
            ..
        } = self.suggestions_mode_model.as_ref(ctx).mode()
        {
            Some(*replacement_start)
        } else {
            None
        };
        if let Some(replacement_start) = replacement_start_opt {
            // The completions menu is already open, in which there are two cases.
            // Case 1: There is a common prefix amongst filtered suggestions that we could fill; so
            //         we fill it in buffer.
            // Case 2: Else, tab should move to next option.
            let (common_prefix_of_filtered_suggestions, is_single_prefix_suggestion) =
                self.input_suggestions.read(ctx, |suggestions, _| {
                    // Ignore fuzzy matches when calculating longest common
                    // prefix of suggestions. So even if there are fuzzy
                    // matches, we can find a common prefix and try to insert it.
                    let suggestion_texts = suggestions
                        .items()
                        .iter()
                        .filter(|item| {
                            matches!(
                                item.match_type(),
                                MatchType::Prefix {
                                    is_case_sensitive: true
                                } | MatchType::Exact {
                                    is_case_sensitive: true
                                }
                            )
                        })
                        .map(|item| item.text())
                        .collect_vec();
                    let num_suggestions = suggestion_texts.len();
                    (
                        longest_common_prefix(suggestion_texts).map(|x| x.to_owned()),
                        num_suggestions == 1,
                    )
                });
            if let Some(common_prefix) = common_prefix_of_filtered_suggestions {
                let input_text = self.editor.as_ref(ctx).buffer_text(ctx);
                // Determine the current word in the editor that will be replaced by the tab
                // completion. We use the start index of the selection since the completer only sees
                // the text up to the start of the selection when generating completion results.
                let current_word = &input_text
                    [replacement_start..self.start_byte_index_of_last_selection(ctx).as_usize()];

                // Insert the common prefix if it is longer than what the user has currently typed
                // This check is necessary because the suggestions are case-insensitive, while the
                // common prefix logic is necessarily case-sensitive. That can lead to the common
                // prefix being shorter, causing confusing behavior where the input is shortened.
                // Also, we check if the replacement
                if common_prefix.len() > current_word.len()
                    && common_prefix.starts_with(current_word)
                {
                    self.insert_completion_prefix_into_editor(
                        ctx,
                        &common_prefix,
                        replacement_start,
                    );
                    // If there was only a single completion remaining and we just inserted it into the editor,
                    // close the completions menu.
                    if is_single_prefix_suggestion {
                        self.close_input_suggestions(true, ctx)
                    }
                    return;
                }
            }
            self.input_suggestions.update(ctx, |suggestions, ctx| {
                suggestions.select_next(ctx);
            });
        } else if is_tab_bound_to_open_completions && self.cursor_positioned_for_completion(ctx) {
            self.open_completion_suggestions(CompletionsTrigger::Keybinding, ctx);
        } else {
            // Otherwise, pass the tab down to the editor
            self.editor.update(ctx, |input, ctx| input.handle_tab(ctx));
        }
    }

    /// Opens the completion suggestions menu if the cursor is in a valid position to generate
    /// suggestions and the menu is not already open.
    ///
    /// This is called when [`InputAction::MaybeOpenCompletionSuggestions`] is bound to a non-tab
    /// key; tab is the default binding. This is _not_ called when the binding is set to the
    /// default ("tab") because the tab keypress event is actually handled by the child
    /// [`Editor`] view, so the tab event is never actually propagated to this input view. Instead,
    /// the logic to open the completions menu when tab bound is implemented in
    /// [`Self::input_tab()`], which is called when the editor emits an
    /// `EditorEvent::Navigate(NavigationKey::Tab)`.
    ///
    /// Ultimately this weirdness is due to limitations in the UI framework preventing multiple
    /// `View`s from handling/responding to the same `Event`.
    fn maybe_open_completion_suggestions(&mut self, ctx: &mut ViewContext<Self>) {
        if !matches!(
            self.suggestions_mode_model.as_ref(ctx).mode(),
            InputSuggestionsMode::CompletionSuggestions { .. },
        ) && self.cursor_positioned_for_completion(ctx)
        {
            self.open_completion_suggestions(CompletionsTrigger::Keybinding, ctx);
        }
    }

    #[cfg(test)]
    fn user_insert(&mut self, text: &str, ctx: &mut ViewContext<Self>) -> bool {
        self.insert_internal(text, EditOrigin::UserTyped, ctx)
    }

    pub fn user_replace_editor_text(&mut self, text: &str, ctx: &mut ViewContext<Self>) -> bool {
        self.editor.update(ctx, |editor, ctx| {
            editor.select_all(ctx);
        });
        self.insert_internal(text, EditOrigin::UserTyped, ctx)
    }

    // It's the responsibility of the caller to ensure that the text submitted here
    // should be inputted into the input area (i.e. arrow keys should not be
    // included in the string).
    pub fn system_insert(&mut self, text: &str, ctx: &mut ViewContext<Self>) -> bool {
        self.insert_internal(text, EditOrigin::UserInitiated, ctx)
    }

    pub fn has_pending_command(&self) -> bool {
        self.has_pending_command
    }

    pub fn set_pending_command(&mut self, exec: &str, ctx: &mut ViewContext<Self>) {
        self.has_pending_command = true;
        self.system_insert(exec, ctx);
    }

    fn should_enter_accept_completion_suggestion(&self, app: &AppContext) -> bool {
        let InputSuggestionsMode::CompletionSuggestions {
            replacement_start, ..
        } = self.suggestions_mode_model.as_ref(app).mode()
        else {
            return false;
        };
        let completions_while_typing = self.is_completions_while_typing_turned_on(app);
        let selected_item = self.input_suggestions.as_ref(app).get_selected_item_text();

        // If classic completions is enabled, accept the suggestion if an item is selected.
        if self.is_classic_completions_enabled(app) {
            return self
                .input_suggestions
                .as_ref(app)
                .get_selected_item()
                .is_some();
        }
        // If completions as you type is disabled, accept the suggestion if an item is selected.
        if !completions_while_typing {
            return selected_item.is_some();
        }

        let path_separators = self.path_separators(app).all;

        // At this point, we know completions as you type is enabled and classic completions
        // is disabled. Accept the completion unless the buffer already matches the selected item
        // (in which case, just execute the command).
        let current_buffer_text = self.editor.as_ref(app).buffer_text(app);
        selected_item.is_none_or(|selected_item| {
            let Some(replacement) = &current_buffer_text.get(*replacement_start..) else {
                log::error!("Failed to get replacement range in current buffer text");
                return true;
            };
            if replacement == &selected_item {
                return false;
            }
            let Some(no_slash) = selected_item.strip_suffix(path_separators) else {
                return true;
            };
            replacement != &no_slash
        })
    }

    /// Determines whether to insert a newline in the buffer instead of executing a command
    /// when enter is pressed.
    fn should_insert_newline_on_enter(&self, ctx: &AppContext) -> bool {
        let editor = self.editor.as_ref(ctx);
        let shell_family = editor.shell_family();
        editor.chars_preceding_selections(ctx).any(|chars| {
            let mut preceding_chars = chars.rev();
            while let Some(c) = preceding_chars.next() {
                match shell_family {
                    Some(ShellFamily::PowerShell) => {
                        if c == '`' {
                            // Kind of a quirk, but PowerShell only inserts a
                            // newline after a backtick if the character preceding
                            // the backtick is whitespace.
                            if let Some(c) = preceding_chars.next() {
                                if !c.is_ascii_whitespace() {
                                    return false;
                                }
                            }
                            return true;
                        }
                    }
                    Some(ShellFamily::Posix) | None => {
                        if c == '\\' {
                            // Continue if there are more \ characters
                            if let Some(c) = preceding_chars.next() {
                                if c == '\\' {
                                    continue;
                                }
                            }
                            // Odd number of \ characters
                            return true;
                        }
                    }
                }
                return false;
            }
            false
        })
    }



    /// Handles the user's 'Enter' keypress.
    ///
    /// Depending on input state, this method may either execute a command, accept an input
    /// suggestion, or add a newline to the input buffer contents.  If there is an active and long
    /// running command, exits early and does nothing. This method should not be callable if there
    /// is an active and long running command; in such a state, the enter keypress should be
    /// handled by the ongoing process corresponding to the active/long running command.
    pub(crate) fn input_enter(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(Event::Enter);

        if self.should_insert_newline_on_enter(ctx) {
            self.editor.update(ctx, |editor, ctx| {
                editor.user_initiated_insert("\n", PlainTextEditorViewAction::NewLine, ctx)
            });
        } else if matches!(
            self.suggestions_mode_model.as_ref(ctx).mode(),
            InputSuggestionsMode::CompletionSuggestions { .. }
        ) && self.should_enter_accept_completion_suggestion(ctx)
        {
            self.input_suggestions.update(ctx, |suggestions, ctx| {
                suggestions.confirm(ctx);
            })
        } else {
            let command = self.get_command(ctx);
            if !self.try_execute_command(&command, ctx) {
                return;
            }

            if SyncedInputState::as_ref(ctx).is_syncing_any_inputs(ctx.window_id()) {
                ctx.emit(Event::SyncInput(SyncInputType::RanCommand));
            }

            self.model.lock().set_is_input_dirty(false);
        }
    }

    /// Ctrl+Enter previously submitted AI input; that behavior has been removed, so
    /// this is now a no-op. Exposed `pub(crate)` for unit tests.
    pub(crate) fn input_ctrl_enter(&mut self, _ctx: &mut ViewContext<Self>) {}

    fn input_cmd_enter(&mut self, _ctx: &mut ViewContext<Self>) {}










    fn get_command(&mut self, ctx: &mut ViewContext<Self>) -> String {
        // Expand valid abbreviations or aliases, if any
        if let Some(expanded_command) = self.get_expanded_command_on_execute(ctx) {
            return expanded_command;
        }
        self.editor.as_ref(ctx).buffer_text(ctx)
    }

    /// Inserts the given text into the input buffer.
    fn insert_internal(
        &mut self,
        text: &str,
        edit_origin: EditOrigin,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        if matches!(edit_origin, EditOrigin::UserTyped) {
            self.model.lock().set_is_input_dirty(true);
        }

        ctx.focus(&self.editor);
        self.editor.update(ctx, |editor, ctx| match edit_origin {
            EditOrigin::UserTyped => editor.user_insert(text, ctx),
            EditOrigin::UserInitiated => {
                editor.user_initiated_insert(text, PlainTextEditorViewAction::SystemInsert, ctx)
            }
            EditOrigin::SystemEdit => {
                editor.system_insert(text, PlainTextEditorViewAction::SystemInsert, ctx)
            }
            EditOrigin::SyncedTerminalInput | EditOrigin::RemoteEdit => (),
        });
        ctx.notify();
        true
    }



    /// Resets state in the input box that depends on the block lifecycle.
    /// This is on a performance-sensitive path.
    ///
    /// If the newly created block is for an executed user command, the input buffer is cleared.
    pub fn handle_block_completed_event(
        &mut self,
        block_completed_event: BlockCompletedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        // We clear the input box after executing a command here instead of where we
        // execute a command to avoid the input box flashing when its contents are
        // cleared. For the multiline input box case, this also caused contents to go
        // off the screen because we were forcing the long running command to be the same
        // size of the cleared input box.
        if let BlockType::User(_user_block) = &block_completed_event.block_type {
            let should_clear_buffer = true;
            let input_contents_before_prompt_chip_command =
                self.input_contents_before_prompt_chip_command.take();

            if should_clear_buffer {
                // We want to reinitialize the buffer whenever a command is completed so that
                // state does not leak from buffer to buffer (e.g. edit history).
                self.editor
                    .update(ctx, |editor, ctx| editor.reinitialize_buffer(None, ctx));

                // If we have a pending input restore (from a prompt chip command like cd),
                // restore the input contents instead of leaving the buffer empty.
                if let Some(restore_text) = input_contents_before_prompt_chip_command {
                    self.editor.update(ctx, |editor, ctx| {
                        editor.set_buffer_text(&restore_text, ctx);
                    });
                    self.is_editor_empty_on_last_edit = false;
                } else {
                    // This is the one place where buffer contents can change without an `Edit`
                    // -- this is because the buffer semantically isn't being edited, a new one is
                    // being constructed.
                    self.is_editor_empty_on_last_edit = true;
                    ctx.emit(Event::InputEmptyStateChanged {
                        is_empty: true,
                        reason: InputEmptyStateChangeReason::UserCommandCompleted,
                    });
                }
            }

            // Generate autosuggestion if the input is not empty (user had type-ahead).
            self.maybe_generate_autosuggestion(ctx);
        }

        self.input_render_state_model_handle
            .update(ctx, |input_render_state_model, _| {
                input_render_state_model.set_editor_modified_since_block_finished(false);
            });

        // Re-render for anything that depends on the block list (e.g. zero state AM chips).
        ctx.notify();
    }

    /// Performs any post-block completion processing that's relevant to the input.
    ///
    /// This is triggered after [`Self::handle_block_completed_event`] as
    /// the handling of the main block completed event is a sensitive path.
    pub fn handle_after_block_completed_event(
        &mut self,
        block: BlockType,
        ctx: &mut ViewContext<Self>,
    ) {
        if let BlockType::User(_) = block {
            ctx.emit(Event::InputStateChanged(InputState::Enabled));
        } else if block.is_bootstrap_block()
            && self
                .model
                .lock()
                .block_list()
                .is_bootstrapping_precmd_done()
        {
            // If the user typed ahead during bootstrap, the autosuggestion and
            // completions-as-you-type requests were silently skipped (history
            // wasn't queryable, session ID was absent). Now that bootstrap is
            // done, retry them so ghost text appears without the user having to
            // re-type.
            if !self.buffer_text(ctx).is_empty() {
                self.maybe_generate_autosuggestion(ctx);

                if self.should_show_completions_while_typing(ctx) {
                    self.open_completion_suggestions(CompletionsTrigger::AsYouType, ctx);
                }
            }
        }
    }

    /// 'Starts' the active block and sends its command bytes to the pty.
    ///
    /// Additionally, the executed command is recorded to history if appropriate.
    fn start_block_and_write_command_to_pty(
        &mut self,
        command: &str,
        source: CommandExecutionSource,
        ctx: &mut ViewContext<Self>,
    ) {
        start_trace!("command_execution:start");

        // Abort running completions since we're about to execute a command.
        if let Some(abort_handle) = self.completions_abort_handle.take() {
            abort_handle.abort();
        }
        self.abort_latest_autosuggestion_future();

        if let Some(future_handle) = self.decorations_future_handle.take() {
            future_handle.abort_handle().abort();
        }

        let session_id = self
            .active_block_session_id()
            .expect("session_id should be set (via bootstrap) before executing command");

        ctx.emit(Event::ExecuteCommand(Box::new(ExecuteCommandEvent {
            command: command.to_string(),
            workflow_id: None,
            session_id,
            workflow_command: None,
            should_add_command_to_history: true,
            source,
        })));
        end_trace!();
    }

    pub fn notify_and_notify_children(&self, ctx: &mut ViewContext<Self>) {
        ctx.notify();
        // The left notch may have been updated due to the prompt updating, in the case of
        // same-line prompt!
        self.editor.update(ctx, |_editor, ctx| {
            ctx.notify();
        });
    }

    /// Returns a tuple (prompt_text, rprompt_text).
    pub fn prompt_and_rprompt_text(&self, app: &AppContext) -> (String, Option<String>) {
        let model = self.model.lock();
        let appearance = Appearance::as_ref(app);
        let (lprompt_top, lprompt_bottom, rprompt) = self
            .prompt_render_helper
            .render_prompt(&model, appearance, app);
        // Separate this into a helper (follow-up PR?)

        let show_universal_developer_input = self.should_show_universal_developer_input(app);

        let lprompt_top_text = lprompt_top.map(|rendered| rendered.element.text(app));
        let lprompt_bottom_text = lprompt_bottom.map(|rendered| rendered.element.text(app));
        let rprompt_text = rprompt.map(|rendered| rendered.element.text(app));
        if should_render_prompt_on_same_line(show_universal_developer_input, &model, app) {
            if let Some(lprompt_top_text) = lprompt_top_text {
                (
                    lprompt_top_text + "\n" + &lprompt_bottom_text.unwrap_or_default(),
                    rprompt_text,
                )
            } else {
                (lprompt_bottom_text.unwrap_or_default(), rprompt_text)
            }
        } else {
            (lprompt_top_text.unwrap_or_default(), rprompt_text)
        }
    }

    pub fn create_prompt_elements(&self, app: &AppContext) -> SessionNavigationPromptElements {
        let model = self.model.lock();
        let block = self.prompt_render_helper.prompt_block(&model);
        let is_udi = InputSettings::as_ref(app).is_universal_developer_input_enabled(app);
        let mut prompt_elements = SessionNavigationPromptElements {
            ps1_prompt_grid: None,
            prompt_chip_snapshot: None,
        };

        if let Some(block) = block {
            if !is_udi && block.honor_ps1() && model.block_list().is_bootstrapped() {
                // PS1 mode: capture the raw prompt grid so the command palette
                // can render it with full fidelity (CORE-1683).
                prompt_elements.ps1_prompt_grid = Some(block.prompt_grid().clone());
            }
        }

        // Always capture a chip snapshot as the fallback prompt representation.
        // This covers both UDI mode and any edge cases where PS1 is not available
        // (e.g. not yet bootstrapped, block-level honor_ps1 mismatch).
        if prompt_elements.ps1_prompt_grid.is_none() {
            prompt_elements.prompt_chip_snapshot = Some(self.prompt_type.as_ref(app).snapshot(app));
        }
        prompt_elements
    }

    /// This function determines if the subshell flag should be in the input editor. The flag
    /// should show here if there are no blocks in the block list for this subshell session, which
    /// will be the case if no non-hidden blocks have been executed yet or the block list was
    /// cleared.
    fn get_subshell_flag_render_state(
        &self,
        model: &TerminalModel,
        spacing_is_compact: bool,
        app: &AppContext,
    ) -> Option<SubshellRenderState> {
        if spacing_is_compact {
            return None;
        }
        let session_id = self.active_block_session_id()?;
        let should_render = self
            .sessions
            .as_ref(app)
            .get(session_id)
            .and_then(|session| {
                session.subshell_info().as_ref().map(|info| {
                    if let Some(env_var_collection_name) = &info.env_var_collection_name {
                        Some(SubshellRenderState::Flag(SubshellSource::EnvVarCollection(
                            env_var_collection_name.to_owned(),
                        )))
                    } else {
                        info.spawning_command.split_whitespace().next().map(|exec| {
                            SubshellRenderState::Flag(SubshellSource::Command(exec.to_owned()))
                        })
                    }
                })
            })?;

        let block_list = model.block_list();
        let block_before_active_block = block_list
            .prev_non_hidden_block_from_index(block_list.active_block_index())
            .and_then(|index| block_list.block_at(index));

        match block_before_active_block {
            // If there is a block before the editor, and it belongs to this same subshell session,
            // the flag will be in the block list, and hence doesn't need to be in the editor.
            // Only extend the flag into the editor.
            Some(block) if block.session_id() == Some(session_id) => {
                Some(SubshellRenderState::Flagpole)
            }
            // Otherwise, this editor (the active block) is the first in this subshell session, and
            // we should show the flag here.
            _ => should_render,
        }
    }

    pub fn set_active_block_metadata(
        &mut self,
        active_block_metadata: BlockMetadata,
        is_after_in_band_command: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let active_session = active_block_metadata
            .session_id()
            .and_then(|session_id| self.sessions.as_ref(ctx).get(session_id));
        if let Some(session) = active_session {
            let transformer: Option<PathTransformerFn> = session
                .windows_path_converter()
                .map(|convert| Box::new(convert) as PathTransformerFn);
            self.editor.update(ctx, |editor, _| {
                editor.set_shell_family(session.shell().shell_type().into());
                editor.set_drag_drop_path_transformer(transformer);
            });
            self.input_suggestions.update(ctx, |input_suggestions, _| {
                input_suggestions.set_path_separators(session.path_separators());
            });
        }
        self.active_block_metadata = Some(active_block_metadata);

        // If needed, update the prompt display with the now-available session
        // context. In-band commands don't meaningfully change block metadata,
        // so only update prompt display chips if the previous block was not an
        // in-band command (i.e.: was probably a user-executed block).
        //
        // If we update the prompt display chips here, we can get into infinite
        // loops where we run an in-band command to compute an updated value for
        // a chip (e.g.: listing the files in the current directory), which
        // triggers another in-band command, etc. etc.
        if !is_after_in_band_command {
            self.update_prompt_display_chips(ctx);
        }
    }

    pub fn update_prompt_display_chips(&mut self, ctx: &mut ViewContext<Self>) {
        let session_context = self.completion_session_context(ctx);

        self.prompt_render_helper
            .prompt_view()
            .update(ctx, |prompt, prompt_ctx| {
                prompt.update_session_context(session_context.clone(), prompt_ctx);
            });
    }

    pub fn update_repo_path(&mut self, repo_path: Option<PathBuf>, ctx: &mut ViewContext<Self>) {
        self.prompt_render_helper
            .prompt_view()
            .update(ctx, |prompt, prompt_ctx| {
                prompt.update_repo_path(repo_path.clone(), prompt_ctx);
            });
    }

    fn render_input_box(
        &self,
        _show_vim_status: bool,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        // Set editor height to be half of the terminal view height
        let editor_height = self.size_info(app).pane_height_px() / 2.0.into_pixels();

        // Round down editor height to be divisible by line height so we do not see partial lines
        let line_height = self
            .editor
            .as_ref(app)
            .line_height(app.font_cache(), appearance)
            .into_pixels();
        let editor_height_rounded_down =
            (editor_height / line_height).round().max(1.0.into_pixels()) * line_height;

        let terminal_settings = TerminalSettings::as_ref(app);
        let terminal_spacing =
            terminal_settings.terminal_input_spacing(appearance.line_height_ratio(), app);
        // Always render with UDI-style spacing values, regardless of terminal
        // mode or prompt setting.
        let bottom_padding = terminal_spacing.editor_bottom_padding - 4.;

        let input_box = Container::new(
            ConstrainedBox::new(Clipped::new(ChildView::new(&self.editor).finish()).finish())
                .with_max_height(editor_height_rounded_down.as_f32())
                .finish(),
        )
        .with_padding_right(*TERMINAL_VIEW_PADDING_LEFT)
        .with_padding_bottom(bottom_padding)
        .finish();

        let input_editor_save_position_id = self.editor_save_position_id();
        SavePosition::new(
            EventHandler::new(input_box)
                .on_right_mouse_down(move |ctx, _, position| {
                    let input_rect = ctx
                        .element_position_by_id(input_editor_save_position_id.clone())
                        .expect("input editor position id should be saved");
                    let offset_position = position - input_rect.origin();
                    ctx.dispatch_typed_action(TerminalAction::OpenInputContextMenu {
                        position: offset_position,
                    });
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            &self.editor_save_position_id(),
        )
        .finish()
    }


    /// Returns the SavePosition ID for the input.
    ///
    /// This may be used by parent views to position UI elements relative to the input.
    pub fn save_position_id(&self) -> String {
        format!("input_{}", self.view_id)
    }

    /// Returns the position ID for the input editor
    pub fn editor_save_position_id(&self) -> String {
        format!("input_editor_{}", self.view_id)
    }

    /// Returns the position ID for the (left) prompt.
    pub fn prompt_save_position_id(&self) -> String {
        format!("prompt_area_{}", self.view_id)
    }

    /// A save position for the bordered input alone,
    /// not including the status bar.
    pub fn status_free_input_save_position_id(&self) -> String {
        format!("status_free_input_{}", self.view_id)
    }


    pub fn should_show_universal_developer_input(&self, app: &AppContext) -> bool {
        InputSettings::as_ref(app).is_universal_developer_input_enabled(app)
    }


}

impl Entity for Input {
    type Event = Event;
}

impl TypedActionView for Input {
    type Action = InputAction;

    fn action_accessibility_contents(
        &mut self,
        action: &InputAction,
        _: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        match action {
            InputAction::FocusInputBox => {
                ActionAccessibilityContent::Custom(AccessibilityContent::new(
                    INPUT_A11Y_LABEL,
                    // TODO (a11y) use bindings from user settings
                    INPUT_A11Y_HELPER,
                    RiftA11yRole::TextareaRole,
                ))
            }
            _ => ActionAccessibilityContent::Empty,
        }
    }

    fn handle_action(&mut self, action: &InputAction, ctx: &mut ViewContext<Self>) {
        match action {
            InputAction::FocusInputBox => self.focus_input_box(ctx),
            InputAction::Up => self.editor_up(ctx),
            InputAction::PageUp => self.editor_page_up(ctx),
            InputAction::PageDown => self.editor_page_down(ctx),
            InputAction::CtrlD => self.ctrl_d(ctx),
            InputAction::CtrlR => self.ctrl_r(ctx),
            InputAction::ClearScreen => self.clear_screen(ctx),
            InputAction::MaybeOpenCompletionSuggestions => {
                self.maybe_open_completion_suggestions(ctx);
            }
            InputAction::ToggleClassicCompletionsMode => {
                InputSettings::handle(ctx).update(ctx, |settings, ctx| {
                    if let Err(e) = settings.classic_completions_mode.toggle_and_save_value(ctx) {
                        log::warn!(
                            "Failed to toggle and save classic completions mode setting: {e}."
                        )
                    }
                });
            }
            InputAction::UpdateCompletionsMenuWidth(width) => {
                InputSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.completions_menu_width.set_value(*width, ctx));
                });
            }
            InputAction::UpdateCompletionsMenuHeight(height) => {
                InputSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.completions_menu_height.set_value(*height, ctx));
                });
            }
            InputAction::OpenInlineHistoryMenu => {}
        }
    }
}

impl View for Input {
    fn ui_name() -> &'static str {
        "Input"
    }

    fn accessibility_contents(&self, _: &AppContext) -> Option<AccessibilityContent> {
        Some(AccessibilityContent::new(
            INPUT_A11Y_LABEL,
            // TODO (a11y) use bindings from user settings
            INPUT_A11Y_HELPER,
            RiftA11yRole::TextareaRole,
        ))
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            if self.prompt_render_helper.has_open_chip_menu(ctx) {
                // Focus the PromptDisplay, which will in turn focus any open chip menu
                ctx.focus(self.prompt_render_helper.prompt_view());
            } else {
                ctx.focus(&self.editor);
                ctx.notify();
            }
            ctx.dispatch_typed_action(&PaneGroupAction::HandleFocusChange);
        }
    }

    fn keymap_context(&self, app: &AppContext) -> riftui::keymap::Context {
        let mut ctx = Self::default_keymap_context();

        if InputSettings::as_ref(app).is_universal_developer_input_enabled(app) {
            ctx.set.insert("UniversalDeveloperInput");
        }

        ctx.set.insert(flags::TERMINAL_MODE_INPUT);

        if self.buffer_text(app).is_empty() {
            ctx.set.insert(flags::EMPTY_INPUT_BUFFER);
        }

        if self.prompt_render_helper.has_open_chip_menu(app) {
            ctx.set.insert("PromptChipMenuOpen");
        }

        if AppEditorSettings::as_ref(app).vim_mode_enabled() {
            ctx.set.insert("VimModeEnabled");
        }

        if let Some(VimMode::Normal) = self.editor.as_ref(app).vim_mode(app) {
            ctx.set.insert("VimNormalMode");
        }

        let model_lock = self.model.lock();

        if model_lock
            .block_list()
            .active_block()
            .is_active_and_long_running()
        {
            ctx.set.insert("LongRunningCommand");
        }

        if model_lock.is_block_list_empty() {
            ctx.set.insert("TerminalView_EmptyBlockList");
        } else {
            ctx.set.insert("TerminalView_NonEmptyBlockList");
        }

        ctx
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        if self.should_show_universal_developer_input(app) {
            self.render_terminal_input(app)
        } else {
            self.render_classic_input(app)
        }
    }
}

impl Autosuggester for Input {
    fn on_autosuggestion_result(
        &mut self,
        result: AutoSuggestionResult,
        ctx: &mut ViewContext<Self>,
    ) {
        let buffer_text = result.buffer_text;
        if self.editor.as_ref(ctx).buffer_text(ctx) != buffer_text {
            return;
        }

        let autosuggestion_result_substring = result
            .autosuggestion_result
            .as_ref()
            .and_then(|result| result.strip_prefix(buffer_text.as_str()));

        if let Some(autosuggestion) = autosuggestion_result_substring {
            self.set_autosuggestion(
                autosuggestion,
                AutosuggestionType::Command {
                    was_intelligent_autosuggestion: false,
                },
                ctx,
            );
        }
    }

    fn abort_latest_autosuggestion_future(&mut self) {
        if let Some(last_abort_handle) = self.autosuggestions_abort_handle.take() {
            last_abort_handle.abort();
        }
    }

    fn set_autosuggestion_future(&mut self, abort_handle: AbortHandle) {
        self.autosuggestions_abort_handle = Some(abort_handle);
    }
}

#[cfg(feature = "integration_tests")]
impl Input {}

#[cfg(test)]
impl Input {

}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
