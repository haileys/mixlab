use crate::engine::Sample;
use crate::module::ModuleT;

use mixlab_protocol::{PlotterIndication, LineType, Terminal};

#[derive(Debug)]
pub struct Plotter {
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Plotter {
    type Params = ();
    type Indication = PlotterIndication;

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        (
            Self {
                inputs: vec![LineType::Stereo.unlabeled()],
                outputs: vec![],
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

    fn run_tick(&mut self, t: u64, inputs: &[Option<&[Sample]>], _outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        if t % 10 == 1 {
            inputs[0].map(|input| {
                let samples = input.len() / 2;
                let mut left = Vec::with_capacity(samples);
                let mut right = Vec::with_capacity(samples);

                for i in 0..samples {
                    left.push(input[i * 2]);
                    right.push(input[i * 2 + 1]);
                }

                PlotterIndication { inputs: vec![left, right] }
            })
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
