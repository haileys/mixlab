use std::ffi::{CStr, CString};
use std::fmt::{self, Display, Debug};
use std::mem;
use std::os::raw::c_int;
use std::ptr;
use std::slice;

use ffmpeg_dev::sys as ff;

use crate::ffmpeg::{AvCodecContext, AvFrame, AvError, AvDict, AvPacket};

#[derive(Debug)]
pub struct AvcEncoder {
    ctx: AvCodecContext,
}

impl AvcEncoder {
    pub fn new(sample_rate: usize) -> Result<Self, AvError> {
        let codec = unsafe { ff::avcodec_find_encoder(ff::AVCodecID_AV_CODEC_ID_H264) };

        if codec == ptr::null_mut() {
            panic!("avcodec_find_encoder: could not find h264 codec");
        }

        let mut ctx = unsafe { AvCodecContext::alloc(codec) };

        let mut opts = AvDict::new();

        // 17 is more or less visually lossless:
        opts.set("crf", "17");

        // chosen arbitrarily
        opts.set("preset", "veryfast");

        // zero latency
        opts.set("tune", "zerolatency");

        // set initial settings
        unsafe {
            let avctx = &mut *ctx.as_mut_ptr();
            avctx.time_base.num = 1;
            avctx.time_base.den = sample_rate as c_int;
        }

        // open codec
        let rc = unsafe { ff::avcodec_open2(ctx.as_mut_ptr(), codec, opts.as_mut() as *mut *mut _) };

        if rc < 0 {
            return Err(AvError(rc));
        }

        Ok(AvcEncoder { ctx })
    }

    pub fn send_frame(&mut self, frame: AvFrame) -> Result<(), AvError> {
        let rc = unsafe { ff::avcodec_send_frame(self.ctx.as_mut_ptr(), frame.as_ptr()) };

        if rc < 0 {
            return Err(AvError(rc));
        }

        Ok(())
    }

    pub fn recv_packet(&mut self) -> Result<AvPacket, AvError> {
        // set up zeroed packet:
        let mut packet = unsafe {
            AvPacket::new(ff::AVPacket {
                buf: ptr::null_mut(),
                pts: 0,
                dts: 0,
                data: ptr::null_mut(),
                size: 0,
                stream_index: 0,
                flags: 0,
                side_data: ptr::null_mut(),
                side_data_elems: 0,
                duration: 0,
                pos: 0,
                convergence_duration: 0,
            })
        };

        let rc = unsafe { ff::avcodec_receive_packet(self.ctx.as_mut_ptr(), packet.as_mut_ptr()) };

        if rc < 0 {
            return Err(AvError(rc));
        }

        Ok(packet)
    }
}
