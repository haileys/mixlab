use mixlab_protocol::{GateState, LineType, Terminal};

use crate::engine::{self, InputRef, OutputRef};
use crate::module::ModuleT;

#[derive(Debug)]
pub struct Trigger {
    params: GateState,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Trigger {
    type Params = GateState;
    type Indication = ();

    fn create(params: Self::Params, _: engine::ModuleLink<Self>) -> (Self, Self::Indication) {
        (Self {
            params,
            inputs: vec![],
            outputs: vec![LineType::Mono.unlabeled()]
        }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, _t: u64, _: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let output = outputs[0].expect_mono();

        let value = match self.params {
            GateState::Open => 1.0,
            GateState::Closed => 0.0,
        };

        for out in output.iter_mut() {
            *out = value;
        }

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}
