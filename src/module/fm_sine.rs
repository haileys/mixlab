use std::f32;

use mixlab_protocol::FmSineParams;

use crate::engine::{Sample, SAMPLE_RATE, CHANNELS};
use crate::module::Module;

#[derive(Debug)]
pub struct FmSine {
    params: FmSineParams,
}

impl Module for FmSine {
    type Params = FmSineParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (FmSine { params }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, t: u64, inputs: &[&[Sample]], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len() / CHANNELS;

        let freq_amp = (self.params.freq_hi - self.params.freq_lo) / 2.0;
        let freq_mid = self.params.freq_lo + freq_amp;

        for i in 0..len {
            let co = (freq_mid + freq_amp * inputs[0][i * 2]) * 2.0 * f32::consts::PI;
            let t = (t + i as u64) as Sample / SAMPLE_RATE as Sample;
            let x = Sample::sin(co * t);

            for chan in 0..CHANNELS {
                outputs[0][i * CHANNELS + chan] = x;
            }
        }

        None
    }

    fn input_count(&self) -> usize {
        1
    }

    fn output_count(&self) -> usize {
        1
    }
}
