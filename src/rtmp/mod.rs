use std::io::{self, Read};
use std::thread;
use std::time::{Instant, Duration};

use derive_more::From;
use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, HandshakeError, PeerType};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use crate::listen::PeekTcpStream;

#[derive(From, Debug)]
pub enum RtmpError {
    Io(io::Error),
    Handshake(HandshakeError),
}

pub async fn accept(mut stream: PeekTcpStream) -> Result<(), RtmpError> {
    let (handshake, remaining_bytes) = handshake(&mut stream).await?;

    println!("RTMP handshake succeeded!");

    Ok(())
}

async fn handshake(stream: &mut PeekTcpStream) -> Result<(Handshake, Vec<u8>), RtmpError> {
    println!("RTMP incoming!");

    let mut handshake = Handshake::new(PeerType::Server);

    let mut buff = vec![0u8; 4096];

    loop {
        let bytes = stream.read(&mut buff).await?;

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
