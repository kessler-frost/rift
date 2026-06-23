use std::io;
use std::process::ExitStatus;
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use rift_core::channel::{Channel, ChannelState};
use serde::Serialize;

#[cfg(not(target_family = "wasm"))]
mod docker;
#[cfg(not(target_family = "wasm"))]
mod docker_sandbox;
#[cfg(not(target_family = "wasm"))]
mod kubernetes;
#[cfg(not(target_family = "wasm"))]
mod namespace;

/// Environment variable set by the server to identify the isolation platform.
/// The value should match one of the `IsolationPlatformType` variants in snake_case.
#[cfg(not(target_family = "wasm"))]
const RIFT_ISOLATION_PLATFORM_ENV: &str = "RIFT_ISOLATION_PLATFORM";

/// A kind of isolation platform. For our usage, isolation platforms are different ways where Rift
/// can be sandboxed, such as VMs, containers, or cloud hosts. This may also include weaker forms
/// of sandboxing such as Git worktrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationPlatformType {
    /// Rift is running within a Docker container. Note that this does *not* mean this is a Rift-hosted
    /// Docker Sandboxes environment. Instead, it's likely a self-hosted agent.
    #[cfg(not(target_family = "wasm"))]
    Docker,
    /// Rift is running within a Docker Sandbox, likely as a Rift-hosted agent.
    #[cfg(not(target_family = "wasm"))]
    DockerSandbox,
    /// Rift is running within a Kubernetes pod, likely as a self-hosted agent.
    #[cfg(not(target_family = "wasm"))]
    Kubernetes,
    /// Rift is running within a Namespace instance, likely as a Rift-hosted agent.
    #[cfg(not(target_family = "wasm"))]
    Namespace,
}

/// A workload identity token issued by the isolation platform.
#[derive(Debug, Clone)]
pub struct WorkloadToken {
    /// The token string.
    pub token: String,
    /// The expiration time of the token. On some platforms, workload tokens do not expire.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Detect the current isolation platform, if any.
///
/// Results are memoized for the lifetime of the process.
pub fn detect() -> Option<IsolationPlatformType> {
    static DETECTED_PLATFORM: OnceLock<Option<IsolationPlatformType>> = OnceLock::new();

    *DETECTED_PLATFORM.get_or_init(|| {
        // This never applies to integration tests.
        if ChannelState::channel() == Channel::Integration {
            return None;
        }

        // Use a closure so we can early-return.
        #[allow(clippy::redundant_closure_call)]
        let platform = (|| {
            // If the server explicitly told us which platform we're on, trust it.
            // This takes priority over all heuristic-based detection.
            #[cfg(not(target_family = "wasm"))]
            if let Some(platform) = platform_from_env() {
                return Some(platform);
            }

            #[cfg(not(target_family = "wasm"))]
            if namespace::is_in_namespace_instance() {
                return Some(IsolationPlatformType::Namespace);
            }

            #[cfg(not(target_family = "wasm"))]
            if kubernetes::is_in_kubernetes() {
                return Some(IsolationPlatformType::Kubernetes);
            }

            #[cfg(not(target_family = "wasm"))]
            if docker::is_in_docker() {
                return Some(IsolationPlatformType::Docker);
            }

            None
        })();

        match platform {
            Some(platform) => {
                log::debug!("Detected isolation platform: {:?}", platform);
            }
            None => {
                log::info!("No isolation platform detected");
            }
        }

        platform
    })
}

/// Parse the `RIFT_ISOLATION_PLATFORM` environment variable into a platform type.
#[cfg(not(target_family = "wasm"))]
fn platform_from_env() -> Option<IsolationPlatformType> {
    let value = std::env::var(RIFT_ISOLATION_PLATFORM_ENV).ok()?;
    match value.as_str() {
        "docker" => Some(IsolationPlatformType::Docker),
        "docker_sandbox" => Some(IsolationPlatformType::DockerSandbox),
        "kubernetes" => Some(IsolationPlatformType::Kubernetes),
        "namespace" => Some(IsolationPlatformType::Namespace),
        other => {
            log::warn!("Unknown {RIFT_ISOLATION_PLATFORM_ENV} value: {other}");
            None
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IsolationPlatformError {
    #[error("No isolation platform detected")]
    NoIsolationPlatformDetected,

    #[error("Workload token is missing or empty")]
    GenericWorkloadTokenMissing,

    #[error("Required command {command} is unavailable")]
    CommandUnavailable {
        command: String,
        #[source]
        source: io::Error,
    },

    #[error("Command `{command}` exited with non-zero status: {status}")]
    CommandFailed { command: String, status: ExitStatus },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
