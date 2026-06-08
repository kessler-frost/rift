//! Support for pane contents that are shareable.
//!
//! The cloud-backed sharing subsystem (Warp Drive objects, shared sessions, the
//! sharing dialog) has been removed. This module retains no-op shims so the pane
//! header keeps compiling without any sharing UI.

use rift_core::ui::appearance::Appearance;
use rift_core::ui::theme::Fill;
use riftui::elements::ParentElement;
use riftui::{AppContext, ViewContext};

use super::PaneHeader;
use crate::pane_group::BackingView;
use crate::server::telemetry::SharingDialogSource;

impl<P: BackingView> PaneHeader<P> {
    pub fn has_shareable_object<C: riftui::ViewAsRef>(&self, _ctx: &C) -> bool {
        false
    }

    pub fn has_shareable_shared_session<C: riftui::ViewAsRef>(&self, _ctx: &C) -> bool {
        false
    }

    pub fn is_sharing_dialog_enabled<C: riftui::ViewAsRef>(&self, _ctx: &C) -> bool {
        false
    }

    pub fn share_pane_contents(
        &mut self,
        _source: SharingDialogSource,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    pub fn open_shared_session_qr_code(
        &mut self,
        _source: SharingDialogSource,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    /// Render controls for sharing the pane contents. Sharing has been removed,
    /// so this is a no-op.
    pub fn render_sharing_controls(
        &self,
        _element: &mut impl ParentElement,
        _appearance: &Appearance,
        _icon_color_override: Option<Fill>,
        _button_size_override: Option<f32>,
        _app: &AppContext,
    ) {
    }
}
