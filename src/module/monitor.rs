use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::iter;
use std::sync::{Arc, Mutex};

use bytes::{Bytes, BytesMut, Buf, BufMut};
use bytes::buf::BufMutExt;
use fdk_aac::enc as aac;
use futures::sink::SinkExt;
use num_rational::Rational64;
use tokio::sync::{broadcast, watch, mpsc};
use uuid::Uuid;
use warp::ws::{self, WebSocket};

use mixlab_codec::avc::{self, Millis, AvcEncoder};
use mixlab_mux::mp4::{self, Mp4Mux, Mp4Params, TrackData, AdtsFrame};
use mixlab_protocol::{LineType, Terminal, MonitorIndication, MonitorTransportPacket};

use crate::engine::{InputRef, OutputRef, SAMPLE_RATE};
use crate::module::ModuleT;
use crate::video::{self, FrameId};

lazy_static::lazy_static! {
    static ref SOCKETS: Mutex<HashMap<Uuid, Stream>> = Mutex::new(HashMap::new());
}

struct Stream {
    live: Arc<broadcast::Sender<StreamSegment>>,
}

#[derive(Clone)]
enum StreamSegment {
    Audio { duration: u32, frame: mp4::AdtsFrame },
    Video { duration: u32, frame: Arc<video::Frame> },
    RawAvc { duration: u32, frame: mp4::AvcFrame },
}

pub async fn stream(socket_id: Uuid, mut client: WebSocket) -> Result<(), ()> {
    let mut stream = (*SOCKETS).lock()
        .unwrap()
        .get(&socket_id)
        .map(|stream| stream.live.subscribe())
        .ok_or(())?;

    let mut active_key_frame: Option<FrameId> = None;

    // TODO if we lag we should catch up to the start of the stream rather
    // than disconnecting the client
    while let Ok(segment) = stream.recv().await {
        match segment {
            StreamSegment::Audio { duration, frame } => {
                // only send audio data if we've sent init packet
                // init packet is sent on receipt of the first video frame, at
                // which point active_key_frame will be Some:
                if active_key_frame.is_some() {
                    send_packet(&mut client, MonitorTransportPacket::Frame {
                        duration,
                        track_data: TrackData::Audio(frame.clone()),
                    }).await?;
                }
            }
            StreamSegment::Video { duration, frame } => {
                if active_key_frame.is_none() {
                    // send DCR if this is the first video frame received
                    let dcr = frame.specific.bitstream.dcr.clone();
                    let dcr_bytes = {
                        let mut buff = Vec::new();
                        dcr.write_to(&mut buff);
                        buff
                    };

                    send_packet(&mut client, MonitorTransportPacket::Init {
                        params: Mp4Params {
                            timescale: SAMPLE_RATE as u32,
                            width: dcr.picture_width() as u32,
                            height: dcr.picture_height() as u32,
                            dcr: Cow::Owned(dcr_bytes),
                        },
                    }).await?;
                }

                if frame.is_key_frame() {
                    active_key_frame = Some(frame.id());
                } else {
                    if let Some(key_frame) = &frame.key_frame {
                        if active_key_frame != Some(key_frame.id()) {
                            send_packet(&mut client, MonitorTransportPacket::Frame {
                                duration: 0,
                                track_data: TrackData::Video(avc_frame_to_mp4(&key_frame.specific)),
                            }).await?;

                            active_key_frame = Some(key_frame.id());
                        }
                    }
                }

                send_packet(&mut client, MonitorTransportPacket::Frame {
                    duration,
                    track_data: TrackData::Video(avc_frame_to_mp4(&frame.specific)),
                }).await?;
            }
            StreamSegment::RawAvc { duration, frame } => {
                if active_key_frame.is_some() {
                    send_packet(&mut client, MonitorTransportPacket::Frame {
                        duration,
                        track_data: TrackData::Video(frame),
                    }).await?;
                }
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

static FILLER_BYTES: &[u8] = &[0, 0, 0, 1, 12];

static FILL_FRAME: mp4::AvcFrame = mp4::AvcFrame {
    is_key_frame: false,
    composition_time: 0,
    data: Bytes::from_static(FILLER_BYTES),
};

#[derive(Debug)]
struct AacFrame {
    data: Vec<u8>,
    timestamp: u64,
}

#[derive(Debug)]
pub struct Monitor {
    mux: Option<Mp4Mux>,
    socket_id: Uuid,
    segments_tx: Arc<broadcast::Sender<StreamSegment>>,
    file: File,
    engine_epoch: Option<u64>,
    aac: aac::Encoder,
    audio_pcm_buff: Vec<i16>,
    video_codec: AvcEncoder,
    filled_video_to: Rational64,
    previous_video_frame: Option<Arc<video::Frame>>,
    inputs: Vec<Terminal>,
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

        let video_codec = AvcEncoder::new(SAMPLE_RATE).unwrap();

        let file = File::create("dump.mp4").unwrap();

        let module = Monitor {
            mux: None,
            socket_id,
            segments_tx,
            file,
            engine_epoch: None,
            aac: aac,
            audio_pcm_buff: Vec::new(),
            video_codec: video_codec,
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

        // initialise muxer and write init segment
        let mux = match (self.mux.as_mut(), video) {
            (Some(mux), _) => mux,
            (None, None) => {
                // init segment requires information from the DCR, so we can't do
                // anything until we've received the first video frame
                return None;
            }
            (None, Some(frame)) => {
                let dcr = &frame.data.specific.bitstream.dcr;
                let dcr_bytes = {
                    let mut buff = Vec::new();
                    dcr.write_to(&mut buff);
                    buff
                };

                let (mux, init) = Mp4Mux::new(Mp4Params {
                    timescale: SAMPLE_RATE as u32,
                    width: dcr.picture_width() as u32,
                    height: dcr.picture_height() as u32,
                    dcr: Cow::Owned(dcr_bytes),
                });

                println!("writing init ({} bytes)", init.len());
                self.file.write_all(&init).unwrap();

                self.mux = Some(mux);
                self.mux.as_mut().unwrap()
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
            let segment = mux.write_track(duration, &track_data);
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

            let encoded = self.video_codec.send_frame(video_frame.data.decoded.clone());

            println!("encoded: {:#?}", encoded);

            let track_data = TrackData::Video(avc_frame_to_mp4(&video_frame.data.specific));
            let segment = mux.write_track(duration, &track_data);
            let _ = self.file.write_all(&segment);

            // only fails if no active receives:
            let _ = self.segments_tx.send(StreamSegment::Video {
                duration: duration,
                frame: video_frame.data.clone(),
            });

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

fn avc_frame_to_mp4(frame: &avc::AvcFrame) -> mp4::AvcFrame {
    let Millis(composition_time_ms) = frame.composition_time;
    let composition_time = (composition_time_ms * SAMPLE_RATE as u64) / 1000;

    mp4::AvcFrame {
        is_key_frame: frame.frame_type.is_key_frame(),
        composition_time: composition_time as u32,
        data: {
            let mut data = BytesMut::new();
            frame.bitstream.write_to(&mut data);
            data.freeze()
        }
    }
}

//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////

#[cfg(feature="mpeg_ts")]
mod ts {

use mpeg2ts::es::StreamId;
use mpeg2ts::time::Timestamp as MpegTimestamp;
use mpeg2ts::ts::{self, TsPacketWriter, TsPacket, TsHeader, TsPayload, ContinuityCounter, Pid, WriteTsPacket};

const VIDEO_ES_PID: u16 = 257;
const AUDIO_ES_PID: u16 = 258;

// see https://en.wikipedia.org/wiki/Packetized_elementary_stream
const PES_VIDEO_STREAM_ID: u8 = 0xe0;
const PES_AUDIO_STREAM_ID: u8 = 0xc0;

fn ts_header(pid: u16, continuity_counter: ContinuityCounter) -> TsHeader {
    use mpeg2ts::ts::TransportScramblingControl;

    TsHeader {
        transport_error_indicator: false,
        transport_priority: false,
        pid: Pid::new(pid).unwrap(),
        transport_scrambling_control: TransportScramblingControl::NotScrambled,
        continuity_counter,
    }
}

fn write_audio(ctx: &mut WriteCtx, mut data: &[u8], timestamp: Rational64) -> Result<(), mpeg2ts::Error> {
    use mpeg2ts::pes::PesHeader;
    use mpeg2ts::ts::payload::Bytes;

    let first_payload_len = cmp::min(153, data.len());
    let (first_payload, rest) = data.split_at(first_payload_len);
    data = rest;

    let millis = (timestamp * 1000).to_integer() as u64;

    ctx.ts.write_ts_packet(&TsPacket {
        header: ts_header(AUDIO_ES_PID, ctx.audio_continuity_counter.clone()),
        adaptation_field: None,
        payload: Some(TsPayload::Pes(ts::payload::Pes {
            header: PesHeader {
                stream_id: StreamId::new(PES_AUDIO_STREAM_ID),
                priority: false,
                data_alignment_indicator: false,
                copyright: false,
                original_or_copy: false,
                pts: Some(MpegTimestamp::new(millis * 90)?), // pts is in 90hz or something?
                dts: None,
                escr: None,
            },
            pes_packet_len: 0,
            data: Bytes::new(first_payload)?,
        })),
    })?;

    ctx.audio_continuity_counter.increment();

    while data.len() > 0 {
        let payload_len = cmp::min(Bytes::MAX_SIZE, data.len());
        let (payload, rest) = data.split_at(payload_len);
        data = rest;

        ctx.ts.write_ts_packet(&TsPacket {
            header: ts_header(AUDIO_ES_PID, ctx.audio_continuity_counter.clone()),
            adaptation_field: None,
            payload: Some(TsPayload::Raw(Bytes::new(&payload)?)),
        })?;

        ctx.audio_continuity_counter.increment();
    }

    Ok(())
}

#[derive(From, Debug)]
enum WriteVideoError {
    Mpeg2Ts(mpeg2ts::Error),
    Avc(AvcError),
}

fn write_video(ctx: &mut WriteCtx, frame: &AvcFrame, timestamp: Rational64) -> Result<(), WriteVideoError> {
    use mpeg2ts::es::StreamId;
    use mpeg2ts::pes::PesHeader;
    use mpeg2ts::time::ClockReference;
    use mpeg2ts::ts::AdaptationField;
    use mpeg2ts::ts::payload::{Bytes, Pes};

    let frame_data_timestamp = timestamp + frame.tick_offset;
    let frame_presentation_timestamp = frame_data_timestamp + Rational64::new(frame.data.composition_time.0 as i64, 1000);

    let frame_data_millis = (frame_data_timestamp * 1000).to_integer() as u64;
    let frame_presentation_millis = (frame_presentation_timestamp * 1000).to_integer() as u64;

    let mut buf = frame.data.bitstream.into_bytes()?;
    let pes_data_len = cmp::min(153, buf.remaining());
    let pes_data = buf.split_to(pes_data_len);
    let pcr = ClockReference::new(frame_data_millis * 90)?;

    let adaptation_field = if frame.data.frame_type.is_key_frame() {
        Some(AdaptationField {
            discontinuity_indicator: false,
            random_access_indicator: true,
            es_priority_indicator: false,
            pcr: Some(pcr),
            opcr: None,
            splice_countdown: None,
            transport_private_data: Vec::new(),
            extension: None,
        })
    } else {
        None
    };

    let pts = MpegTimestamp::new(frame_presentation_millis * 90)?;
    let dts = MpegTimestamp::new(frame_data_millis * 90)?;

    let packet = TsPacket {
        header: ts_header(VIDEO_ES_PID, ctx.video_continuity_counter.clone()),
        adaptation_field,
        payload: Some(TsPayload::Pes(Pes {
            header: PesHeader {
                stream_id: StreamId::new(PES_VIDEO_STREAM_ID),
                priority: false,
                data_alignment_indicator: false,
                copyright: false,
                original_or_copy: false,
                pts: Some(pts),
                dts: Some(dts),
                escr: None,
            },
            pes_packet_len: 0,
            data: Bytes::new(&pes_data)?,
        })),
    };

    ctx.ts.write_ts_packet(&packet)?;
    ctx.video_continuity_counter.increment();

    while buf.remaining() > 0 {
        let pes_data_len = cmp::min(Bytes::MAX_SIZE, buf.remaining());
        let pes_data = buf.split_to(pes_data_len);

        let packet = TsPacket {
            header: ts_header(VIDEO_ES_PID, ctx.video_continuity_counter.clone()),
            adaptation_field: None,
            payload: Some(TsPayload::Raw(Bytes::new(&pes_data)?)),
        };

        ctx.ts.write_ts_packet(&packet)?;
        ctx.video_continuity_counter.increment();
    }

    Ok(())
}

}
