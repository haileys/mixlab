use std::convert::TryInto;
use std::ptr;

use ffmpeg_dev::sys as ff;

use crate::ffmpeg::{AvFrame, PictureSettings, PictureData, PictureDataMut};

#[derive(Debug)]
pub struct SwsContext {
    ptr: *mut ff::SwsContext,
    input: PictureSettings,
    output: PictureSettings,
}

impl SwsContext {
    pub fn new(input: PictureSettings, output: PictureSettings) -> Self {
        let input_width: i32 = input.width.try_into().expect("input_width too large");
        let input_height: i32 = input.height.try_into().expect("input_height too large");
        let output_width: i32 = output.width.try_into().expect("output_width too large");
        let output_height: i32 = output.height.try_into().expect("output_height too large");

        let ptr = unsafe {
            ff::sws_getContext(
                input_width, input_height, input.pixel_format.into_raw(),
                output_width, output_height, output.pixel_format.into_raw(),
                ff::SWS_BICUBIC as i32, ptr::null_mut(), ptr::null_mut(), ptr::null(),
            )
        };

        if ptr == ptr::null_mut() {
            panic!("sws_context_alloc: ENOMEM");
        }

        SwsContext {
            ptr,
            input,
            output,
        }
    }

    pub fn input_settings(&self) -> &PictureSettings {
        &self.input
    }

    pub fn output_settings(&self) -> &PictureSettings {
        &self.output
    }

    pub fn process(&mut self, input: &PictureData, output: &mut PictureDataMut) {
        let input_settings = input.picture_settings();
        let output_settings = output.picture_settings();

        if input_settings != &self.input {
            panic!("wrong picture settings for input frame: {:?}; expected: {:?}", input_settings, self.input);
        }

        if output_settings != &self.output {
            panic!("wrong picture settings for output frame: {:?}; expected: {:?}", output_settings, self.output);
        }

        let input_data = input.data.as_ptr() as *const *const _;
        let input_stride = input.stride.as_ptr();

        let output_data = output.data.as_mut_ptr();
        let output_stride = output.stride.as_mut_ptr();

        unsafe {
            ff::sws_scale(self.ptr, input_data, input_stride, 0, input_settings.height as i32, output_data, output_stride);
        }
    }
}

impl Drop for SwsContext {
    fn drop(&mut self) {
        unsafe { ff::sws_freeContext(self.ptr); }
    }
}
