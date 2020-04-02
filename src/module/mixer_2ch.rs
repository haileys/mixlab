use crate::engine::Sample;
use crate::module::Module;

#[derive(Debug)]
pub struct Mixer2ch;

impl Module for Mixer2ch {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        (Mixer2ch, ())
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[&[Sample]], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len();

        for i in 0..len {
            outputs[0][i] = inputs[0][i] + inputs[1][i];
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
