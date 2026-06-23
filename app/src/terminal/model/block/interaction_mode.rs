//! Terminal block interaction mode. Blocks are always in plain terminal
//! mode; this trivial marker is retained for the block's `interaction_mode`
//! field.

/// Interaction mode for a terminal block.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InteractionMode;

impl InteractionMode {
    /// Blocks are never hidden due to interaction mode.
    pub fn should_hide_block(&self) -> bool {
        false
    }
}
