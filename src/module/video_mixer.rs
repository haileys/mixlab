use mixlab_codec::ffmpeg::{AvFrame, PictureSettings, PixelFormat};
use mixlab_protocol::{VideoMixerParams, LineType, Terminal, VIDEO_MIXER_CHANNELS};
use mixlab_util::time::{MediaTime, MediaDuration};

use crate::engine::{self, InputRef, OutputRef, SAMPLE_RATE, TICKS_PER_SECOND};
use crate::module::ModuleT;
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
    width: 1120,
    height: 700,
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
            *out_a = self.params.a
                .and_then(|a| inputs.get(a))
                .and_then(|input| input.expect_video())
                .cloned();

            *out_b = self.params.b
                .and_then(|b| inputs.get(b))
                .and_then(|input| input.expect_video())
                .cloned();
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
            let output = output_frame.frame_data_mut();

            let channel_a = self.params.a
                .and_then(|a| self.channels.get(a))
                .and_then(|ch| ch.stored.as_ref())
                .map(|stored| stored.frame.frame_data());

            let channel_b = self.params.b
                .and_then(|b| self.channels.get(b))
                .and_then(|ch| ch.stored.as_ref())
                .map(|stored| stored.frame.frame_data());

            let crossfade = (self.params.fader * 255.0) as u8;

            unsafe {
                for component in pixfmt.components() {
                    // we assume 1 byte per pixel per plane
                    assert!(component.step() == 1);
                    assert!(component.offset() == 0);

                    let width = pict.width >> component.log2_horz();
                    let height = pict.height >> component.log2_vert();
                    let plane = component.plane();

                    let (a_ptr, a_linesize) = match channel_a.as_ref() {
                        Some(a) => (a.data(plane), a.stride(plane)),
                        None => (output.data(plane) as *const _, output.stride(plane)),
                    };

                    let (b_ptr, b_linesize) = match channel_b.as_ref() {
                        Some(b) => (b.data(plane), b.stride(plane)),
                        None => (output.data(plane) as *const _, output.stride(plane)),
                    };

                    let out_ptr = output.data(plane);
                    let out_linesize = output.stride(plane) as usize;

                    // assert that pointers and linesizes all have expected
                    // alignments before hitting loop, so that we can skip
                    // alignment checks within
                    assert!(a_ptr.align_offset(32) == 0);
                    assert!(b_ptr.align_offset(32) == 0);
                    assert!(out_ptr.align_offset(32) == 0);
                    assert!(a_linesize % 32 == 0);
                    assert!(b_linesize % 32 == 0);
                    assert!(out_linesize % 32 == 0);

                    for y in 0..height {
                        let a_ptr = a_ptr.add(y * a_linesize);
                        let b_ptr = b_ptr.add(y * b_linesize);
                        let out_ptr = out_ptr.add(y * out_linesize);

                        fade_line(out_ptr, a_ptr, b_ptr, width, crossfade);

                        #[inline(never)]
                        unsafe fn fade_line(mut out: *mut u8, mut a: *const u8, mut b: *const u8, len: usize, fade: u8) {
                            use std::slice;
                            use packed_simd::{u8x32, u16x32, Cast};

                            let a_fade = u16x32::splat(fade as u16);
                            let b_fade = u16x32::splat((255 - fade) as u16);
                            let div = u16x32::splat(255);

                            let end = out.add(len);
                            while out < end {
                                let a_vals: u16x32 = u8x32::from_slice_aligned_unchecked(slice::from_raw_parts(a, 32)).cast();
                                let b_vals: u16x32 = u8x32::from_slice_aligned_unchecked(slice::from_raw_parts(b, 32)).cast();

                                let a_comp = a_vals * a_fade;
                                let b_comp = b_vals * b_fade;

                                let crossfaded: u8x32 = ((a_comp + b_comp) / div).cast();

                                crossfaded.write_to_slice_aligned_unchecked(slice::from_raw_parts_mut(out, 32));

                                a = a.add(32);
                                b = b.add(32);
                                out = out.add(32);
                            }
                        }
                    }
                }
            }
        }

        *out = Some(engine::VideoFrame {
            data: video::Frame {
                decoded: output_frame,
                duration_hint: MediaDuration::new(1, TICKS_PER_SECOND as i64), // TODO this assumes 1 output frame per tick
            },
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
