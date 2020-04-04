use std::f32;

use mixlab_protocol::{SineGeneratorParams, LineType};

use crate::engine::{Sample, SAMPLE_RATE};
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

    fn run_tick(&mut self, t: u64, _inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len();
        let co = self.params.freq as f32 * 2.0 * f32::consts::PI;

        for i in 0..len {
            let t = (t + i as u64) as Sample / SAMPLE_RATE as Sample;
            let x = Sample::sin(co * t);
            outputs[0][i] = x;
        }

        None
    }

    fn inputs(&self) -> &[LineType] {
        &[]
    }

    fn outputs(&self)-> &[LineType] {
        &[LineType::Mono]
    }
}
