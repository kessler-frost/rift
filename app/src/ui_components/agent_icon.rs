//! Source-facing helpers that centralize the derivation of the agent-icon shape
//! ([`IconWithStatusVariant`]) from the underlying state models. The invariant the
//! helpers enforce: any single logical agent run renders as the same brand color, glyph,
//! and ambient-vs-local treatment regardless of which surface is rendering it (vertical
//! tabs, pane header, conversation list, notifications mailbox).
//!
//! Each helper is a thin adapter over one data source. Surfaces call the helper for
//! whichever source they hold and feed the resulting variant into
//! [`render_icon_with_status`]. The pure inner functions in this module are exercised
//! directly by the cross-surface consistency tests in `agent_icon_tests.rs`.
use rift_cli::agent::Harness;
use riftui::{AppContext, SingletonEntity};

use crate::terminal::cli_agent_sessions::CLIAgentSessionsModel;
use crate::terminal::view::TerminalView;
use crate::terminal::CLIAgent;
use crate::ui_components::icon_with_status::IconWithStatusVariant;

/// Returns the agent-icon variant for a live [`TerminalView`], or `None` when the terminal is
/// not an agent surface (plain terminal / shell / empty conversation).
///
/// Resolution order:
/// 1. A [`CLIAgentSessionsModel`] session with a known agent wins. Plugin-backed sessions
///    surface rich status; command-detected sessions don't.
/// 2. A task-backed run uses task status and harness so the terminal chrome and the
///    matching conversation list card stay in lockstep.
/// 3. Live ambient pre-dispatch or a selected local conversation falls through to the
///    no-task waterfall.
/// 4. Everything else returns `None` so the caller renders a plain-terminal indicator.
pub(crate) fn terminal_view_agent_icon_variant(
    _terminal_view: &TerminalView,
    _app: &AppContext,
) -> Option<IconWithStatusVariant> {
    // Agent/conversation status icons were removed; terminals use the plain terminal icon.
    None
}


#[cfg(test)]
#[path = "agent_icon_tests.rs"]
mod tests;
