use std::mem;
use std::os::raw::c_int;
use std::ptr;
use std::slice;

use ffmpeg_dev::sys as ff;

use crate::ffmpeg::{AvCodecContext, AvFrame, AvError, AvDict};

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

    pub fn send_packet(&mut self, pkt: Packet) -> Result<(), AvError> {
        let side_data = pkt.dcr.map(|dcr| {
            ff::AVPacketSideData {
                data: dcr.as_ptr() as *mut _, // never mutated
                size: dcr.len() as c_int,
                type_: ff::AVPacketSideDataType_AV_PKT_DATA_NEW_EXTRADATA,
            }
        });

        let side_data_list = side_data.as_ref().map(slice::from_ref).unwrap_or(&[]);

        let av_pkt = ff::AVPacket {
            buf: ptr::null_mut(),
            pts: pkt.pts,
            dts: pkt.dts,
            data: pkt.data.as_ptr() as *mut _, // send_packet never mutates data
            size: pkt.data.len() as c_int,
            stream_index: 0,
            flags: 0,
            side_data: side_data_list.as_ptr() as *mut _, // never mutated
            side_data_elems: side_data_list.len() as c_int,
            duration: 0,
            pos: -1,
            convergence_duration: 0,
        };

        let rc = unsafe { ff::avcodec_send_packet(self.ctx.as_mut_ptr(), &av_pkt) };

        mem::drop(av_pkt);

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

pub struct Packet<'a> {
    pub pts: i64,
    pub dts: i64,
    pub data: &'a [u8],
    pub dcr: Option<&'a [u8]>,
}

pub enum RecvFrameError {
    NeedMoreInput,
    Eof,
    Codec(AvError),
}
