use mixlab_protocol::{ClockParams, LineType};

use crate::engine::Sample;
use crate::module::ModuleT;

#[derive(Debug)]
pub struct Clock {
    params: ClockParams,
}

impl ModuleT for Clock {
    type Params = ClockParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Clock { params }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, _t: u64, _inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        None
    }

    fn inputs(&self) -> &[LineType] {
        &[]
    }

    fn outputs(&self) -> &[LineType] {
        &[LineType::Mono]
    }
}
