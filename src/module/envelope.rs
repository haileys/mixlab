use crate::engine::{self, InputRef, OutputRef, SAMPLE_RATE};
use crate::module::{ModuleT, LineType, Terminal};

use mixlab_protocol::EnvelopeParams;

type SampleSeq = u64;

#[derive(Debug)]
enum EnvelopeState {
    Initial,
    TriggerOn {on: SampleSeq},
    TriggerOff {off: SampleSeq, off_amplitude: f64},
}

type Ms = f64;
fn sample_seq_duration_ms(first: SampleSeq, last: SampleSeq) -> Ms {
    (last - first) as f64 / SAMPLE_RATE as f64 * 1000.0
}

fn clamp(x: f64) -> f64 {
    if x > 1.0 {
        1.0
    } else if x < 0.0 {
        0.0
    } else {
        x
    }
}

fn invert(x: f64) -> f64 {
    1.0 - x
}

fn amplitude(params: &EnvelopeParams, state: &EnvelopeState, t: SampleSeq) -> f64 {
    match state {
        EnvelopeState::Initial => 0.0,
        EnvelopeState::TriggerOn {on} => {
            let ms_since_on = sample_seq_duration_ms(*on, t);

            if ms_since_on < params.attack_ms {
                // Currently in attack phase
                1.0 / params.attack_ms * ms_since_on
            } else {
                // In decay/sustain phase
                let ms_since_decay_started = ms_since_on - params.attack_ms;
                let decay_amplitude = invert(clamp(1.0 / params.decay_ms * ms_since_decay_started));

                params.sustain_amplitude + ((1.0 - params.sustain_amplitude) * decay_amplitude)
            }
        }
        EnvelopeState::TriggerOff {off, off_amplitude} => {
            let ms_since_off = sample_seq_duration_ms(*off, t);
            let release_amplitude = invert(clamp(1.0 / params.release_ms * ms_since_off));

            off_amplitude * release_amplitude
        }
    }
}

#[derive(Debug)]
pub struct Envelope {
    params: EnvelopeParams,
    state: EnvelopeState,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for Envelope {
    type Params = EnvelopeParams;
    type Indication = ();

    fn create(params: Self::Params, _: engine::ModuleLink<Self>) -> (Self, Self::Indication) {
        (Self {
            params,
            state: EnvelopeState::Initial,
            inputs: vec![LineType::Mono.unlabeled()],
            outputs: vec![LineType::Mono.unlabeled()],
        }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        self.params = params;
        None
    }

    fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let input = inputs[0].expect_mono();
        let output = outputs[0].expect_mono();

        let len = input.len();
        for i in 0..len {
            let sample_seq = t + i as u64;

            // First, process input
            match self.state {
                EnvelopeState::Initial | EnvelopeState::TriggerOff { .. } => {
                    if input[i] == 1.0 {
                        self.state = EnvelopeState::TriggerOn { on: sample_seq };
                    }
                }
                EnvelopeState::TriggerOn {..} => {
                    if input[i] == 0.0 {
                        self.state = EnvelopeState::TriggerOff {
                            off: sample_seq,
                            off_amplitude: amplitude(&self.params, &self.state, sample_seq)
                        };
                    }
                }
            }
            // Then set output
            output[i] = amplitude(&self.params, &self.state, sample_seq) as f32;
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
