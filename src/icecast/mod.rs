pub mod http;
pub mod registry;

use std::io::{self, Read};
use std::thread;
use std::time::{Instant, Duration};

use derive_more::From;
use futures::executor::block_on;
use tokio::io::{AsyncRead, AsyncWriteExt};

use crate::codec::{AudioStream, StreamRead, StreamError};
use crate::codec::ogg::{self, OggStream};
use crate::engine::{SAMPLE_RATE, Sample};

use crate::listen::PeekTcpStream;
use http::ContentType;
use registry::SourceSend;

pub struct SyncRead<T>(T);

impl<T: AsyncRead + Unpin> io::Read for SyncRead<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        use tokio::io::AsyncReadExt;
        block_on(self.0.read(buf))
    }
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

    let send = match registry::connect(&req.path) {
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

    let start = Instant::now();
    let mut total_samples = 0u64;

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

                // we intentionally ignore the return value of write indicating
                // how many samples were written to the ring buffer. if it's
                // ever less than the number of samples on hand, the receive
                // side is suffering from serious lag and the best we can do
                // is drop the data

                let _ = send.write(&samples)
                    .map_err(|()| DecodeThreadError::ListenerDisconnected)?;

                // throttle ourselves according to how long these samples should
                // take to play through. this ensures that fast clients don't
                // fill the ring buffer up on us

                let elapsed = Duration::from_micros((total_samples * 1_000_000) / SAMPLE_RATE as u64);
                let sleep_until = start + elapsed;
                let now = Instant::now();

                if now < sleep_until {
                    thread::sleep(sleep_until - now);
                }

                total_samples += sample_count as u64;
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
