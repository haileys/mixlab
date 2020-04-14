use std::collections::VecDeque;
use std::io;
use std::mem;
use std::thread;

use derive_more::From;
use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, HandshakeError, PeerType};
use rml_rtmp::sessions::{ServerSession, ServerSessionConfig, ServerSessionResult, ServerSessionError, ServerSessionEvent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
    {
        let results = session.handle_input(&remaining_bytes)?;
        handle_session_results(&mut stream, &mut session, results).await?;
        mem::drop(remaining_bytes);
    }

    loop {
        let bytes = stream.read(&mut buff).await?;

        if bytes == 0 {
            return Ok(());
        }

        let results = session.handle_input(&buff[0..bytes])?;
        handle_session_results(&mut stream, &mut session, results).await?;
    }
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

struct UserRequest {

}

async fn handle_session_results(stream: &mut PeekTcpStream, session: &mut ServerSession, actions: Vec<ServerSessionResult>) -> Result<(), RtmpError> {
    let mut actions: VecDeque<_> = actions.into();

    while let Some(action) = actions.pop_front() {
        match action {
            ServerSessionResult::OutboundResponse(packet) => {
                stream.write_all(&packet.bytes).await?;
            }
            ServerSessionResult::RaisedEvent(ev) => {
                actions.extend(handle_event(session, &ev).await?);
            }
            ServerSessionResult::UnhandleableMessageReceived(msg) => {
                println!("rtmp: UnhandleableMessageReceived: {:?}", msg);
            }
        }
    }

    Ok(())
}

async fn handle_event(session: &mut ServerSession, event: &ServerSessionEvent) -> Result<Vec<ServerSessionResult>, RtmpError> {
    match event {
        ServerSessionEvent::ConnectionRequested { request_id, app_name } => {
            println!("rtmp: connection requested on {:?}, accepting", app_name);
            Ok(session.accept_request(*request_id)?)
        }
        ServerSessionEvent::PublishStreamRequested { request_id, app_name, stream_key, mode } => {
            println!("rtmp: publish requested on {:?}, stream_key {:?}, mode {:?}, accepting",
                app_name, stream_key, mode);
            Ok(session.accept_request(*request_id)?)
        }
        ServerSessionEvent::AudioDataReceived { app_name, stream_key, data, timestamp } => {
            Ok(Vec::new())
        }
        ServerSessionEvent::VideoDataReceived { app_name, stream_key, data, timestamp } => {
            Ok(Vec::new())
        }
        _ => {
            eprintln!("rtmp: unhandled event: {:?}", event);
            Ok(Vec::new())
        }
    }
}
