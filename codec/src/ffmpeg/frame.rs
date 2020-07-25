use std::convert::TryInto;
use std::marker::PhantomData;
use std::os::raw::c_int;
use std::ptr;

use ffmpeg_dev::sys as ff;

use crate::ffmpeg::{AvError, PixelFormat, ColorFormat};

#[derive(Debug)]
pub struct AvFrame {
    ptr: *mut ff::AVFrame,
}

// ffmpeg buffer refcounts are threadsafe
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

    pub fn blank(settings: &PictureSettings) -> Self {
        println!("blank");

        let mut frame = Self::new();

        let underlying = frame.as_underlying_mut();
        underlying.width = settings.width.try_into().expect("width too large");
        underlying.height = settings.height.try_into().expect("height too large");
        underlying.format = settings.pixel_format.into_raw();

        unsafe {
            ff::av_frame_get_buffer(frame.as_mut_ptr(), 0);
        }

        // zero frame

        let frame_data = frame.frame_data_mut();
        let pixdesc = settings.pixel_format.descriptor();
        let mut cleared = [false; 8];

        for (idx, comp) in pixdesc.components().enumerate() {
            let plane = comp.plane();

            if cleared[plane] {
                continue;
            }

            cleared[plane] = true;

            let is_chroma = match pixdesc.color() {
                ColorFormat::Yuv => idx > 0,
                _ => false,
            };

            let width = if is_chroma {
                settings.width >> pixdesc.log2_chroma_w()
            } else {
                settings.width
            };

            let height = if is_chroma {
                settings.height >> pixdesc.log2_chroma_h()
            } else {
                settings.height
            };

            let stride: usize = frame_data.stride[plane].try_into().unwrap();
            let data = frame_data.data[plane];

            // we must be careful not to use stride for every line lest we
            // overrun the end of the buffer. the padding in stride is only
            // guaranteed to exist between lines.
            let size = stride * height.saturating_sub(1) + comp.step() * width;

            let byte = if is_chroma {
                0x80
            } else {
                0
            };

            unsafe { ptr::write_bytes(data, byte, size); }
        }

        frame
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

    // TODO - get rid of the cropping for mixlab's internal frame type
    pub fn picture_width(&self) -> usize {
        let underlying = self.as_underlying();
        self.coded_width() - underlying.crop_left - underlying.crop_right
    }

    // TODO - get rid of the cropping for mixlab's internal frame type
    pub fn picture_height(&self) -> usize {
        let underlying = self.as_underlying();
        self.coded_height() - underlying.crop_top - underlying.crop_bottom
    }

    pub fn pixel_format(&self) -> PixelFormat {
        unsafe { PixelFormat::from_raw(self.as_underlying().format) }
    }

    pub fn is_key_frame(&self) -> bool {
        self.as_underlying().key_frame != 0
    }

    pub fn picture_type(&self) -> ff::AVPictureType {
        self.as_underlying().pict_type
    }

    pub fn set_picture_type(&mut self, pict_type: ff::AVPictureType) {
        self.as_underlying_mut().pict_type = pict_type;
    }

    pub fn color_space(&self) -> ff::AVColorSpace {
        self.as_underlying().colorspace
    }

    pub fn decode_timestamp(&self) -> i64 {
        self.as_underlying().pkt_dts
    }

    pub fn presentation_timestamp(&self) -> i64 {
        self.as_underlying().pts
    }

    pub fn set_presentation_timestamp(&mut self, pts: i64) {
        self.as_underlying_mut().pts = pts;
    }

    pub fn packet_duration(&self) -> i64 {
        self.as_underlying().pkt_duration
    }

    pub fn picture_settings(&self) -> PictureSettings {
        PictureSettings {
            width: self.coded_width(),
            height: self.coded_height(),
            pixel_format: self.pixel_format(),
        }
    }

    pub fn copy_props_from(&mut self, other: &AvFrame) {
        unsafe {
            let rc = ff::av_frame_copy_props(self.as_mut_ptr(), other.as_ptr());

            if rc != 0 {
                panic!("av_frame_copy_props: {:?}", AvError(rc));
            }
        }
    }

    pub fn frame_data(&self) -> PictureData {
        let underlying = self.as_underlying();

        PictureData {
            picture: self.picture_settings(),
            data: underlying.data,
            stride: underlying.linesize,
            _phantom: PhantomData,
        }
    }

    pub fn frame_data_mut(&mut self) -> PictureDataMut {
        unsafe {
            let rc = ff::av_frame_make_writable(self.ptr);

            if rc != 0 {
                panic!("av_frame_make_writable: {:?}", AvError(rc))
            }
        }

        let picture = self.picture_settings();
        let underlying = self.as_underlying_mut();

        PictureDataMut {
            picture: picture,
            data: underlying.data,
            stride: underlying.linesize,
            _phantom: PhantomData,
        }
    }

    fn subframe(&self, x: usize, y: usize, w: usize, h: usize) -> (PictureSettings, PlanarData, PlanarStride) {
        let right = x.checked_add(w).expect("x + w overflow");
        let bottom = y.checked_add(h).expect("y + h overflow");

        if x > self.coded_width() || right > self.coded_width() {
            panic!("horizontal section out of bounds (x: {:?}, w: {:?}, picture width: {:?})",
                x, w, self.coded_width());
        }

        if y > self.coded_height() || bottom > self.coded_height() {
            panic!("vertical section out of bounds (y: {:?}, h: {:?}, picture height: {:?})",
                y, h, self.coded_height());
        }

        // scale x and y to align to log2_chroma boundary
        let pixdesc = self.pixel_format().descriptor();

        let x = pixdesc.align_horizontal(x);
        let y = pixdesc.align_vertical(y);

        let w = pixdesc.align_horizontal(right) - x;
        let h = pixdesc.align_vertical(bottom) - y;

        let picture = PictureSettings {
            width: w,
            height: h,
            pixel_format: self.pixel_format(),
        };

        let mut data = [ptr::null_mut(); 8];
        let underlying = self.as_underlying();

        // TODO - this should work just fine for non-planar pixfmts as long as
        // we don't mutate data - only assign into it from underlying.data
        for (idx, component) in pixdesc.components().enumerate() {
            let is_chroma = match pixdesc.color() {
                ColorFormat::Yuv => idx > 0,
                _ => false,
            };

            let plane = component.plane();

            let x_off = if is_chroma {
                x >> pixdesc.log2_chroma_w()
            } else {
                x
            };

            let y_off = if is_chroma {
                y >> pixdesc.log2_chroma_h()
            } else {
                y
            };

            data[plane] = unsafe {
                underlying.data[plane]
                    .add(x_off * component.step())
                    .add(y_off * underlying.linesize[plane] as usize)
            };
        }

        (picture, data, underlying.linesize)
    }

    pub fn subframe_data(&self, x: usize, y: usize, w: usize, h: usize) -> PictureData {
        let (picture, data, stride) = self.subframe(x, y, w, h);

        PictureData {
            picture,
            data,
            stride,
            _phantom: PhantomData,
        }
    }

    pub fn subframe_data_mut(&self, x: usize, y: usize, w: usize, h: usize) -> PictureDataMut {
        let (picture, data, stride) = self.subframe(x, y, w, h);

        PictureDataMut {
            picture,
            data,
            stride,
            _phantom: PhantomData,
        }
    }
}

type PlanarData = [*mut u8; ff::AV_NUM_DATA_POINTERS as usize];
type PlanarStride = [c_int; ff::AV_NUM_DATA_POINTERS as usize];

pub struct PictureData<'a> {
    pub(in crate::ffmpeg) picture: PictureSettings,
    pub(in crate::ffmpeg) data: PlanarData,
    pub(in crate::ffmpeg) stride: PlanarStride,
    _phantom: PhantomData<&'a AvFrame>,
}

impl<'a> PictureData<'a> {
    pub fn picture_settings(&self) -> &PictureSettings {
        &self.picture
    }

    pub unsafe fn data(&self, plane: usize) -> *const u8 {
        self.data[plane] as *const u8
    }

    pub unsafe fn stride(&self, plane: usize) -> usize {
        self.stride[plane] as usize
    }
}

pub struct PictureDataMut<'a> {
    pub(in crate::ffmpeg) picture: PictureSettings,
    pub(in crate::ffmpeg) data: PlanarData,
    pub(in crate::ffmpeg) stride: PlanarStride,
    _phantom: PhantomData<&'a mut AvFrame>,
}

impl<'a> PictureDataMut<'a> {
    pub fn picture_settings(&self) -> &PictureSettings {
        &self.picture
    }

    pub unsafe fn data(&self, plane: usize) -> *mut u8 {
        self.data[plane]
    }

    pub unsafe fn stride(&self, plane: usize) -> usize {
        self.stride[plane] as usize
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureSettings {
    pub width: usize,
    pub height: usize,
    pub pixel_format: PixelFormat,
}

impl PictureSettings {
    pub fn yuv420p(width: usize, height: usize) -> Self {
        PictureSettings {
            width,
            height,
            pixel_format: PixelFormat::yuv420p(),
        }
    }
}
