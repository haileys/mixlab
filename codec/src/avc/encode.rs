use std::convert::TryInto;
use std::os::raw::c_int;
use std::ptr;
use std::slice;

use bytes::Bytes;
use ffmpeg_dev::sys as ff;

use crate::avc::{bitstream, nal, AvcError, DecoderConfigurationRecord};
use crate::ffmpeg::{AvCodecContext, AvFrame, AvError, AvDict, AvPacket};

#[derive(Debug)]
pub struct AvcEncoder {
    ctx: AvCodecContext,
}

pub struct AvcParams {
    pub time_base: usize,
    pub pixel_format: ff::AVPixelFormat,
    pub color_space: ff::AVColorSpace,
    pub picture_width: usize,
    pub picture_height: usize,
}

impl AvcEncoder {
    pub fn new(params: AvcParams) -> Result<Self, AvError> {
        let codec = unsafe { ff::avcodec_find_encoder(ff::AVCodecID_AV_CODEC_ID_H264) };

        if codec == ptr::null_mut() {
            panic!("avcodec_find_encoder: could not find h264 codec");
        }

        let mut ctx = unsafe { AvCodecContext::alloc(codec) };

        let mut opts = AvDict::new();

        // 17 is more or less visually lossless:
        opts.set("crf", "17");

        // chosen arbitrarily
        opts.set("preset", "veryfast");

        // zero latency
        opts.set("tune", "zerolatency");

        // disable annex-b encoding (on by default)
        opts.set("x264-params", "annexb=0");

        // set codec context params
        unsafe {
            let avctx = &mut *ctx.as_mut_ptr();
            avctx.profile = ff::FF_PROFILE_H264_HIGH as i32;
            avctx.level = 41;
            avctx.width = params.picture_width.try_into().expect("picture_width too large");
            avctx.height = params.picture_height.try_into().expect("picture_height too large");
            avctx.colorspace = params.color_space;
            avctx.pix_fmt = params.pixel_format;
            avctx.time_base.num = 1;
            avctx.time_base.den = params.time_base as c_int;
            avctx.flags |= ff::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
        }

        // open codec
        let rc = unsafe { ff::avcodec_open2(ctx.as_mut_ptr(), codec, opts.as_mut() as *mut *mut _) };

        if rc < 0 {
            return Err(AvError(rc));
        }

        Ok(AvcEncoder { ctx })
    }

    pub fn header_nals(&self) -> impl Iterator<Item = Result<nal::Unit, AvcError>> {
        unsafe {
            let ctx = &*self.ctx.as_ptr();
            let data = slice::from_raw_parts(ctx.extradata,
                ctx.extradata_size.try_into().expect("extradata_size >= 0"));
            bitstream::read(Bytes::copy_from_slice(data), 4 /* hardcode 4 for now... */)
        }
    }

    pub fn decoder_configuration_record(&self) -> DecoderConfigurationRecord {
        let mut header_nals = self.header_nals();

        let sps = header_nals.next().expect("expected SPS").unwrap();
        if sps.kind != nal::UnitType::SequenceParameterSet {
            panic!("first nal in extradata is not SPS");
        }

        let pps = header_nals.next().expect("expected PPS").unwrap();
        if pps.kind != nal::UnitType::PictureParameterSet {
            panic!("second nal in extradata is not PPS");
        }

        if sps.data.len() < 3 {
            panic!("SPS len < 3");
        }

        DecoderConfigurationRecord {
            version: 1,
            // these fields appear to simply be the same as the first fields in the SPS:
            profile_indication: sps.data[0],
            profile_compatibility: sps.data[1],
            level_indication: sps.data[2],
            nalu_size: 4,
            sps: vec![sps],
            pps: vec![pps],
        }
    }

    pub fn send_frame(&mut self, frame: AvFrame) -> Result<(), AvError> {
        let rc = unsafe { ff::avcodec_send_frame(self.ctx.as_mut_ptr(), frame.as_ptr()) };

        if rc < 0 {
            return Err(AvError(rc));
        }

        Ok(())
    }

    pub fn recv_packet(&mut self) -> Result<AvPacket, AvError> {
        // set up zeroed packet:
        let mut packet = unsafe {
            AvPacket::new(ff::AVPacket {
                buf: ptr::null_mut(),
                pts: 0,
                dts: 0,
                data: ptr::null_mut(),
                size: 0,
                stream_index: 0,
                flags: 0,
                side_data: ptr::null_mut(),
                side_data_elems: 0,
                duration: 0,
                pos: 0,
                convergence_duration: 0,
            })
        };

        let rc = unsafe { ff::avcodec_receive_packet(self.ctx.as_mut_ptr(), packet.as_mut_ptr()) };

        if rc < 0 {
            return Err(AvError(rc));
        }

        Ok(packet)
    }
}
