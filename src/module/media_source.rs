use crate::engine::{Sample, InputRef, OutputRef};
use crate::module::{ModuleT, LineType, Terminal};

use mixlab_protocol::MediaSourceParams;

#[derive(Debug)]
pub struct MediaSource {
    params: MediaSourceParams,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for MediaSource {
    type Params = MediaSourceParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        (Self {
            params,
            inputs: vec![],
            outputs: vec![
                LineType::Video.unlabeled(),
                LineType::Stereo.unlabeled(),
            ],
        }, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        self.params = params;
        None
    }

    fn run_tick(&mut self, _t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}
