use std::f64;

use mixlab_protocol::{SineGeneratorParams, LineType, Terminal};

use crate::engine::{Sample, SAMPLE_RATE};
use crate::module::ModuleT;

#[derive(Debug)]
pub struct SineGenerator {
    params: SineGeneratorParams,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for SineGenerator {
    type Params = SineGeneratorParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Self {
            params,
            inputs: vec![],
            outputs: vec![LineType::Mono.unlabeled()],
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
        let len = outputs[0].len();
        let co = self.params.freq as f64 * 2.0 * f64::consts::PI;

        for i in 0..len {
            let t = (t + i as u64) as f64 / SAMPLE_RATE as f64;
            let x = f64::sin(co * t);
            outputs[0][i] = x as f32;
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
