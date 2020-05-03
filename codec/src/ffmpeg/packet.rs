use std::convert::TryInto;
use std::slice;

use ffmpeg_dev::sys as ff;

#[derive(Debug)]
pub struct AvPacket {
    packet: ff::AVPacket,
}

impl AvPacket {
    pub unsafe fn new(raw: ff::AVPacket) -> Self {
        AvPacket { packet: raw }
    }

    pub fn as_ptr(&self) -> *const ff::AVPacket {
        &self.packet as *const _
    }

    pub fn as_mut_ptr(&mut self) -> *mut ff::AVPacket {
        &mut self.packet as *mut _
    }

    fn as_underlying(&self) -> &ff::AVPacket {
        unsafe { &*self.as_ptr() }
    }

    pub fn data(&self) -> &[u8] {
        let underlying = self.as_underlying();
        unsafe {
            slice::from_raw_parts(
                underlying.data,
                underlying.size.try_into().expect("packet buffer too large"))
        }
    }

    pub fn composition_time(&self) -> u32 {
        let underlying = self.as_underlying();
        (underlying.pts - underlying.dts).try_into().unwrap()
    }
}

impl Drop for AvPacket {
    fn drop(&mut self) {
        unsafe { ff::av_packet_unref(self.as_mut_ptr()); }
    }
}
