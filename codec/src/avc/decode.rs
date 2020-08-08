use std::mem;
use std::os::raw::c_int;
use std::ptr;
use std::slice;

use ffmpeg_dev::sys as ff;

use crate::ffmpeg::{AvCodecContext, AvFrame, AvError, AvDict, AvPacket};

#[derive(Debug)]
pub struct AvcDecoder {
    ctx: AvCodecContext,
}

impl AvcDecoder {
    pub fn new(time_base: usize) -> Result<Self, ()> {
        let codec = unsafe { ff::avcodec_find_decoder(ff::AVCodecID_AV_CODEC_ID_H264) };

        if codec == ptr::null_mut() {
            return Err(());
        }

        let mut ctx = unsafe { AvCodecContext::alloc(codec) };

        let mut opts = AvDict::new();

        // use avcc encoding rather than default of annex-b default:
        opts.set("is_avc", "1");

        // set codec context params
        unsafe {
            let avctx = &mut *ctx.as_mut_ptr();
            avctx.time_base.num = 1;
            avctx.time_base.den = time_base as c_int;
        }

        let rc = unsafe { ff::avcodec_open2(ctx.as_mut_ptr(), codec, opts.as_mut() as *mut *mut _) };

        if rc < 0 {
            return Err(());
        }

        Ok(AvcDecoder { ctx })
    }

    pub fn send_packet(&mut self, pkt: &AvPacket) -> Result<(), AvError> {
        let rc = unsafe { ff::avcodec_send_packet(self.ctx.as_mut_ptr(), pkt.as_ptr()) };

        if rc == 0 {
            Ok(())
        } else {
            Err(AvError(rc))
        }
    }

    pub fn recv_frame(&mut self) -> Result<AvFrame, RecvFrameError> {
        let mut frame = AvFrame::new();
        let rc = unsafe {
            ff::avcodec_receive_frame(self.ctx.as_mut_ptr(), frame.as_mut_ptr())
        };

        const AGAIN: c_int = -(ff::EAGAIN as c_int);
        const EOF: c_int = -0x20464f45; // 'EOF '

        match rc {
            0 => Ok(frame),
            AGAIN => Err(RecvFrameError::NeedMoreInput),
            EOF => Err(RecvFrameError::Eof),
            err => Err(RecvFrameError::Codec(AvError(err))),
        }
    }
}

pub enum RecvFrameError {
    NeedMoreInput,
    Eof,
    Codec(AvError),
}
