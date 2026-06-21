use riftui::{Entity, ModelContext, SingletonEntity};

pub enum TeamUpdateManagerEvent {}

/// Offline stub for the former server-polling team update manager.
///
/// Rift is fully offline: there is no server to poll for workspace/team metadata. All network
/// polling, retry strategies, and team-client calls have been removed.
pub struct TeamUpdateManager {}

impl TeamUpdateManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {}
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn mock(ctx: &mut ModelContext<Self>) -> Self {
        Self::new(ctx)
    }
}

impl Entity for TeamUpdateManager {
    type Event = TeamUpdateManagerEvent;
}

impl SingletonEntity for TeamUpdateManager {}

#[cfg(test)]
#[path = "update_manager_tests.rs"]
mod tests;
