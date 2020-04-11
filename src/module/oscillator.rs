use std::f64;

use mixlab_protocol::{OscillatorParams, Waveform, LineType, Terminal};

use crate::engine::{Sample, SAMPLE_RATE};
use crate::module::ModuleT;

#[derive(Debug)]
pub struct Oscillator {
    params: OscillatorParams,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Oscillator {
    type Params = OscillatorParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Self {
            params,
            inputs: vec![],
            outputs: vec![
                LineType::Mono.labeled("Mono"),
                LineType::Stereo.labeled("Stereo"),
            ],
        }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, t: u64, _inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        const MONO: usize = 0;
        const STEREO: usize = 1;

        let len = outputs[MONO].len();
        let co = self.params.freq as f64 * 2.0 * f64::consts::PI;

        for i in 0..len {
            let t0 = (t + i as u64) as f64 / SAMPLE_RATE as f64;
            let sample: f32 = match &self.params.waveform {
                Waveform::Sine => f64::sin(co * t0),
                Waveform::On => 1.0,
                Waveform::Off => 0.0,
            } as f32;

            outputs[MONO][i] = sample;
            outputs[STEREO][i * 2 + 0] = sample;
            outputs[STEREO][i * 2 + 1] = sample;
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
