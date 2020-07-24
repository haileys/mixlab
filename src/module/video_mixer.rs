use std::ptr;
use std::sync::Arc;

use mixlab_codec::ffmpeg::{AvFrame, PictureSettings, PixelFormat};
use mixlab_protocol::{VideoMixerParams, LineType, Terminal, VIDEO_MIXER_CHANNELS};
use mixlab_util::time::{MediaTime, MediaDuration};

use crate::engine::{self, Sample, InputRef, OutputRef, SAMPLE_RATE, TICKS_PER_SECOND};
use crate::module::ModuleT;
use crate::util;
use crate::video;
use crate::video::encode::DynamicScaler;

#[derive(Debug)]
pub struct VideoMixer {
    params: VideoMixerParams,
    inputs: Vec<Terminal>,
    outputs: Vec<Terminal>,
    channels: Vec<Channel>,
}

#[derive(Debug)]
struct Channel {
    stored: Option<StoredFrame>,
    scaler: DynamicScaler,
}

#[derive(Debug)]
struct StoredFrame {
    active_until: MediaTime,
    frame: AvFrame,
}

const OUTPUT_SETTINGS: PictureSettings = PictureSettings {
    width: 560,
    height: 350,
    pixel_format: PixelFormat::yuv420p(),
};

impl ModuleT for VideoMixer {
    type Params = VideoMixerParams;
    type Indication = ();

    fn create(params: Self::Params) -> (Self, Self::Indication) {
        let mixer = VideoMixer {
            params,
            inputs: (0..VIDEO_MIXER_CHANNELS).map(|i|
                LineType::Video.labeled(&(i + 1).to_string())
            ).collect(),
            outputs: vec![
                LineType::Video.labeled("Output"),
                LineType::Video.labeled("A"),
                LineType::Video.labeled("B"),
            ],
            channels: (0..VIDEO_MIXER_CHANNELS).map(|_| {
                let scaler = DynamicScaler::new(OUTPUT_SETTINGS);

                Channel {
                    stored: None,
                    scaler,
                }
            }).collect(),
        };

        (mixer, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn update(&mut self, new_params: VideoMixerParams) -> Option<Self::Indication> {
        self.params = new_params;
        None
    }

    fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        let (out, out_a, out_b) = match &mut outputs[0..3] {
            [a, b, c] => (a, b, c),
            _ => unreachable!(),
        };
        let out = out.expect_video();
        let out_a = out_a.expect_video();
        let out_b = out_b.expect_video();

        // send channel specific outputs
        {
            let in_a = self.params.a
                .and_then(|a| inputs.get(a))
                .and_then(|input| input.expect_video());

            let in_b = self.params.b
                .and_then(|b| inputs.get(b))
                .and_then(|input| input.expect_video());

            if let Some(a) = self.params.a {
                *out_a = in_a.cloned();
            }

            if let Some(b) = self.params.b {
                *out_b = in_b.cloned();
            }
        }

        let absolute_timestamp = MediaTime::new(t as i64, SAMPLE_RATE as i64);

        // expire stored frames
        for channel in &mut self.channels {
            if let Some(frame) = &channel.stored {
                if absolute_timestamp >= frame.active_until {
                    channel.stored = None;
                }
            }
        }

        // receive new input frames
        for (idx, input) in inputs.iter().enumerate() {
            if let Some(video) = input.expect_video() {
                let channel = &mut self.channels[idx];

                let mut frame = video.data.decoded.clone();
                let scaled = channel.scaler.scale(&mut frame).clone();

                self.channels[idx].stored = Some(StoredFrame {
                    active_until: absolute_timestamp + video.tick_offset + video.data.duration_hint,
                    frame: scaled,
                });
            }
        }

        // compose output frame
        let mut output_frame = AvFrame::blank(&OUTPUT_SETTINGS);

        {
            let pict = output_frame.picture_settings();
            let pixfmt = pict.pixel_format.descriptor();
            let mut output = output_frame.frame_data_mut();

            let channel_a = self.params.a
                .and_then(|a| self.channels.get(a))
                .and_then(|ch| ch.stored.as_ref())
                .map(|stored| stored.frame.frame_data());

            let channel_b = self.params.b
                .and_then(|b| self.channels.get(b))
                .and_then(|ch| ch.stored.as_ref())
                .map(|stored| stored.frame.frame_data());

            let crossfade = (self.params.fader * 255.0) as u32;

            unsafe {
                for (plane, plane_info) in pixfmt.components().iter().enumerate() {
                    let height =
                        if plane == 0 {
                            pict.height
                        } else {
                            pict.height >> pixfmt.log2_chroma_h()
                        };

                    let width =
                        if plane == 0 {
                            pict.width
                        } else {
                            pict.width >> pixfmt.log2_chroma_w()
                        };

                    let (a_ptr, a_linesize) = match channel_a.as_ref() {
                        Some(a) => (a.data()[plane], a.stride()[plane] as usize),
                        None => (output.data()[plane], output.stride()[plane] as usize),
                    };

                    let (b_ptr, b_linesize) = match channel_b.as_ref() {
                        Some(b) => (b.data()[plane], b.stride()[plane] as usize),
                        None => (output.data()[plane], output.stride()[plane] as usize),
                    };

                    let out_ptr = output.data()[plane];
                    let out_linesize = output.stride()[plane] as usize;

                    for y in 0..height {
                        let a_ptr = a_ptr.add(y * a_linesize);
                        let b_ptr = b_ptr.add(y * b_linesize);
                        let out_ptr = out_ptr.add(y * out_linesize);

                        for x in 0..width {
                            let a_ptr = a_ptr.add(x);
                            let b_ptr = b_ptr.add(x);
                            let out_ptr = out_ptr.add(x);

                            // TODO we assume 1 byte per pixel per plane here
                            let value_a = ptr::read(a_ptr);
                            let value_b = ptr::read(b_ptr);

                            let crossfaded =
                                ((value_a as u32) * crossfade
                                   + (value_b as u32) * (255 - crossfade))
                                    / 255;

                            ptr::write(out_ptr, crossfaded as u8);
                        }
                    }
                }
            }
        }

        *out = Some(engine::VideoFrame {
            data: Arc::new(video::Frame {
                decoded: output_frame,
                duration_hint: MediaDuration::new(1, TICKS_PER_SECOND as i64), // TODO this assumes 1 output frame per tick
            }),
            tick_offset: MediaDuration::new(0, 1),
        });

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &self.inputs
    }

    fn outputs(&self) -> &[Terminal] {
        &self.outputs
    }
}

// #[inline(never)]
// fn crossfade(out: &mut FrameDataMut, a: &FrameData, b: &FrameData, crossfade: u16)
