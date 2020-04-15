// This file derives from the javelin-codec project
// Copyright (C) 2018  Patrick Auernig

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use bytes::{Bytes, BytesMut, Buf, BufMut};


#[allow(dead_code)]
#[derive(Debug, Clone)]
enum Version {
    Mpeg4 = 0,
    Mpeg2 = 1,
}


/// Bits | Description
/// ---- | -----------
/// 12   | Sync word, constant 0x0FFF
/// 1    | MPEG version
/// 2    | Layer, constant 0x00
/// 1    | Protection flag
/// 2    | Profile
/// 4    | MPEG-4 sampling frequency index
/// 1    | Private, constant 0x00
/// 3    | MPEG-4 channel configuration
/// 1    | Originality
/// 1    | Home
/// 1    | Copyrighted ID
/// 1    | Copyrighted ID start
/// 13   | Frame length
/// 11   | Buffer fullness
/// 2    | Number of AAC frames - 1
/// 16   | CRC if protection flag not set
///
/// See [ADTS - Multimedia Wiki][adts_mm_wiki] for more info.
///
/// [adts_mm_wiki]: https://wiki.multimedia.cx/index.php/ADTS
#[derive(Debug, Clone)]
pub struct AudioDataTransportStream {
    version: Version,
    profile: u8,
    sampling_frequency_index: u8,
    channel_configuration: u8,
    crc: Option<String>,
    payload: Bytes,
}

impl AudioDataTransportStream {
    const SYNCWORD: u16 = 0xFFF0;
    const PROTECTION_FLAG: u8 = 0x01;

    pub fn new(payload: Bytes, asc: AudioSpecificConfiguration) -> Self {
        assert!(payload.len() <= (std::u16::MAX & 0x1FFF) as usize);

        let profile = (asc.object_type as u8) - 1;

        Self {
            version: Version::Mpeg4,
            profile,
            sampling_frequency_index: asc.sampling_frequency_index,
            channel_configuration: asc.channel_configuration,
            crc: None,
            payload,
        }
    }
}

impl Into<Bytes> for AudioDataTransportStream {
    fn into(self) -> Bytes {
        let mut tmp = BytesMut::with_capacity(56 + self.payload.len());

        // Syncword (12 bits), MPEG version (1 bit),
        // layer (2 bits = 0) and protection absence (1 bit = 1)
        let mpeg_version = (self.version as u16) << 3;
        let protection = u16::from(Self::PROTECTION_FLAG);
        tmp.put_u16(Self::SYNCWORD | mpeg_version | protection);

        // Profile (2 bits = 0), sampling frequency index (4 bits),
        // private (1 bit = 0) and channel configuration (1 bit)
        let profile = self.profile << 6;
        let sampling_frequency_index = self.sampling_frequency_index << 2;
        assert!(sampling_frequency_index != 0x0F, "Sampling frequency index 15 forbidden");
        let channel_configuration1 = (self.channel_configuration & 0x07) >> 2;
        tmp.put_u8(profile | sampling_frequency_index | channel_configuration1);

        // Header is 7 bytes long if protection is absent,
        // 9 bytes otherwise (CRC requires 2 bytes).
        let header_length =  if self.crc.is_some() { 9 } else { 7 };
        let frame_length = (self.payload.len() + header_length) as u16;

        // Channel configuration cont. (2 bits), originality (1 bit = 0),
        // home (1 bit = 0), copyrighted id (1 bit = 0)
        // copyright id start (1 bit = 0) and frame length (2 bits)
        let channel_configuration2 = (self.channel_configuration & 0x03) << 6;
        let frame_length1 = ((frame_length & 0x1FFF) >> 11) as u8;
        tmp.put_u8(channel_configuration2 | frame_length1);

        // Frame length cont. (11 bits) and buffer fullness (5 bits)
        let frame_length2 = ((frame_length & 0x7FF) << 5) as u16;
        tmp.put_u16(frame_length2 | 0b0000_0000_0001_1111);

        // Buffer fullness cont. (6 bits) and number of AAC frames minus one (2 bits = 0)
        tmp.put_u8(0b1111_1100);

        tmp.put(self.payload);

        tmp.freeze()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::aac::{
        config::AudioObjectType,
    };

    #[test]
    fn can_be_converted_into_bytes() {
        let expected = Bytes::from_static(&[
            0b1111_1111, 0b1111_0001, 0b0100_1000, 0b1000_0000,
            0b0000_0001, 0b0111_1111, 0b1111_1100,
            0b0100_1110, 0b0010_1111, 0b1001_0011, 0b1111_0010  // dummy payload
        ]);

        let asc = AudioSpecificConfiguration {
            object_type: AudioObjectType::AacLowComplexity,
            sampling_frequency_index: 2,
            channel_configuration: 2,
            frame_length_flag: true,
            depends_on_core_coder: true,
            extension_flag: true,
        };

        let dummy_payload = Bytes::from_static(&[0b0100_1110, 0b0010_1111, 0b1001_0011, 0b1111_0010]);
        let adts = AudioDataTransportStream::new(dummy_payload, asc);
        let actual: Bytes = adts.into();

        assert_eq!(expected[..], actual[..]);
    }
}

/// See [MPEG-4 Audio Object Types][audio_object_types]
///
/// [audio_object_types]: https://en.wikipedia.org/wiki/MPEG-4_Part_3#MPEG-4_Audio_Object_Types
#[allow(clippy::enum_variant_names, dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AudioObjectType {
    AacMain = 1,
    AacLowComplexity = 2,
    AacScalableSampleRate = 3,
    AacLongTermPrediction = 4
}

#[derive(Debug)]
pub struct UnsupportedAudioObjectType;

impl AudioObjectType {
    pub fn try_from_u8(value: u8) -> Result<Self, UnsupportedAudioObjectType> {
        let val = match  value {
            1 => AudioObjectType::AacMain,
            2 => AudioObjectType::AacLowComplexity,
            3 => AudioObjectType::AacScalableSampleRate,
            4 => AudioObjectType::AacLongTermPrediction,
            _ => return Err(UnsupportedAudioObjectType),
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

pub enum AscError {
    EarlyEof,
    UnsupportedAudioObjectType,
}

impl AudioSpecificConfiguration {
    pub fn try_from_buf<B>(buf: &mut B) -> Result<Self, AscError>
        where B: Buf
    {
        if buf.remaining() < 2 {
            return Err(AscError::EarlyEof);
        }

        let x = buf.get_u8();
        let y = buf.get_u8();

        let object_type = AudioObjectType::try_from_u8((x & 0xF8) >> 3)
            .map_err(|_| AscError::UnsupportedAudioObjectType)?;
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


#[cfg(test)]
mod tests {
    use bytes::{Bytes, IntoBuf};
    use super::*;

    #[test]
    fn can_parse_sequence_header() {
        let expected = AudioSpecificConfiguration {
            object_type: AudioObjectType::AacLowComplexity,
            sampling_frequency_index: 4,
            channel_configuration: 2,
            frame_length_flag: false,
            depends_on_core_coder: false,
            extension_flag: false,
        };

        let raw = Bytes::from_static(&[
            0b0001_0010, 0b0001_0000,
            0b0101_0110, 0b1110_0101, 0b0000_0000
        ]);

        let actual = AudioSpecificConfiguration::try_from_buf(&mut raw.clone().into_buf()).unwrap();

        assert_eq!(expected, actual);
    }
}
