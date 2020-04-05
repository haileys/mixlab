use crate::engine::Sample;
use crate::module::Module;

use mixlab_protocol::{PlotterIndication, LineType};

#[derive(Debug)]
pub struct Plotter;

impl Module for Plotter {
    type Params = ();
    type Indication = PlotterIndication;

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        (Plotter, PlotterIndication { inputs: Vec::new() })
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, t: u64, inputs: &[Option<&[Sample]>], _outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        if t % 100 == 0 {
            let inputs: Vec<_> = inputs.iter().map(|input| {
                input.map(|x|x.to_vec())
            }).collect();

            Some(PlotterIndication { inputs })
        } else {
            None
        }
    }

    fn inputs(&self) -> &[LineType] {
        &[LineType::Mono, LineType::Mono]
    }

    fn outputs(&self) -> &[LineType] {
        &[]
    }
}
