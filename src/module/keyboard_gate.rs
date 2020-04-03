use mixlab_protocol::{Gate, KeyboardGateParams};

use crate::engine::{Sample, CHANNELS};
use crate::module::Module;

#[derive(Debug)]
pub struct KeyboardGate {
    params: KeyboardGateParams,
}

impl Module for KeyboardGate {
    type Params = KeyboardGateParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (KeyboardGate { params }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, _t: u64, _inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len() / CHANNELS;

        let value = match self.params.gate {
            Gate::Open => 1.0,
            Gate::Closed => 0.0,
        };

        for i in 0..len {
            outputs[0][i] = value;
        }

        None
    }

    fn input_count(&self) -> usize {
        0
    }

    fn output_count(&self) -> usize {
        1
    }
}
