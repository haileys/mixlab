use bytes::{Bytes, Buf};

pub enum AudioPacket {
    AacSequenceHeader(Bytes),
    AacRawData(Bytes),
    Unknown(Bytes)
}

// See https://www.adobe.com/content/dam/acom/en/devnet/flv/video_file_format_spec_v10_1.pdf
// Section E.4.2.1 AUDIODATA for reference
impl AudioPacket {
    pub fn parse(mut bytes: Bytes) -> AudioPacket {
        let original = bytes.clone();

        if bytes.len() >= 2 {
            let tag = bytes.get_u8();

            if tag == 0xaf {
                // AAC
                let packet_type = bytes.get_u8();

                if packet_type == 0 {
                    return AudioPacket::AacSequenceHeader(bytes);
                } else if packet_type == 1 {
                    return AudioPacket::AacRawData(bytes);
                }
            }
        }

        AudioPacket::Unknown(original)
    }
}

#[derive(Debug, Clone)]
pub enum VideoFrameType {
    KeyFrame,
    InterFrame,
    DisposableInterFrame,
    GeneratedKeyFrame,
    VideoInfoFrame,
}

#[derive(Debug)]
pub enum VideoPacketError {
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

pub struct VideoPacket {
    pub frame_type: VideoFrameType,
    pub avc_packet_type: AvcPacketType,
    pub composition_time: u32,
    pub data: Bytes,
}

impl VideoPacket {
    pub fn parse(mut bytes: Bytes) -> Result<VideoPacket, VideoPacketError> {
        if bytes.remaining() < 1 {
            return Err(VideoPacketError::Eof);
        }

        let ident = bytes.get_u8();

        let frame_type = match ident >> 4 {
            1 => VideoFrameType::KeyFrame,
            2 => VideoFrameType::InterFrame,
            3 => VideoFrameType::DisposableInterFrame,
            4 => VideoFrameType::GeneratedKeyFrame,
            5 => VideoFrameType::VideoInfoFrame,
            x => return Err(VideoPacketError::BadFrameType(x)),
        };

        match ident & 0x0f {
            7 => { /* avc codec */ }
            x => return Err(VideoPacketError::BadCodec(x)),
        };

        if bytes.remaining() < 4 {
            return Err(VideoPacketError::Eof);
        }

        let avc_packet_type = match bytes.get_u8() {
            0 => AvcPacketType::SequenceHeader,
            1 => AvcPacketType::Nalu,
            2 => AvcPacketType::EndOfSequence,
            x => return Err(VideoPacketError::BadAvcPacketType(x)),
        };

        let composition_time = bytes.get_uint(3) as u32;

        let data = bytes.to_bytes();

        Ok(VideoPacket {
            frame_type,
            avc_packet_type,
            composition_time,
            data,
        })
    }
}
