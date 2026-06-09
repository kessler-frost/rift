use riftui::{Entity, ModelContext, SingletonEntity};

#[derive(Clone)]
pub struct TeamTesterStatus {}

impl TeamTesterStatus {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {}
    }

    #[cfg(test)]
    pub fn mock(ctx: &mut ModelContext<Self>) -> Self {
        Self::new(ctx)
    }

    /// Emit an event to start the cloud object and workspace metadata pollers.
    /// Polling is started when a user logs in.
    pub fn initiate_data_pollers(&mut self, ctx: &mut ModelContext<Self>) {
        ctx.emit(TeamTesterStatusEvent::InitiateDataPollers)
    }
}

pub enum TeamTesterStatusEvent {
    InitiateDataPollers,
}

impl Entity for TeamTesterStatus {
    type Event = TeamTesterStatusEvent;
}

impl SingletonEntity for TeamTesterStatus {}
