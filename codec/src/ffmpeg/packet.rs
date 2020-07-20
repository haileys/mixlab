use std::os::raw::c_int;
use std::convert::TryInto;
use std::slice;

use ffmpeg_dev::sys as ff;

#[derive(Debug)]
pub struct AvPacket {
    packet: ff::AVPacket,
}

// ffmpeg buffer refcounts are threadsafe
unsafe impl Sync for AvPacket {}
unsafe impl Send for AvPacket {}

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

    pub fn decode_timestamp(&self) -> i64 {
        self.as_underlying().dts
    }

    pub fn presentation_timestamp(&self) -> i64 {
        self.as_underlying().pts
    }

    fn flags(&self) -> c_int {
        self.as_underlying().flags
    }

    pub fn is_key_frame(&self) -> bool {
        (self.flags() & ff::AV_PKT_FLAG_KEY as i32) != 0
    }
}

impl Drop for AvPacket {
    fn drop(&mut self) {
        unsafe { ff::av_packet_unref(self.as_mut_ptr()); }
    }
}
