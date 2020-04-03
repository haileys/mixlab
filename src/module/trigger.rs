use mixlab_protocol::{GateState, LineType};

use crate::engine::Sample;
use crate::module::Module;

#[derive(Debug)]
pub struct Trigger {
    params: GateState,
}

impl Module for Trigger {
    type Params = GateState;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Trigger { params }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, _t: u64, _inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len();

        let value = match self.params {
            GateState::Open => 1.0,
            GateState::Closed => 0.0,
        };

        for i in 0..len {
            outputs[0][i] = value;
        }

        None
    }

    fn inputs(&self) -> &[LineType] {
        &[]
    }

    fn outputs(&self) -> &[LineType] {
        &[LineType::Stereo]
    }
}
