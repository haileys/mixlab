pub mod http;

use std::fmt::Debug;
use std::io::{self, Read};
use std::thread;

use derive_more::From;
use tokio::io::AsyncWriteExt;

use crate::codec::ogg::{self, OggStream};
use crate::codec::{AudioStream, StreamRead, StreamError};
use crate::engine::{SAMPLE_RATE, Sample};
use crate::listen::PeekTcpStream;
use crate::source::{Registry, ListenError, SourceRecv, SourceSend};
use crate::util::SyncRead;

use http::ContentType;

lazy_static::lazy_static! {
    static ref MOUNTPOINTS: Registry = Registry::new();
}

pub async fn accept(mut stream: PeekTcpStream) {
    let req = match http::parse(&mut stream).await {
        Ok(req) => req,
        Err(_) => { return; }
    };

    // any partial stream data which we might have caught in the http::parse above
    let stream_data = req.stream_data;

    let content_type = if let Some(ty) = req.content_type {
        ty
    } else {
        // unknown content type
        return;
    };

    let send = match MOUNTPOINTS.connect(&req.path) {
        Ok(send) => send,
        Err(e) => {
            eprintln!("could not connect to icecast mountpoint: {:?}", e);
            return;
        }
    };

    stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n").await
        .expect("stream.write_all");

    thread::spawn(move || {
        let stream = stream_data.chain(SyncRead(stream));

        match run_decode_thread(send, stream, content_type) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("error in decode thread: {:?}", e);
            }
        }
    });
}

pub fn listen(mountpoint: &str) -> Result<SourceRecv, ListenError> {
    MOUNTPOINTS.listen(mountpoint)
}

#[derive(From, Debug)]
enum DecodeThreadError {
    ListenerDisconnected,
    Ogg(ogg::VorbisError),
    Io(io::Error),
}

fn run_decode_thread(mut send: SourceSend, stream: impl io::Read, content_type: ContentType)
    -> Result<(), DecodeThreadError>
{
    let mut audio = match content_type {
        ContentType::Ogg => {
            let ogg = OggStream::new(stream)?;
            Box::new(ogg) as Box<dyn AudioStream>
        }
    };

    let channels = audio.channels();

    if channels == 0 {
        // is this even possible?
        // let's guard against it so that we don't panic at least
        return Ok(());
    }

    if audio.sample_rate() != SAMPLE_RATE {
        // not much we can do for now. TODO implement resampling
        return Ok(());
    }

    while let Some(packet) = audio.read().transpose() {
        match packet {
            Ok(StreamRead::Audio(pcm)) => {
                // we need to munge samples from StreamRead into the right
                // format for the mixlab engine. the icecast source always
                // outputs stereo, regardless of input channel count.

                let sample_count = pcm[0].len();

                let mut samples = Vec::with_capacity(sample_count * 2);

                if channels == 1 {
                    for sample in pcm[0].iter() {
                        let sample = convert_sample(*sample);
                        samples.push(sample);
                        samples.push(sample);
                    }
                } else {
                    for (left, right) in pcm[0].iter().zip(pcm[1].iter()) {
                        samples.push(convert_sample(*left));
                        samples.push(convert_sample(*right));
                    }
                }

                send.write(&samples)
                    .map_err(|()| DecodeThreadError::ListenerDisconnected)?;
            }
            Ok(StreamRead::Metadata(_)) => {
                // ignore metadata for now
                continue;
            }
            Err(StreamError::IoError(e)) => {
                return Err(e.into());
            }
            Err(StreamError::BadPacket) => {
                // skip bad packet
                continue;
            }
        }
    }

    Ok(())
}

fn convert_sample(sample: i16) -> Sample {
    // i16::min_value is a greater absolute distance away from 0 than max_value
    // divide by it rather than max_value to prevent clipping
    let divisor = -(i16::min_value() as Sample);

    sample as Sample / divisor
}
