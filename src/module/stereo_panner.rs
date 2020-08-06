use crate::engine::{self, InputRef, OutputRef};
use crate::module::{ModuleT, LineType, Terminal};

#[derive(Debug)]
pub struct StereoPanner {
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for StereoPanner {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params, _: engine::ModuleCtx<Self>) -> (Self, Self::Indication) {
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

    fn run_tick(&mut self, _t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let left = inputs[0].expect_mono();
        let right = inputs[1].expect_mono();
        let output = outputs[0].expect_stereo();

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
