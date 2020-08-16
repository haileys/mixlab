use std::convert::TryInto;
use std::ffi::CStr;
use std::fmt::{self, Debug};
use std::slice;

use ffmpeg_dev::sys as ff;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PixelFormat(ff::AVPixelFormat);

impl PixelFormat {
    pub const fn yuv420p() -> Self {
        PixelFormat(ff::AVPixelFormat_AV_PIX_FMT_YUV420P)
    }

    pub const fn rgba() -> Self {
        PixelFormat(ff::AVPixelFormat_AV_PIX_FMT_RGBA)
    }

    pub unsafe fn from_raw(pixfmt: ff::AVPixelFormat) -> Self {
        PixelFormat(pixfmt)
    }

    pub fn into_raw(self) -> ff::AVPixelFormat {
        self.0
    }

    pub fn name(&self) -> &'static str {
        unsafe {
            let ptr = ff::av_get_pix_fmt_name(self.0);
            CStr::from_ptr(ptr).to_str().expect("CStr::to_str")
        }
    }

    pub fn descriptor(&self) -> PixFmtDescriptor {
        PixFmtDescriptor {
            desc: unsafe { &*ff::av_pix_fmt_desc_get(self.0) },
        }
    }
}

impl Debug for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PixelFormat({:?}; {:?})",
            self.0,
            self.name())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PixFmtDescriptor {
    desc: &'static ff::AVPixFmtDescriptor,
}

impl PixFmtDescriptor {
    pub fn components(&self) -> impl Iterator<Item = PlaneInfo> {
        let components = unsafe {
            let ptr = self.desc.comp.as_ptr() as *const ff::AVComponentDescriptor;
            let len = self.desc.nb_components.into();
            slice::from_raw_parts(ptr, len)
        };

        let desc = *self;

        components.iter().enumerate().map(move |(idx, comp)| {
            PlaneInfo {
                desc,
                idx,
                comp,
            }
        })
    }

    pub fn planar(&self) -> bool {
        (self.desc.flags & ff::AV_PIX_FMT_FLAG_PLANAR as u64) != 0
    }

    pub fn rgb(&self) -> bool {
        (self.desc.flags & ff::AV_PIX_FMT_FLAG_RGB as u64) != 0
    }

    pub fn color(&self) -> ColorFormat {
        let flags = self.desc.flags;

        if (flags & ff::AV_PIX_FMT_FLAG_RGB as u64) != 0 {
            ColorFormat::Rgb
        } else if (flags & ff::AV_PIX_FMT_FLAG_HWACCEL as u64) != 0 {
            ColorFormat::Hwaccel
        } else if (flags & ff::AV_PIX_FMT_FLAG_PAL as u64) != 0 {
            ColorFormat::Palette
        } else if (flags & ff::AV_PIX_FMT_FLAG_PSEUDOPAL as u64) != 0 {
            ColorFormat::PseudoPalette
        } else {
            ColorFormat::Yuv
        }
    }

    /// Amount to shift the luma (Y) width right to find the chroma (U, V) width
    pub fn log2_chroma_w(&self) -> usize {
        self.desc.log2_chroma_w as usize
    }

    /// Amount to shift the luma (Y) height right to find the chroma (U, V) height
    pub fn log2_chroma_h(&self) -> usize {
        self.desc.log2_chroma_h as usize
    }

    pub fn align_horizontal(&self, value: usize) -> usize {
        value & (usize::max_value() << self.log2_chroma_w())
    }

    pub fn align_vertical(&self, value: usize) -> usize {
        value & (usize::max_value() << self.log2_chroma_h())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat {
    Yuv,
    Rgb,
    Hwaccel,
    Palette,
    PseudoPalette,
}

#[derive(Debug)]
pub struct PlaneInfo {
    desc: PixFmtDescriptor,
    idx: usize,
    comp: &'static ff::AVComponentDescriptor,
}

impl PlaneInfo {
    pub fn plane(&self) -> usize {
        self.comp.plane.try_into().unwrap()
    }

    pub fn log2_horz(&self) -> usize {
        if self.desc.color() == ColorFormat::Yuv && self.idx == 0 {
            // luma
            0
        } else {
            self.desc.log2_chroma_w()
        }
    }

    pub fn log2_vert(&self) -> usize {
        if self.desc.color() == ColorFormat::Yuv && self.idx == 0 {
            // luma
            0
        } else {
            self.desc.log2_chroma_h()
        }
    }

    pub fn step(&self) -> usize {
        self.comp.step.try_into().unwrap()
    }

    pub fn offset(&self) -> usize {
        self.comp.offset.try_into().unwrap()
    }

    pub fn shift(&self) -> usize {
        self.comp.shift.try_into().unwrap()
    }

    pub fn depth(&self) -> usize {
        self.comp.depth.try_into().unwrap()
    }
}
