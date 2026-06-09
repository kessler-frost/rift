//! Keyboard-shortcut rendering helper recovered inline from a deleted
//! agent-view shortcuts module. Used by message-bar and
//! slash-command rows to render a keystroke chip with optional color overrides.

use pathfinder_color::ColorU;
use riftui::keymap::Keystroke;
use riftui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use riftui::{AppContext, Element, SingletonEntity};

use crate::appearance::Appearance;
use crate::ui_components::blended_colors;

pub fn render_keystroke_with_color_overrides(
    keystroke: &Keystroke,
    color: Option<ColorU>,
    background_color: Option<ColorU>,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let font_size = appearance.monospace_font_size() - 2.;
    let keystroke_size = font_size + 2.;
    appearance
        .ui_builder()
        .keyboard_shortcut(keystroke)
        .lowercase_modifier()
        .with_space_between_keys(2.)
        .with_style(UiComponentStyles {
            margin: Some(Coords::default()),
            padding: Some(Coords::default()),
            border_width: Some(1.),
            background: Some(
                background_color
                    .unwrap_or_else(|| blended_colors::neutral_3(theme))
                    .into(),
            ),
            font_color: Some(color.unwrap_or_else(|| theme.foreground().into_solid())),
            font_family_id: Some(appearance.ui_font_family()),
            font_size: Some(font_size),
            width: Some(keystroke_size),
            height: Some(keystroke_size),
            ..Default::default()
        })
        .with_line_height_ratio(1.0)
        .build()
        .finish()
}
