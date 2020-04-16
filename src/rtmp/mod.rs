use std::io;
use std::mem;
use std::sync::Arc;
use std::thread;

use bytes::Bytes;
use derive_more::From;
use faad2::Decoder;
use futures::executor::block_on;
use futures::stream::{self, StreamExt};
use rml_rtmp::time::RtmpTimestamp;
use rml_rtmp::handshake::HandshakeError;
use rml_rtmp::sessions::{ServerSession, ServerSessionResult, ServerSessionError, ServerSessionEvent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, Sender, Receiver};

use crate::codec::avc::{DecoderConfigurationRecord, Bitstream};
use crate::listen::PeekTcpStream;
use crate::source::{Registry, ConnectError, SourceRecv, SourceSend, ListenError};

mod incoming;
mod packet;

use packet::{AudioPacket, VideoPacket, VideoPacketError, AvcPacketType};

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

#[derive(From, Debug)]
pub enum RtmpError {
    Io(io::Error),
    Handshake(HandshakeError),
    Session(ServerSessionError),
    SourceConnect(ConnectError),
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

    let (audio_tx, audio_rx) = mpsc::channel(1);
    let (video_tx, video_rx) = mpsc::channel(1);

    let mut ctx = ReceiveContext {
        stream,
        session,
        audio_tx,
        video_tx,
    };

    thread::spawn(move || {
        run_decode_thread(audio_rx, video_rx, source);
    });

    run_receive(&mut ctx, buff).await?;

    Ok(())
}

fn run_decode_thread(audio_rx: Receiver<(Bytes, RtmpTimestamp)>, video_rx: Receiver<(Bytes, RtmpTimestamp)>, mut source: SourceSend) {
    enum Packet {
        Audio(AudioPacket),
        Video(Result<VideoPacket, VideoPacketError>),
    }

    let aac_packets = audio_rx.map(|(packet, _)| AudioPacket::parse(packet));
    let avc_packets = video_rx.map(|(packet, _)| VideoPacket::parse(packet));
    let mut packets = stream::select(
        aac_packets.map(Packet::Audio),
        avc_packets.map(Packet::Video),
    );

    let mut audio_codec = None;
    let mut video_dcr = None;

    use std::io::Write;
    let mut video_dump = std::fs::File::create("dump.h264").unwrap();

    while let Some(packet) = block_on(packets.next()) {
        match packet {
            Packet::Audio(AudioPacket::AacSequenceHeader(bytes)) => {
                if audio_codec.is_some() {
                    eprintln!("rtmp: received second aac sequence header?");
                }

                // TODO - validate user input before passing it to faad2
                audio_codec = Some(Decoder::new(&bytes).expect("Decoder::new"));
            }
            Packet::Audio(AudioPacket::AacRawData(bytes)) => {
                if let Some(codec) = &mut audio_codec {
                    let decode_info = codec.decode(&bytes).expect("codec.decode");
                    match source.write(decode_info.samples) {
                        Ok(_) => {}
                        Err(_) => { break; }
                    }
                } else {
                    eprintln!("rtmp: received aac data packet before sequence header, dropping");
                }
            }
            Packet::Audio(AudioPacket::Unknown(_)) => {
                eprintln!("rtmp: received unknown audio packet, dropping");
            }
            Packet::Video(Ok(mut packet)) => {
                if let AvcPacketType::SequenceHeader = packet.avc_packet_type {
                    match DecoderConfigurationRecord::parse(&mut packet.data) {
                        Ok(dcr) => {
                            if video_dcr.is_some() {
                                if audio_codec.is_some() {
                                    eprintln!("rtmp: received second avc sequence header?");
                                }
                            }
                            eprintln!("rtmp: received avc dcr: {:?}", dcr);
                            video_dcr = Some(Arc::new(dcr));
                        }
                        Err(e) => {
                            eprintln!("rtmp: could not read avc dcr: {:?}", e);
                        }
                    }
                }

                // println!("packet timestamp: {:?}", packet.timestamp);

                if let Some(dcr) = video_dcr.clone() {
                    match Bitstream::parse(packet.data, dcr) {
                        Ok(bitstream) => {
                            video_dump.write_all(&bitstream.try_as_bytes().unwrap()).unwrap();
                            // do stuff!
                        }
                        Err(e) => {
                            eprintln!("rtmp: could not read avc bitstream: {:?}", e);
                        }
                    }
                } else {
                    eprintln!("rtmp: cannot read avc frame without dcr");
                }

                // println!("frame_type: {:?}, avc_packet_type: {:?}, composition_time: {:?}",
                //     packet.frame_type, packet.avc_packet_type, packet.composition_time);

                // println!("data: (len = {:8}) {:x?}", packet.data.len(), &packet.data[0..32]);

                // match packet.avc_packet_type {
                //     AvcPacketType::SequenceHeader => {}
                //     AvcPacketType::EndOfSequence => {}
                //     _ => {
                //         video_dump.write_all(&packet.data).unwrap();
                //     }
                // }
            }
            Packet::Video(Err(e)) => {
                eprintln!("rtmp: received unknown video packet: {:?}", e);
            }
        }
    }
}

struct ReceiveContext {
    stream: PeekTcpStream,
    session: ServerSession,
    audio_tx: Sender<(Bytes, RtmpTimestamp)>,
    video_tx: Sender<(Bytes, RtmpTimestamp)>,
}

#[derive(From)]
enum ReceiveError {
    Rtmp(RtmpError),
    Exit,
}

async fn run_receive(ctx: &mut ReceiveContext, mut buff: Vec<u8>) -> Result<(), RtmpError> {
    loop {
        match ctx.stream.read(&mut buff).await? {
            0 => {
                return Ok(());
            }
            bytes => {
                let actions = ctx.session.handle_input(&buff[0..bytes])?;
                match handle_session_results(ctx, actions).await {
                    Ok(()) => {}
                    Err(ReceiveError::Exit) => {
                        return Ok(());
                    }

                    Err(ReceiveError::Rtmp(e)) => {
                        return Err(e);
                    }
                }
            }
        }
    }
}

async fn handle_session_results(
    ctx: &mut ReceiveContext,
    actions: Vec<ServerSessionResult>,
) -> Result<(), ReceiveError> {
    for action in actions {
        match action {
            ServerSessionResult::OutboundResponse(packet) => {
                ctx.stream.write_all(&packet.bytes).await
                    .map_err(RtmpError::from)?;
            }
            ServerSessionResult::RaisedEvent(ev) => {
                handle_event(ctx, ev).await?;
            }
            ServerSessionResult::UnhandleableMessageReceived(msg) => {
                println!("rtmp: UnhandleableMessageReceived: {:?}", msg);
            }
        }
    }

    Ok(())
}

async fn handle_event(
    ctx: &mut ReceiveContext,
    event: ServerSessionEvent,
) -> Result<(), ReceiveError> {
    match event {
        ServerSessionEvent::AudioDataReceived { app_name: _, stream_key: _, data, timestamp } => {
            ctx.audio_tx.send((data, timestamp))
                .await
                .map_err(|_| ReceiveError::Exit)?;

            Ok(())
        }
        ServerSessionEvent::VideoDataReceived { data, timestamp, .. } => {
            ctx.video_tx.send((data, timestamp))
                .await
                .map_err(|_| ReceiveError::Exit)?;

            Ok(())
        }
        _ => {
            println!("unknown event received: {:?}", event);
            Ok(())
        }
    }
}

