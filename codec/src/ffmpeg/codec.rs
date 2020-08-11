use std::convert::TryFrom;
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::raw::c_int;
use std::ptr;

use derive_more::From;
use ffmpeg_dev::sys as ff;
use mixlab_util::time::TimeBase;

use crate::ffmpeg::{AvError, AvDict, AvPacket, AvFrame, AGAIN, EOF};
use crate::ffmpeg::media::{Video, MediaType};

pub struct CodecBuilder<'a, FrameType> {
    codec: &'static ff::AVCodec,
    time_base: TimeBase,
    opts: AvDict,
    parameters: Option<AvCodecParameters<'a>>,
    extradata: Option<&'a [u8]>,
    phantom: PhantomData<FrameType>,
}

impl<'a> CodecBuilder<'a, Video> {
    pub fn h264(time_base: TimeBase) -> CodecBuilder<'a, Video> {
        match Self::new(ff::AVCodecID_AV_CODEC_ID_H264, time_base) {
            Ok(builder) => builder,
            Err(BuildError::MediaTypeMismatch) => unreachable!(),
            Err(BuildError::CodecNotFound) => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum BuildError {
    MediaTypeMismatch,
    CodecNotFound,
}

impl<'a, FrameType: MediaType> CodecBuilder<'a, FrameType> {
    pub fn new(codec_id: ff::AVCodecID, time_base: TimeBase) -> Result<Self, BuildError> {
        let codec = unsafe { ff::avcodec_find_decoder(codec_id) };

        if codec == ptr::null_mut() {
            return Err(BuildError::CodecNotFound);
        }

        let codec = unsafe { &*codec };

        if codec.type_ != FrameType::FFMPEG_MEDIA_TYPE {
            return Err(BuildError::MediaTypeMismatch);
        }

        Ok(Self {
            codec,
            time_base,
            opts: AvDict::new(),
            parameters: None,
            extradata: None,
            phantom: PhantomData,
        })
    }

    pub fn with_opt(mut self, name: &str, value: &str) -> Self {
        self.opts.set(name, value);
        self
    }

    pub fn with_parameters(mut self, parameters: AvCodecParameters<'a>) -> Self {
        self.parameters = Some(parameters);
        self
    }

    pub fn with_extradata(mut self, extradata: &'a [u8]) -> Self {
        self.extradata = Some(extradata);
        self
    }

    pub fn open_decoder(mut self) -> Result<Decode<Video>, OpenError> {
        // alloc codec
        let mut ctx = unsafe { AvCodecContext::alloc(self.codec) };

        // copy codec parameters
        if let Some(parameters) = self.parameters {
            let rc = unsafe {
                ff::avcodec_parameters_to_context(ctx.as_mut_ptr(), parameters.0)
            };

            if rc != 0 {
                return Err(OpenError::Av(AvError(rc)));
            }
        }

        // set certain params directly on context
        unsafe {
            let underlying = &mut *ctx.as_mut_ptr();

            underlying.time_base.num = *self.time_base.as_rational().numer();
            underlying.time_base.den = *self.time_base.as_rational().denom();

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
            ff::avcodec_open2(ctx.as_mut_ptr(), self.codec, self.opts.as_mut() as *mut *mut _)
        };

        if rc != 0 {
            return Err(OpenError::Av(AvError(rc)));
        }

        Ok(Decode::new(ctx))
    }
}

/// wrapper type around a reference to AVCodecParameters with an unsafe
/// constructor that vouches for the soundness of its parameters
pub struct AvCodecParameters<'a>(&'a ff::AVCodecParameters);

impl<'a> AvCodecParameters<'a> {
    pub unsafe fn from_raw(params: &'a ff::AVCodecParameters) -> Self {
        AvCodecParameters(params)
    }
}

impl<'a> Deref for AvCodecParameters<'a> {
    type Target = ff::AVCodecParameters;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct Decode<FrameType> {
    ctx: AvCodecContext,
    phantom: PhantomData<FrameType>,
}

impl<FrameType> Decode<FrameType> {
    fn new(ctx: AvCodecContext) -> Self {
        Decode {
            ctx,
            phantom: PhantomData,
        }
    }

    pub fn send_packet(&mut self, pkt: &AvPacket) -> Result<(), AvError> {
        let rc = unsafe { ff::avcodec_send_packet(self.ctx.as_mut_ptr(), pkt.as_ptr()) };

        if rc == 0 {
            Ok(())
        } else {
            Err(AvError(rc))
        }
    }

    pub fn end_of_stream(&mut self) -> Result<(), AvError> {
        let rc = unsafe { ff::avcodec_send_packet(self.ctx.as_mut_ptr(), ptr::null()) };

        if rc == 0 {
            Ok(())
        } else {
            Err(AvError(rc))
        }
    }

    pub fn flush_buffers(&mut self) {
        unsafe { ff::avcodec_flush_buffers(self.ctx.as_mut_ptr()); }
    }

    pub fn recv_frame(&mut self) -> Result<AvFrame, RecvFrameError> {
        let mut frame = AvFrame::new();
        let rc = unsafe {
            ff::avcodec_receive_frame(self.ctx.as_mut_ptr(), frame.as_mut_ptr())
        };

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
