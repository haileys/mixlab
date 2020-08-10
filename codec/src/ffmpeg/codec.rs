use std::ptr;

use ffmpeg_dev::sys as ff;

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

