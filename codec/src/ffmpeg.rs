use std::ffi::{CStr, CString};
use std::fmt::{self, Debug, Display};
use std::os::raw::c_int;
use std::ptr;

use ffmpeg_dev::sys as ff;

#[derive(Debug)]
pub struct AvCodecContext {
    ptr: *mut ff::AVCodecContext,
}

unsafe impl Send for AvCodecContext {}

impl AvCodecContext {
    pub unsafe fn alloc(codec: *const ff::AVCodec) -> Self {
        let ptr = unsafe { ff::avcodec_alloc_context3(codec) };

        if ptr == ptr::null_mut() {
            panic!("avcodec_alloc_context3: ENOMEM");
        }

        AvCodecContext { ptr }
    }

    pub fn as_ptr(&mut self) -> *const ff::AVCodecContext {
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

#[derive(Debug)]
pub struct AvFrame {
    ptr: *mut ff::AVFrame,
}

unsafe impl Sync for AvFrame {}
unsafe impl Send for AvFrame {}

impl AvFrame {
    pub fn new() -> Self {
        let ptr = unsafe { ff::av_frame_alloc() };

        if ptr == ptr::null_mut() {
            panic!("av_frame_alloc: ENOMEM");
        }

        AvFrame { ptr }
    }

    pub fn as_ptr(&self) -> *const ff::AVFrame {
        self.ptr as *const _
    }

    pub fn as_mut_ptr(&mut self) -> *mut ff::AVFrame {
        self.ptr
    }
}

impl Clone for AvFrame {
    fn clone(&self) -> Self {
        let ptr = unsafe { ff::av_frame_clone(self.ptr) };

        if ptr == ptr::null_mut() {
            panic!("av_frame_clone: ENOMEM")
        }

        AvFrame { ptr }
    }
}

impl Drop for AvFrame {
    fn drop(&mut self) {
        unsafe {
            ff::av_frame_free(&mut self.ptr as *mut *mut _);
        }
    }
}

pub struct AvError(pub(crate) c_int);

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

#[derive(Debug)]
pub struct AvPacket {
    packet: ff::AVPacket,
}

impl AvPacket {
    pub unsafe fn new(raw: ff::AVPacket) -> Self {
        AvPacket { packet: raw }
    }

    pub fn as_mut_ptr(&mut self) -> *mut ff::AVPacket {
        &mut self.packet as *mut ff::AVPacket
    }
}

impl Drop for AvPacket {
    fn drop(&mut self) {
        unsafe { ff::av_packet_unref(self.as_mut_ptr()); }
    }
}
