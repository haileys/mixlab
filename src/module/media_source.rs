use crate::engine::{InputRef, OutputRef, ModuleCtx};
use crate::module::{ModuleT, LineType, Terminal};
use crate::project::media;
use crate::project::stream::ReadStream;

use mixlab_protocol::{MediaId, MediaSourceParams};

#[derive(Debug)]
pub struct MediaSource {
    ctx: ModuleCtx<Self>,
    media: Option<OpenMedia>,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

#[derive(Debug)]
pub enum MediaSourceEvent {
    SetMedia(Option<OpenMedia>),
}

#[derive(Debug)]
pub struct OpenMedia {
    media_id: MediaId,
    stream: ReadStream,
}

impl ModuleT for MediaSource {
    type Params = MediaSourceParams;
    type Indication = ();
    type Event = MediaSourceEvent;

    fn create(params: Self::Params, ctx: ModuleCtx<Self>) -> (Self, Self::Indication) {
        (Self {
            ctx,
            media: None,
            inputs: vec![],
            outputs: vec![
                LineType::Video.unlabeled(),
                LineType::Stereo.unlabeled(),
            ],
        }, ())
    }

    fn params(&self) -> Self::Params {
        MediaSourceParams {
            media_id: self.current_media_id(),
        }
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        if params.media_id != self.current_media_id() {
            let project = self.ctx.project();

            self.ctx.spawn_async(async move {
                let media = match params.media_id {
                    Some(media_id) => {
                        match media::open(project, media_id).await {
                            Ok(Some(stream)) => Some(OpenMedia { media_id, stream }),
                            Ok(None) => None,
                            Err(e) => {
                                eprintln!("media_source: could not open {:?}: {:?}", media_id, e);
                                None
                            }
                        }
                    }
                    None => None,
                };

                MediaSourceEvent::SetMedia(media)
            });
        }
        None
    }

    fn run_tick(&mut self, _t: u64, _inputs: &[InputRef], _outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}

impl MediaSource {
    fn current_media_id(&self) -> Option<MediaId> {
        self.media.as_ref().map(|m| m.media_id)
    }
}
