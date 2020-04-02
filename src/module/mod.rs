pub mod output_device;

use crate::engine::Sample;

pub trait Module: Sized {
    type Params;
    type Indication;

    fn create(params: Self::Params) -> (Self, Self::Indication);
    fn params(&self) -> Self::Params;
    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication>;
    fn run_tick(&mut self, t: u64, inputs: &[&[Sample]], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication>;
    fn input_count(&self) -> usize;
    fn output_count(&self) -> usize;
}
