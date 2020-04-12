use std::fmt::{self, Debug};

use fftw::plan::{Plan, R2CPlan, C2RPlan, Plan64};
use fftw::types::{c64, Flag};

use mixlab_protocol::EqThreeParams;

use crate::engine::{Sample, SAMPLE_RATE, ZERO_BUFFER_MONO};
use crate::module::{ModuleT, LineType, Terminal};

const BUFFER_SIZE: usize = 1024;
const FREQ_LO: f64 = 250.0;
const FREQ_HI: f64 = 2500.0;

pub struct EqThree {
    params: EqThreeParams,
    fft: Plan<f64, c64, Plan64>,
    ifft: Plan<c64, f64, Plan64>,
    input_buffer: Vec<f64>,
    eq_buffer: Vec<c64>,
    output_buffer: Vec<f64>,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for EqThree {
    type Params = EqThreeParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        let mut input_buffer = vec![0.0; BUFFER_SIZE];
        let mut eq_buffer = vec![c64 { re: 0.0, im: 0.0 }; BUFFER_SIZE];
        let mut output_buffer = vec![0.0; BUFFER_SIZE];

        let fft = R2CPlan::new(
                &[BUFFER_SIZE],
                &mut input_buffer,
                &mut eq_buffer,
                Flag::Estimate | Flag::DestroyInput,
            ).expect("R2CPlan::new");

        let ifft = C2RPlan::new(
                &[BUFFER_SIZE],
                &mut eq_buffer,
                &mut output_buffer,
                Flag::Estimate | Flag::DestroyInput,
            ).expect("C2RPlan::new");

        let eq_three = Self {
            params,
            fft,
            ifft,
            input_buffer,
            eq_buffer,
            output_buffer,
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

        // shift input buffer down a bit. it would be nice to avoid this copy
        self.input_buffer.drain(0..input.len());
        self.input_buffer.extend(input.iter().map(|sample| *sample as f64));

        // do fft
        self.fft.r2c(&mut self.input_buffer, &mut self.eq_buffer)
            .expect("R2CPlan::r2c");

        for (i, value) in self.eq_buffer.iter_mut().enumerate() {
            let i = if i * 2 > BUFFER_SIZE {
                BUFFER_SIZE - i
            } else {
                i
            };

            let freq_per_bin = (SAMPLE_RATE as f64 / BUFFER_SIZE as f64);
            let bin_lo_freq = i as f64 * freq_per_bin;
            let bin_hi_freq = (i + 1) as f64 * freq_per_bin;

            if bin_lo_freq >= FREQ_HI {
                *value = value.scale(self.params.gain_hi.to_linear());
            }
        }

        // do inverse fft
        self.ifft.c2r(&mut self.eq_buffer, &mut self.output_buffer)
            .expect("C2RPlan::c2r");

        // FFT results must be normalised by dividing by the square root of the
        // sample count. because we're doing two transforms, we skip the square
        // root:
        let normal_factor = BUFFER_SIZE as f64;

        let output_values = &self.output_buffer[(self.output_buffer.len() - input.len())..];
        let output = &mut outputs[0];
        for (i, value) in output_values.iter().enumerate() {
            output[i] = (*value / normal_factor) as f32;
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

impl Debug for EqThree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EqThree {{ params: {:?} }}", self.params)
    }
}
