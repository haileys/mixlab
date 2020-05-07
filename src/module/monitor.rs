use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use fdk_aac::enc as aac;
use futures::sink::SinkExt;
use num_rational::Rational64;
use tokio::sync::broadcast;
use uuid::Uuid;
use warp::ws::{self, WebSocket};

use mixlab_codec::avc::DecoderConfigurationRecord;
use mixlab_codec::avc::encode::{AvcEncoder, AvcParams};
use mixlab_codec::ffmpeg::AvFrame;
use mixlab_codec::ffmpeg::sys;
use mixlab_mux::mp4::{self, Mp4Mux, Mp4Params, TrackData, AdtsFrame};
use mixlab_protocol::{LineType, Terminal, MonitorIndication, MonitorTransportPacket};

use crate::engine::{InputRef, OutputRef, Sample, SAMPLE_RATE};
use crate::module::ModuleT;

const MONITOR_WIDTH: usize = 1120;
const MONITOR_HEIGHT: usize = 700;

lazy_static::lazy_static! {
    static ref SOCKETS: Mutex<HashMap<Uuid, Stream>> = Mutex::new(HashMap::new());
}

struct Stream {
    params: Mp4Params<'static>,
    live: Arc<broadcast::Sender<StreamSegment>>,
}

#[derive(Clone, Debug)]
enum StreamSegment {
    Audio { duration: u32, frame: mp4::AdtsFrame },
    Video { duration: u32, frame: mp4::AvcFrame },
}

pub async fn stream(socket_id: Uuid, mut client: WebSocket) -> Result<(), ()> {
    let (params, mut stream) = (*SOCKETS).lock()
        .unwrap()
        .get(&socket_id)
        .map(|stream| (stream.params.clone(), stream.live.subscribe()))
        .ok_or(())?;

    send_packet(&mut client, MonitorTransportPacket::Init { params }).await?;

    // TODO if we lag we should catch up to the start of the stream rather
    // than disconnecting the client
    while let Ok(segment) = stream.recv().await {
        match segment {
            StreamSegment::Audio { duration, frame } => {
                send_packet(&mut client, MonitorTransportPacket::Frame {
                    duration,
                    track_data: TrackData::Audio(frame.clone()),
                }).await?;
            }
            StreamSegment::Video { duration, frame } => {
                send_packet(&mut client, MonitorTransportPacket::Frame {
                    duration,
                    track_data: TrackData::Video(frame),
                }).await?;
            }
        }
    }

    Ok(())
}

async fn send_packet(websocket: &mut WebSocket, packet: MonitorTransportPacket) -> Result<(), ()> {
    // should never fail:
    let bytes = bincode::serialize(&packet).unwrap();

    websocket.send(ws::Message::binary(bytes)).await
        .map_err(|_| ())
}

const CHANNELS: usize = 2;

// must match AAC encoder's granule size
const SAMPLES_PER_CHANNEL_PER_FRAGMENT: usize = 1024;

#[derive(Debug)]
struct AacFrame {
    data: Vec<u8>,
    timestamp: u64,
}

#[derive(Debug)]
pub struct Monitor {
    epoch: Option<Rational64>,
    socket_id: Uuid,
    segments_tx: Arc<broadcast::Sender<StreamSegment>>,
    file: File,
    mux: Mp4Mux,
    scheduler: Scheduler,
    inputs: Vec<Terminal>,
}

impl ModuleT for Monitor {
    type Params = ();
    type Indication = MonitorIndication;

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        // create audio ctx
        let audio_ctx = AudioCtx::new();

        // create video ctx
        let video_ctx = VideoCtx::new();

        // mp4 params placeholder
        let mp4_params = {
            let dcr = video_ctx.decoder_configuration_record();
            let mut dcr_bytes = vec![];
            dcr.write_to(&mut dcr_bytes);

            Mp4Params {
                timescale: SAMPLE_RATE as u32,
                width: MONITOR_WIDTH as u32,
                height: MONITOR_HEIGHT as u32,
                dcr: Cow::Owned(dcr_bytes),
            }
        };

        // set up mp4 params and create mux
        let (mux, init_segment) = Mp4Mux::new(mp4_params.clone());

        // create dump file and write init segment
        let mut file = File::create("dump.mp4").unwrap();
        println!("writing init ({} bytes)", init_segment.len());
        file.write_all(&init_segment).unwrap();

        // register socket
        let socket_id = Uuid::new_v4();
        let (segments_tx, _) = broadcast::channel(1024);
        let segments_tx = Arc::new(segments_tx);
        (*SOCKETS).lock().unwrap().insert(socket_id, Stream {
            params: mp4_params,
            live: segments_tx.clone(),
        });

        // create scheduler
        let scheduler = Scheduler::new(audio_ctx, video_ctx);

        let module = Monitor {
            epoch: None,
            scheduler,
            socket_id,
            segments_tx,
            mux,
            file,
            inputs: vec![
                LineType::Video.labeled("Video"),
                LineType::Stereo.labeled("Audio"),
            ]
        };

        (module, MonitorIndication { socket_id })
    }

    fn params(&self) -> Self::Params {
        ()
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, time: u64, inputs: &[InputRef], _: &mut [OutputRef]) -> Option<Self::Indication> {
        let (video, audio) = match inputs {
            [video, audio] => (video.expect_video(), audio.expect_stereo()),
            _ => unreachable!()
        };

        let timestamp = Rational64::new(time as i64, SAMPLE_RATE as i64);
        let epoch = *self.epoch.get_or_insert(timestamp);

        self.scheduler.send_audio(audio);

        if let Some(video_frame) = video {
            let frame = video_frame.data.decoded.clone();
            let frame_timestamp = timestamp - epoch + video_frame.tick_offset;

            self.scheduler.send_video(frame_timestamp, video_frame.data.duration_hint, frame);
        }

        self.scheduler.barrier(timestamp - epoch);

        while let Some(segment) = self.scheduler.recv_segment() {
            // send segment to connected monitors
            // this only errors if there are no connected clients
            let _ = self.segments_tx.send(segment.clone());

            // write segment to dump file
            let (duration, track_data) = match segment {
                StreamSegment::Audio { duration, frame } => (duration, TrackData::Audio(frame)),
                StreamSegment::Video { duration, frame } => (duration, TrackData::Video(frame)),
            };

            let segment = self.mux.write_track(duration, &track_data);
            let _ = self.file.write_all(&segment);
        }

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self)-> &[Terminal] {
        &[]
    }
}

#[derive(Debug)]
struct Scheduler {
    segments: VecDeque<StreamSegment>,
    timescale: i64,
    audio_timestamp: Rational64,
    audio_ctx: AudioCtx,
    video_timestamp: Rational64,
    video_ctx: VideoCtx,
}

#[derive(Debug)]
struct AudioCtx {
    codec: aac::Encoder,
    pcm_buff: Vec<i16>,
}

impl Scheduler {
    pub fn new(audio_ctx: AudioCtx, video_ctx: VideoCtx) -> Self {
        Scheduler {
            segments: VecDeque::new(),
            timescale: SAMPLE_RATE as i64,
            audio_timestamp: Rational64::new(0, 1),
            audio_ctx,
            video_timestamp: Rational64::new(0, 1),
            video_ctx,
        }
    }

    pub fn send_audio(&mut self, samples: &[f32]) {
        if let Some((duration, frame)) = self.audio_ctx.send_audio(samples) {
            let new_timestamp = self.audio_timestamp + duration;

            let previous_ts = (self.audio_timestamp * self.timescale).to_integer();
            let new_ts = (new_timestamp * self.timescale).to_integer();
            let duration = new_ts - previous_ts;

            let duration = duration.try_into().expect("duration too large");

            self.segments.push_back(StreamSegment::Audio { duration, frame });

            self.audio_timestamp = new_timestamp;
        }
    }

    pub fn send_video(&mut self, timestamp: Rational64, duration_hint: Rational64, frame: AvFrame) {
        let end_timestamp = timestamp + duration_hint;

        if end_timestamp < self.video_timestamp {
            // frame ends before current time stamp, drop it
            return;
        }

        // recalculate duration as being the time span between end of the last
        // frame and the end of this frame to account for small gaps between the
        // end of the last frame and start of this frame due to timestamp
        // imprecision on the input side:
        let duration = end_timestamp - self.video_timestamp;

        self.encode_video(duration, frame);
    }

    pub fn barrier(&mut self, timestamp: Rational64) {
        if self.video_timestamp < timestamp {
            let duration = timestamp - self.video_timestamp;
            let frame = self.video_ctx.blank_frame();
            self.encode_video(duration, frame);
        }
    }

    fn encode_video(&mut self, duration: Rational64, frame: AvFrame) {
        let frame = self.video_ctx.encode_frame(frame);

        let new_timestamp = self.video_timestamp + duration;

        let previous_ts = (self.video_timestamp * self.timescale).to_integer();
        let new_ts = (new_timestamp * self.timescale).to_integer();
        let duration = new_ts - previous_ts;

        let duration = duration.try_into().expect("duration too large");

        self.segments.push_back(StreamSegment::Video { duration, frame });

        self.video_timestamp = new_timestamp;

    }

    pub fn recv_segment(&mut self) -> Option<StreamSegment> {
        self.segments.pop_front()
    }
}

impl AudioCtx {
    fn new() -> Self {
        let aac_params = aac::EncoderParams {
            bit_rate: aac::BitRate::VbrVeryHigh,
            sample_rate: 44100,
            transport: aac::Transport::Adts,
        };

        let codec = aac::Encoder::new(aac_params).expect("aac::Encoder::new");

        AudioCtx {
            codec,
            pcm_buff: Vec::new(),
        }
    }

    fn send_audio(&mut self, samples: &[Sample]) -> Option<(Rational64, AdtsFrame)> {
        self.pcm_buff.extend(samples.iter().copied().map(|sample| {
            // TODO set CLIP flag if sample is out of range
            let sample = if sample > 1.0 {
                1.0
            } else if sample < -1.0 {
                -1.0
            } else {
                sample
            };

            (sample * i16::max_value() as f32) as i16
        }));

        let audio_frame_sample_count = CHANNELS * SAMPLES_PER_CHANNEL_PER_FRAGMENT;

        if self.pcm_buff.len() > audio_frame_sample_count {
            // encode frame
            let fragment_pcm = &self.pcm_buff[0..audio_frame_sample_count];

            let mut aac_buff = [0u8; 4096];

            let encode_result = self.codec.encode(&fragment_pcm, &mut aac_buff)
                .expect("aac.encode");

            if encode_result.input_consumed != audio_frame_sample_count {
                eprintln!("monitor: aac encoder did not consume exactly {} samples (consumed {})",
                    audio_frame_sample_count, encode_result.input_consumed);
            }

            let duration = Rational64::new(SAMPLES_PER_CHANNEL_PER_FRAGMENT as i64, SAMPLE_RATE as i64);
            let adts = AdtsFrame(Bytes::copy_from_slice(&aac_buff[0..encode_result.output_size]));
            self.pcm_buff.drain(0..audio_frame_sample_count);

            Some((duration, adts))
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct VideoCtx {
    codec: AvcEncoder,
    blank_frame: AvFrame,
}

impl VideoCtx {
    pub fn new() -> Self {
        let params = AvcParams {
            time_base: 44100,
            pixel_format: sys::AVPixelFormat_AV_PIX_FMT_YUV420P,
            color_space: sys::AVColorSpace_AVCOL_SPC_UNSPECIFIED,
            picture_width: MONITOR_WIDTH,
            picture_height: MONITOR_HEIGHT,
        };

        let blank_frame = AvFrame::blank(
            params.picture_width,
            params.picture_height,
            params.pixel_format,
        );

        let codec = AvcEncoder::new(params).unwrap();

        VideoCtx { codec, blank_frame }
    }

    pub fn decoder_configuration_record(&self) -> DecoderConfigurationRecord {
        self.codec.decoder_configuration_record()
    }

    pub fn encode_frame(&mut self, mut frame: AvFrame) -> mp4::AvcFrame {
        frame.set_picture_type(mixlab_codec::ffmpeg::sys::AVPictureType_AV_PICTURE_TYPE_I);
        self.codec.send_frame(frame).unwrap();

        let video_packet = self.codec.recv_packet().unwrap();

        // if dts = pts for all frames, we can safely ignore both and attach our own timing to the frame:
        assert!(video_packet.decode_timestamp() == video_packet.presentation_timestamp());

        // and if all frames are key frames, we can stream directly to clients with no buffering:
        assert!(video_packet.is_key_frame());

        mp4::AvcFrame {
            is_key_frame: true, // all frames are key frames
            composition_time: 0, // dts always equals pts
            data: Bytes::copy_from_slice(video_packet.data()),
        }
    }

    pub fn blank_frame(&self) -> AvFrame {
        self.blank_frame.clone()
    }
}
