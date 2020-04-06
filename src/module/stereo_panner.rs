use crate::engine::{Sample, ZERO_BUFFER_MONO};
use crate::module::{ModuleT, LineType, Terminal};

#[derive(Debug)]
pub struct StereoPanner {
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for StereoPanner {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        (Self {
            inputs: vec![LineType::Mono.labeled("L"), LineType::Mono.labeled("R")],
            outputs: vec![LineType::Stereo.unlabeled()],
        }, ())
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let left = &inputs[0].unwrap_or(&ZERO_BUFFER_MONO);
        let right = &inputs[1].unwrap_or(&ZERO_BUFFER_MONO);
        let output = &mut outputs[0];

        for i in 0..left.len() {
            output[i * 2 + 0] = left[i];
            output[i * 2 + 1] = right[i];
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
