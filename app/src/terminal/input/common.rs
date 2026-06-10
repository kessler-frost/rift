use std::sync::Arc;

use rift_completer::completer::Description;
use riftui::elements::{
    AnchorPair, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, Element, EventHandler, Flex, OffsetPositioning, OffsetType,
    ParentElement, ParentOffsetBounds, PositionedElementOffsetBounds, PositioningAxis, Radius,
    Shrinkable, Stack, XAxisAnchor,
};
use riftui::fonts::Weight;
use riftui::ui_components::components::{UiComponent, UiComponentStyles};
use riftui::AppContext;

use crate::appearance::Appearance;
use crate::terminal::input::{Input, InputAction, InputSuggestionsMode, MenuPositioning};
use crate::terminal::model::TerminalModel;
use crate::terminal::view::{TerminalAction, PADDING_LEFT};

/// Whether the terminal input message bar should be shown.
///
/// The message bar is hidden when AI is disabled, the user has turned it off in settings,
/// or the session is a shared ambient agent session.
pub(super) fn should_show_terminal_input_message_bar(
    model: &TerminalModel,
    app: &AppContext,
) -> bool {
    let _ = (model, app);
    false
}

/// Wraps the given column, assumed to represent the full input content, with appropriate
/// left padding to be consistent with the terminal content, as well as an event handler to
/// focus the input view when clicked.
pub(super) fn wrap_input_with_terminal_padding_and_focus_handler(
    is_active_session: bool,
    column: Box<dyn Element>,
    use_adjusted_padding: bool,
) -> Box<dyn Element> {
    let terminal_padding = if use_adjusted_padding {
        *PADDING_LEFT / 1.5
    } else {
        *PADDING_LEFT
    };

    if is_active_session {
        let mut flex_row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::End);

        flex_row.add_child(
            Shrinkable::new(
                1.,
                Container::new(column)
                    .with_padding_left(terminal_padding)
                    .finish(),
            )
            .finish(),
        );

        EventHandler::new(flex_row.finish())
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(TerminalAction::ClearSelectionsWhenShellMode);
                ctx.dispatch_typed_action(InputAction::FocusInputBox);
                DispatchEventResult::StopPropagation
            })
            .finish()
    } else {
        Container::new(column)
            .with_padding_left(terminal_padding)
            .finish()
    }
}


/// Renders the appropriate input suggestions overlay over the input, based on the current input
/// suggestions mode (if any).
pub(super) fn add_input_suggestions_overlays(
    input: &Input,
    stack: &mut Stack,
    appearance: &Appearance,
    menu_positioning: MenuPositioning,
    app: &AppContext,
) {
    match input.suggestions_mode_model().as_ref(app).mode() {
        InputSuggestionsMode::HistoryUp { .. } => {
            stack.add_positioned_overlay_child(
                input.render_history_up_menu(appearance, menu_positioning),
                OffsetPositioning::from_axes(
                    PositioningAxis::relative_to_parent(
                        ParentOffsetBounds::WindowByPosition,
                        OffsetType::Pixel(0.),
                        AnchorPair::new(XAxisAnchor::Left, XAxisAnchor::Left),
                    ),
                    PositioningAxis::relative_to_parent(
                        ParentOffsetBounds::Unbounded,
                        menu_positioning.history_y_offset(),
                        menu_positioning.history_y_anchor(),
                    ),
                ),
            );
        }
        InputSuggestionsMode::CompletionSuggestions { menu_position, .. } => {
            let relative_position_id = menu_position.to_position_id(input.editor.id());
            stack.add_positioned_overlay_child(
                input.render_completion_suggestions_menu(appearance, menu_positioning),
                OffsetPositioning::from_axes(
                    PositioningAxis::relative_to_stack_child(
                        &relative_position_id,
                        PositionedElementOffsetBounds::WindowByPosition,
                        OffsetType::Pixel(0.),
                        AnchorPair::new(XAxisAnchor::Left, XAxisAnchor::Left),
                    ),
                    PositioningAxis::relative_to_stack_child(
                        &relative_position_id,
                        PositionedElementOffsetBounds::Unbounded,
                        OffsetType::Pixel(0.),
                        menu_positioning.completion_suggestions_y_anchor(),
                    ),
                ),
            );
        }
        // SlashCommandsMenu is rendered separately via inline_slash_commands_menu_view
        // Conversation menu is rendered separately via inline_conversation_menu_view
        // Model selector is rendered separately via inline_model_selector_view
        // Profile selector is rendered separately via inline_profile_selector_view
        // Prompts menu is rendered separately via inline_prompts_menu_view
        // Skill menu is rendered separately via inline_skill_selector_view
        // User query menu is rendered separately via user_query_menu_view
        // Inline history menu is rendered separately via inline_history_menu_view
        // Repos menu is rendered separately via inline_repos_menu_view
        // Plan menu is rendered separately via inline_plan_menu_view
        InputSuggestionsMode::Closed => {}
    }
}

/// Renders the command xray overlay on the input using the command x ray-specific position id.
pub(super) fn add_command_xray_overlay(
    input: &Input,
    stack: &mut Stack,
    token_description: &Arc<Description>,
    appearance: &Appearance,
    menu_positioning: MenuPositioning,
    app: &AppContext,
) {
    let command_x_ray_position_id = format!("editor:command_x_ray_{}", input.editor.id());
    let line_height = input
        .editor
        .as_ref(app)
        .line_height(app.font_cache(), appearance);
    let offset = match menu_positioning {
        MenuPositioning::AboveInputBox => OffsetType::Pixel(0.),
        MenuPositioning::BelowInputBox => OffsetType::Pixel(line_height),
    };
    stack.add_positioned_overlay_child(
        render_command_token_description(token_description, appearance),
        OffsetPositioning::from_axes(
            PositioningAxis::relative_to_stack_child(
                &command_x_ray_position_id,
                PositionedElementOffsetBounds::ParentByPosition,
                OffsetType::Pixel(0.),
                AnchorPair::new(XAxisAnchor::Left, XAxisAnchor::Left),
            ),
            PositioningAxis::relative_to_stack_child(
                &command_x_ray_position_id,
                PositionedElementOffsetBounds::Unbounded,
                offset,
                menu_positioning.command_xray_y_anchor(),
            ),
        ),
    );
}

fn render_command_token_description(
    description: &Arc<Description>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    // Append an ellipsis to the description if the token has more characters than the max
    // number of characters that are allowed.
    const MAX_XRAY_LABEL_CHARS: usize = 16;
    const TOKEN_DESCRIPTION_PADDING: f32 = 12.;
    const TOKEN_DESCRIPTION_MARGIN: f32 = 10.;
    const TOKEN_DESCRIPTION_WIDTH: f32 = 240.;
    const TOKEN_LABEL_HORIZONTAL_PADDING: f32 = 8.;
    const TOKEN_LABEL_VERTICAL_PADDING: f32 = 4.;

    let truncated_label = match description
        .token
        .item
        .char_indices()
        .nth(MAX_XRAY_LABEL_CHARS)
    {
        None => description.token.item.clone(),
        Some((byte_index, _)) => format!("{}...", &description.token[..byte_index]),
    };

    let theme = appearance.theme();
    let ui_builder = appearance.ui_builder();

    let mut command_description = Flex::column().with_child(
        Flex::row()
            .with_child(
                Container::new(
                    ui_builder
                        .paragraph(truncated_label)
                        .with_style(UiComponentStyles {
                            font_family_id: Some(appearance.monospace_font_family()),
                            font_color: Some(theme.active_ui_text_color().into()),
                            font_size: Some(appearance.monospace_font_size()),
                            font_weight: Some(Weight::Bold),
                            ..Default::default()
                        })
                        .build()
                        .finish(),
                )
                .with_padding_top(2.)
                .finish(),
            )
            .with_child(
                Container::new(
                    ui_builder
                        .paragraph(description.suggestion_type.to_name().to_string())
                        .with_style(UiComponentStyles {
                            font_family_id: Some(appearance.ui_font_family()),
                            font_color: Some(theme.active_ui_text_color().into()),
                            font_size: Some(appearance.monospace_font_size() * 0.75),
                            ..Default::default()
                        })
                        .build()
                        .finish(),
                )
                .with_background(theme.outline())
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                .with_margin_left(TOKEN_DESCRIPTION_MARGIN)
                .with_padding_left(TOKEN_LABEL_HORIZONTAL_PADDING)
                .with_padding_right(TOKEN_LABEL_HORIZONTAL_PADDING)
                .with_padding_top(TOKEN_LABEL_VERTICAL_PADDING)
                .with_padding_bottom(TOKEN_LABEL_VERTICAL_PADDING)
                .finish(),
            )
            .finish(),
    );

    if let Some(description_text) = description.description_text.clone() {
        command_description.add_child(
            Container::new(
                ui_builder
                    .paragraph(description_text)
                    .with_style(UiComponentStyles {
                        font_family_id: Some(appearance.ui_font_family()),
                        font_color: Some(theme.sub_text_color(theme.surface_2()).into()),
                        font_size: Some(appearance.monospace_font_size() * 0.9),
                        ..Default::default()
                    })
                    .build()
                    .finish(),
            )
            .with_margin_top(TOKEN_DESCRIPTION_MARGIN)
            .finish(),
        );
    }

    ConstrainedBox::new(
        Container::new(command_description.finish())
            .with_uniform_padding(TOKEN_DESCRIPTION_PADDING)
            .with_margin_bottom(TOKEN_DESCRIPTION_MARGIN)
            .with_border(Border::all(1.).with_border_fill(theme.split_pane_border_color()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .with_background_color(theme.surface_2().into_solid())
            .finish(),
    )
    .with_width(TOKEN_DESCRIPTION_WIDTH)
    .finish()
}
