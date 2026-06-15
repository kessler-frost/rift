use std::borrow::Cow;
use std::sync::Arc;

use channel_versions::overrides::TargetOS;
use parking_lot::RwLock;
use rift_core::semantic_selection::SemanticSelection;
use rift_core::ui::theme::RiftTheme;
use riftui::elements::{
    Border, Container, CrossAxisAlignment, Flex, Icon, MainAxisAlignment, MainAxisSize,
    ParentElement, SelectableArea, SelectionHandle, Text,
};
use riftui::{AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext};

use super::render::HORIZONTAL_TEXT_MARGIN;
use super::settings::RiftifySettings;
use super::{render, subshell_bootstrap_success_block_bytes};
use crate::appearance::Appearance;
use crate::terminal::model::terminal_model::SubshellInitializationInfo;
use crate::terminal::shell::Shell;
use crate::ui_components::blended_colors;
use crate::ui_components::icons::Icon as UiIcon;

const VERTICAL_TEXT_MARGIN: f32 = 16.;

#[derive(Debug, Clone)]
pub enum RiftifySuccessBlockEvent {
    OpenRiftifySettings,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RiftifySuccessBlockAction {
    ClearAutoRiftifySnippet,
    OpenRiftifySettings,
}

struct AutoRiftifySnippet {
    /// On subshell initialization, this will contain the output grid to display,
    /// containing info like how to auto-riftify the subshell.
    output_grid: Cow<'static, str>,
    /// The output grid needs to be selectable to allow users to copy the command to their clipboard.
    selection_handle: SelectionHandle,
    selected_text: Arc<RwLock<Option<String>>>,

    description: Cow<'static, str>,
}

pub struct RiftifySuccessBlock {
    spawning_command: String,
    auto_riftify_snippet: Option<AutoRiftifySnippet>,
}

impl RiftifySuccessBlock {
    #[allow(clippy::new_without_default)]
    pub fn new(
        spawning_command: String,
        subshell_info: Option<SubshellInitializationInfo>,
        shell: Shell,
        disable_tmux: bool,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&RiftifySettings::handle(ctx), move |_, _, _, ctx| {
            ctx.notify();
        });

        // Mac + Linux have the same behavior. We'd need to handle
        // getting the OS to write to the correct RC file.
        let remote_os = TargetOS::Linux;

        let is_auto_riftify_configured = subshell_info
            .as_ref()
            .map(|info| info.was_triggered_by_rc_file_snippet)
            .unwrap_or_default();

        let auto_riftify_snippet = if is_auto_riftify_configured {
            None
        } else {
            subshell_info.and_then(|subshell_info| {
                // If riftification wasn't triggered automatically, show a snippet about
                // how to automatically riftify.
                (!subshell_info.was_triggered_by_rc_file_snippet).then(|| {
                    let (command, is_executable) = subshell_bootstrap_success_block_bytes(
                        &subshell_info,
                        shell.shell_type(),
                        remote_os,
                        disable_tmux,
                    );
                    if command.is_empty() {
                        return ("".into(), false);
                    }
                    (
                        String::from_utf8(command)
                            .map(|content| {
                                // Ensure a blank line between the output grid and the learn more link.
                                content + "\n"
                            })
                            .unwrap_or_default(),
                        is_executable,
                    )
                })
            })
        };
        let auto_riftify_snippet = auto_riftify_snippet.map(|(output_grid, _can_write_to_rc)| {
            AutoRiftifySnippet {
                description: (if !output_grid.is_empty() {
                    "Run the following to automatically Riftify in the future:"
                } else {
                    "In remote subshells, Rift runs commands in the background to power completions, syntax highlighting, and other features."
                }).into(),
                output_grid: output_grid.into(),
                selection_handle: Default::default(),
                selected_text: Default::default(),
            }
        });

        Self {
            spawning_command,
            auto_riftify_snippet,
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        self.auto_riftify_snippet
            .as_ref()
            .and_then(|snippet| snippet.selected_text.read().clone())
    }

    pub fn render_spawning_command(
        &self,
        theme: &RiftTheme,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let spawning_command = self.spawning_command.clone();
        render::build_command_row(spawning_command, theme, appearance, true)
            .with_margin_bottom(VERTICAL_TEXT_MARGIN)
            .finish()
    }

    pub fn render_title_ui(&self, theme: &RiftTheme, appearance: &Appearance) -> Box<dyn Element> {
        let header_contents = render::build_header_row(
            "Session Riftified",
            Icon::new(UiIcon::Rift.into(), theme.active_ui_detail()),
            theme,
            appearance,
        )
        .with_margin_right(8.)
        .finish();
        let header_contents = Container::new(
            Flex::row()
                .with_children([header_contents])
                .finish(),
        )
        .finish();

        Container::new(
            Flex::row()
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_cross_axis_alignment(CrossAxisAlignment::End)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(header_contents)
                .finish(),
        )
        .with_horizontal_margin(HORIZONTAL_TEXT_MARGIN)
        .with_margin_top(VERTICAL_TEXT_MARGIN)
        .finish()
    }

    /// Fired when a block ends and we are not in a Riftified session.
    pub fn on_riftified_session_complete(&mut self, ctx: &mut ViewContext<Self>) {
        self.clear_auto_riftify_snippet(ctx);
    }

    pub fn clear_auto_riftify_snippet(&mut self, ctx: &mut ViewContext<Self>) {
        self.auto_riftify_snippet = None;
        ctx.notify();
    }

    /// If there is an output grid to display, render it.
    pub fn render_output_grid(
        &self,
        app: &AppContext,
        appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        let theme = appearance.theme();
        let auto_riftify_snippet = self.auto_riftify_snippet.as_ref()?;

        if auto_riftify_snippet.output_grid.is_empty() {
            return None;
        }

        let runnable_command = Container::new(
            Text::new(
                auto_riftify_snippet.output_grid.to_string(),
                appearance.monospace_font_family(),
                appearance.monospace_font_size() - 1.,
            )
            .with_color(blended_colors::text_main(theme, theme.background()))
            .finish(),
        )
        .with_uniform_padding(8.)
        .with_background(theme.surface_1())
        .finish();

        let semantic_selection = SemanticSelection::as_ref(app);
        let selected_text = auto_riftify_snippet.selected_text.clone();

        // TODO(Simon): Implement full selection and copying functionality for the RiftifySuccessBlock.
        // Look to the `EnvVarCollectionBlock` for the existing implementation paradigm. We don't
        // yet have a robust way of ensuring that every aspect of text selection is implemented
        // properly, so be extra careful not to miss any details!
        let output_grid = SelectableArea::new(
            auto_riftify_snippet.selection_handle.clone(),
            move |selection_args, _, _| {
                *selected_text.write() = selection_args.selection;
            },
            runnable_command,
        )
        .with_word_boundaries_policy(semantic_selection.word_boundary_policy())
        .with_smart_select_fn(semantic_selection.smart_select_fn())
        .finish();

        let output_grid = Flex::column()
            .with_child(
                Container::new(
                    Text::new(
                        auto_riftify_snippet.description.clone(),
                        appearance.monospace_font_family(),
                        appearance.monospace_font_size(),
                    )
                    .with_color(blended_colors::text_main(theme, theme.background()))
                    .finish(),
                )
                .with_horizontal_margin(HORIZONTAL_TEXT_MARGIN)
                .with_margin_bottom(VERTICAL_TEXT_MARGIN)
                .finish(),
            )
            .with_child(
                Container::new(output_grid)
                    .with_horizontal_margin(HORIZONTAL_TEXT_MARGIN)
                    .with_margin_bottom(VERTICAL_TEXT_MARGIN)
                    .finish(),
            )
            .finish();
        Some(output_grid)
    }
}

impl Entity for RiftifySuccessBlock {
    type Event = RiftifySuccessBlockEvent;
}

pub const RIFTIFY_SUCCESS_BLOCK_VISIBLE_KEY: &str = "RiftifySuccessBlockVisible";

impl View for RiftifySuccessBlock {
    fn ui_name() -> &'static str {
        "RiftifySuccessBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut content = Flex::column();

        content.add_children([
            self.render_title_ui(theme, appearance),
            self.render_spawning_command(theme, appearance),
        ]);

        if let Some(output_grid) = self.render_output_grid(app, appearance) {
            content.add_child(output_grid);
        }

        Container::new(content.finish())
            .with_background(theme.foreground().with_opacity(10))
            .with_border(Border::top(1.).with_border_fill(theme.outline()))
            .finish()
    }
}

impl TypedActionView for RiftifySuccessBlock {
    type Action = RiftifySuccessBlockAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            RiftifySuccessBlockAction::OpenRiftifySettings => {
                ctx.emit(RiftifySuccessBlockEvent::OpenRiftifySettings);
            }
            RiftifySuccessBlockAction::ClearAutoRiftifySnippet => {
                self.clear_auto_riftify_snippet(ctx);
            }
        }
    }
}
