use std::convert::TryInto;
use std::ptr;

use ffmpeg_dev::sys as ff;

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

    fn as_underlying(&self) -> &ff::AVFrame {
        unsafe { &*self.as_ptr() }
    }

    fn as_underlying_mut(&mut self) -> &mut ff::AVFrame {
        unsafe { &mut *self.as_mut_ptr() }
    }

    pub fn coded_width(&self) -> usize {
        self.as_underlying().width.try_into().expect("width >= 0")
    }

    pub fn coded_height(&self) -> usize {
        self.as_underlying().height.try_into().expect("height >= 0")
    }

    pub fn picture_width(&self) -> usize {
        let underlying = self.as_underlying();
        self.coded_width() - underlying.crop_left - underlying.crop_right
    }

    pub fn picture_height(&self) -> usize {
        let underlying = self.as_underlying();
        self.coded_height() - underlying.crop_top - underlying.crop_bottom
    }

    pub fn pixel_format(&self) -> ff::AVPixelFormat {
        self.as_underlying().format
    }

    pub fn is_key_frame(&self) -> bool {
        self.as_underlying().key_frame != 0
    }

    pub fn picture_type(&self) -> ff::AVPictureType {
        self.as_underlying().pict_type
    }

    pub fn color_space(&self) -> ff::AVColorSpace {
        self.as_underlying().colorspace
    }

    pub fn set_picture_type(&mut self, pict_type: ff::AVPictureType) {
        self.as_underlying_mut().pict_type = pict_type;
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

