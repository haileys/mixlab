use std::thread;

use crate::engine::{InputRef, OutputRef, ModuleCtx};
use crate::module::{ModuleT, LineType, Terminal};
use crate::project::media;
use crate::project::stream::ReadStream;
use crate::project::ProjectBaseRef;

use mixlab_codec::ffmpeg::{AvIoError, AvIoReader, InputContainer};
use mixlab_protocol::{MediaId, MediaSourceParams};

#[derive(Debug)]
pub struct MediaSource {
    ctx: ModuleCtx<Self>,
    params: MediaSourceParams,
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
}

impl ModuleT for MediaSource {
    type Params = MediaSourceParams;
    type Indication = ();
    type Event = MediaSourceEvent;

    fn create(params: Self::Params, ctx: ModuleCtx<Self>) -> (Self, Self::Indication) {
        let mut module = Self {
            ctx,
            params: MediaSourceParams::default(),
            media: None,
            inputs: vec![],
            outputs: vec![
                LineType::Video.unlabeled(),
                LineType::Stereo.unlabeled(),
            ],
        };

        module.update(params);

        (module, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, params: Self::Params) -> Option<Self::Indication> {
        if self.params.media_id != params.media_id {
            self.params.media_id = params.media_id;

            let project = self.ctx.project();

            self.ctx.spawn_async(async move {
                let media = match params.media_id {
                    Some(media_id) => open_media(project, media_id).await,
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

async fn open_media(project: ProjectBaseRef, media_id: MediaId) -> Option<OpenMedia> {
    println!("open_media!");

    match media::open(project, media_id).await {
        Ok(Some(stream)) => {
            println!("stream -> {:?}", stream);
            thread::spawn(move || {
                let result = run_decode_thread(stream);
                println!("decode thread said: {:?}", result);
            });
            Some(OpenMedia { media_id })
        }
        Ok(None) => None,
        Err(e) => {
            eprintln!("media_source: could not open {:?}: {:?}", media_id, e);
            None
        }
    }
}

fn run_decode_thread(stream: ReadStream) -> Result<(), AvIoError<ReadStream>> {
    let mut container = InputContainer::open(AvIoReader::new(stream))?;

    for (idx, stream) in container.streams().iter().enumerate() {
        println!("Stream #{}: {}", idx, stream.codec_name().unwrap_or("-"));
        println!("            Time base: {}", stream.time_base());
    }

    for i in 0..10 {
        println!("----- packet #{}:\n", i);
        println!("{:#?}", container.read_packet());
    }

    Ok(())
}
