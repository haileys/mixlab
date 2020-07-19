use bytes::{Bytes, Buf, BytesMut, BufMut};

pub enum AudioPacket {
    AacSequenceHeader(Bytes),
    AacRawData(Bytes),
}

#[derive(Debug)]
pub enum AudioPacketError {
    Eof,
    BadTag(u8),
    BadPacketType(u8),
}

// See https://www.adobe.com/content/dam/acom/en/devnet/flv/video_file_format_spec_v10_1.pdf
// Section E.4.2.1 AUDIODATA for reference
impl AudioPacket {
    pub fn parse(mut bytes: Bytes) -> Result<AudioPacket, AudioPacketError> {
        if bytes.len() >= 2 {
            let tag = bytes.get_u8();

            if tag == 0xaf {
                // AAC
                let packet_type = bytes.get_u8();

                if packet_type == 0 {
                    Ok(AudioPacket::AacSequenceHeader(bytes))
                } else if packet_type == 1 {
                    Ok(AudioPacket::AacRawData(bytes))
                } else {
                    Err(AudioPacketError::BadPacketType(packet_type))
                }
            } else {
                Err(AudioPacketError::BadTag(tag))
            }
        } else {
            Err(AudioPacketError::Eof)
        }
    }

    pub fn write_to(&self, out: &mut BytesMut) {
        match self {
            AudioPacket::AacSequenceHeader(bytes) => {
                out.put_u8(0xaf);
                out.put_u8(0);
                out.extend_from_slice(bytes);
            }
            AudioPacket::AacRawData(bytes) => {
                out.put_u8(0xaf);
                out.put_u8(1);
                out.extend_from_slice(bytes);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFrameType {
    KeyFrame,
    InterFrame,
    DisposableInterFrame,
    GeneratedKeyFrame,
    VideoInfoFrame,
}

impl VideoFrameType {
    pub fn is_key_frame(&self) -> bool {
        *self == VideoFrameType::KeyFrame || *self == VideoFrameType::GeneratedKeyFrame
    }
}

#[derive(Debug)]
pub enum VideoPacketError {
    Eof,
    BadFrameType(u8),
    BadCodec(u8),
    BadVideoPacketType(u8),
}

#[derive(Debug)]
pub enum VideoPacketType {
    SequenceHeader,
    Nalu,
    EndOfSequence,
}

#[derive(Debug)]
pub struct VideoPacket {
    pub frame_type: VideoFrameType,
    pub packet_type: VideoPacketType,
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

        let packet_type = match bytes.get_u8() {
            0 => VideoPacketType::SequenceHeader,
            1 => VideoPacketType::Nalu,
            2 => VideoPacketType::EndOfSequence,
            x => return Err(VideoPacketError::BadVideoPacketType(x)),
        };

        let composition_time = bytes.get_uint(3) as u32;

        let data = bytes.to_bytes();

        Ok(VideoPacket {
            frame_type,
            packet_type,
            composition_time,
            data,
        })
    }

    pub fn write_to(&self, out: &mut BytesMut) {
        let frame_type: u8 = match self.frame_type {
            VideoFrameType::KeyFrame => 1,
            VideoFrameType::InterFrame => 2,
            VideoFrameType::DisposableInterFrame => 3,
            VideoFrameType::GeneratedKeyFrame => 4,
            VideoFrameType::VideoInfoFrame => 5,
        };

        let codec: u8 = 7; // AVC

        // write ident (frame type + codec)
        out.put_u8((frame_type << 4) | codec);

        // write packet type
        out.put_u8(match self.packet_type {
            VideoPacketType::SequenceHeader => 0,
            VideoPacketType::Nalu => 1,
            VideoPacketType::EndOfSequence => 2,
        });

        // write composition time as BE24
        out.put_uint(self.composition_time as u64, 3usize);

        // write data
        out.extend_from_slice(&self.data);
    }
}
