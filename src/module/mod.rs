pub mod amplifier;
pub mod envelope;
pub mod fm_sine;
pub mod mixer_2ch;
pub mod mixer_4ch;
pub mod output_device;
pub mod sine_generator;
pub mod stereo_panner;
pub mod stereo_splitter;
pub mod trigger;

use mixlab_protocol::LineType;

use crate::engine::Sample;

pub trait Module: Sized {
    type Params;
    type Indication;

    fn create(params: Self::Params) -> (Self, Self::Indication);
    fn params(&self) -> Self::Params;
    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication>;
    fn run_tick(&mut self, t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication>;
    fn inputs(&self) -> &[LineType];
    fn outputs(&self) -> &[LineType];
}
