use crate::engine::{Sample, ZERO_BUFFER_STEREO};
use crate::module::{Module, LineType};

#[derive(Debug)]
pub struct StereoSplitter;

impl Module for StereoSplitter {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        (StereoSplitter, ())
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let input = &inputs[0].unwrap_or(&ZERO_BUFFER_STEREO);

        if let [left, right] = outputs {
            for i in 0..left.len() {
                left[i] = input[i * 2 + 0];
                right[i] = input[i * 2 + 1];
            }
        } else {
            unreachable!();
        }

        None
    }

    fn inputs(&self) -> &[LineType] {
        &[LineType::Stereo]
    }

    fn outputs(&self) -> &[LineType] {
        &[LineType::Mono, LineType::Mono]
    }
}
