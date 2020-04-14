use mixlab_protocol::{IcecastInputParams, LineType, Terminal};

use crate::engine::Sample;
use crate::icecast;
use crate::module::ModuleT;
use crate::source::SourceRecv;
use crate::util;

#[derive(Debug)]
pub struct IcecastInput {
    params: IcecastInputParams,
    recv: Option<SourceRecv>,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for IcecastInput {
    type Params = IcecastInputParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        let recv = params.mountpoint.as_ref().and_then(|mountpoint|
            // TODO - listen returning an error means the mountpoint is already
            // in use. tell the user this via an indication
            icecast::listen(mountpoint).ok());

        let module = IcecastInput {
            params,
            recv,
            inputs: vec![],
            outputs: vec![LineType::Stereo.unlabeled()],
        };

        (module, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        let current_mountpoint = self.recv.as_ref().map(|recv| recv.channel_name());
        let new_mountpoint = new_params.mountpoint.as_ref().map(String::as_str);

        if current_mountpoint != new_mountpoint {
            match new_mountpoint {
                None => {
                    self.recv = None;
                }
                Some(mountpoint) => {
                    // TODO - tell the user about this one too
                    self.recv = icecast::listen(mountpoint).ok();
                }
            }
        }

        self.params = new_params;

        None
    }

    fn run_tick(&mut self, _t: u64, _inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication> {
        let samples = self.recv.as_mut()
            .map(|recv| recv.read(&mut outputs[0]))
            .unwrap_or(0);

        util::zero(&mut outputs[0][samples..]);

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}
