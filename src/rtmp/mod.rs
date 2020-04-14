use std::io::{self, Read};
use std::mem;
use std::thread;
use std::time::{Instant, Duration};

use derive_more::From;
use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, HandshakeError, PeerType};
use rml_rtmp::sessions::{ServerSession, ServerSessionConfig, ServerSessionResult, ServerSessionError};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use crate::listen::PeekTcpStream;

#[derive(From, Debug)]
pub enum RtmpError {
    Io(io::Error),
    Handshake(HandshakeError),
    Session(ServerSessionError),
}

pub async fn accept(mut stream: PeekTcpStream) -> Result<(), RtmpError> {
    let mut buff = vec![0u8; 4096];

    let (_, remaining_bytes) = handshake(&mut stream, &mut buff).await?;
    let mut session = setup_session(&mut stream).await?;

    // handle any bytes read after handshake that have not yet been processed
    handle_input(&mut stream, &mut session, &remaining_bytes).await?;
    mem::drop(remaining_bytes);

    loop {
        let bytes = stream.read(&mut buff).await?;
        handle_input(&mut stream, &mut session, &buff[0..bytes]).await?;
    }

    Ok(())
}

async fn handshake(stream: &mut PeekTcpStream, buff: &mut [u8]) -> Result<(Handshake, Vec<u8>), RtmpError> {
    println!("RTMP incoming!");

    let mut handshake = Handshake::new(PeerType::Server);

    loop {
        let bytes = stream.read(buff).await?;

        match handshake.process_bytes(&buff[0..bytes])? {
            HandshakeProcessResult::InProgress { response_bytes } => {
                stream.write_all(&response_bytes).await?;
            }
            HandshakeProcessResult::Completed { response_bytes, remaining_bytes } => {
                stream.write_all(&response_bytes).await?;
                return Ok((handshake, remaining_bytes));
            }
        }
    }
}

async fn setup_session(stream: &mut PeekTcpStream) -> Result<ServerSession, RtmpError> {
    // go with defaults for now. TODO investigate whether any should be changed
    // - specifically peer_bandwidth
    let session_config = ServerSessionConfig::new();

    let (session, results) = ServerSession::new(session_config)?;

    // send initial packets from session setup
    for result in results {
        match result {
            ServerSessionResult::OutboundResponse(packet) => {
                stream.write_all(&packet.bytes).await?;
            }
            ServerSessionResult::RaisedEvent(_) |
            ServerSessionResult::UnhandleableMessageReceived(_) => {
                // can never in ServerSession::new
                unreachable!();
            }
        }
    }

    Ok(session)
}

async fn handle_input(stream: &mut PeekTcpStream, session: &mut ServerSession, data: &[u8]) -> Result<(), RtmpError> {
    let results = session.handle_input(data)?;

    for result in results {
        match result {
            ServerSessionResult::OutboundResponse(packet) => {
                stream.write_all(&packet.bytes).await?;
            }
            ServerSessionResult::RaisedEvent(ev) => {
                println!("rtmp: RaisedEvent: {:?}", ev);
            }
            ServerSessionResult::UnhandleableMessageReceived(msg) => {
                println!("rtmp: UnhandleableMessageReceived: {:?}", msg);
            }
        }
    }

    Ok(())
}
