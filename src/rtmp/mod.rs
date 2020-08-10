use std::io;
use std::mem;
use std::thread;

use bytes::Bytes;
use derive_more::From;
use futures::executor::block_on;
use num_rational::{Rational32, Rational64};
use rml_rtmp::handshake::HandshakeError;
use rml_rtmp::sessions::{ServerSession, ServerSessionResult, ServerSessionError, ServerSessionEvent};
use rml_rtmp::time::RtmpTimestamp;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use mixlab_codec::aac;
use mixlab_codec::ffmpeg::codec::{self, CodecBuilder, DecodeContext, RecvFrameError};
use mixlab_codec::ffmpeg::{AvError, AvPacketRef, PacketInfo};
use mixlab_util::time::{MediaDuration, MediaTime};

use crate::listen::PeekTcpStream;
use crate::source::{Registry, ConnectError, SourceRecv, SourceSend, ListenError};
use crate::video;

pub mod client;
pub mod incoming;
pub mod packet;

use packet::{AudioPacket, VideoPacket, VideoPacketType};

lazy_static::lazy_static! {
    static ref MOUNTPOINTS: Registry = {
        let reg = Registry::new();
        mem::forget(reg.listen("my_stream_endpoint"));
        reg
    };
}

pub fn listen(mountpoint: &str) -> Result<SourceRecv, ListenError> {
    MOUNTPOINTS.listen(mountpoint)
}

pub const TIME_BASE: i32 = 1000;

#[derive(From, Debug)]
pub enum RtmpError {
    Io(io::Error),
    Handshake(HandshakeError),
    Session(ServerSessionError),
    SourceConnect(ConnectError),
    MetadataNotYetSent,
    UnsupportedStream,
    SourceSend,
    Aac(aac::AacError),
    AacCodec(fdk_aac::dec::DecoderError),
    CodecOpen(codec::OpenError),
    AvCodec(AvError),
}

pub async fn accept(mut stream: PeekTcpStream) -> Result<(), RtmpError> {
    let mut buff = vec![0u8; 4096];

    let (_, remaining_bytes) = incoming::handshake(&mut stream, &mut buff).await?;
    let mut session = incoming::setup_session(&mut stream).await?;
    let publish = incoming::handle_new_client(&mut stream, &mut session, remaining_bytes, &mut buff).await?;

    let source = match publish {
        Some(publish) => {
            println!("rtmp: client wants to publish on {:?} with stream_key {:?}",
                publish.app_name, publish.stream_key);

            // TODO handle stream keys

            let source = MOUNTPOINTS.connect(&publish.app_name)?;

            incoming::accept_publish(&mut stream, &mut session, &publish).await?;

            source
        }
        None => { return Ok(()); }
    };

    let mut audio_codec = fdk_aac::dec::Decoder::new(fdk_aac::dec::Transport::Adts);

    // enable automatic stereo mix-down:
    audio_codec.set_min_output_channels(2)?;
    audio_codec.set_max_output_channels(2)?;

    let mut ctx = ReceiveContext {
        stream,
        session,
        source,
        meta: None,
        audio_codec,
        audio_asc: None,
        audio_timestamp: MediaTime::new(0, 1),
        video_codec: None,
    };

    thread::spawn(move || {
        run_receive_thread(&mut ctx, buff)
    });

    Ok(())
}

struct ReceiveContext {
    stream: PeekTcpStream,
    session: ServerSession,
    source: SourceSend,
    meta: Option<StreamMeta>,
    audio_codec: fdk_aac::dec::Decoder,
    audio_asc: Option<aac::AudioSpecificConfiguration>,
    audio_timestamp: MediaTime,
    video_codec: Option<DecodeContext>,
}

struct StreamMeta {
    video_frame_duration: MediaDuration,
}

fn run_receive_thread(ctx: &mut ReceiveContext, mut buff: Vec<u8>) -> Result<(), RtmpError> {
    loop {
        match block_on(ctx.stream.read(&mut buff))? {
            0 => {
                return Ok(());
            }
            bytes => {
                let actions = ctx.session.handle_input(&buff[0..bytes])?;
                handle_session_results(ctx, actions)?;
            }
        }
    }
}

fn handle_session_results(
    ctx: &mut ReceiveContext,
    actions: Vec<ServerSessionResult>,
) -> Result<(), RtmpError> {
    for action in actions {
        match action {
            ServerSessionResult::OutboundResponse(packet) => {
                block_on(ctx.stream.write_all(&packet.bytes))?;
            }
            ServerSessionResult::RaisedEvent(ev) => {
                handle_event(ctx, ev)?;
            }
            ServerSessionResult::UnhandleableMessageReceived(msg) => {
                println!("rtmp: UnhandleableMessageReceived: {:?}", msg);
            }
        }
    }

    Ok(())
}

fn handle_event(
    ctx: &mut ReceiveContext,
    event: ServerSessionEvent,
) -> Result<(), RtmpError> {
    match event {
        ServerSessionEvent::AudioDataReceived { app_name: _, stream_key: _, data, timestamp } => {
            receive_audio_packet(ctx, data, timestamp)?;
            Ok(())
        }
        ServerSessionEvent::VideoDataReceived { data, timestamp, .. } => {
            receive_video_packet(ctx, data, timestamp)?;
            Ok(())
        }
        ServerSessionEvent::StreamMetadataChanged { app_name: _, stream_key: _, metadata } => {
            let video_frame_duration =
                if let Some(frame_rate) = metadata.video_frame_rate {
                    let frame_rate = Rational64::new((frame_rate * TIME_BASE as f32) as i64, TIME_BASE.into());
                    MediaDuration::from(frame_rate.recip())
                } else {
                    eprintln!("rtmp: no frame rate in metadata");
                    return Err(RtmpError::UnsupportedStream);
                };

            ctx.meta = Some(StreamMeta {
                video_frame_duration,
            });

            Ok(())
        }
        _ => {
            println!("unknown event received: {:?}", event);
            Ok(())
        }
    }
}

fn receive_audio_packet(
    ctx: &mut ReceiveContext,
    data: Bytes,
    _timestamp: RtmpTimestamp,
) -> Result<(), RtmpError> {
    let packet = AudioPacket::parse(data);

    match packet {
        Ok(AudioPacket::AacSequenceHeader(bytes)) => {
            let asc = aac::AudioSpecificConfiguration::parse(bytes)?;
            ctx.audio_asc = Some(asc);
        }
        Ok(AudioPacket::AacRawData(bytes)) => {
            let asc = if let Some(asc) = &ctx.audio_asc {
                asc
            } else {
                eprintln!("rtmp: received aac data packet before sequence header, dropping");
                return Ok(());
            };

            // AAC standard defines a frame to be 1024 samples per channel:
            let mut pcm_buffer = vec![0; 2048];

            let adts = aac::AudioDataTransportStream::new(bytes, asc);
            let adts_bytes = adts.into_bytes();

            let bytes_consumed = ctx.audio_codec.fill(&adts_bytes).unwrap();

            if bytes_consumed < adts_bytes.len() {
                eprintln!("rtmp: codec did not read all bytes from audio packet");
                return Ok(());
            }

            match ctx.audio_codec.decode_frame(&mut pcm_buffer) {
                Ok(()) => {
                    let sample_rate = ctx.audio_codec.stream_info().sampleRate;

                    if sample_rate != 44100 {
                        // TODO fix me
                        panic!("expected stream sample rate to be 44100");
                    }

                    let frame_time = MediaDuration::new(pcm_buffer.len() as i64 / 2, sample_rate as i64);

                    pcm_buffer.truncate(ctx.audio_codec.decoded_frame_size());
                    // println!("decoded frame! timestamp: {:?}, frame size: {}", timestamp, pcm_buffer.len());

                    // TODO do we use ctx.audio_timestamp or the rtmp timestamp here?

                    ctx.source.write_audio(ctx.audio_timestamp, pcm_buffer)
                        .map_err(|()| RtmpError::SourceSend)?;

                    ctx.audio_timestamp += frame_time;
                }
                Err(e) => {
                    eprintln!("rtmp: audio codec frame decode error: {:?}", e);
                    return Ok(());
                }
            }
        }
        Err(e) => {
            eprintln!("rtmp: could not parse audio packet ({:?}), dropping", e);
        }
    }

    Ok(())
}

fn receive_video_packet(
    ctx: &mut ReceiveContext,
    data: Bytes,
    timestamp: RtmpTimestamp,
) -> Result<(), RtmpError> {
    let meta = ctx.meta.as_ref().ok_or(RtmpError::MetadataNotYetSent)?;

    let packet = match VideoPacket::parse(data) {
        Ok(packet) => packet,
        Err(e) => {
            println!("rtmp: could not parse video packet ({:?}), dropping", e);
            return Ok(());
        }
    };

    match packet.packet_type {
        VideoPacketType::SequenceHeader => {
            let time_base = Rational32::new(1, TIME_BASE);

            let decode = CodecBuilder::h264(time_base)
                // h264 extradata is the decoder configuration record:
                .with_extradata(&packet.data)
                // use avcc encoding (length-prefixed NALs) rather than default of annex-b:
                .with_opt("is_avc", "1")
                .open_decoder()?;

            ctx.video_codec = Some(decode);
        }
        VideoPacketType::Nalu => {
            let codec = match ctx.video_codec.as_mut() {
                Some(codec) => codec,
                None => {
                    // nothing we can do with this nalu until receiving dcr
                    // drop packet
                    return Ok(());
                }
            };

            // TODO rtmp timestamps are only 32 bit and have arbitrary
            // user-defined epochs - we need to handle rollover
            let dts = timestamp.value as i64;
            let pts = dts + packet.composition_time as i64;

            let av_packet = AvPacketRef::borrowed(PacketInfo {
                dts,
                pts,
                data: &packet.data,
            });

            codec.send_packet(&av_packet)
                .expect("avc::decode::send_packet in rtmp");

            loop {
                match codec.recv_frame() {
                    Ok(decoded) => {
                        let timestamp = MediaTime::new(decoded.presentation_timestamp(), TIME_BASE.into());

                        let frame = video::Frame {
                            decoded: decoded,
                            duration_hint: meta.video_frame_duration,
                        };

                        let _ = ctx.source.write_video(timestamp, frame);
                    }
                    Err(RecvFrameError::NeedMoreInput) => break,
                    Err(RecvFrameError::Eof) => panic!("EOF should never happen"),
                    Err(RecvFrameError::Codec(e)) => {
                        return Err(RtmpError::AvCodec(e));
                    }
                }
            }
        }
        VideoPacketType::EndOfSequence => {
            // do nothing
        }
    }

    Ok(())
}
