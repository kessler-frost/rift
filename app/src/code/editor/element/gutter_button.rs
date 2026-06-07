use rift_core::ui::appearance::Appearance;
use rift_core::ui::color::contrast::MinimumAllowedContrast;
use rift_core::ui::color::ContrastingColor;
use rift_core::ui::theme::color::internal_colors;
use rift_core::ui::theme::Fill;
use rift_core::ui::Icon;
use riftui::elements::MouseState;

use crate::view_components::action_button::{
    ActionButtonTheme, DisabledSecondaryTheme, SecondaryTheme,
};

/// A button rendered within the gutter of the editor.
pub(super) trait GutterButton {
    /// The icon color for the gutter.
    fn icon_color(&self, mouse_state: &MouseState, appearance: &Appearance) -> Fill {
        let button_background = self.background_color(mouse_state, appearance);

        let is_hovered = mouse_state.is_hovered();
        let color = if self.is_enabled() {
            SecondaryTheme.text_color(is_hovered, Some(button_background), appearance)
        } else {
            DisabledSecondaryTheme.text_color(is_hovered, Some(button_background), appearance)
        };

        let contrast_shifted_color = color.on_background(
            button_background.into_solid(),
            MinimumAllowedContrast::NonText,
        );
        contrast_shifted_color.into()
    }

    /// The background color of the button.
    fn background_color(&self, mouse_state: &MouseState, appearance: &Appearance) -> Fill {
        if self.is_enabled() {
            if mouse_state.is_hovered() {
                Fill::Solid(internal_colors::neutral_3(appearance.theme()))
            } else {
                Fill::Solid(internal_colors::neutral_1(appearance.theme()))
            }
        } else {
            Fill::Solid(internal_colors::neutral_1(appearance.theme()))
        }
    }

    /// Whether the button is currently enabled. If false, the button is rendered in a disabled
    /// state.
    fn is_enabled(&self) -> bool;

    /// The tooltip text displayed when the button is hovered.
    fn tooltip_text(&self) -> Option<&'static str>;

    /// The icon of the button.
    fn icon(&self) -> Icon;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RevertHunkButton {
    is_enabled: bool,
}

impl RevertHunkButton {
    pub fn new(is_enabled: bool) -> Self {
        Self { is_enabled }
    }
}

impl GutterButton for RevertHunkButton {
    fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    fn tooltip_text(&self) -> Option<&'static str> {
        if self.is_enabled {
            Some("Revert diff hunk")
        } else {
            Some("Save changes to revert")
        }
    }

    fn icon(&self) -> Icon {
        Icon::ReverseLeft
    }
}
