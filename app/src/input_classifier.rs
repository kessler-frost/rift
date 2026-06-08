use riftui::{Entity, ModelContext, SingletonEntity};

pub struct InputClassifierModel {}

impl InputClassifierModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {}
    }
}

impl Entity for InputClassifierModel {
    type Event = ();
}

impl SingletonEntity for InputClassifierModel {}
