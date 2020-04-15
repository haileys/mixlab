use std::io;
use std::mem;

use bytes::{Bytes, Buf};
use derive_more::From;
use futures::future;
use futures::stream::{Stream, StreamExt};
use rml_rtmp::handshake::HandshakeError;
use rml_rtmp::sessions::{ServerSession, ServerSessionConfig, ServerSessionResult, ServerSessionError, ServerSessionEvent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, Sender};

use crate::codec::aac::Aac;
use crate::listen::PeekTcpStream;
use crate::source::{Registry, ConnectError, SourceSend};

mod adts;
mod incoming;

use adts::{AudioSpecificConfiguration, AudioDataTransportStream};

lazy_static::lazy_static! {
    static ref MOUNTPOINTS: Registry = {
        let mut reg = Registry::new();
        mem::forget(reg.listen("my_stream_endpoint"));
        reg
    };
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

    let (audio_tx, audio_rx) = mpsc::channel(1);

    let mut ctx = ReceiveContext {
        stream,
        session,
        audio_tx,
    };

    // authenticate client
    let source = match publish {
        Some(publish) => {
            println!("rtmp: client wants to publish on {:?} with stream_key {:?}",
                publish.app_name, publish.stream_key);

            // TODO handle stream keys

            let source = MOUNTPOINTS.connect(&publish.app_name)?;
            let results = ctx.session.accept_request(publish.request_id)?;

            for result in results {
                if let ServerSessionResult::OutboundResponse(resp) = result {
                    ctx.stream.write_all(&resp.bytes).await?;
                } else {
                    // accept_request never returns any variant of ServerSessionResult other than OutboundReponse:
                    panic!("rtmp: unexpected result from accept_request: {:?}", result);
                }
            }

            source
        }
        None => return Ok(())
    };

    tokio::spawn(async move {
        let aac_packets = audio_rx
            .map(|packet| classify_audio_packet(packet))
            .filter_map({
                let mut current_asc = None;

                move |packet| {
                    future::ready(match packet {
                        AudioPacket::AacSequenceHeader(mut bytes) => {
                            if let Some(asc) = AudioSpecificConfiguration::try_from_buf(&mut bytes).ok() {
                                current_asc = Some(asc);
                            }
                            None
                        }
                        AudioPacket::AacRawData(bytes) => {
                            if let Some(asc) = current_asc.clone() {
                                let adts = AudioDataTransportStream::new(bytes, asc);
                                let packet: Bytes = adts.into();
                                Some(packet)
                            } else {
                                eprintln!("rtmp: received aac data packet before sequence header, dropping");
                                None
                            }
                        }
                        AudioPacket::Unknown(bytes) => {
                            println!("rtmp: received unknown audio packet, dropping");
                            None
                        }
                    })
                }
            })
            .map(Ok);

        let reader = tokio::io::stream_reader(aac_packets);
        let aac = Aac::new(reader).await.unwrap();

        println!("**** HERE *****");
    });

    match run_receive(&mut ctx, buff).await {
        Ok(()) => {}
        Err(e) => {
            eprintln!("rtmp: client error, dropping: {:?}", e);
        }
    }

    unimplemented!()
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
        ServerSessionEvent::AudioDataReceived { app_name, stream_key, data, timestamp } => {
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
