use mixlab_protocol::{StreamInputParams, LineType, Terminal, StreamProtocol};

use crate::engine::{InputRef, OutputRef};
use crate::icecast;
use crate::module::ModuleT;
use crate::rtmp;
use crate::source::SourceRecv;
use crate::util;

#[derive(Debug)]
pub struct StreamInput {
    params: StreamInputParams,
    recv: Option<SourceRecv>,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

impl ModuleT for StreamInput {
    type Params = StreamInputParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        let recv = params.mountpoint.as_ref().and_then(|mountpoint|
            // TODO - listen returning an error means the mountpoint is already
            // in use. tell the user this via an indication
            icecast::listen(mountpoint).ok());

        let module = StreamInput {
            params,
            recv,
            inputs: vec![],
            outputs: vec![
                LineType::Avc.labeled("Video"),
                LineType::Stereo.labeled("Audio"),
            ],
        };

        (module, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        let current_mountpoint = self.recv.as_ref().map(|recv| recv.channel_name());
        let new_mountpoint = new_params.mountpoint.as_ref().map(String::as_str);

        if current_mountpoint != new_mountpoint || self.params.protocol != new_params.protocol {
            // TODO - tell the user about this one too
            self.recv = listen_mountpoint(&new_params);
        }

        self.params = new_params;

        None
    }

    fn run_tick(&mut self, _t: u64, _: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let output = outputs[1].expect_stereo();

        let samples = self.recv.as_mut()
            .map(|recv| recv.read(output))
            .unwrap_or(0);

        util::zero(&mut output[samples..]);

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}

fn listen_mountpoint(params: &StreamInputParams) -> Option<SourceRecv> {
    let mountpoint = params.mountpoint.as_ref()?;

    match params.protocol? {
        StreamProtocol::Icecast => icecast::listen(mountpoint).ok(),
        StreamProtocol::Rtmp => rtmp::listen(mountpoint).ok(),
    }
}
