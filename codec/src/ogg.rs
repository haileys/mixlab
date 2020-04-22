use std::io;

pub use lewton::VorbisError;

use ogg::{PacketReader, OggReadError};
use lewton::inside_ogg::read_headers;
use lewton::audio::{read_audio_packet, PreviousWindowRight, AudioReadError};
use lewton::header::{read_header_comment, IdentHeader, CommentHeader, SetupHeader};

use crate::{AudioStream, StreamRead, StreamError, Metadata};

struct NonSeekStream<T: io::Read> {
    stream: T,
}

impl<T> io::Read for NonSeekStream<T> where T: io::Read {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.stream.read(buf) {
            Ok(sz) => Ok(sz),
            Err(e) => Err(e),
        }
    }
}

impl<T> io::Seek for NonSeekStream<T> where T: io::Read {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        panic!("trying to seek NonSeekStream: {:?}", pos);
    }
}

impl<T> NonSeekStream<T> where T: io::Read {
    pub fn new(stream: T) -> NonSeekStream<T> {
        NonSeekStream { stream: stream }
    }
}

impl From<CommentHeader> for Metadata {
    fn from(header: CommentHeader) -> Metadata {
        let mut artist = None;
        let mut title = None;

        for (name, value) in header.comment_list {
            match name.as_ref() {
                "ARTIST" => artist = Some(value),
                "TITLE" => title = Some(value),
                _ => (),
            }
        }

        Metadata { artist, title }
    }
}

pub struct OggStream<T: io::Read> {
    rdr: PacketReader<NonSeekStream<T>>,
    pwr: PreviousWindowRight,

    pub ident_hdr: IdentHeader,
    pub comment_hdr: CommentHeader,
    pub setup_hdr: SetupHeader,
}

impl<T: io::Read> OggStream<T> {
    pub fn new(io: T) -> Result<Self, VorbisError> {
        let mut rdr = PacketReader::new(NonSeekStream::new(io));

        let ((ident_hdr, comment_hdr, setup_hdr), _) = read_headers(&mut rdr)?;

        Ok(OggStream {
            rdr,
            pwr: PreviousWindowRight::new(),
            ident_hdr,
            comment_hdr,
            setup_hdr,
        })
    }
}

impl<T: io::Read> AudioStream for OggStream<T> {
    fn codec_name(&self) -> &'static str {
        "Vorbis"
    }

    fn sample_rate(&self) -> usize {
        self.ident_hdr.audio_sample_rate as usize
    }

    fn channels(&self) -> usize {
        self.ident_hdr.audio_channels as usize
    }

    fn bitrate_nominal(&self) -> usize {
        self.ident_hdr.bitrate_nominal as usize
    }

    fn read(&mut self) -> Result<Option<StreamRead>, StreamError> {
        let packet = match self.rdr.read_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => return Ok(None),
            Err(OggReadError::ReadError(e)) => return Err(StreamError::IoError(e)),
            Err(OggReadError::NoCapturePatternFound) |
            Err(OggReadError::InvalidStreamStructVer(_)) |
            Err(OggReadError::HashMismatch(_, _)) |
            Err(OggReadError::InvalidData) => return Err(StreamError::BadPacket),
        };

        let decoded_packet = read_audio_packet(&self.ident_hdr,
            &self.setup_hdr, &packet.data, &mut self.pwr);

        match decoded_packet {
            Ok(pcm) => return Ok(Some(StreamRead::Audio(pcm))),
            Err(AudioReadError::AudioIsHeader) => {
                match read_header_comment(&packet.data) {
                    Ok(comment) => Ok(Some(StreamRead::Metadata(comment.into()))),
                    Err(_) => Err(StreamError::BadPacket),
                }
            },
            Err(_) => return Err(StreamError::BadPacket),
        }
    }
}
