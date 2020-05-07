// Originally taken from Javelin under GPL 3
// https://github.com/valeth/javelin
// Copyright (C) 2018  Patrick Auernig

// Modified by Charlie Somerville for Mixlab
// https://github.com/charliesome/mixlab

use bytes::{Bytes, Buf, BufMut};
use serde_derive::{Deserialize, Serialize};

use super::{nal, AvcError};

/// AVC decoder configuration record
///
/// Bits | Name
/// ---- | ----
/// 8    | Version
/// 8    | Profile Indication
/// 8    | Profile Compatability
/// 8    | Level Indication
/// 6    | Reserved
/// 2    | NALU Length
/// 3    | Reserved
/// 5    | SPS Count
/// 16   | SPS Length
/// var  | SPS
/// 8    | PPS Count
/// 16   | PPS Length
/// var  | PPS
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoderConfigurationRecord {
    pub version: u8,
    pub profile_indication: u8,
    pub profile_compatibility: u8,
    pub level_indication: u8,
    pub nalu_size: u8,
    pub sps: Vec<nal::Unit>,
    pub pps: Vec<nal::Unit>,
}

impl DecoderConfigurationRecord {
    pub fn parse(buf: &mut Bytes) -> Result<Self, AvcError> {
        if buf.remaining() < 6 {
            return Err(AvcError::NotEnoughData)
        }

        let version = buf.get_u8();
        if version != 1 {
            return Err(AvcError::UnsupportedConfigurationRecordVersion(version));
        }

        let profile_indication = buf.get_u8();
        let profile_compatibility = buf.get_u8();
        let level_indication = buf.get_u8();
        let nalu_size = (buf.get_u8() & 0x03) + 1;

        let sps_count = buf.get_u8() & 0x1f;
        let mut sps = Vec::new();
        for _ in 0..sps_count {
            if buf.remaining() < 2 {
                return Err(AvcError::NotEnoughData);
            }

            let sps_length = buf.get_u16() as usize;

            if buf.remaining() < sps_length {
                return Err(AvcError::NotEnoughData);
            }

            let unit = buf.split_to(sps_length);
            sps.push(nal::Unit::parse(unit)?);
        }

        if buf.remaining() < 1 {
            return Err(AvcError::NotEnoughData);
        }

        let pps_count = buf.get_u8();
        let mut pps = Vec::new();
        for _ in 0..pps_count {
            if buf.remaining() < 2 {
                return Err(AvcError::NotEnoughData);
            }

            let pps_length = buf.get_u16() as usize;

            if buf.remaining() < pps_length {
                return Err(AvcError::NotEnoughData);
            }

            let tmp: Bytes = buf.split_to(pps_length);
            pps.push(nal::Unit::parse(tmp)?);
        }

        Ok(Self {
            version,
            profile_indication,
            profile_compatibility,
            level_indication,
            nalu_size,
            sps,
            pps,
        })
    }

    pub fn write_to(&self, mut out: impl BufMut) {
        out.put_u8(self.version);
        out.put_u8(self.profile_indication);
        out.put_u8(self.profile_compatibility);
        out.put_u8(self.level_indication);

        let nalu_size = (self.nalu_size - 1) & 0x03;
        out.put_u8(0b1111_1100 /* reserved */ | nalu_size);

        let sps_count = self.sps.len() as u8 & 0x1f;
        out.put_u8(0b1110_0000 /* reserved */ | sps_count);

        for sps in &self.sps {
            out.put_u16(sps.byte_size() as u16);
            sps.write_to(&mut out);
        }

        let pps_count = self.pps.len() as u8;
        out.put_u8(pps_count);

        for pps in &self.pps {
            out.put_u16(pps.byte_size() as u16);
            pps.write_to(&mut out);
        }
    }
}
