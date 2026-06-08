//! Tests for the agent-icon helpers.
//!
//! The cross-surface equivalence suite that previously lived here exercised the
//! AI/cloud agent-run, conversation, and CLI-session-status machinery. That layer
//! has been removed: the only surviving helper is
//! [`super::terminal_view_agent_icon_variant`], which now always returns `None`
//! because agent/conversation status icons no longer exist. The obsolete tests
//! were deleted along with the symbols they referenced.
