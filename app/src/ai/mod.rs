//! Cross-cutting AI functionality retained after the agent-product strip.
//!
//! Only inline command autocomplete remains (see [`predict`]). [`block_context`]
//! is the shared per-block context type the predict path consumes.

use riftui::AppContext;

pub(crate) mod block_context;
pub(crate) mod predict;

/// Retained entry point; the agent-product action registrations were removed.
pub fn init(_app: &mut AppContext) {}
