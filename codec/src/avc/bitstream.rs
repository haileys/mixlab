// Originally taken from Javelin under GPL 3
// https://github.com/valeth/javelin
// Copyright (C) 2018  Patrick Auernig

// Modified by Charlie Somerville for Mixlab
// https://github.com/charliesome/mixlab

use std::iter;
use std::sync::Arc;

use bytes::{Bytes, Buf, BufMut};

use super::{AvcError, DecoderConfigurationRecord};
use super::nal::{self, UnitType};


#[derive(Debug, Clone)]
pub struct Bitstream {
    pub dcr: Arc<DecoderConfigurationRecord>,
    pub bytes: Bytes,
}

impl Bitstream {
    const DELIMITER1: &'static [u8] = &[0x00, 0x00, 0x01];
    const DELIMITER2: &'static [u8] = &[0x00, 0x00, 0x00, 0x01];
    const ACCESS_UNIT_DELIMITER: &'static [u8] = &[0x00, 0x00, 0x00, 0x01, 0x09, 0xF0];

    pub fn new(bytes: Bytes, dcr: Arc<DecoderConfigurationRecord>) -> Self {
        Bitstream { dcr, bytes }
    }

    pub fn nal_units(&self) -> impl Iterator<Item = Result<nal::Unit, AvcError>> {
        let dcr = self.dcr.clone();
        let mut bytes = self.bytes.clone();

        iter::from_fn(move || {
            if bytes.remaining() == 0 {
                return None;
            }

            if bytes.remaining() < dcr.nalu_size as usize {
                // make sure bytes is empty for next iteration:
                bytes = Bytes::new();
                return Some(Err(AvcError::NotEnoughData));
            }

            // TODO nalu size could be > 8... validate
            let nalu_length = bytes.get_uint(dcr.nalu_size as usize) as usize;

            if bytes.remaining() < nalu_length {
                return Some(Err(AvcError::NotEnoughData));
            }

            let nalu_data = bytes.split_to(nalu_length);
            Some(nal::Unit::parse(nalu_data))
        })
    }

    pub fn write_byte_stream(&self, mut out: impl BufMut) -> Result<(), AvcError> {
        let mut aud_appended = false;
        let mut sps_and_pps_appended = false;
        let dcr = &self.dcr;

        for nalu in self.nal_units() {
            let nalu = nalu?;

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

    pub fn write_to(&self, mut out: impl BufMut) -> Result<(), AvcError> {
        for nalu in self.nal_units() {
            let nalu = nalu?;

            match &nalu.kind {
                | UnitType::SequenceParameterSet
                | UnitType::PictureParameterSet
                | UnitType::AccessUnitDelimiter
                | UnitType::FillerData => {}
                _ => {
                    out.put_uint(nalu.byte_size() as u64, self.dcr.nalu_size as usize);
                    nalu.write_to(&mut out);
                }
            }
        }

        Ok(())
    }
}
