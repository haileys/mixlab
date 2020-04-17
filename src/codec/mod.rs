pub mod aac;
pub mod avc;
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

// impl<T: AsyncRead> AsyncRead for PcmStream<T> {
//     fn poll_read(
//         self: Pin<&mut Self>,
//         cx: &mut Context,
//         buf: &mut [u8]
//     ) -> Poll<Result<usize, io::Error>> {
//         let underlying = unsafe { self.map_unchecked(|stream| &mut stream.underlying) };
//         underlying.poll_read(cx, buf)
//     }
// }
