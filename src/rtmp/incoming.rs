use std::collections::VecDeque;
use std::mem;

use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, PeerType};
use rml_rtmp::sessions::{ServerSession, ServerSessionConfig, ServerSessionResult, ServerSessionEvent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::listen::PeekTcpStream;
use crate::rtmp::RtmpError;

pub async fn handshake(stream: &mut PeekTcpStream, buff: &mut [u8]) -> Result<(Handshake, Vec<u8>), RtmpError> {
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

pub async fn setup_session(stream: &mut PeekTcpStream) -> Result<ServerSession, RtmpError> {
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

pub struct PublishInfo {
    pub request_id: u32,
    pub app_name: String,
    pub stream_key: String,
}

pub async fn handle_new_client(
    stream: &mut PeekTcpStream,
    session: &mut ServerSession,
    initial_data: Vec<u8>,
    buff: &mut [u8],
) -> Result<Option<PublishInfo>, RtmpError> {
    // handle any bytes read after handshake that have not yet been processed
    let results = session.handle_input(&initial_data)?;

    if let Some(info) = handle_session_results(stream, session, results).await? {
        return Ok(Some(info));
    }

    mem::drop(initial_data);

    loop {
        let bytes = stream.read(buff).await?;

        if bytes == 0 {
            return Ok(None);
        }

        let results = session.handle_input(&buff[0..bytes])?;

        if let Some(info) = handle_session_results(stream, session, results).await? {
            return Ok(Some(info))
        }
    }
}

pub async fn accept_publish(
    stream: &mut PeekTcpStream,
    session: &mut ServerSession,
    publish: &PublishInfo,
) -> Result<(), RtmpError> {
    let results = session.accept_request(publish.request_id)?;

    for result in results {
        if let ServerSessionResult::OutboundResponse(resp) = result {
            stream.write_all(&resp.bytes).await?;
        } else {
            // accept_request never returns any variant of ServerSessionResult other than OutboundReponse:
            panic!("rtmp: unexpected result from accept_request: {:?}", result);
        }
    }

    Ok(())
}

async fn handle_session_results(stream: &mut PeekTcpStream, session: &mut ServerSession, actions: Vec<ServerSessionResult>) -> Result<Option<PublishInfo>, RtmpError> {
    let mut actions: VecDeque<_> = actions.into();
    let mut publish_info = None;

    while let Some(action) = actions.pop_front() {
        match action {
            ServerSessionResult::OutboundResponse(packet) => {
                stream.write_all(&packet.bytes).await?;
            }
            ServerSessionResult::RaisedEvent(ev) => {
                match handle_event(session, ev).await? {
                    Some(EventResult::Actions(new_actions)) => {
                        actions.extend(new_actions);
                    }
                    Some(EventResult::Publish(info)) => {
                        if publish_info.is_some() {
                            eprintln!("rtmp: received multiple publish stream requests, ignoring all but first");
                        }

                        publish_info = Some(info);
                    }
                    None => {}
                }
            }
            ServerSessionResult::UnhandleableMessageReceived(msg) => {
                println!("rtmp: UnhandleableMessageReceived: {:?}", msg);
            }
        }
    }

    Ok(publish_info)
}

enum EventResult {
    Actions(Vec<ServerSessionResult>),
    Publish(PublishInfo),
}

async fn handle_event(session: &mut ServerSession, event: ServerSessionEvent) -> Result<Option<EventResult>, RtmpError> {
    match event {
        ServerSessionEvent::ConnectionRequested { request_id, app_name } => {
            println!("rtmp: connection requested on {:?}, accepting", app_name);
            let action = session.accept_request(request_id)?;
            Ok(Some(EventResult::Actions(action)))
        }
        ServerSessionEvent::PublishStreamRequested { request_id, app_name, stream_key, mode: _ } => {
            let info = PublishInfo {
                request_id,
                app_name,
                stream_key,
            };

            Ok(Some(EventResult::Publish(info)))
        }
        _ => {
            eprintln!("rtmp: ignoring pre-publish event: {:?}", event);
            Ok(None)
        }
    }
}
