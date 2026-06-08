use rift_core::send_telemetry_from_ctx;
use riftui::UpdateView;

use super::{
    mark_feature_used_and_write_to_user_defaults, ContextMenuState, MenuItem, TerminalAction, TerminalView, Tip, TipHint, ViewContext, CONTEXT_MENU_WIDTH,
};

impl TerminalView {
    pub(super) fn show_context_menu(
        &mut self,
        menu_state: ContextMenuState,
        items: Vec<MenuItem<TerminalAction>>,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.update_view(&self.context_menu, |context_menu, view_ctx| {
            context_menu.set_origin(menu_state.menu_type.origin());
            context_menu.set_width(CONTEXT_MENU_WIDTH);
            // This will also reset the selection.
            context_menu.set_items(items, view_ctx);
        });

        self.context_menu_state = Some(menu_state);
        ctx.focus(&self.context_menu);
        ctx.notify();

        send_telemetry_from_ctx!(
            TelemetryEvent::OpenContextMenu {
                context_menu_info: ContextMenuInfo {
                    menu_type: menu_state.menu_type,
                }
            },
            ctx
        );
        self.tips_completed.update(ctx, |tips, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Hint(TipHint::BlockAction),
                tips,
                ctx,
            );
            ctx.notify();
        });
    }
}
