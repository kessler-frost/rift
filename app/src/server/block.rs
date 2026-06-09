//! Minimal local block-display type.
//!
//! The cloud "share block" client (`BlockClient`) and its GraphQL conversions have been removed for
//! the offline build. Only the small `DisplaySetting` enum is retained here, since it is pure local
//! data used by the terminal block model and telemetry payloads (it describes how much of a block —
//! command, output, or both — to display).

/// Describes which parts of a block to display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplaySetting {
    Command,
    Output,
    CommandAndOutput,
    Other(String),
}
