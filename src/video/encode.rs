use std::collections::VecDeque;
use std::convert::TryInto;

use bytes::Bytes;
use fdk_aac::enc as aac;
use num_rational::Rational64;

use mixlab_codec::avc::DecoderConfigurationRecord;
use mixlab_codec::avc::encode::{AvcEncoder, AvcParams};
use mixlab_codec::ffmpeg::AvFrame;
use mixlab_codec::ffmpeg::sys;
use mixlab_mux::mp4::{self, AdtsFrame};

use crate::engine::Sample;

// must match AAC encoder's granule size
const SAMPLES_PER_CHANNEL_PER_FRAGMENT: usize = 1024;

const AUDIO_CHANNELS: usize = 2;

#[derive(Debug)]
pub struct EncodeStream {
    segments: VecDeque<StreamSegment>,
    audio_timestamp: Rational64,
    audio_ctx: AudioCtx,
    video_timestamp: Rational64,
    video_ctx: VideoCtx,
}

impl EncodeStream {
    pub fn new(audio_ctx: AudioCtx, video_ctx: VideoCtx) -> Self {
        EncodeStream {
            segments: VecDeque::new(),
            audio_timestamp: Rational64::new(0, 1),
            audio_ctx,
            video_timestamp: Rational64::new(0, 1),
            video_ctx,
        }
    }

    pub fn send_audio(&mut self, samples: &[f32]) {
        if let Some((duration, frame)) = self.audio_ctx.send_audio(samples) {
            let timescale = self.audio_ctx.sample_rate;

            let new_timestamp = self.audio_timestamp + duration;

            let previous_ts = (self.audio_timestamp * timescale).to_integer();
            let new_ts = (new_timestamp * timescale).to_integer();
            let duration = new_ts - previous_ts;

            let duration = duration.try_into().expect("duration too large");

            self.segments.push_back(StreamSegment::Audio { duration, frame });

            self.audio_timestamp = new_timestamp;
        }
    }

    pub fn send_video(&mut self, timestamp: Rational64, duration_hint: Rational64, frame: AvFrame) {
        let end_timestamp = timestamp + duration_hint;

        if end_timestamp < self.video_timestamp {
            // frame ends before current time stamp, drop it
            return;
        }

        // recalculate duration as being the time span between end of the last
        // frame and the end of this frame to account for small gaps between the
        // end of the last frame and start of this frame due to timestamp
        // imprecision on the input side:
        let duration = end_timestamp - self.video_timestamp;

        self.encode_video(duration, frame);
    }

    pub fn barrier(&mut self, timestamp: Rational64) {
        if self.video_timestamp < timestamp {
            let duration = timestamp - self.video_timestamp;
            let frame = self.video_ctx.blank_frame();
            self.encode_video(duration, frame);
        }
    }

    fn encode_video(&mut self, duration: Rational64, frame: AvFrame) {
        let frame = self.video_ctx.encode_frame(frame);

        let timescale = self.video_ctx.time_base;

        let new_timestamp = self.video_timestamp + duration;

        let previous_ts = (self.video_timestamp * timescale).to_integer();
        let new_ts = (new_timestamp * timescale).to_integer();
        let duration = new_ts - previous_ts;

        let duration = duration.try_into().expect("duration too large");

        self.segments.push_back(StreamSegment::Video { duration, frame });

        self.video_timestamp = new_timestamp;

    }

    pub fn recv_segment(&mut self) -> Option<StreamSegment> {
        self.segments.pop_front()
    }
}

#[derive(Clone, Debug)]
pub enum StreamSegment {
    Audio { duration: u32, frame: mp4::AdtsFrame },
    Video { duration: u32, frame: mp4::AvcFrame },
}

#[derive(Debug)]
pub struct AudioCtx {
    codec: aac::Encoder,
    pcm_buff: Vec<i16>,
    sample_rate: i64,
}

pub struct AudioParams {
    pub bit_rate: aac::BitRate,
    pub sample_rate: usize,
}

impl AudioCtx {
    pub fn new(params: AudioParams) -> Self {
        let sample_rate = params.sample_rate.try_into().expect("sample_rate into u32");

        let aac_params = aac::EncoderParams {
            bit_rate: params.bit_rate,
            sample_rate: sample_rate,
            transport: aac::Transport::Adts,
        };

        let codec = aac::Encoder::new(aac_params).expect("aac::Encoder::new");

        AudioCtx {
            codec,
            pcm_buff: Vec::new(),
            sample_rate: sample_rate.into(),
        }
    }

    fn send_audio(&mut self, samples: &[Sample]) -> Option<(Rational64, AdtsFrame)> {
        self.pcm_buff.extend(samples.iter().copied().map(|sample| {
            // TODO set CLIP flag if sample is out of range
            let sample = if sample > 1.0 {
                1.0
            } else if sample < -1.0 {
                -1.0
            } else {
                sample
            };

            (sample * i16::max_value() as f32) as i16
        }));

        let audio_frame_sample_count = AUDIO_CHANNELS * SAMPLES_PER_CHANNEL_PER_FRAGMENT;

        if self.pcm_buff.len() > audio_frame_sample_count {
            // encode frame
            let fragment_pcm = &self.pcm_buff[0..audio_frame_sample_count];

            let mut aac_buff = [0u8; 4096];

            let encode_result = self.codec.encode(&fragment_pcm, &mut aac_buff)
                .expect("aac.encode");

            if encode_result.input_consumed != audio_frame_sample_count {
                eprintln!("monitor: aac encoder did not consume exactly {} samples (consumed {})",
                    audio_frame_sample_count, encode_result.input_consumed);
            }

            let duration = Rational64::new(SAMPLES_PER_CHANNEL_PER_FRAGMENT as i64, self.sample_rate);
            let adts = AdtsFrame(Bytes::copy_from_slice(&aac_buff[0..encode_result.output_size]));
            self.pcm_buff.drain(0..audio_frame_sample_count);

            Some((duration, adts))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct VideoCtx {
    codec: AvcEncoder,
    blank_frame: AvFrame,
    time_base: i64,
}

pub struct VideoParams {
    pub width: usize,
    pub height: usize,
    pub time_base: usize,
    pub pixel_format: PixelFormat,
}

pub enum PixelFormat {
    Yuv420p,
}

impl VideoCtx {
    pub fn new(params: VideoParams) -> Self {
        let time_base = params.time_base;

        let params = AvcParams {
            time_base: time_base,
            pixel_format: match params.pixel_format {
                PixelFormat::Yuv420p => sys::AVPixelFormat_AV_PIX_FMT_YUV420P,
            },
            color_space: sys::AVColorSpace_AVCOL_SPC_UNSPECIFIED,
            picture_width: params.width,
            picture_height: params.height,
        };

        let blank_frame = AvFrame::blank(
            params.picture_width,
            params.picture_height,
            params.pixel_format,
        );

        let codec = AvcEncoder::new(params).unwrap();

        VideoCtx {
            codec,
            blank_frame,
            time_base: time_base.try_into().unwrap(),
        }
    }

    pub fn decoder_configuration_record(&self) -> DecoderConfigurationRecord {
        self.codec.decoder_configuration_record()
    }

    pub fn encode_frame(&mut self, mut frame: AvFrame) -> mp4::AvcFrame {
        frame.set_picture_type(mixlab_codec::ffmpeg::sys::AVPictureType_AV_PICTURE_TYPE_I);
        self.codec.send_frame(frame).unwrap();

        let video_packet = self.codec.recv_packet().unwrap();

        // if dts = pts for all frames, we can safely ignore both and attach our own timing to the frame:
        assert!(video_packet.decode_timestamp() == video_packet.presentation_timestamp());

        // and if all frames are key frames, we can stream directly to clients with no buffering:
        assert!(video_packet.is_key_frame());

        mp4::AvcFrame {
            is_key_frame: true, // all frames are key frames
            composition_time: 0, // dts always equals pts
            data: Bytes::copy_from_slice(video_packet.data()),
        }
    }

    pub fn blank_frame(&self) -> AvFrame {
        self.blank_frame.clone()
    }
}
