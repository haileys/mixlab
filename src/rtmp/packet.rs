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
