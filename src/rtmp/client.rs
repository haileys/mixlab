use std::collections::VecDeque;
use std::fmt;
use std::iter::{self, IntoIterator};
use std::mem;

use futures::future;
use futures::stream::{self, Stream, StreamExt};
use bytes::Bytes;
use derive_more::From;
use rml_rtmp::time::RtmpTimestamp;
use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, PeerType, HandshakeError};
use rml_rtmp::sessions::{ClientSession, ClientSessionConfig, ClientSessionResult, ClientSessionEvent, ClientSessionError, PublishRequestType};
use tokio::net::{tcp, TcpStream};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::rtmp::RtmpError;

pub use rml_rtmp::sessions::StreamMetadata;

#[derive(Debug, From)]
pub enum Error {
    #[from(ignore)]
    EarlyEof,
    Io(io::Error),
    Handshake(HandshakeError),
    Session(ClientSessionError),
    #[from(ignore)]
    RtmpConnectionRefused(String),
    #[from(ignore)]
    DeadRecvTask,
    #[from(ignore)]
    UnexpectedEvent(ClientSessionEvent),
}

enum ClientCommand {
    PublishVideo { data: Bytes, timestamp: RtmpTimestamp },
    PublishAudio { data: Bytes, timestamp: RtmpTimestamp },
}

enum Event {
    Command(ClientCommand),
    CommandEof,
    ServerData(Bytes),
    ServerEof,
}

pub async fn start(mut stream: TcpStream) -> Result<PrepublishClient, Error> {
    let mut handshake = Handshake::new(PeerType::Client);

    stream.write_all(&handshake.generate_outbound_p0_and_p1()?).await?;

    let mut buff = [0; 4096];

    let bytes_after_handshake = loop {
        let bytes = stream.read(&mut buff).await?;

        match handshake.process_bytes(&buff[0..bytes])? {
            HandshakeProcessResult::InProgress { response_bytes } => {
                stream.write_all(&response_bytes).await?;
            }
            HandshakeProcessResult::Completed { response_bytes, remaining_bytes } => {
                stream.write_all(&response_bytes).await?;
                break remaining_bytes;
            }
        }
    };

    // go with defaults for now. TODO investigate whether any should be changed
    // - specifically peer_bandwidth
    let session_config = ClientSessionConfig::new();

    let (session, results) = ClientSession::new(session_config)?;

    // send initial packets from session setup
    for result in results {
        match result {
            ClientSessionResult::OutboundResponse(packet) => {
                stream.write_all(&packet.bytes).await?;
            }
            ClientSessionResult::RaisedEvent(_) |
            ClientSessionResult::UnhandleableMessageReceived(_) => {
                todo!();
            }
        }
    }

    let (mut rtmp_rx, rtmp_tx) = stream.into_split();
    let (mut recv_tx, recv_rx) = mpsc::channel(1);

    // setup server receive task
    tokio::spawn(async move {
        let mut buff = [0; 4096];

        loop {
            let data = match rtmp_rx.read(&mut buff).await {
                Ok(0) => break,
                Ok(n) => Bytes::copy_from_slice(&buff[0..n]),
                Err(e) => {
                    eprintln!("rtmp::client recv error: {:?}", e);
                    break;
                }
            };

            match recv_tx.send(data).await {
                Ok(()) => continue,
                Err(e) => break,
            }
        }
    });

    Ok(PrepublishClient {
        client: ClientState {
            session,
            rtmp_tx,
            rtmp_events: VecDeque::new(),
        },
        recv_rx,
    })
}

#[derive(Debug)]
pub struct PublishInfo {
    pub app_name: String,
    pub stream_key: String,
    pub meta: StreamMetadata,
}

pub struct PrepublishClient {
    client: ClientState,
    recv_rx: mpsc::Receiver<Bytes>,
}

impl PrepublishClient {
    async fn new(mut client: ClientState, recv_rx: mpsc::Receiver<Bytes>, bytes_after_handshake: Vec<u8>) -> Result<Self, Error> {
        let actions = client.session.handle_input(&bytes_after_handshake)?;
        handle_session_results(&mut client, actions).await?;

        Ok(PrepublishClient {
            client,
            recv_rx,
        })
    }

    pub async fn publish(mut self, info: PublishInfo) -> Result<PublishClient, Error> {
        eprintln!("publish!");

        // request connection:
        let action = self.client.session.request_connection(info.app_name)?;
        handle_session_results(&mut self.client, iter::once(action)).await?;

        loop {
            match self.wait_event().await? {
                ClientSessionEvent::ConnectionRequestAccepted => {
                    break;
                }
                ClientSessionEvent::ConnectionRequestRejected { description } => {
                    return Err(Error::RtmpConnectionRefused(description));
                }
                ev => {
                    println!("rtmp::client unexpected event: {:?}", ev);
                    return Err(Error::UnexpectedEvent(ev));
                }
            }
        }

        eprintln!("accepted connect");

        // request publish:
        let action = self.client.session.request_publishing(info.stream_key, PublishRequestType::Live)?;
        handle_session_results(&mut self.client, iter::once(action)).await?;

        loop {
            eprintln!("waiting on publish");
            match self.wait_event().await? {
                ClientSessionEvent::PublishRequestAccepted => {
                    break;
                }
                ev => {
                    println!("rtmp::client unexpected event: {:?}", ev);
                    return Err(Error::UnexpectedEvent(ev));
                }
            }
        }

        eprintln!("accepted publish");

        // send publish metadata:
        let action = self.client.session.publish_metadata(&info.meta)?;
        handle_session_results(&mut self.client, iter::once(action)).await?;

        // set up publish client:
        let PrepublishClient { client, recv_rx } = self;
        let (command_tx, command_rx) = mpsc::channel(100); // high buffer so that we never block the realtime engine thread

        // setup incoming events stream for run_client
        let events = stream::select(
            recv_rx.map(Event::ServerData).chain(stream::once(future::ready(Event::ServerEof))),
            command_rx.map(Event::Command).chain(stream::once(future::ready(Event::CommandEof))),
        );

        // run client
        tokio::spawn(async move {
            match run_client(client, events).await {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("rtmp::client client task errored: {:?}", e);
                }
            }
        });

        Ok(PublishClient { command_tx })
    }

    async fn wait_event(&mut self) -> Result<ClientSessionEvent, Error> {
        if let Some(event) = self.client.rtmp_events.pop_front() {
            return Ok(event);
        }

        loop {
            let bytes = self.recv_rx.next().await.ok_or(Error::EarlyEof)?;

            println!("got bytes -> {:?}", bytes);

            let actions = self.client.session.handle_input(&bytes)?;
            handle_session_results(&mut self.client, actions).await?;

            if let Some(event) = self.client.rtmp_events.pop_front() {
                return Ok(event);
            }
        }
    }
}

pub struct PublishClient {
    command_tx: mpsc::Sender<ClientCommand>,
}

impl fmt::Debug for PublishClient {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PublishClient")
    }
}

#[derive(Debug)]
pub struct PublishError;

impl PublishClient {
    pub async fn publish_audio(&mut self, data: Bytes, timestamp: RtmpTimestamp) -> Result<(), PublishError> {
        self.command_tx.send(ClientCommand::PublishAudio { data, timestamp })
            .await
            .map_err(|_| PublishError)
    }

    pub async fn publish_video(&mut self, data: Bytes, timestamp: RtmpTimestamp) -> Result<(), PublishError> {
        self.command_tx.send(ClientCommand::PublishVideo { data, timestamp })
            .await
            .map_err(|_| PublishError)
    }
}

struct ClientState {
    session: ClientSession,
    rtmp_tx: tcp::OwnedWriteHalf,
    rtmp_events: VecDeque<ClientSessionEvent>,
}

async fn run_client(mut client: ClientState, mut events: impl Stream<Item = Event> + Unpin) -> Result<(), Error> {
    while let Some(event) = events.next().await {
        match event {
            Event::ServerData(bytes) => {
                let actions = client.session.handle_input(&bytes)?;
                handle_session_results(&mut client, actions).await?;
            }
            Event::ServerEof => {
                break;
            }
            Event::Command(ClientCommand::PublishAudio { data, timestamp }) => {
                let action = client.session.publish_audio_data(data, timestamp, false)?;
                handle_session_results(&mut client, iter::once(action)).await?;
            }
            Event::Command(ClientCommand::PublishVideo { data, timestamp }) => {
                let action = client.session.publish_video_data(data, timestamp, false)?;
                handle_session_results(&mut client, iter::once(action)).await?;
            }
            Event::CommandEof => {
                break;
            }
        }
    }

    Ok(())
}

async fn handle_session_results(client: &mut ClientState, actions: impl IntoIterator<Item = ClientSessionResult>) -> Result<(), Error> {
    for action in actions {
        println!("action -> {:?}", action);
        match action {
            ClientSessionResult::OutboundResponse(packet) => {
                client.rtmp_tx.write_all(&packet.bytes).await?;
            }
            ClientSessionResult::RaisedEvent(ev) => {
                println!("received event! {:?}", ev);
                client.rtmp_events.push_back(ev);
            }
            ClientSessionResult::UnhandleableMessageReceived(msg) => {
                println!("ClientSessionResult::UnhandleableMessageReceived: {:?}", msg);
                println!("    data -> {:?}", msg.data);
            }
        }
    }

    Ok(())
}
