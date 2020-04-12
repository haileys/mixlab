use std::fmt::{self, Debug};

use rustfft::{FFTplanner, FFT};
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;

use crate::engine::{Sample, ZERO_BUFFER_MONO};
use crate::module::{ModuleT, LineType, Terminal};

pub struct Fft {
    planner: FFTplanner<f64>,
    input_buffer: Vec<Complex<f64>>,
    output_buffer: Vec<Complex<f64>>,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Fft {
    type Params = ();
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Self {
            planner: FFTplanner::new(false),
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            inputs: vec![LineType::Mono.unlabeled()],
            outputs: vec![LineType::Mono.unlabeled()],
        }, ())
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let input = inputs[0].unwrap_or(&ZERO_BUFFER_MONO);

        // ensure buffers are the right size:
        self.input_buffer.resize(input.len(), Complex::zero());
        self.output_buffer.resize(input.len(), Complex::zero());

        // set up input buffer
        for (i, sample) in input.iter().enumerate() {
            self.input_buffer[i].re = *sample as f64;
            self.input_buffer[i].im = 0.0;
        }

        // do fft
        let fft = self.planner.plan_fft(input.len());
        fft.process(&mut self.input_buffer, &mut self.output_buffer);

        // normalise and write absolute value of fft result into output
        let normal_factor = (input.len() as f64).sqrt();

        let mut output = &mut outputs[0];
        for (i, bin) in self.output_buffer.iter().enumerate() {
            output[i] = bin.unscale(normal_factor).norm() as f32;
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

impl Debug for Fft {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Fft")
    }
}
