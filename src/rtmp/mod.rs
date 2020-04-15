use std::io;
use std::mem;

use bytes::{Bytes, Buf};
use derive_more::From;
use faad2::Decoder;
use futures::stream::StreamExt;
use rml_rtmp::handshake::HandshakeError;
use rml_rtmp::sessions::{ServerSession, ServerSessionResult, ServerSessionError, ServerSessionEvent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, Sender};

use crate::listen::PeekTcpStream;
use crate::source::{Registry, ConnectError, SourceRecv, ListenError};

// mod adts;
mod incoming;

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

    let (mut now, mut source) = match publish {
        Some(publish) => {
            println!("rtmp: client wants to publish on {:?} with stream_key {:?}",
                publish.app_name, publish.stream_key);

            // TODO handle stream keys

            let source = MOUNTPOINTS.connect(&publish.app_name)?;
            let now = std::time::Instant::now();

            incoming::accept_publish(&mut stream, &mut session, &publish).await?;

            (Some(now), source)
        }
        None => { return Ok(()); }
    };

    let (audio_tx, audio_rx) = mpsc::channel(1);

    let mut ctx = ReceiveContext {
        stream,
        session,
        audio_tx,
    };

    tokio::spawn(async move {
        let mut aac_packets = audio_rx.map(|packet| classify_audio_packet(packet));

        let mut codec = None;

        while let Some(packet) = aac_packets.next().await {
            match packet {
                AudioPacket::AacSequenceHeader(bytes) => {
                    // TODO - validate user input before passing it to faad2
                    codec = Some(Decoder::new(&bytes).expect("Decoder::new"));
                }
                AudioPacket::AacRawData(bytes) => {
                    if let Some(now) = now.take() {
                        println!("took {} ms for initial data", (std::time::Instant::now() - now).as_millis());
                    }

                    if let Some(codec) = &mut codec {
                        let decode_info = codec.decode(&bytes).expect("codec.decode");
                        match source.write(decode_info.samples) {
                            Ok(_) => {}
                            Err(_) => { break; }
                        }
                    } else {
                        eprintln!("rtmp: received aac data packet before sequence header, dropping");
                    }
                }
                AudioPacket::Unknown(_) => {
                    eprintln!("rtmp: received unknown audio packet, dropping");
                }
            }
        }
    });

    run_receive(&mut ctx, buff).await?;

    Ok(())
}

struct ReceiveContext {
    stream: PeekTcpStream,
    session: ServerSession,
    audio_tx: Sender<Bytes>,
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
        ServerSessionEvent::AudioDataReceived { app_name: _, stream_key: _, data, timestamp: _ } => {
            ctx.audio_tx.send(data)
                .await
                .map_err(|_| ReceiveError::Exit)?;

            Ok(())
        }
        ServerSessionEvent::VideoDataReceived { .. } => {
            // ignore for now
            Ok(())
        }
        _ => {
            println!("unknown event received: {:?}", event);
            Ok(())
        }
    }
}

enum AudioPacket {
    AacSequenceHeader(Bytes),
    AacRawData(Bytes),
    Unknown(Bytes)
}

// See https://www.adobe.com/content/dam/acom/en/devnet/flv/video_file_format_spec_v10_1.pdf
// Section E.4.2.1 AUDIODATA for reference
fn classify_audio_packet(mut bytes: Bytes) -> AudioPacket {
    let original = bytes.clone();

    if bytes.len() >= 2 {
        let tag = bytes.get_u8();

        if tag == 0xaf {
            // AAC
            let packet_type = bytes.get_u8();

            if packet_type == 0 {
                return AudioPacket::AacSequenceHeader(bytes);
            } else if packet_type == 1 {
                return AudioPacket::AacRawData(bytes);
            }
        }
    }

    AudioPacket::Unknown(original)
}
