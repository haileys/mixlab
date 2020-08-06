use crate::engine::{self, InputRef, OutputRef};
use crate::module::ModuleT;

use mixlab_protocol::{PlotterIndication, LineType, Terminal};

#[derive(Debug)]
pub struct Plotter {
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
    count: usize,
}

impl ModuleT for Plotter {
    type Params = ();
    type Indication = PlotterIndication;

    fn create(_: Self::Params, _: engine::ModuleLink<Self>) -> (Self, Self::Indication) {
        (
            Self {
                inputs: vec![LineType::Stereo.unlabeled()],
                outputs: vec![],
                count: 0,
            },
            Self::Indication { inputs: vec![vec![], vec![]] }
        )
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _: u64, inputs: &[InputRef], _: &mut [OutputRef]) -> Option<Self::Indication> {
        self.count += 1;

        if self.count % 6 == 0 && inputs[0].connected() {
            let input = inputs[0].expect_stereo();

            let samples = input.len() / 2;
            let mut left = Vec::with_capacity(samples);
            let mut right = Vec::with_capacity(samples);

            for i in 0..samples {
                left.push(input[i * 2]);
                right.push(input[i * 2 + 1]);
            }

            Some(PlotterIndication { inputs: vec![left, right] })
        } else {
            None
        }
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}
