//! Terminal block interaction mode.
//!
//! The agent interaction modes (agent tag-in, agent control of long-running
//! commands, subagent handoff, etc.) were removed with the AI agent product.
//! Blocks are now always in plain terminal mode, so this is a trivial marker
//! retained for the block's `interaction_mode` field.


/// Interaction mode for a terminal block. Retained after the agent product was
/// removed; a terminal block has no agent interaction state.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InteractionMode;

impl InteractionMode {
    /// Blocks are never hidden due to interaction mode (no agent monitoring).
    pub fn should_hide_block(&self) -> bool {
        false
    }
}

