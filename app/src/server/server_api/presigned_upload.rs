use rift_core::errors::{register_error, ErrorExt};
use thiserror::Error;

/// Typed error for HTTP-backed operations so downstream classifiers can decide transient vs
/// permanent failures without string-parsing the anyhow Display.
///
/// Emitted as the source cause of a request failure; callers typically also attach a
/// human-facing context message via `.context(...)` so `err.to_string()` remains useful.
#[derive(Debug, Error)]
#[error("HTTP request failed with status {status}: {body}")]
pub struct HttpStatusError {
    pub status: u16,
    pub body: String,
}

impl ErrorExt for HttpStatusError {
    fn is_actionable(&self) -> bool {
        !matches!(self.status, 408 | 429)
    }
}
register_error!(HttpStatusError);
