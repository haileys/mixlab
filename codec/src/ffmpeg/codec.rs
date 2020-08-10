use std::convert::TryFrom;
use std::os::raw::c_int;
use std::ptr;

use derive_more::From;
use ffmpeg_dev::sys as ff;
use num_rational::Rational32;

use crate::ffmpeg::{AvError, AvDict, AvPacket, AvFrame};

pub struct CodecBuilder<'a> {
    codec_id: ff::AVCodecID,
    time_base: Rational32,
    opts: AvDict,
    parameters: Option<&'a ff::AVCodecParameters>,
    extradata: Option<&'a [u8]>,
}

impl<'a> CodecBuilder<'a> {
    pub fn h264(time_base: Rational32) -> Self {
        Self::new(ff::AVCodecID_AV_CODEC_ID_H264, time_base)
    }

    pub fn new(codec_id: ff::AVCodecID, time_base: Rational32) -> Self {
        Self {
            codec_id,
            time_base,
            opts: AvDict::new(),
            parameters: None,
            extradata: None,
        }
    }

    pub fn with_opt(mut self, name: &str, value: &str) -> Self {
        self.opts.set(name, value);
        self
    }

    /// unsafe because params contains pointers that must be valid
    pub unsafe fn with_parameters(mut self, parameters: &'a ff::AVCodecParameters) -> Self {
        self.parameters = Some(parameters);
        self
    }

    pub fn with_extradata(mut self, extradata: &'a [u8]) -> Self {
        self.extradata = Some(extradata);
        self
    }

    pub fn open_decoder(mut self) -> Result<DecodeContext, OpenError> {
        // find codec
        let codec = unsafe { ff::avcodec_find_decoder(self.codec_id) };

        if codec == ptr::null_mut() {
            return Err(OpenError::CodecNotFound);
        }

        // alloc codec
        let mut ctx = unsafe { AvCodecContext::alloc(codec) };

        // copy codec parameters
        if let Some(parameters) = self.parameters {
            let rc = unsafe {
                ff::avcodec_parameters_to_context(ctx.as_mut_ptr(), parameters)
            };

            if rc != 0 {
                return Err(OpenError::Av(AvError(rc)));
            }
        }

        // set certain params directly on context
        unsafe {
            let underlying = &mut *ctx.as_mut_ptr();

            underlying.time_base.num = *self.time_base.numer();
            underlying.time_base.den = *self.time_base.denom();

            if let Some(extradata_bytes) = self.extradata {
                let extradata_int_len = c_int::try_from(extradata_bytes.len())
                    .expect("extradata length too large for c_int");

                // extradata must be allocated through ffmpeg allocator
                let extradata_alloc_len = extradata_bytes.len() + ff::AV_INPUT_BUFFER_PADDING_SIZE as usize;
                let extradata = ff::av_mallocz(extradata_alloc_len) as *mut u8;
                if extradata == ptr::null_mut() {
                    panic!("could not allocate extradata");
                }

                // copy extradata
                ptr::copy(extradata_bytes.as_ptr(), extradata, extradata_bytes.len());
                underlying.extradata = extradata;
                underlying.extradata_size = extradata_int_len;
            }
        }

        // open codec
        let rc = unsafe {
            ff::avcodec_open2(ctx.as_mut_ptr(), codec, self.opts.as_mut() as *mut *mut _)
        };

        if rc != 0 {
            return Err(OpenError::Av(AvError(rc)));
        }

        Ok(DecodeContext { ctx })
    }
}

#[derive(Debug)]
pub struct DecodeContext {
    ctx: AvCodecContext,
}

impl DecodeContext {
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

#[derive(Debug, From)]
pub enum OpenError {
    CodecNotFound,
    Av(AvError),
}


#[derive(Debug, From)]
pub enum RecvFrameError {
    NeedMoreInput,
    Eof,
    Codec(AvError),
}

#[derive(Debug)]
pub struct AvCodecContext {
    ptr: *mut ff::AVCodecContext,
}

unsafe impl Send for AvCodecContext {}

impl AvCodecContext {
    pub unsafe fn alloc(codec: *const ff::AVCodec) -> Self {
        let ptr = ff::avcodec_alloc_context3(codec);

        if ptr == ptr::null_mut() {
            panic!("avcodec_alloc_context3: ENOMEM");
        }

        AvCodecContext { ptr }
    }

    pub fn as_ptr(&self) -> *const ff::AVCodecContext {
        self.ptr as *const _
    }

    pub fn as_mut_ptr(&mut self) -> *mut ff::AVCodecContext {
        self.ptr
    }
}

impl Drop for AvCodecContext {
    fn drop(&mut self) {
        unsafe {
            ff::avcodec_free_context(&mut self.ptr as *mut *mut _);
        }
    }
}
