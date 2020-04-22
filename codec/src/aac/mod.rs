pub mod adts;
pub mod config;

pub use adts::AudioDataTransportStream;
pub use config::AudioSpecificConfiguration;

#[derive(Debug)]
pub enum AacError {
    EarlyEof,
    UnsupportedAudioObjectType(u8),
}
