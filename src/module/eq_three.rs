use std::f64;
use std::fmt::{self, Debug};

use fftw::plan::{Plan, R2CPlan, C2RPlan, Plan64};
use fftw::types::{c64, Flag};

use mixlab_protocol::EqThreeParams;

use crate::engine::{Sample, SAMPLE_RATE, ZERO_BUFFER_MONO};
use crate::module::{ModuleT, LineType, Terminal};

const BUFFER_SIZE: usize = 1024;
const FREQ_LO: f64 = 250.0;
const FREQ_HI: f64 = 2500.0;

const VSA: f64 = 1.0 / 4294967295.0; // Very small amount (Denormal Fix)

#[derive(Debug)]
pub struct EqThree {
    params: EqThreeParams,

    // filter 1 (low band)
    lo_poles: [f64; 4],

    // filter 2 (high band)
    hi_poles: [f64; 4],

    // sample history
    history: [f64; 3],

    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for EqThree {
    type Params = EqThreeParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        let eq_three = Self {
            params,
            lo_poles: [0.0; 4],
            hi_poles: [0.0; 4],
            history: [0.0; 3],
            inputs: vec![LineType::Mono.unlabeled()],
            outputs: vec![LineType::Mono.unlabeled()],
        };

        (eq_three, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        self.params = params;
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let input = inputs[0].unwrap_or(&ZERO_BUFFER_MONO);
        let output = &mut outputs[0];

        let freq_lo = 2.0 * f64::sin(f64::consts::PI * FREQ_LO / (SAMPLE_RATE as f64));
        let freq_hi = 2.0 * f64::sin(f64::consts::PI * FREQ_HI / (SAMPLE_RATE as f64));

        let gain_lo = self.params.gain_lo.to_linear();
        let gain_mid = self.params.gain_mid.to_linear();
        let gain_hi = self.params.gain_hi.to_linear();

        for (input, output) in input.iter().copied().zip(output.iter_mut()) {
            let sample = input as f64;

            // lo pass:

            self.lo_poles[0] += freq_lo * (sample - self.lo_poles[0]) + VSA;
            self.lo_poles[1] += freq_lo * (self.lo_poles[0] - self.lo_poles[1]);
            self.lo_poles[2] += freq_lo * (self.lo_poles[1] - self.lo_poles[2]);
            self.lo_poles[3] += freq_lo * (self.lo_poles[2] - self.lo_poles[3]);

            let lo = self.lo_poles[3];

            // hi pass

            self.hi_poles[0] += freq_hi * (sample - self.hi_poles[0]) + VSA;
            self.hi_poles[1] += freq_hi * (self.hi_poles[0] - self.hi_poles[1]);
            self.hi_poles[2] += freq_hi * (self.hi_poles[1] - self.hi_poles[2]);
            self.hi_poles[3] += freq_hi * (self.hi_poles[2] - self.hi_poles[3]);

            let hi = self.history[0] - self.hi_poles[3];

            // mid range

            let mid = self.history[0] - (hi + lo);

            // shift history
            self.history[0] = self.history[1];
            self.history[1] = self.history[2];
            self.history[2] = sample;

            // apply gain

            let lo = lo * gain_lo;
            let mid = mid * gain_mid;
            let hi = hi * gain_hi;

            *output = (lo + mid + hi) as f32;
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

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::fs::File;

    use crate::module::ModuleT;
    use mixlab_protocol::{Decibel, EqThreeParams};
    use super::EqThree;

    fn bytes_to_f32s(bytes: &[u8]) -> Vec<f32> {
        bytes.chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect::<Vec<_>>()
    }

    fn f32s_to_bytes(floats: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::new();

        for float in floats {
            bytes.extend(&float.to_le_bytes());
        }

        bytes
    }

    #[test]
    fn basic_smoke_test() {
        let input = bytes_to_f32s(include_bytes!("../../fixtures/module/eq_three/chronos.f32.raw"));

        let (mut eq, _) = EqThree::create(EqThreeParams {
            gain_lo: Decibel(4.0),
            gain_mid: Decibel(0.0),
            gain_hi: Decibel(4.0),
        });

        let mut output = vec![0.0; input.len()];

        eq.run_tick(0, &[Some(&input)], &mut [&mut output]);

        let expected_output = bytes_to_f32s(include_bytes!("../../fixtures/module/eq_three/chronos-eq.f32.raw"));

        assert!(output == expected_output);
    }
}
