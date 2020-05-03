use std::iter;

use bytes::{Bytes, Buf};

use super::AvcError;
use super::nal::Unit;

pub fn read(mut bytes: Bytes, nalu_size: usize) -> impl Iterator<Item = Result<Unit, AvcError>> {
    iter::from_fn(move || {
        if bytes.remaining() == 0 {
            return None;
        }

        if bytes.remaining() < nalu_size {
            // make sure bytes is empty for next iteration:
            bytes = Bytes::new();
            return Some(Err(AvcError::NotEnoughData));
        }

        let nalu_length = bytes.get_uint(nalu_size) as usize;

        if bytes.remaining() < nalu_length {
            return Some(Err(AvcError::NotEnoughData));
        }

        let nalu_data = bytes.split_to(nalu_length);
        Some(Unit::parse(nalu_data))
    })
}
