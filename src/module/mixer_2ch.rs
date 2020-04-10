use mixlab_protocol::{LineType, Terminal};

use crate::engine::{Sample, ZERO_BUFFER_STEREO};
use crate::module::ModuleT;

#[derive(Debug)]
pub struct Mixer2ch {
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Mixer2ch {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        (Self {
            inputs: vec![
                LineType::Stereo.labeled("1"),
                LineType::Stereo.labeled("2"),
            ],
            outputs: vec![
                LineType::Stereo.labeled("Master"),
            ],
        }, ())
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len();

        let input0 = &inputs[0].unwrap_or(&ZERO_BUFFER_STEREO);
        let input1 = &inputs[1].unwrap_or(&ZERO_BUFFER_STEREO);

        for i in 0..len {
            outputs[0][i] = input0[i] + input1[i];
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
