// Originally taken from Javelin under GPL 3
// https://github.com/valeth/javelin
// Copyright (C) 2018  Patrick Auernig

// Modified by Charlie Somerville for Mixlab
// https://github.com/charliesome/mixlab

use std::sync::Arc;

use bytes::{Bytes, BytesMut, Buf};

use super::{nal, AvcError, DecoderConfigurationRecord};


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

    pub fn into_bytes(&self) -> Result<Bytes, AvcError> {
        use self::nal::UnitType;

        let mut tmp = BytesMut::new();
        let mut aud_appended = false;
        let mut sps_and_pps_appended = false;
        let nalus = self.nal_units.clone();
        let dcr = &self.dcr;

        for nalu in nalus {
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
                        tmp.extend(Self::ACCESS_UNIT_DELIMITER);
                        aud_appended = true;
                    }
                }
                UnitType::IdrPicture => {
                    if !aud_appended {
                        tmp.extend(Self::ACCESS_UNIT_DELIMITER);
                        aud_appended = true;
                    }

                    if !sps_and_pps_appended {
                        if let Some(sps) = dcr.sps.first() {
                            tmp.extend(Self::DELIMITER2);
                            let unit: Bytes = sps.clone().into();
                            tmp.extend(unit);
                        }

                        if let Some(pps) = dcr.pps.first() {
                            tmp.extend(Self::DELIMITER2);
                            let unit: Bytes = pps.clone().into();
                            tmp.extend(unit);
                        }

                        sps_and_pps_appended = true;
                    }
                }
                t => eprintln!("avc: received unhandled nalu type {:?}", t),

            }

            if nalu.data.len() < 5 {
                return Err(AvcError::NotEnoughData);
            }

            tmp.extend(Self::DELIMITER1);
            let nalu_data: Bytes = nalu.into();
            tmp.extend(nalu_data);
        }

        Ok(tmp.freeze())
    }
}
