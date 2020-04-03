use crate::engine::{Sample, ZERO_BUFFER, ONE_BUFFER};
use crate::module::Module;

use mixlab_protocol::AmplifierParams;

#[derive(Debug)]
pub struct Amplifier {
    params: AmplifierParams
}

impl Module for Amplifier {
    type Params = AmplifierParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Amplifier {params}, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        self.params = params;
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let AmplifierParams {mod_depth, amplitude} = self.params;
        let len = outputs[0].len();

        let input = &inputs[0].unwrap_or(&ZERO_BUFFER);
        let mod_input = &inputs[1].unwrap_or(&ONE_BUFFER);
        let output = &mut outputs[0];

        for i in 0..len {
            output[i] = input[i] * depth(mod_input[i], mod_depth) * amplitude;
        }

        None
    }

    fn input_count(&self) -> usize {
        2
    }

    fn output_count(&self) -> usize {
        1
    }
}

pub fn depth(value: f32, depth: f32) -> f32 {
    1.0 - depth + depth * value
}
