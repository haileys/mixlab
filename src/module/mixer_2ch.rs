use mixlab_protocol::LineType;

use crate::engine::{Sample, ZERO_BUFFER_STEREO};
use crate::module::ModuleT;

#[derive(Debug)]
pub struct Mixer2ch;

impl ModuleT for Mixer2ch {
    type Params = ();
    type Indication = ();

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        (Mixer2ch, ())
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len();

        let input0 = &inputs[0].unwrap_or(&ZERO_BUFFER_STEREO);
        let input1 = &inputs[1].unwrap_or(&ZERO_BUFFER_STEREO);

        for i in 0..len {
            outputs[0][i] = input0[i] + input1[i];
        }

        None
    }

    fn inputs(&self) -> &[LineType] {
        &[LineType::Stereo, LineType::Stereo]
    }

    fn outputs(&self) -> &[LineType] {
        &[LineType::Stereo]
    }
}
