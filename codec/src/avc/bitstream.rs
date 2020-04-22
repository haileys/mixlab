// Originally taken from Javelin under GPL 3
// https://github.com/valeth/javelin
// Copyright (C) 2018  Patrick Auernig

// Modified by Charlie Somerville for Mixlab
// https://github.com/charliesome/mixlab

use std::sync::Arc;

use bytes::{Bytes, Buf, BufMut};

use super::{AvcError, DecoderConfigurationRecord};
use super::nal::{self, UnitType};


#[derive(Debug)]
pub struct Bitstream {
    pub dcr: Arc<DecoderConfigurationRecord>,
    pub nal_units: Vec<nal::Unit>,
}

impl Bitstream {
    const DELIMITER1: &'static [u8] = &[0x00, 0x00, 0x01];
    const DELIMITER2: &'static [u8] = &[0x00, 0x00, 0x00, 0x01];
    const ACCESS_UNIT_DELIMITER: &'static [u8] = &[0x00, 0x00, 0x00, 0x01, 0x09, 0xF0];

    pub fn parse(mut buf: Bytes, dcr: Arc<DecoderConfigurationRecord>) -> Result<Self, AvcError> {
        let mut nal_units = Vec::new();

        while buf.has_remaining() {
            if buf.remaining() < dcr.nalu_size as usize {
                return Err(AvcError::NotEnoughData);
            };

            // TODO nalu size could be > 8... validate
            let nalu_length = buf.get_uint(dcr.nalu_size as usize) as usize;

            if buf.remaining() < nalu_length {
                return Err(AvcError::NotEnoughData);
            }

            let nalu_data = buf.split_to(nalu_length);
            nal_units.push(nal::Unit::parse(nalu_data)?)
        };

        if buf.has_remaining() {
            eprintln!("avc::bitstream: {} bytes remaining in buffer", buf.remaining());
        }

        Ok(Self { nal_units, dcr })
    }

    pub fn write_byte_stream(&self, mut out: impl BufMut) -> Result<(), AvcError> {
        let mut aud_appended = false;
        let mut sps_and_pps_appended = false;
        let dcr = &self.dcr;

        for nalu in &self.nal_units {
            match &nalu.kind {
                | UnitType::SequenceParameterSet
                | UnitType::PictureParameterSet
                | UnitType::AccessUnitDelimiter
                | UnitType::FillerData => {
                    continue;
                }
                | UnitType::NonIdrPicture
                | UnitType::SupplementaryEnhancementInformation => {
                    if !aud_appended {
                        out.put(Self::ACCESS_UNIT_DELIMITER);
                        aud_appended = true;
                    }
                }
                UnitType::IdrPicture => {
                    if !aud_appended {
                        out.put(Self::ACCESS_UNIT_DELIMITER);
                        aud_appended = true;
                    }

                    if !sps_and_pps_appended {
                        if let Some(sps) = dcr.sps.first() {
                            out.put(Self::DELIMITER2);
                            sps.write_to(&mut out);
                        }

                        if let Some(pps) = dcr.pps.first() {
                            out.put(Self::DELIMITER2);
                            pps.write_to(&mut out);
                        }

                        sps_and_pps_appended = true;
                    }
                }
                t => eprintln!("avc: received unhandled nalu type {:?}", t),

            }

            if nalu.data.len() < 5 {
                return Err(AvcError::NotEnoughData);
            }

            out.put(Self::DELIMITER1);
            nalu.write_to(&mut out);
        }

        Ok(())
    }

    pub fn write_to(&self, mut out: impl BufMut) {
        for nalu in &self.nal_units {
            match &nalu.kind {
                | UnitType::SequenceParameterSet
                | UnitType::PictureParameterSet
                | UnitType::AccessUnitDelimiter
                | UnitType::FillerData => {}
                _ => {
                    out.put_u32(nalu.byte_size() as u32);
                    nalu.write_to(&mut out);
                }
            }
        }
    }
}
