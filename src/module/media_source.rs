use std::collections::VecDeque;
use std::sync::mpsc::{self, SyncSender, Receiver, TryRecvError};
use std::thread;

use derive_more::From;
use mixlab_codec::ffmpeg::codec::{self, CodecBuilder, RecvFrameError, Decode};
use mixlab_codec::ffmpeg::{AvError, AvIoError, AvIoReader, IoReader, InputContainer};
use mixlab_protocol::{MediaId, MediaSourceParams};
use mixlab_util::time::{MediaTime, MediaDuration, TimeBase};

use crate::engine::{InputRef, OutputRef, VideoFrame, ModuleCtx, SAMPLE_RATE, TICKS_PER_SECOND};
use crate::module::{ModuleT, LineType, Terminal};
use crate::project::media;
use crate::project::ProjectBaseRef;
use crate::project::stream::ReadStream;
use crate::throttle::MediaThrottle;
use crate::video;

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
    rx: Receiver<Frame>,
    epoch: Option<MediaTime>,
    video_buffer: VecDeque<Frame>,
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

    fn receive_event(&mut self, event: MediaSourceEvent) {
        match event {
            MediaSourceEvent::SetMedia(media) => {
                self.media = media;
            }
        }
    }

    fn run_tick(&mut self, t: u64, _: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let start_of_frame = MediaTime::new(t as i64, SAMPLE_RATE as i64);
        let end_of_frame = start_of_frame + MediaDuration::new(1, TICKS_PER_SECOND as i64);

        if let Some(media) = &mut self.media {
            match media.rx.try_recv() {
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    eprintln!("media_source: decode thread died");
                }
                Ok(frame) => {
                    let epoch = *media.epoch.get_or_insert(start_of_frame);

                    media.video_buffer.push_back(Frame {
                        pts: frame.pts.add_epoch(epoch),
                        frame: frame.frame,
                    });
                }
            }

            if let Some(frame) = media.video_buffer.front() {
                if frame.pts < end_of_frame {
                    *outputs[0].expect_video() = Some(VideoFrame {
                        data: frame.frame.clone(),
                        tick_offset: frame.pts - start_of_frame,
                    });

                    media.video_buffer.pop_front();
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

async fn open_media(project: ProjectBaseRef, media_id: MediaId) -> Option<OpenMedia> {
    match media::open(project, media_id).await {
        Ok(Some(stream)) => {
            let (tx, rx) = mpsc::sync_channel(2);
            thread::spawn(move || {
                let result = run_decode_thread(stream, tx);
                println!("decode thread said: {:?}", result);
            });
            Some(OpenMedia {
                media_id,
                rx,
                epoch: None,
                video_buffer: VecDeque::new(),
            })
        }
        Ok(None) => None,
        Err(e) => {
            eprintln!("media_source: could not open {:?}: {:?}", media_id, e);
            None
        }
    }
}

#[derive(Debug)]
struct Frame {
    pts: MediaTime,
    frame: video::Frame,
}

#[derive(Debug, From)]
enum DecodeError {
    CodecOpen(codec::OpenError),
    NoFrames,
    RecvFrame(RecvFrameError),
    Av(AvError),
    Io(<ReadStream as IoReader>::Error),
}

impl From<AvIoError<ReadStream>> for DecodeError {
    fn from(e: AvIoError<ReadStream>) -> DecodeError {
        match e {
            AvIoError::Av(e) => DecodeError::Av(e),
            AvIoError::Io(e) => DecodeError::Io(e),
        }
    }
}

fn run_decode_thread(stream: ReadStream, tx: SyncSender<Frame>) -> Result<(), DecodeError> {
    let mut container = InputContainer::open(AvIoReader::new(stream))?;

    for (idx, stream) in container.streams().iter().enumerate() {
        println!("Stream #{}: {}", idx, stream.codec_name().unwrap_or("-"));
        // println!("            Time base: {}", stream.time_base());
    }

    let video_stream = &container.streams()[0];
    let video_time_base = video_stream.time_base();
    let video_codec_params = video_stream.codec_parameters();

    let mut video_decode = CodecBuilder::new(video_codec_params.codec_id, video_time_base)
        .with_parameters(video_codec_params)
        .open_decoder()?;

    let mut play = PlaybackContext {
        container,
        video_decode,
        video_time_base,
        throttle: MediaThrottle::new(),
        tx,
    };

    let mut iter_start = MediaTime::zero();

    while let Some(iter_end) = play_once(&mut play, iter_start)? {
        play.video_decode.flush_buffers();
        play.container.seek(MediaTime::zero())?;
        iter_start = iter_end;
    }

    Ok(())
}

struct PlaybackContext {
    container: InputContainer<ReadStream>,
    video_decode: Decode,
    video_time_base: TimeBase,
    throttle: MediaThrottle,
    tx: SyncSender<Frame>,
}

fn play_once(play: &mut PlaybackContext, iter_start: MediaTime) -> Result<Option<MediaTime>, DecodeError> {
    let mut iter_end = None;
    let mut reached_end_of_stream = false;

    loop {
        // read packet and send to decoder
        if !reached_end_of_stream {
            match play.container.read_packet()? {
                Some(pkt) => {
                    if pkt.stream_index() != 0 {
                        continue;
                    }

                    play.video_decode.send_packet(&pkt)?;
                }
                None => {
                    play.video_decode.end_of_stream()?;
                    reached_end_of_stream = true;
                }
            }
        }

        // receive decoded frame from codec if ready
        match play.video_decode.recv_frame() {
            Ok(decoded) => {
                // TODO what to do if packet duration is ever 0? some container
                // formats do not encode frame duration. assert for now and
                // deal with it later
                assert!(decoded.packet_duration() != 0);

                let pts = play.video_time_base
                    .scale_timestamp(decoded.presentation_timestamp())
                    .add_epoch(iter_start);

                let duration = play.video_time_base
                    .scale_duration(decoded.packet_duration());

                iter_end = Some(pts + duration);

                let frame = Frame {
                    pts: pts,
                    frame: video::Frame {
                        decoded,
                        duration_hint: duration,
                    },
                };

                match play.tx.send(frame) {
                    Ok(()) => {}
                    Err(_) => {
                        // receiver disconnected
                        return Ok(None);
                    }
                }

                play.throttle.wait_until(pts);
            }
            Err(RecvFrameError::NeedMoreInput) => { continue; }
            Err(RecvFrameError::Eof) => { break; }
            Err(e) => { return Err(e.into()); }
        }
    }

    Ok(Some(iter_end.ok_or(DecodeError::NoFrames)?))
}
