use std::f32;

use mixlab_protocol::SineGeneratorParams;

use crate::engine::{Sample, SAMPLES_PER_TICK, SAMPLE_RATE, CHANNELS};
use crate::module::Module;

#[derive(Debug)]
pub struct SineGenerator {
    params: SineGeneratorParams,
}

impl Module for SineGenerator {
    type Params = SineGeneratorParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (SineGenerator { params }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, t: u64, _inputs: &[&[Sample]], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let co = self.params.freq as f32 * 2.0 * f32::consts::PI;
        let t = t as Sample * SAMPLES_PER_TICK as Sample;

        for i in 0..SAMPLES_PER_TICK {
            let t = (t + i as Sample) / SAMPLE_RATE as Sample;
            let x = Sample::sin(co * t);

            for chan in 0..CHANNELS {
                outputs[0][i * CHANNELS + chan] = x;
            }
        }

        None
    }

    fn input_count(&self) -> usize {
        0
    }

    fn output_count(&self) -> usize {
        1
    }
}
