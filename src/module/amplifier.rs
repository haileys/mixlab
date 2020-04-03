use crate::engine::Sample;
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

    fn run_tick(&mut self, _t: u64, inputs: &[&[Sample]], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len();

        let input = &inputs[0];
        let mod_input = &inputs[1];
        let output = &mut outputs[0];

        for i in 0..len {
            output[i] = input[i] * mod_input[i] * self.params.amplitude;
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