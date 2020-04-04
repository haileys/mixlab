use crate::engine::{Sample, ZERO_BUFFER_STEREO, ONE_BUFFER_MONO};
use crate::module::{Module, LineType};

use mixlab_protocol::EnvelopeParams;

#[derive(Debug)]
pub struct Envelope {
    params: EnvelopeParams
}

impl Module for Envelope {
    type Params = EnvelopeParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Envelope {params}, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        self.params = params;
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        unimplemented!()
    }

    fn inputs(&self) -> &[LineType] {
        &[LineType::Mono]
    }

    fn outputs(&self) -> &[LineType] {
        &[LineType::Mono]
    }
}
