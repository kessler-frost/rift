//! Minimal shared-session status/source types retained after the shared-session
//! subsystem was removed. Sharing is no longer functional; these types exist so
//! the terminal model and its consumers keep compiling. `shared_session_status()`
//! now always reports [`SharedSessionStatus::NotShared`].
use session_sharing_protocol::common::Role;
use session_sharing_protocol::sharer::SessionSourceType;

/// `SessionSourceType` paired with the orchestrator `task_id` that rides
/// on the `source_task_id` sidecar.
#[derive(Debug, Clone)]
pub struct SharedSessionSource {
    pub source_type: SessionSourceType,
    pub source_task_id: Option<String>,
}

impl SharedSessionSource {
    pub fn user(source_task_id: Option<String>) -> Self {
        Self {
            source_type: SessionSourceType::User,
            source_task_id,
        }
    }

    pub fn ambient_agent(task_id: Option<String>) -> Self {
        Self {
            source_type: SessionSourceType::AmbientAgent {
                task_id: task_id.clone(),
            },
            source_task_id: task_id,
        }
    }

    /// Sidecar first, then `AmbientAgent.task_id` for legacy producers.
    pub fn orchestrator_task_id(&self) -> Option<&str> {
        self.source_task_id.as_deref().or(match &self.source_type {
            SessionSourceType::AmbientAgent { task_id } => task_id.as_deref(),
            SessionSourceType::User => None,
        })
    }
}

impl Default for SharedSessionSource {
    fn default() -> Self {
        Self::user(None)
    }
}

/// The type of shared session a particular session is, if applicable.
#[derive(Debug, Clone, Default)]
pub enum SharedSessionStatus {
    /// This session is not a shared session.
    #[default]
    NotShared,
    /// We're in the process of joining the session.
    ViewPending,
    /// This session is a shared session that we are actively viewing.
    ActiveViewer { role: Role },
    /// We were viewing a shared session but it ended.
    FinishedViewer,
    /// We haven't yet attempted to share the session because it is not bootstrapped yet.
    SharePendingPreBootstrap { source: SharedSessionSource },
    /// The session is bootstrapped and we're in the process of sharing the session.
    SharePending,
    /// This session is actively being shared.
    ActiveSharer,
}

impl SharedSessionStatus {
    pub fn reader() -> Self {
        Self::ActiveViewer { role: Role::Reader }
    }

    pub fn executor() -> Self {
        Self::ActiveViewer {
            role: Role::Executor,
        }
    }

    pub fn is_view_pending(&self) -> bool {
        matches!(self, SharedSessionStatus::ViewPending)
    }

    pub fn is_active_viewer(&self) -> bool {
        matches!(self, SharedSessionStatus::ActiveViewer { .. })
    }

    pub fn is_finished_viewer(&self) -> bool {
        matches!(self, SharedSessionStatus::FinishedViewer)
    }

    pub fn is_viewer(&self) -> bool {
        self.is_view_pending() || self.is_active_viewer() || self.is_finished_viewer()
    }

    pub fn is_executor(&self) -> bool {
        matches!(self, SharedSessionStatus::ActiveViewer { role } if role.can_execute())
    }

    pub fn is_reader(&self) -> bool {
        matches!(
            self,
            SharedSessionStatus::ActiveViewer { role: Role::Reader }
        )
    }

    pub fn is_share_pending(&self) -> bool {
        matches!(
            self,
            SharedSessionStatus::SharePending | SharedSessionStatus::SharePendingPreBootstrap { .. }
        )
    }

    pub fn is_active_sharer(&self) -> bool {
        matches!(self, SharedSessionStatus::ActiveSharer)
    }

    pub fn is_sharer(&self) -> bool {
        self.is_share_pending() || self.is_active_sharer()
    }

    pub fn is_sharer_or_viewer(&self) -> bool {
        !matches!(self, Self::NotShared)
    }

    pub fn as_keymap_context(&self) -> &'static str {
        match self {
            Self::NotShared => "SharedSessionStatus_NotShared",
            Self::ViewPending => "SharedSessionStatus_ViewPending",
            Self::ActiveViewer { role: Role::Reader } => "SharedSessionStatus_Reader",
            Self::ActiveViewer {
                role: Role::Executor | Role::Full,
            } => "SharedSessionStatus_Executor",
            Self::FinishedViewer => "SharedSessionStatus_FinishedViewer",
            Self::SharePendingPreBootstrap { .. } => "SharedSessionStatus_SharePendingPreBootstrap",
            Self::SharePending => "SharedSessionStatus_SharePending",
            Self::ActiveSharer => "SharedSessionStatus_ActiveSharer",
        }
    }
}
