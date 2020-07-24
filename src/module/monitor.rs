use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use fdk_aac::enc as aac;
use futures::sink::SinkExt;
use tokio::sync::broadcast;
use uuid::Uuid;
use warp::ws::{self, WebSocket};

use mixlab_codec::ffmpeg::PictureSettings;
use mixlab_mux::mp4::{Mp4Params, TrackData, AdtsFrame, AvcFrame};
use mixlab_protocol::{LineType, Terminal, MonitorIndication, MonitorTransportPacket};
use mixlab_util::time::MediaTime;

use crate::engine::{self, InputRef, OutputRef, SAMPLE_RATE};
use crate::module::ModuleT;
use crate::video::encode::{EncodeStream, AudioCtx, AudioParams, VideoCtx, VideoParams, StreamSegment, Profile};

const MONITOR_WIDTH: usize = 560;
const MONITOR_HEIGHT: usize = 350;

lazy_static::lazy_static! {
    static ref SOCKETS: Mutex<HashMap<Uuid, Stream>> = Mutex::new(HashMap::new());
}

struct Stream {
    params: Mp4Params<'static>,
    live: Arc<broadcast::Sender<StreamSegment>>,
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
            StreamSegment::Audio(audio) => {
                send_packet(&mut client, MonitorTransportPacket::Frame {
                    duration: audio.duration,
                    track_data: TrackData::Audio(AdtsFrame(audio.frame.clone())),
                }).await?;
            }
            StreamSegment::Video(video) => {
                send_packet(&mut client, MonitorTransportPacket::Frame {
                    duration: video.duration,
                    track_data: TrackData::Video(AvcFrame {
                        is_key_frame: video.frame.is_key_frame,
                        composition_time: video.frame.composition_time,
                        data: video.frame.data.clone(),
                    }),
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

#[derive(Debug)]
pub struct Monitor {
    epoch: Option<MediaTime>,
    socket_id: Uuid,
    codec: AsyncCodec,
    inputs: Vec<Terminal>,
}

impl ModuleT for Monitor {
    type Params = ();
    type Indication = MonitorIndication;

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        let socket_id = Uuid::new_v4();
        let codec = AsyncCodec::start(socket_id);

        let module = Monitor {
            epoch: None,
            socket_id,
            codec,
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

        let absolute_timestamp = MediaTime::new(time as i64, SAMPLE_RATE as i64);
        let epoch = *self.epoch.get_or_insert(absolute_timestamp);
        let timestamp = absolute_timestamp.remove_epoch(epoch);

        let result = self.codec.send(Tick {
            timestamp,
            audio: audio.to_vec(),
            video: video.cloned(),
        });

        if let Err(()) = result {
            // TODO handle gracefully
            panic!("monitor: codec thread died")
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
struct AsyncCodec {
    codec_tx: mpsc::SyncSender<Tick>,
}

impl AsyncCodec {
    pub fn start(socket_id: Uuid) -> AsyncCodec {
        let (codec_tx, codec_rx) = mpsc::sync_channel(2);
        thread::spawn(move || run_codec_thread(socket_id, codec_rx));

        AsyncCodec {
            codec_tx,
        }
    }

    pub fn send(&mut self, tick: Tick) -> Result<(), ()> {
        use mpsc::TrySendError;

        match self.codec_tx.try_send(tick) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                // codec thread lagging
                println!("monitor: codec not keeping up, dropping tick");
                Ok(())
            }
            Err(TrySendError::Disconnected(_)) => {
                Err(())
            }
        }
    }
}

struct Tick {
    timestamp: MediaTime,
    audio: Vec<engine::Sample>,
    video: Option<engine::VideoFrame>,
}

fn run_codec_thread(socket_id: Uuid, rx: mpsc::Receiver<Tick>) {
    // create encoders
    let audio_ctx = AudioCtx::new(AudioParams {
        bit_rate: aac::BitRate::VbrVeryHigh,
        sample_rate: SAMPLE_RATE,
        transport: aac::Transport::Adts,
    });

    let video_ctx = VideoCtx::new(VideoParams {
        picture: PictureSettings::yuv420p(MONITOR_WIDTH, MONITOR_HEIGHT),
        time_base: SAMPLE_RATE,
        profile: Profile::Monitor,
    });

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

    // register socket
    let (segments_tx, _) = broadcast::channel(1024);
    let segments_tx = Arc::new(segments_tx);
    (*SOCKETS).lock().unwrap().insert(socket_id, Stream {
        params: mp4_params,
        live: segments_tx.clone(),
    });

    // create encode stream
    let mut encode = EncodeStream::new(audio_ctx, video_ctx);

    // run codec
    while let Ok(tick) = rx.recv() {
        encode.send_audio(&tick.audio);

        if let Some(video_frame) = tick.video {
            let frame_timestamp = tick.timestamp + video_frame.tick_offset;
            let frame = video_frame.data.decoded.clone();

            encode.send_video(frame_timestamp, video_frame.data.duration_hint, frame);
        }

        encode.barrier(tick.timestamp);

        while let Some(segment) = encode.recv_segment() {
            if let StreamSegment::Video(video) = &segment {
                // if dts = pts for all frames, we can safely ignore both and attach our own timing to the frame:
                assert!(video.frame.composition_time.is_zero());

                // and if all frames are key frames, we can stream directly to clients with no buffering:
                assert!(video.frame.is_key_frame);
            }

            // send segment to connected monitors
            // this only errors if there are no connected clients
            let _ = segments_tx.send(segment.clone());
        }
    }
}
