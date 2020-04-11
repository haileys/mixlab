use mixlab_protocol::{ClockParams, LineType};

use crate::engine::{Sample, SAMPLE_RATE};
use crate::module::ModuleT;
use crate::util::sample_seq_duration_ms;

#[derive(Debug)]
pub struct Clock {
    params: ClockParams,
}

impl ModuleT for Clock {
    type Params = ClockParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Clock { params }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, t: u64, _inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let output = &mut outputs[0];

        let len = output.len();
        for i in 0..len {
            let elapsed_ms = sample_seq_duration_ms(SAMPLE_RATE, 0, t + i as u64);
            let beats_per_second = 60.0 / self.params.bpm;
            let current_beat_elapsed_seconds = (elapsed_ms / 1000.0) % beats_per_second;
            let current_beat_elapsed_percent = current_beat_elapsed_seconds / beats_per_second;

            // For the first 25% of the beat, output high, otherwise, output low
            if current_beat_elapsed_percent < 0.25 {
                output[i] = 1.0;
            } else {
                output[i] = 0.0;
            }
        }


        None
    }

    fn inputs(&self) -> &[LineType] {
        &[]
    }

    fn outputs(&self) -> &[LineType] {
        &[LineType::Mono]
    }
}
