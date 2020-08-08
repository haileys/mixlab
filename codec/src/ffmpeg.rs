use std::ffi::{CStr, CString};
use std::fmt::{self, Debug, Display};
use std::os::raw::c_int;
use std::ptr;

pub use ffmpeg_dev::sys as sys;
use sys as ff;

mod format;
mod frame;
mod ioctx;
mod packet;
mod pixfmt;
mod scale;

pub use frame::{AvFrame, PictureSettings, PictureData, PictureDataMut};
pub use packet::{AvPacket, AvPacketRef, PacketInfo};
pub use scale::SwsContext;
pub use pixfmt::{PixelFormat, PixFmtDescriptor, PlaneInfo, ColorFormat};

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

pub const MIXLAB_IOCTX_ERROR: c_int = -0x6d786c00; // 'M' 'X' 'L' 0x00
pub const MIXLAB_IOCTX_PANIC: c_int = -0x6d786c01; // 'M' 'X' 'L' 0x01

pub struct AvError(pub(crate) c_int);

impl AvError {
    pub fn again(&self) -> bool {
        self.0 == -(ff::EAGAIN as c_int)
    }
}

impl Display for AvError {
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

impl Debug for AvError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AvError {{ code: {:?}, message: {} }}", self.0, self)
    }
}

pub struct AvDict {
    dict: *mut ff::AVDictionary,
}

impl AvDict {
    pub fn new() -> Self {
        AvDict { dict: ptr::null_mut() }
    }

    pub fn as_mut(&mut self) -> &mut *mut ff::AVDictionary {
        &mut self.dict
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let key = CString::new(key).unwrap();
        let value = CString::new(value).unwrap();

        let rc = unsafe {
            ff::av_dict_set(&mut self.dict as *mut *mut _, key.as_ptr(), value.as_ptr(), 0)
        };

        if rc != 0 {
            // only possible failure is ENOMEM
            panic!("av_dict_set_int: ENOMEM");
        }
    }
}

impl Drop for AvDict {
    fn drop(&mut self) {
        unsafe {
            ff::av_dict_free(&mut self.dict as *mut _);
        }
    }
}
