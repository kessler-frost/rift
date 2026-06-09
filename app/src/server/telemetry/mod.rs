mod collector;
pub mod context_provider;
mod events;
mod macros;
pub mod secret_redaction;

pub use collector::*;
pub use events::*;

/// Removes all telemetry events from the app telemetry event queue.
pub fn clear_event_queue() {
    let _ = riftui::telemetry::flush_events();
}
