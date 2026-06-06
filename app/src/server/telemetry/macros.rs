//! Telemetry is disabled in Rift (local-only fork). These macros are no-ops:
//! the event/context arguments are intentionally discarded (and therefore not
//! type-checked), so call sites referencing removed cloud types compile away.
//!
//! NOTE: these `#[macro_export]` macros are still used by KEEP code. When the
//! `server/` subsystem is deleted, relocate these definitions to `rift_core`.

#[macro_export]
macro_rules! send_telemetry_sync_from_ctx {
    ($event:expr, $ctx:expr) => {{}};
}

#[macro_export]
macro_rules! send_telemetry_sync_from_app_ctx {
    ($event:expr, $app_ctx:expr) => {{}};
}

#[macro_export]
macro_rules! send_telemetry_on_executor {
    ($auth_state:expr, $event:expr, $executor:expr) => {{}};
}
