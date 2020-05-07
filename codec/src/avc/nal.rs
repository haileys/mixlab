// Originally taken from Javelin under GPL 3
// https://github.com/valeth/javelin
// Copyright (C) 2018  Patrick Auernig

// Modified by Charlie Somerville for Mixlab
// https://github.com/charliesome/mixlab

use std::fmt;
use bytes::{Bytes, BytesMut, Buf, BufMut};
use serde_derive::{Deserialize, Serialize};
use super::AvcError;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Deserialize, Serialize)]
pub enum UnitType {
    NonIdrPicture = 1,
    DataPartitionA = 2,
    DataPartitionB = 3,
    DataPartitionC = 4,
    IdrPicture = 5,
    SupplementaryEnhancementInformation = 6,
    SequenceParameterSet = 7,
    PictureParameterSet = 8,
    AccessUnitDelimiter = 9,
    SequenceEnd = 10,
    StreamEnd = 11,
    FillerData = 12,
    SequenceParameterSetExtension = 13,
    Prefix = 14,
    SequenceParameterSubset = 15,
    NotAuxiliaryCoded = 19,
    CodedSliceExtension = 20,
}

impl UnitType {
    fn try_from(value: u8) -> Result<Self, AvcError> {
        let val = match value {
            1 => UnitType::NonIdrPicture,
            2 => UnitType::DataPartitionA,
            3 => UnitType::DataPartitionB,
            4 => UnitType::DataPartitionC,
            5 => UnitType::IdrPicture,
            6 => UnitType::SupplementaryEnhancementInformation,
            7 => UnitType::SequenceParameterSet,
            8 => UnitType::PictureParameterSet,
            9 => UnitType::AccessUnitDelimiter,
            10 => UnitType::SequenceEnd,
            11 => UnitType::StreamEnd,
            12 => UnitType::FillerData,
            13 => UnitType::SequenceParameterSetExtension,
            14 => UnitType::Prefix,
            15 => UnitType::SequenceParameterSubset,
            19 => UnitType::NotAuxiliaryCoded,
            20 => UnitType::CodedSliceExtension,
            16 | 17 | 18 | 22 | 23 => {
                return Err(AvcError::ReservedNalUnitType(value));
            },
            _ => {
                return Err(AvcError::UnknownNalUnitType(value));
            },
        };

        Ok(val)
    }
}


/// Network Abstraction Layer Unit (aka NALU) of a H.264 bitstream.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Unit {
    pub ref_idc: u8,
    pub kind: UnitType,
    pub data: Bytes, // Raw Byte Sequence Payload (RBSP)
}

impl Unit {
    pub fn parse(mut buf: Bytes) -> Result<Self, AvcError> {
        if buf.remaining() < 1 {
            return Err(AvcError::NotEnoughData);
        }

        let header = buf.get_u8();
        assert_eq!(header >> 7, 0);
        let ref_idc = (header >> 5) & 0x03;
        let kind = UnitType::try_from(header & 0x1F)?;

        Ok(Self { ref_idc, kind, data: buf })
    }

    pub fn byte_size(&self) -> usize {
        1 + self.data.len()
    }

    pub fn write_to(&self, mut buf: impl BufMut) {
        let header = ((self.ref_idc & 0x03) << 5)
                   | ((self.kind as u8) & 0x1f);

        buf.put_u8(header);
        buf.put(self.data.clone());
    }
}

impl Into<Bytes> for Unit {
    fn into(self) -> Bytes {
        let mut tmp = BytesMut::with_capacity(self.byte_size());
        self.write_to(&mut tmp);
        tmp.freeze()
    }
}

impl fmt::Debug for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Unit")
            .field("ref_idc", &self.ref_idc)
            .field("kind", &self.kind)
            .finish()
    }
}
