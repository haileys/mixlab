use bytes::{Bytes, Buf};
use derive_more::From;

pub mod bitstream;
pub mod dcr;
pub mod decode;
pub mod encode;
pub mod nal;

pub use decode::AvcDecoder;
pub use encode::AvcEncoder;
pub use dcr::DecoderConfigurationRecord;

#[derive(Debug, Clone, Copy)]
pub struct Millis(pub u64);

#[derive(Debug, From)]
pub enum AvcError {
    NotEnoughData,
    #[from(ignore)] UnsupportedConfigurationRecordVersion(u8),
    #[from(ignore)] ReservedNalUnitType(u8),
    #[from(ignore)] UnknownNalUnitType(u8),
    NoSps,
}
