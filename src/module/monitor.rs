use std::borrow::Cow;
use std::collections::HashMap;
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

use mixlab_codec::avc::encode::{AvcEncoder, AvcParams};
use mixlab_mux::mp4::{self, Mp4Mux, Mp4Params, TrackData, AdtsFrame};
use mixlab_protocol::{LineType, Terminal, MonitorIndication, MonitorTransportPacket};

use crate::engine::{InputRef, OutputRef, SAMPLE_RATE};
use crate::module::ModuleT;
use crate::video;

lazy_static::lazy_static! {
    static ref SOCKETS: Mutex<HashMap<Uuid, Stream>> = Mutex::new(HashMap::new());
}

struct Stream {
    live: Arc<broadcast::Sender<StreamSegment>>,
}

#[derive(Clone)]
enum StreamSegment {
    Audio { duration: u32, frame: mp4::AdtsFrame },
    // Video { duration: u32, frame: Arc<video::Frame> },
    RawAvc { duration: u32, frame: mp4::AvcFrame },
}

pub async fn stream(socket_id: Uuid, mut client: WebSocket) -> Result<(), ()> {
    let mut stream = (*SOCKETS).lock()
        .unwrap()
        .get(&socket_id)
        .map(|stream| stream.live.subscribe())
        .ok_or(())?;

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
            // StreamSegment::Video { duration, frame } => {
            //     println!("begin");
            //     let key_frame =
            //         if frame.is_key_frame() {
            //             println!("is key frame");
            //             Some(frame.clone())
            //         } else {
            //             println!("not key frame");
            //             Some(frame.key_frame.clone().unwrap())
            //         };

            //     let key_frame_for_init =
            //         if active_key_frame.is_none() {
            //             println!("active key frame is none");
            //             key_frame.clone()
            //         } else {
            //             println!("active key frame is some");
            //             None
            //         };

            //     if let Some(key_frame) = key_frame_for_init {
            //         println!("sending init");

            //         // send DCR if this is the first video frame received
            //         let dcr = frame.specific.bitstream.dcr.clone();
            //         let dcr_bytes = {
            //             let mut buff = Vec::new();
            //             dcr.write_to(&mut buff);
            //             buff
            //         };

            //         send_packet(&mut client, MonitorTransportPacket::Init {
            //             params: Mp4Params {
            //                 timescale: SAMPLE_RATE as u32,
            //                 width: dcr.picture_width() as u32,
            //                 height: dcr.picture_height() as u32,
            //                 dcr: Cow::Owned(dcr_bytes),
            //             },
            //         }).await?;
            //     }

            //     if frame.is_key_frame() {
            //         println!("is key frame");
            //         active_key_frame = Some(frame.id());
            //     } else {
            //         println!("not key frame");
            //         if let Some(key_frame) = key_frame {
            //             if active_key_frame != Some(key_frame.id()) {
            //                 println!("changing active key frame");
            //                 send_packet(&mut client, MonitorTransportPacket::Frame {
            //                     duration: 0,
            //                     track_data: TrackData::Video(avc_frame_to_mp4(&key_frame.specific)),
            //                 }).await?;

            //                 active_key_frame = Some(key_frame.id());
            //             }
            //         }
            //     }

            //     println!("sending frame");
            //     send_packet(&mut client, MonitorTransportPacket::Frame {
            //         duration,
            //         track_data: TrackData::Video(avc_frame_to_mp4(&frame.specific)),
            //     }).await?;
            // }
            StreamSegment::RawAvc { duration, frame } => {
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
    socket_id: Uuid,
    segments_tx: Arc<broadcast::Sender<StreamSegment>>,
    file: File,
    engine_epoch: Option<u64>,
    aac: aac::Encoder,
    audio_pcm_buff: Vec<i16>,
    video_ctx: Option<VideoCtx>,
    filled_video_to: Rational64,
    previous_video_frame: Option<Arc<video::Frame>>,
    inputs: Vec<Terminal>,
}

#[derive(Debug)]
struct VideoCtx {
    codec: AvcEncoder,
    mux: Mp4Mux,
}

impl ModuleT for Monitor {
    type Params = ();
    type Indication = MonitorIndication;

    fn create(_: Self::Params) -> (Self, Self::Indication) {
        // register socket
        let socket_id = Uuid::new_v4();
        let (segments_tx, _) = broadcast::channel(1024);
        let segments_tx = Arc::new(segments_tx);
        (*SOCKETS).lock().unwrap().insert(socket_id, Stream {
            live: segments_tx.clone(),
        });

        // setup codecs
        let aac_params = aac::EncoderParams {
            bit_rate: aac::BitRate::VbrVeryHigh,
            sample_rate: 44100,
            transport: aac::Transport::Adts,
        };

        let aac = aac::Encoder::new(aac_params).expect("aac::Encoder::new");

        let file = File::create("dump.mp4").unwrap();

        let module = Monitor {
            socket_id,
            segments_tx,
            file,
            engine_epoch: None,
            aac: aac,
            audio_pcm_buff: Vec::new(),
            video_ctx: None,
            filled_video_to: Rational64::new(0, 1),
            previous_video_frame: None,
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

        let engine_epoch = *self.engine_epoch.get_or_insert(time);
        let timestamp = Rational64::new((time - engine_epoch) as i64, SAMPLE_RATE as i64);
        let tick_len = Rational64::new(audio.len() as i64 / 2, SAMPLE_RATE as i64);
        let end_of_tick = timestamp + tick_len;

        // initialise video codec and muxer
        let video_ctx = match (self.video_ctx.as_mut(), video) {
            (Some(video_ctx), _) => video_ctx,
            (None, None) => {
                // can't do anything until we've received the first video frame
                return None;
            }
            (None, Some(frame)) => {
                let decoded = &frame.data.decoded;

                println!("pix fmt : {:?}", decoded.pixel_format());

                let codec = AvcEncoder::new(AvcParams {
                    time_base: SAMPLE_RATE,
                    pixel_format: decoded.pixel_format(),
                    picture_width: decoded.picture_width(),
                    picture_height: decoded.picture_height(),
                    color_space: decoded.color_space(),
                }).unwrap();

                let dcr = codec.decoder_configuration_record();
                let mut dcr_bytes = vec![];
                dcr.write_to(&mut dcr_bytes);

                let (mux, init) = Mp4Mux::new(Mp4Params {
                    timescale: SAMPLE_RATE as u32,
                    width: frame.data.decoded.picture_width().try_into().expect("picture_width too large"),
                    height: frame.data.decoded.picture_height().try_into().expect("picture_height too large"),
                    dcr: Cow::Owned(dcr_bytes),
                });

                println!("writing init ({} bytes)", init.len());
                self.file.write_all(&init).unwrap();

                self.video_ctx = Some(VideoCtx {
                    codec,
                    mux,
                });

                self.video_ctx.as_mut().unwrap()
            }
        };

        // convert input audio samples from f32 to i16
        self.audio_pcm_buff.extend(audio.iter().copied().map(|sample| {
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

        // process incoming audio
        let audio_frame_sample_count = CHANNELS * SAMPLES_PER_CHANNEL_PER_FRAGMENT;

        if self.audio_pcm_buff.len() > audio_frame_sample_count {
            let fragment_pcm = &self.audio_pcm_buff[0..audio_frame_sample_count];

            let mut aac_buff = [0u8; 4096];

            let encode_result = self.aac.encode(&fragment_pcm, &mut aac_buff).expect("aac.encode");

            if encode_result.input_consumed != audio_frame_sample_count {
                eprintln!("monitor: aac encoder did not consume exactly {} samples (consumed {})",
                    audio_frame_sample_count, encode_result.input_consumed);
            }

            let adts = AdtsFrame(Bytes::copy_from_slice(&aac_buff[0..encode_result.output_size]));
            let duration = SAMPLES_PER_CHANNEL_PER_FRAGMENT as u32;
            let track_data = TrackData::Audio(adts.clone());
            let segment = video_ctx.mux.write_track(duration, &track_data);
            let _ = self.file.write_all(&segment);
            // only fails if no active receives:
            let _ = self.segments_tx.send(StreamSegment::Audio {
                duration: duration,
                frame: adts,
            });

            self.audio_pcm_buff.drain(0..audio_frame_sample_count);
        }

        // process incoming video
        // let fill_to = video.as_ref()
        //     .map(|frame| timestamp + frame.tick_offset)
        //     .unwrap_or(end_of_tick);

        // if self.filled_video_to < fill_to {
        //     let fill_duration = fill_to - self.filled_video_to;

        //     // round out to samples for video segment:
        //     let fill_duration = (fill_duration * SAMPLE_RATE as i64).to_integer();

        //     if let Some(frame) = self.previous_video_frame.clone() {
        //         let _ = self.segments_tx.send(StreamSegment::Video {
        //             duration: fill_duration as u32,
        //             frame: frame.clone(),
        //         });
        //     } else {
        //         let _ = self.segments_tx.send(StreamSegment::RawAvc {
        //             duration: fill_duration as u32,
        //             frame: FILL_FRAME.clone(),
        //         });
        //     }

        //     // and use the rounded samples as numerator in new fraction to ensure precision:
        //     self.filled_video_to += Rational64::new(fill_duration, 44100);
        // }

        if let Some(video_frame) = video {
            // TODO keep a running total of durations/frame timestamps and correct
            // for compounding inaccuracy over time:
            let duration = (video_frame.data.duration_hint * SAMPLE_RATE as i64).to_integer() as u32;

            let mut frame = video_frame.data.decoded.clone();
            frame.set_picture_type(mixlab_codec::ffmpeg::sys::AVPictureType_AV_PICTURE_TYPE_I);
            video_ctx.codec.send_frame(frame).unwrap();

            let video_packet = video_ctx.codec.recv_packet().unwrap();

            println!("{:?}", &video_packet.data()[0..16]);

            let track_data = TrackData::Video(av_packet_to_mp4(video_frame.data.is_key_frame(), &video_packet));
            let segment = video_ctx.mux.write_track(duration, &track_data);
            let _ = self.file.write_all(&segment);

            // only fails if no active receives:
            // let _ = self.segments_tx.send(StreamSegment::Video {
            //     duration: duration,
            //     frame: video_frame.data.clone(),
            // });

            // self.previous_video_frame = Some(video_frame.data.clone());
            // self.filled_video_to += Rational64::new(duration as i64, SAMPLE_RATE as i64);
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

fn av_packet_to_mp4(key_frame: bool, pkt: &mixlab_codec::ffmpeg::AvPacket) -> mp4::AvcFrame {
    // let Millis(composition_time_ms) = frame.composition_time;
    // let composition_time = (composition_time_ms * SAMPLE_RATE as u64) / 1000;

    mp4::AvcFrame {
        is_key_frame: key_frame,
        // composition_time: composition_time as u32,
        composition_time: pkt.composition_time(),
        data: Bytes::copy_from_slice(pkt.data()),
    }
}
