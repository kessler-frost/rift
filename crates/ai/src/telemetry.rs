use std::time::Duration;

use rift_core::features::FeatureFlag;
use rift_core::register_telemetry_event;
use rift_core::telemetry::{EnablementState, TelemetryEvent, TelemetryEventDesc};
use serde::Serialize;
use serde_json::{json, Value};
use strum_macros::{EnumDiscriminants, EnumIter};

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
#[derive(Clone, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter))]
pub enum AITelemetryEvent {
    SyncCodebaseContextSuccess {
        total_sync_duration: Duration,
        flushed_node_count: usize,
        flushed_fragment_count: usize,
        total_fragment_size_bytes: usize,
        sync_type: CodebaseContextSyncType,
        cache_population_error: Option<String>,
    },
    SyncCodebaseContextFailed {
        error: String,
        sync_type: CodebaseContextSyncType,
    },
    BuildTreeFailed {
        error: String,
    },
}

#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
#[derive(Clone, Serialize)]
pub enum CodebaseContextSyncType {
    Incremental,
}

impl TelemetryEvent for AITelemetryEvent {
    fn name(&self) -> &'static str {
        AITelemetryEventDiscriminants::from(self).name()
    }

    fn description(&self) -> &'static str {
        AITelemetryEventDiscriminants::from(self).description()
    }

    fn enablement_state(&self) -> EnablementState {
        AITelemetryEventDiscriminants::from(self).enablement_state()
    }

    fn payload(&self) -> Option<Value> {
        match self {
            Self::SyncCodebaseContextSuccess {
                total_sync_duration,
                sync_type,
                flushed_node_count,
                flushed_fragment_count,
                total_fragment_size_bytes,
                cache_population_error,
            } => Some(json!({
                "total_sync_duration": total_sync_duration,
                "sync_type": sync_type,
                "flushed_node_count": flushed_node_count,
                "flushed_fragment_count": flushed_fragment_count,
                "total_fragment_size_bytes": total_fragment_size_bytes,
                "cache_population_error": cache_population_error
            })),
            Self::SyncCodebaseContextFailed { error, sync_type } => Some(json!({
                "error": error,
                "sync_type": sync_type
            })),
            Self::BuildTreeFailed { error } => Some(json!({
                "error": error
            })),
        }
    }

    fn contains_ugc(&self) -> bool {
        match self {
            Self::SyncCodebaseContextFailed { .. }
            | Self::SyncCodebaseContextSuccess { .. }
            | Self::BuildTreeFailed { .. } => false,
        }
    }

    fn event_descs() -> impl Iterator<Item = Box<dyn TelemetryEventDesc>> {
        rift_core::telemetry::enum_events::<Self>()
    }
}

impl TelemetryEventDesc for AITelemetryEventDiscriminants {
    fn name(&self) -> &'static str {
        match self {
            Self::SyncCodebaseContextSuccess => "AgentMode.SyncCodebaseContext.Success",
            Self::SyncCodebaseContextFailed => "AgentMode.SyncCodebaseContext.Failed",
            Self::BuildTreeFailed => "AgentMode.SyncCodebaseContext.BuildTree.Failed",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::SyncCodebaseContextSuccess => "Successfully synced codebase context",
            Self::SyncCodebaseContextFailed => "Failed to sync codebase context",
            Self::BuildTreeFailed => "Failed to build merkle tree for codebase context",
        }
    }

    fn enablement_state(&self) -> EnablementState {
        match self {
            Self::SyncCodebaseContextFailed
            | Self::SyncCodebaseContextSuccess
            | Self::BuildTreeFailed => EnablementState::Flag(FeatureFlag::FullSourceCodeEmbedding),
        }
    }
}

register_telemetry_event!(AITelemetryEvent);
