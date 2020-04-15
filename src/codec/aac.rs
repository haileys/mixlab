use std::process::Stdio;

use bytes::Bytes;
use derive_more::From;
use faad2::Decoder;
use futures::stream::{Stream, StreamExt};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Command, Child, ChildStdin, ChildStdout, ChildStderr};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::codec::AudioStream;
use crate::engine::SAMPLE_RATE;

pub struct Aac {
    decoder: Decoder,
}

#[derive(Debug)]
pub enum AacError {
    InitCodec,
}

impl Aac {
    pub fn new(asc: &[u8], input: impl Stream<Item = Bytes>) -> Result<Aac, AacError> {
        let mut decoder = Decoder::new(asc).map_err(|_| AacError::InitCodec)?;

        let mut buff = [0u8; 4096];
        futures::pin_mut!(input);

        while let Some(frame) = input.next().await {

        }

        todo!()
    }

    pub fn decode()
}
