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
use crate::source::{Registry, ConnectError};

// mod adts;
mod incoming;

lazy_static::lazy_static! {
    static ref MOUNTPOINTS: Registry = {
        let reg = Registry::new();
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
    let _source = match publish {
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
        let mut aac_packets = audio_rx.map(|packet| classify_audio_packet(packet));

        let mut codec = None;

        use std::io::Write;
        let mut tmp = std::fs::File::create("tmp.raw").unwrap();

        while let Some(packet) = aac_packets.next().await {
            match packet {
                AudioPacket::AacSequenceHeader(bytes) => {
                    // TODO - validate user input before passing it to faad2
                    codec = Some(Decoder::new(&bytes).expect("Decoder::new"));
                }
                AudioPacket::AacRawData(bytes) => {
                    if let Some(codec) = &mut codec {
                        let decode_info = codec.decode(&bytes).expect("codec.decode");
                        for sample in decode_info.samples {
                            let _ = tmp.write_all(&sample.to_le_bytes());
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
