use std::convert::{TryInto, TryFrom};
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::raw::c_int;
use std::ptr;
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

    pub fn stream_index(&self) -> i32 {
        self.as_underlying().stream_index
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

    pub fn duration(&self) -> Option<i64> {
        let duration = self.as_underlying().duration;

        if duration == 0 {
            None
        } else {
            Some(duration)
        }
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

pub struct AvPacketRef<'a> {
    packet: AvPacket,
    phantom: PhantomData<&'a [u8]>
}

impl<'a> AvPacketRef<'a> {
    pub fn borrowed(info: PacketInfo<'a>) -> Self {
        let packet = ff::AVPacket {
            buf: ptr::null_mut(),
            pts: info.pts,
            dts: info.dts,
            data: info.data.as_ptr() as *mut _, // send_packet never mutates data
            size: c_int::try_from(info.data.len()).expect("packet size too large for c_int"),
            stream_index: 0,
            flags: 0,
            side_data: info.side_data.as_ptr() as *mut _, // never mutated
            side_data_elems: c_int::try_from(info.side_data.len()).expect("side_data_elems too large for c_int"),
            duration: 0,
            pos: -1,
            convergence_duration: 0,
        };

        AvPacketRef {
            packet: AvPacket { packet },
            phantom: PhantomData,
        }
    }
}

impl<'a> Deref for AvPacketRef<'a> {
    type Target = AvPacket;

    fn deref(&self) -> &AvPacket {
        &self.packet
    }
}

pub struct PacketInfo<'a> {
    pub pts: i64,
    pub dts: i64,
    pub data: &'a [u8],
    pub dcr: Option<&'a [u8]>,
    pub side_data: &'a [PacketSideData<'a>],
}

#[repr(transparent)]
pub struct PacketSideData<'a> {
    side_data: ff::AVPacketSideData,
    phantom: PhantomData<&'a [u8]>,
}

impl<'a> PacketSideData<'a> {
    pub fn borrowed(data: &'a [u8], type_: ff::AVPacketSideDataType) -> Self {
        PacketSideData {
            side_data: ff::AVPacketSideData {
                data: data.as_ptr() as *mut u8, // never mutated
                size: data.len().try_into().expect("c_int from side data size"),
                type_,
            },
            phantom: PhantomData,
        }
    }
}
