use mixlab_protocol::{MixerParams, LineType, Terminal};

use crate::engine::Sample;
use crate::module::ModuleT;

#[derive(Debug)]
pub struct Mixer {
    params: MixerParams,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
    channel_gain: Vec<f64>,
}

impl ModuleT for Mixer {
    type Params = MixerParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        let mixer = Mixer {
            inputs: params.channels.iter().enumerate().map(|(i, _)| {
                LineType::Stereo.labeled(&(i+1).to_string())
            }).collect(),
            outputs: vec![
                LineType::Stereo.labeled("Master"),
                LineType::Stereo.labeled("Cue"),
            ],
            channel_gain: vec![0.0; params.channels.len()],
            params,
        };

        (mixer, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        let (new, _) = Self::create(params);
        *self = new;
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        const MASTER: usize = 0;
        const CUE: usize = 1;

        let len = outputs[0].len();

        for (ch, channel) in self.params.channels.iter().enumerate() {
            self.channel_gain[ch] = channel.fader * channel.gain.to_linear();

            for i in 0..len {
                if ch == 0 {
                    outputs[MASTER][i] = 0.0;
                    outputs[CUE][i] = 0.0;
                }

                if let Some(input) = &inputs[ch] {
                    let channel = &self.params.channels[ch];

                    outputs[MASTER][i] += (input[i] as f64 * self.channel_gain[ch]) as Sample;

                    if channel.cue {
                        outputs[CUE][i] += input[i];
                    }
                }
            }
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
