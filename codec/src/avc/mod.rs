use bytes::{Bytes, Buf};
use derive_more::From;

pub mod bitstream;
pub mod dcr;
pub mod decode;
pub mod encode;
pub mod nal;

pub use decode::AvcDecoder;
pub use encode::AvcEncoder;
pub use dcr::DecoderConfigurationRecord;

#[derive(Debug, Clone, Copy)]
pub struct Millis(pub u64);

#[derive(Debug, From)]
pub enum AvcError {
    NotEnoughData,
    #[from(ignore)] UnsupportedConfigurationRecordVersion(u8),
    #[from(ignore)] ReservedNalUnitType(u8),
    #[from(ignore)] UnknownNalUnitType(u8),
    NoSps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvcFrameType {
    KeyFrame,
    InterFrame,
    DisposableInterFrame,
    GeneratedKeyFrame,
    VideoInfoFrame,
}

impl AvcFrameType {
    pub fn is_key_frame(&self) -> bool {
        *self == AvcFrameType::KeyFrame || *self == AvcFrameType::GeneratedKeyFrame
    }
}

#[derive(Debug)]
pub enum AvcPacketError {
    Eof,
    BadFrameType(u8),
    BadCodec(u8),
    BadAvcPacketType(u8),
}

#[derive(Debug)]
pub enum AvcPacketType {
    SequenceHeader,
    Nalu,
    EndOfSequence,
}

#[derive(Debug)]
pub struct AvcPacket {
    pub frame_type: AvcFrameType,
    pub packet_type: AvcPacketType,
    pub composition_time: u32,
    pub data: Bytes,
}

impl AvcPacket {
    pub fn parse(mut bytes: Bytes) -> Result<AvcPacket, AvcPacketError> {
        if bytes.remaining() < 1 {
            return Err(AvcPacketError::Eof);
        }

        let ident = bytes.get_u8();

        let frame_type = match ident >> 4 {
            1 => AvcFrameType::KeyFrame,
            2 => AvcFrameType::InterFrame,
            3 => AvcFrameType::DisposableInterFrame,
            4 => AvcFrameType::GeneratedKeyFrame,
            5 => AvcFrameType::VideoInfoFrame,
            x => return Err(AvcPacketError::BadFrameType(x)),
        };

        match ident & 0x0f {
            7 => { /* avc codec */ }
            x => return Err(AvcPacketError::BadCodec(x)),
        };

        if bytes.remaining() < 4 {
            return Err(AvcPacketError::Eof);
        }

        let packet_type = match bytes.get_u8() {
            0 => AvcPacketType::SequenceHeader,
            1 => AvcPacketType::Nalu,
            2 => AvcPacketType::EndOfSequence,
            x => return Err(AvcPacketError::BadAvcPacketType(x)),
        };

        let composition_time = bytes.get_uint(3) as u32;

        let data = bytes.to_bytes();

        Ok(AvcPacket {
            frame_type,
            packet_type,
            composition_time,
            data,
        })
    }
}
