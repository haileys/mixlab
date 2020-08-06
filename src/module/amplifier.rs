use crate::engine::{self, Sample, InputRef, OutputRef};
use crate::module::{ModuleT, LineType, Terminal};

use mixlab_protocol::AmplifierParams;

#[derive(Debug)]
pub struct Amplifier {
    params: AmplifierParams,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Amplifier {
    type Params = AmplifierParams;
    type Indication = ();

    fn create(params: Self::Params, _: engine::ModuleLink<Self>) -> (Self, Self::Indication) {
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

    fn run_tick(&mut self, _t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let AmplifierParams {mod_depth, amplitude} = self.params;

        let input = inputs[0].expect_stereo();
        let mod_input = if inputs[1].connected() {
            Some(inputs[1].expect_mono())
        } else {
            None
        };

        let output = outputs[0].expect_stereo();

        let len = input.len();

        for i in 0..len {
            // mod input is a mono channel and so half the length:
            let mod_value = mod_input.map(|buff| buff[i / 2] as f64).unwrap_or(1.0);

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
