use std::f64;

use mixlab_protocol::EqThreeParams;

use crate::engine::{self, InputRef, OutputRef, SAMPLE_RATE};
use crate::module::{ModuleT, LineType, Terminal};

const FREQ_LO: f64 = 420.0;
const FREQ_HI: f64 = 2700.0;

const VSA: f64 = 1.0 / 4294967295.0; // Very small amount (Denormal Fix)

#[derive(Debug)]
pub struct EqThree {
    params: EqThreeParams,

    // filter 1 (low band)
    lo: LowPass,
    hi: LowPass,

    // sample history
    history: [f64; 3],

    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for EqThree {
    type Params = EqThreeParams;
    type Indication = ();

    fn create(params: Self::Params, _: engine::ModuleCtx<Self>) -> (Self, Self::Indication) {
        let lo = LowPass::new(FREQ_LO);
        let hi = LowPass::new(FREQ_HI);

        let eq_three = Self {
            params,
            lo,
            hi,
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

    fn run_tick(&mut self, _t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let input = inputs[0].expect_mono();
        let output = outputs[0].expect_mono();

        let gain_lo = self.params.gain_lo.to_linear();
        let gain_mid = self.params.gain_mid.to_linear();
        let gain_hi = self.params.gain_hi.to_linear();

        for (input, output) in input.iter().copied().zip(output.iter_mut()) {
            let sample = input as f64;

            let lo = self.lo.pump(sample);
            let hi = self.history[0] - self.hi.pump(sample);

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

#[derive(Debug)]
struct LowPass {
    freq: f64,
    poles: [f64; 4],
}

impl LowPass {
    pub fn new(freq: f64) -> Self {
        let mut filter = LowPass { freq: 0.0, poles: [0.0, 0.0, 0.0, 0.0] };
        filter.set_freq(freq);
        filter
    }

    pub fn set_freq(&mut self, freq: f64) {
        self.freq = 2.0 * f64::sin(f64::consts::PI * freq / (SAMPLE_RATE as f64));
    }

    pub fn pump(&mut self, sample: f64) -> f64 {
        self.poles[0] += self.freq * (sample - self.poles[0]) + VSA;
        self.poles[1] += self.freq * (self.poles[0] - self.poles[1]);
        self.poles[2] += self.freq * (self.poles[1] - self.poles[2]);
        self.poles[3] += self.freq * (self.poles[2] - self.poles[3]);

        self.poles[3]
    }
}

#[cfg(test)]
mod tests {
    use crate::module::{ModuleT, InputRef, OutputRef};
    use mixlab_protocol::{Decibel, EqThreeParams};
    use super::EqThree;

    fn bytes_to_f32s(bytes: &[u8]) -> Vec<f32> {
        bytes.chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect::<Vec<_>>()
    }

    #[allow(unused)]
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

        eq.run_tick(0, &[InputRef::Mono(&input)], &mut [OutputRef::Mono(&mut output)]);

        let expected_output = bytes_to_f32s(include_bytes!("../../fixtures/module/eq_three/chronos-eq.f32.raw"));

        assert!(output == expected_output);
    }
}
