use rift_core::ui::icons::Icon as RiftIcon;
use rift_core::ui::theme::color::internal_colors;
use rift_core::ui::theme::{Fill as RiftThemeFill, RiftTheme};
use riftui::elements::{ConstrainedBox, Container, CornerRadius, Element, Radius};

// The glyph occupies `NEUTRAL_GLYPH_RATIO * total_size`, matching the old sizing where
// a 24px container held a 16px glyph (16/24 ≈ 0.667).
const NEUTRAL_GLYPH_RATIO: f32 = 16.0 / 24.0;

/// Renders a circular icon sized entirely from a single `total_size`: a full-`total_size`
/// container with the glyph at `NEUTRAL_GLYPH_RATIO * total_size` on an overlay background.
pub(crate) fn render_icon_with_status(
    icon: RiftIcon,
    icon_color: RiftThemeFill,
    total_size: f32,
    theme: &RiftTheme,
) -> Box<dyn Element> {
    let glyph = total_size * NEUTRAL_GLYPH_RATIO;
    let padding = (total_size - glyph) / 2.;
    let inner = ConstrainedBox::new(icon.to_riftui_icon(icon_color).finish())
        .with_width(glyph)
        .with_height(glyph)
        .finish();
    Container::new(inner)
        .with_uniform_padding(padding)
        .with_background(internal_colors::fg_overlay_2(theme))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(total_size / 2.)))
        .finish()
}
