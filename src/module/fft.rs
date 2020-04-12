use crate::engine::{Sample, ZERO_BUFFER_STEREO, ONE_BUFFER_MONO};
use crate::module::{ModuleT, LineType, Terminal};

#[derive(Debug)]
pub struct Fft {,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Amplifier {
    type Params = AmplifierParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Self {
            params,
            inputs: vec![
                LineType::Stereo.labeled("Input"),
                LineType::Mono.labeled("Control")
            ],
            outputs: vec![LineType::Stereo.unlabeled()]
        }, ())
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

        let input = &inputs[0].unwrap_or(&ZERO_BUFFER_STEREO);
        let mod_input = &inputs[1].unwrap_or(&ONE_BUFFER_MONO);
        let output = &mut outputs[0];

        let len = input.len();

        for i in 0..len {
            // mod input is a mono channel and so half the length:
            let mod_value = mod_input[i / 2] as f64;

            output[i] = (input[i] as f64 * depth(mod_value, mod_depth) * amplitude) as Sample;
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

pub fn depth(value: f64, depth: f64) -> f64 {
    1.0 - depth + depth * value
}
