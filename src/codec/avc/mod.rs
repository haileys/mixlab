use bytes::{Bytes, Buf};

pub mod bitstream;
pub mod dcr;
pub mod nal;

pub use dcr::DecoderConfigurationRecord;
pub use bitstream::Bitstream;

#[derive(Debug, Clone, Copy)]
pub struct Millis(pub u64);

#[derive(Debug)]
pub enum AvcError {
    NotEnoughData,
    UnsupportedConfigurationRecordVersion(u8),
    ReservedNalUnitType(u8),
    UnknownNalUnitType(u8),
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

#[derive(Debug)]
pub struct AvcFrame {
    pub frame_type: AvcFrameType,
    pub timestamp: Millis,
    pub presentation_timestamp: Millis,
    pub bitstream: Bitstream,
}