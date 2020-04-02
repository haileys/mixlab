use crate::engine::{Sample, SAMPLES_PER_TICK, CHANNELS};
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
        for i in 0..SAMPLES_PER_TICK {
            for chan in 0..CHANNELS {
                let j = i * CHANNELS + chan;
                outputs[0][j] = inputs[0][j] + inputs[1][j];
            }
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
