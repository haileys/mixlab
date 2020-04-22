// Originally taken from Javelin under GPL 3
// https://github.com/valeth/javelin
// Copyright (C) 2018  Patrick Auernig

// Modified by Charlie Somerville for Mixlab
// https://github.com/charliesome/mixlab

use bytes::{Bytes, Buf};
use super::AacError;

/// See [MPEG-4 Audio Object Types][audio_object_types]
///
/// [audio_object_types]: https://en.wikipedia.org/wiki/MPEG-4_Part_3#MPEG-4_Audio_Object_Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AudioObjectType {
    AacMain = 1,
    AacLowComplexity = 2,
    AacScalableSampleRate = 3,
    AacLongTermPrediction = 4
}

impl AudioObjectType {
    pub fn try_from_u8(value: u8) -> Result<Self, AacError> {
        let val = match  value {
            1 => AudioObjectType::AacMain,
            2 => AudioObjectType::AacLowComplexity,
            3 => AudioObjectType::AacScalableSampleRate,
            4 => AudioObjectType::AacLongTermPrediction,
            _ => return Err(AacError::UnsupportedAudioObjectType(value)),
        };

        Ok(val)
    }
}

/// Bits | Description
/// ---- | -----------
/// 5    | Audio object type
/// 4    | Sampling frequency index
/// 4    | Channel configuration
/// 1    | Frame length flag
/// 1    | Depends on core coder
/// 1    | Extension flag
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSpecificConfiguration {
    pub object_type: AudioObjectType,
    pub sampling_frequency_index: u8,
    pub channel_configuration: u8,
    pub frame_length_flag: bool,
    pub depends_on_core_coder: bool,
    pub extension_flag: bool,
}

impl AudioSpecificConfiguration {
    pub fn parse(mut buf: Bytes) -> Result<Self, AacError> {
        if buf.remaining() < 2 {
            return Err(AacError::EarlyEof);
        }

        let x = buf.get_u8();
        let y = buf.get_u8();

        let object_type = AudioObjectType::try_from_u8((x & 0xF8) >> 3)?;
        let sampling_frequency_index = ((x & 0x07) << 1) | (y >> 7);
        let channel_configuration = (y >> 3) & 0x0F;

        let frame_length_flag = (y & 0x04) == 0x04;
        let depends_on_core_coder = (y & 0x02) == 0x02;
        let extension_flag = (y & 0x01) == 0x01;

        Ok(Self {
            object_type,
            sampling_frequency_index,
            channel_configuration,
            frame_length_flag,
            depends_on_core_coder,
            extension_flag,
        })
    }
}


