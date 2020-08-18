use crate::engine::{InputRef, OutputRef, VideoFrame, TICKS_PER_SECOND};
use crate::module::{ModuleT, ModuleCtx};
use crate::video;

use mixlab_codec::ffmpeg::AvFrame;
use mixlab_codec::ffmpeg::media::Video;
use mixlab_graphics::ShaderContext;
use mixlab_graphics::compile::FragmentShader;
use mixlab_protocol::{ShaderParams, LineType, Terminal};
use mixlab_util::time::MediaDuration;

#[derive(Debug)]
pub struct Shader {
    ctx: ModuleCtx<Self>,
    params: ShaderParams,
    shader: Option<ShaderContext>,
    frame: Option<AvFrame<Video>>,
    outputs: Vec<Terminal>,
}

pub enum ShaderEvent {
    NoOp,
    SetShader(ShaderContext),
}

impl ModuleT for Shader {
    type Params = ShaderParams;
    type Indication = ();
    type Event = ShaderEvent;

    fn create(params: Self::Params, ctx: ModuleCtx<Self>) -> (Self, Self::Indication) {
        ctx.spawn_async(compile_fragment_shader(params.fragment_shader_source.clone()));

        let module = Self {
            ctx,
            params,
            shader: None,
            frame: None,
            outputs: vec![LineType::Video.unlabeled()],
        };

        (module, ())
    }

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn receive_event(&mut self, ev: ShaderEvent) {
        match ev {
            ShaderEvent::NoOp => {}
            ShaderEvent::SetShader(shader) => {
                self.shader = Some(shader);
            }
        }
    }

    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication> {
        if self.params.fragment_shader_source != new_params.fragment_shader_source {
            self.params.fragment_shader_source = new_params.fragment_shader_source;
            self.ctx.spawn_async(compile_fragment_shader(self.params.fragment_shader_source.clone()));
        }

        None
    }

    fn run_tick(&mut self, _: u64, _: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication> {
        if let Some(shader) = &mut self.shader {
            let frame = self.ctx.runtime().block_on(shader.render());

            *outputs[0].expect_video() = Some(VideoFrame {
                data: video::Frame {
                    decoded: frame,
                    duration_hint: MediaDuration::new(1, TICKS_PER_SECOND as i64)
                },
                tick_offset: MediaDuration::new(0, 1),
            });
        }

        None
    }

    fn inputs(&self) -> &[Terminal] {
        &[]
    }

    fn outputs(&self)-> &[Terminal] {
        &self.outputs
    }
}

async fn compile_fragment_shader(source: String) -> ShaderEvent {
    match FragmentShader::compile(&source) {
        Ok(fragment_shader) => {
            let shader = ShaderContext::new(1120, 700, fragment_shader).await;
            ShaderEvent::SetShader(shader)
        }
        Err(e) => {
            eprintln!("could not compile fragment shader:\n{:#?}", e);
            ShaderEvent::NoOp
        }
    }
}
