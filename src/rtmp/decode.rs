use std::ffi::{CStr, CString};
use std::fmt::{self, Display, Debug};
use std::mem;
use std::os::raw::c_int;
use std::ptr;
use std::slice;

use ffmpeg_dev::sys as ff;

use mixlab_codec::ffmpeg::AvFrame;

pub struct H264Decoder {
    ctx: CodecContext,
}

impl H264Decoder {
    pub fn new() -> Result<Self, ()> {
        let ctx = CodecContext::open()?;

        Ok(H264Decoder { ctx })
    }

    pub fn send_packet(&mut self, pkt: Packet) -> Result<(), AVError> {
        let side_data = pkt.dcr.map(|dcr| {
            ff::AVPacketSideData {
                data: dcr.as_ptr() as *mut _, // never mutated
                size: dcr.len() as c_int,
                type_: ff::AVPacketSideDataType_AV_PKT_DATA_NEW_EXTRADATA,
            }
        });

        let side_data_list = side_data.as_ref().map(slice::from_ref).unwrap_or(&[]);

        let flags = if pkt.is_key_frame {
            ff::AV_PKT_FLAG_KEY as i32
        } else {
            0
        };

        let av_pkt = ff::AVPacket {
            buf: ptr::null_mut(),
            pts: pkt.pts,
            dts: pkt.dts,
            data: pkt.data.as_ptr() as *mut _, // send_packet never mutates data
            size: pkt.data.len() as c_int,
            stream_index: 0,
            flags: flags,
            side_data: side_data_list.as_ptr() as *mut _, // never mutated
            side_data_elems: side_data_list.len() as c_int,
            duration: 0,
            pos: -1,
            convergence_duration: 0,
        };

        let rc = unsafe { ff::avcodec_send_packet(self.ctx.ptr, &av_pkt) };

        mem::drop(av_pkt);

        if rc == 0 {
            Ok(())
        } else {
            Err(AVError(rc))
        }
    }

    pub fn recv_frame(&mut self) -> Result<AvFrame, RecvFrameError> {
        let mut frame = AvFrame::new();
        let rc = unsafe {
            ff::avcodec_receive_frame(self.ctx.ptr, frame.as_mut_ptr())
        };

        const AGAIN: c_int = -(ff::EAGAIN as c_int);
        const EOF: c_int = -0x20464f45; // 'EOF '

        match rc {
            0 => Ok(frame),
            AGAIN => Err(RecvFrameError::NeedMoreInput),
            EOF => Err(RecvFrameError::Eof),
            err => Err(RecvFrameError::Codec(AVError(err))),
        }
    }
}

pub struct Packet<'a> {
    pub pts: i64,
    pub dts: i64,
    pub data: &'a [u8],
    pub dcr: Option<&'a [u8]>,
    pub is_key_frame: bool,
}

pub enum RecvFrameError {
    NeedMoreInput,
    Eof,
    Codec(AVError),
}

pub struct AVError(c_int);

impl Display for AVError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut msg_buf = [0i8; ff::AV_ERROR_MAX_STRING_SIZE as usize];
        let rc = unsafe { ff::av_strerror(self.0, msg_buf.as_mut_ptr(), msg_buf.len()) };

        if rc < 0 {
            return write!(f, "Unknown");
        }

        let msg = unsafe { CStr::from_ptr(&msg_buf as *const _) };
        write!(f, "{}", msg.to_string_lossy())
    }
}

impl Debug for AVError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AVError {{ code: {:?}, message: {} }}", self.0, self)
    }
}

struct CodecContext {
    ptr: *mut ff::AVCodecContext,
}

unsafe impl Send for CodecContext {}

impl CodecContext {
    pub fn open() -> Result<Self, ()> {
        unsafe {
            let codec = ff::avcodec_find_decoder(ff::AVCodecID_AV_CODEC_ID_H264);

            if codec == ptr::null_mut() {
                return Err(());
            }

            let ptr = ff::avcodec_alloc_context3(codec);

            if ptr == ptr::null_mut() {
                return Err(());
            }

            let mut opts = Dict::new();
            opts.set_int(&CString::new("is_avc").unwrap(), 1);

            let rc = ff::avcodec_open2(ptr, codec, &mut opts.dict as *mut *mut _);

            if rc < 0 {
                return Err(());
            }

            Ok(CodecContext { ptr })
        }
    }
}

impl Drop for CodecContext {
    fn drop(&mut self) {
        unsafe {
            ff::avcodec_free_context(&mut self.ptr as *mut *mut _);
        }
    }
}

struct Dict {
    dict: *mut ff::AVDictionary,
}

impl Dict {
    pub fn new() -> Self {
        Dict { dict: ptr::null_mut() }
    }

    pub fn set_int(&mut self, key: &CStr, value: i64) {
        let rc = unsafe {
            ff::av_dict_set_int(&mut self.dict as *mut *mut _, key.as_ptr(), value, 0)
        };

        if rc != 0 {
            // only possible failure is ENOMEM
            panic!("av_dict_set_int: ENOMEM");
        }
    }
}

impl Drop for Dict {
    fn drop(&mut self) {
        unsafe {
            ff::av_dict_free(&mut self.dict as *mut _);
        }
    }
}
