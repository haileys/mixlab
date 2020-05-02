pub mod aac;
pub mod avc;
pub mod ffmpeg;
pub mod ogg;

use std::io;

use futures::stream::Stream;

type PcmData = Vec<Vec<i16>>;

pub enum StreamRead {
    Audio(PcmData),
    Metadata(Metadata),
}

pub enum StreamError {
    IoError(io::Error),
    BadPacket,
}

pub trait AudioStream {
    fn codec_name(&self) -> &'static str;
    fn sample_rate(&self) -> usize;
    fn channels(&self) -> usize;
    fn bitrate_nominal(&self) -> usize;
    fn read(&mut self) -> Result<Option<StreamRead>, StreamError>;
}

#[derive(Debug)]
pub struct Metadata {
    pub artist: Option<String>,
    pub title: Option<String>,
}

pub trait PcmRead: Stream<Item = Vec<f32>> {
    fn channels() -> usize;
}
