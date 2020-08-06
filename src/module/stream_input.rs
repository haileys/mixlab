use std::cmp;

use mixlab_protocol::{StreamInputParams, LineType, Terminal, StreamProtocol};
use mixlab_util::time::{MediaTime, MediaDuration};

use crate::engine::{self, InputRef, OutputRef, Sample, VideoFrame, SAMPLE_RATE};
use crate::icecast;
use crate::module::ModuleT;
use crate::rtmp;
use crate::source::{SourceRecv, SourceId, Frame, AudioData, VideoData};
use crate::util;

#[derive(Debug)]
pub struct StreamInput {
    params: StreamInputParams,
    recv: Option<SourceRecv>,
    source: Option<SourceTiming>,
    audio_frame: Option<Frame<AudioData>>,
    video_frame: Option<Frame<VideoData>>,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
}

#[derive(Debug)]
struct SourceTiming {
    id: SourceId,
    epoch: MediaTime,
}

impl ModuleT for StreamInput {
    type Params = StreamInputParams;
    type Indication = ();

    fn create(params: Self::Params, _: engine::ModuleLink<Self>) -> (Self, Self::Indication) {
        let recv = params.mountpoint.as_ref().and_then(|mountpoint|
            // TODO - listen returning an error means the mountpoint is already
            // in use. tell the user this via an indication
            icecast::listen(mountpoint).ok());

        let module = StreamInput {
            params,
            recv,
            source: None,
            audio_frame: None,
            video_frame: None,
            inputs: vec![],
            outputs: vec![
                LineType::Video.labeled("Video"),
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

    fn run_tick(&mut self, engine_time: u64, _: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let engine_time = MediaTime::new(engine_time as i64, SAMPLE_RATE as i64);

        let (video_out, mut audio_out) = match outputs {
            [video, audio] => (video.expect_video(), audio.expect_stereo()),
            _ => unimplemented!(),
        };

        let tick_duration = MediaDuration::new(audio_out.len() as i64 / 2, SAMPLE_RATE as i64);

        let video_frame = self.video_frame.take()
            .or_else(|| {
                self.recv.as_mut()
                    .and_then(|recv| recv.read_video())
            });

        let existing_source_id = self.source.as_ref().map(|src| src.id);

        // process audio frames. we may have to consume multiple input audio
        // frames to fill the output buffer
        while audio_out.len() > 0 {
            let audio_frame = self.audio_frame.take()
                .or_else(|| {
                    self.recv.as_mut()
                        .and_then(|recv| recv.read_audio())
                    });

            if let Some(mut frame) = audio_frame {
                if existing_source_id != Some(frame.source_id) {
                    // source changed
                    self.source = Some(SourceTiming {
                        id: frame.source_id,
                        epoch: engine_time.remove_epoch(frame.source_time),
                    });
                }

                let len = cmp::min(audio_out.len(), frame.data.len());

                for i in 0..len {
                    audio_out[i] = convert_sample(frame.data[i]);
                }

                audio_out = &mut audio_out[len..];

                if len < frame.data.len() {
                    frame.data.drain(0..len);
                    self.audio_frame = Some(frame);
                }
            } else {
                util::zero(audio_out);
                break;
            }
        }

        *video_out = video_frame.and_then(|frame| {
            let tick_offset = self.source.as_ref()
                .map(|source| {
                    frame.source_time.add_epoch(source.epoch) - engine_time
                })
                .filter(|tick_offset| *tick_offset >= MediaDuration::zero())
                .unwrap_or(MediaDuration::zero());

            if tick_offset > tick_duration {
                // frame is not due for this tick, put it back
                self.video_frame = Some(frame);
                None
            } else {
                // TODO
                // let frame = Rc::new(AvcFrame {
                //     data: frame.data,
                //     tick_offset,
                //     duration,
                //     previous,
                // });

                Some(VideoFrame {
                    data: frame.data,
                    tick_offset,
                })
            }
        });

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

fn convert_sample(sample: i16) -> Sample {
    // i16::min_value is a greater absolute distance away from 0 than max_value
    // divide by it rather than max_value to prevent clipping
    let divisor = -(i16::min_value() as Sample);

    sample as Sample / divisor
}
