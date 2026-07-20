mod anyhow;
mod registration;
mod reqwest;
#[cfg(not(target_family = "wasm"))]
mod tokio;
#[cfg(not(target_family = "wasm"))]
mod websocket;

// Re-export for macro use.
#[doc(hidden)]
pub use inventory::submit;
pub use registration::{ErrorRegistration, RegisteredError, register_error};

pub use self::anyhow::AnyhowErrorExt;

/// The `target` that is set by log entries from this module.
pub const LOG_TARGET: &str = "errors::report_error";

/// Controls how often a [`report_error!`] invocation logs errors.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReportErrorLogMode {
    /// Log every time the error is reported.
    #[default]
    EveryTime,
    /// Log only the first time this macro invocation is reached during the
    /// current app run.
    OncePerRun,
}

/// Reports an error encountered during execution.
///
/// This checks whether or not the error is actionable, and logs an error or
/// warning accordingly.  (Logs at the Error level get reported back to us, so
/// we don't want to log anything at Error level that we aren't able to act
/// upon.)
#[macro_export]
macro_rules! report_error {
    (@log $err:expr_2021) => {{
        #[allow(unused_imports)]
        use $crate::errors::{AnyhowErrorExt as _, ErrorExt as _, LOG_TARGET};
        let err = $err;
        let log_level = if err.is_actionable() {
            log::Level::Error
        } else {
            log::Level::Warn
        };
        log::log!(target: LOG_TARGET, log_level, "{:#}", err);
    }};
    (@once_per_run $err:expr_2021) => {{
        static HAS_LOGGED_REPORT_ERROR: ::std::sync::atomic::AtomicBool =
            ::std::sync::atomic::AtomicBool::new(false);
        if !HAS_LOGGED_REPORT_ERROR.swap(true, ::std::sync::atomic::Ordering::Relaxed) {
            $crate::report_error!(@log $err);
        }
    }};
    ($err:expr_2021) => {{
        $crate::report_error!(@log $err);
    }};
    ($err:expr_2021, $crate::errors::ReportErrorLogMode::EveryTime) => {{
        $crate::report_error!(@log $err);
    }};
    ($err:expr_2021, ReportErrorLogMode::EveryTime) => {{
        $crate::report_error!(@log $err);
    }};
    ($err:expr_2021, $crate::errors::ReportErrorLogMode::OncePerRun) => {{
        $crate::report_error!(@once_per_run $err);
    }};
    ($err:expr_2021, ReportErrorLogMode::OncePerRun) => {{
        $crate::report_error!(@once_per_run $err);
    }};
    ($err:expr_2021, $log_mode:expr_2021) => {{
        match $log_mode {
            $crate::errors::ReportErrorLogMode::EveryTime => {
                $crate::report_error!(@log $err);
            }
            $crate::errors::ReportErrorLogMode::OncePerRun => {
                $crate::report_error!(@once_per_run $err);
            }
        }
    }};
}
pub use report_error;

/// Reports an error if the provided [`Result`] is [`Err`].
///
/// This checks whether or not the error is actionable, and logs an error or
/// warning accordingly.  (Logs at the Error level get reported back to us, so
/// we don't want to log anything at Error level that we aren't able to act
/// upon.)
#[macro_export]
macro_rules! report_if_error {
    ($result:expr_2021) => {{
        if let Err(error) = &$result {
            $crate::report_error!(error);
        }
    }};
    ($result:expr_2021, $log_mode:expr_2021) => {{
        if let Err(error) = &$result {
            $crate::report_error!(error, $log_mode);
        }
    }};
}
pub use report_if_error;

pub trait ErrorExt: RegisteredError + std::error::Error {
    /// Returns whether or not an error is something that is actionable by our
    /// engineering team.
    fn is_actionable(&self) -> bool;
}

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
