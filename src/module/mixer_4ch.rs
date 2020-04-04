use mixlab_protocol::{Mixer4chParams, LineType};

use crate::engine::Sample;
use crate::module::Module;

#[derive(Debug)]
pub struct Mixer4ch {
    params: Mixer4chParams,
}

impl Module for Mixer4ch {
    type Params = Mixer4chParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Mixer4ch { params }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        self.params = params;
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let len = outputs[0].len();

        let mut channel_gain: [f32; 4] = [0.0; 4];

        for ch in 0..4 {
            let channel = &self.params.channels[ch];
            channel_gain[ch] = channel.fader * channel.gain.to_linear();
        }

        for i in 0..len {
            outputs[0][i] = 0.0;
            outputs[1][i] = 0.0;

            for ch in 0..4 {
                if let Some(input) = &inputs[ch] {
                    let channel = &self.params.channels[ch];

                    // master
                    outputs[0][i] += input[i] * channel_gain[ch];

                    // cue
                    if channel.cue {
                        outputs[1][i] += input[i]
                    }
                }
            }
        }

        None
    }

    fn inputs(&self) -> &[LineType] {
        &[
            LineType::Stereo,
            LineType::Stereo,
            LineType::Stereo,
            LineType::Stereo,
        ]
    }

    fn outputs(&self) -> &[LineType] {
        &[LineType::Stereo, LineType::Stereo]
    }
}
