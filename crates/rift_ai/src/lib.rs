//! Local AI for Rift: command completion and natural-language → command,
//! served by a local omlx instance via the Anthropic Messages API.

pub mod config;
pub mod context;
pub mod messages;
pub mod client;
pub mod complete;
pub mod translate;

pub use config::RiftAiConfig;
// TODO(Task 4): pub use context::{CommandContext, ContextMessageInput, RiftContext};
