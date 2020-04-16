pub mod bitstream;
pub mod dcr;
pub mod nal;

#[derive(Debug)]
pub enum AvcError {
    NotEnoughData,
    UnsupportedConfigurationRecordVersion(u8),
    ReservedNalUnitType(u8),
    UnknownNalUnitType(u8),
}

pub use dcr::DecoderConfigurationRecord;
pub use bitstream::Bitstream;
