use mixlab_protocol::{MixerParams, LineType, Terminal};

use crate::engine::{Sample, InputRef, OutputRef};
use crate::module::ModuleT;
use crate::util;

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

    fn run_tick(&mut self, _t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let (master, cue) = match outputs {
            [master, cue] => (master.expect_stereo(), cue.expect_stereo()),
            _ => unreachable!(),
        };

        let len = master.len();

        util::zero(master);
        util::zero(cue);

        for (ch, channel) in self.params.channels.iter().enumerate() {
            let input = inputs[ch].expect_stereo();
            let channel_gain = channel.fader * channel.gain.to_linear();

            for i in 0..len {
                master[i] += (input[i] as f64 * channel_gain) as Sample;

                if channel.cue {
                    cue[i] += input[i];
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
