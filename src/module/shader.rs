use crate::engine::{InputRef, OutputRef, VideoFrame, TICKS_PER_SECOND};
use crate::module::{ModuleT, ModuleCtx};
use crate::video;

use mixlab_codec::ffmpeg::AvFrame;
use mixlab_codec::ffmpeg::media::Video;
use mixlab_protocol::{ShaderParams, LineType, Terminal};
use mixlab_util::time::MediaDuration;

#[derive(Debug)]
pub struct Shader {
    frame: Option<AvFrame<Video>>,
    outputs: Vec<Terminal>,
}

pub enum ShaderEvent {
    GpuFrame(AvFrame<Video>),
}

impl ModuleT for Shader {
    type Params = ShaderParams;
    type Indication = ();
    type Event = ShaderEvent;

    fn create(_: Self::Params, ctx: ModuleCtx<Self>) -> (Self, Self::Indication) {
        ctx.spawn_async(async {
            let frame = mixlab_graphics::render(1120, 700).await;
            ShaderEvent::GpuFrame(frame)
        });

        let module = Self {
            frame: None,
            outputs: vec![LineType::Video.unlabeled()],
        };

        (module, ())
    }

    fn params(&self) -> Self::Params {
        ShaderParams {}
    }

    fn receive_event(&mut self, ev: ShaderEvent) {
        match ev {
            ShaderEvent::GpuFrame(frame) => { self.frame = Some(frame); }
        }
    }

    fn update(&mut self, _: Self::Params) -> Option<Self::Indication> {
        None
    }

    fn run_tick(&mut self, _: u64, _: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        *outputs[0].expect_video() = self.frame.clone().map(|frame| {
            VideoFrame {
                data: video::Frame {
                    decoded: frame,
                    duration_hint: MediaDuration::new(1, TICKS_PER_SECOND as i64)
                },
                tick_offset: MediaDuration::new(0, 1),
            }
        });

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &[]
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}
