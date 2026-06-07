use rift_core::ui::appearance::Appearance;
use riftui::elements::Icon;
use riftui::Element;

use crate::search::result_renderer::ItemHighlightState;

/// Assumes the path is a file, not a folder
pub fn icon_from_file_path(
    _path: &str,
    appearance: &Appearance,
    highlight_state: ItemHighlightState,
) -> Box<dyn Element> {
    Icon::new(
        "bundled/svg/completion-file.svg",
        highlight_state.icon_fill(appearance).into_solid(),
    )
    .finish()
}
