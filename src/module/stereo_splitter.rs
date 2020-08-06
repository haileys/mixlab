use crate::engine::{self, InputRef, OutputRef};
use crate::module::{ModuleT, LineType, Terminal};

#[derive(Debug)]
pub struct StereoSplitter {
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for StereoSplitter {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params, _: engine::ModuleCtx<Self>) -> (Self, Self::Indication) {
        (Self {
            inputs: vec![LineType::Stereo.unlabeled()],
            outputs: vec![
                LineType::Mono.labeled("L"),
                LineType::Mono.labeled("R")
            ],
        }, ())
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let input = inputs[0].expect_stereo();

        let (left, right) = match outputs {
            [left, right] => (left.expect_mono(), right.expect_mono()),
            _ => unreachable!(),
        };

        for i in 0..left.len() {
            left[i] = input[i * 2 + 0];
            right[i] = input[i * 2 + 1];
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
