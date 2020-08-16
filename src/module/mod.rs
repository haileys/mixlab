use std::any::Any;

use mixlab_protocol::{Terminal, LineType};

use crate::engine::{InputRef, OutputRef, ModuleCtx};

pub trait ModuleT: Any + Sized {
    type Params;
    type Indication;
    type Event: Send;

    fn create(params: Self::Params, ctx: ModuleCtx<Self>) -> (Self, Self::Indication);
    fn params(&self) -> Self::Params;
    fn receive_event(&mut self, _: Self::Event) {}
    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication>;
    fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication>;
    fn inputs(&self) -> &[Terminal];
    fn outputs(&self) -> &[Terminal];
}

macro_rules! gen_modules {
    ($( $mod_name:ident::$module:ident , )*) => {
        $( pub mod $mod_name; )*
    }
}

#[macro_export]
macro_rules! enumerate_modules {
    (then $cb:ident!) => {
        $cb!{
            amplifier::Amplifier,
            envelope::Envelope,
            eq_three::EqThree,
            fm_sine::FmSine,
            mixer::Mixer,
            monitor::Monitor,
            oscillator::Oscillator,
            output_device::OutputDevice,
            plotter::Plotter,
            shader::Shader,
            stereo_panner::StereoPanner,
            stereo_splitter::StereoSplitter,
            stream_input::StreamInput,
            stream_output::StreamOutput,
            trigger::Trigger,
            video_mixer::VideoMixer,
            media_source::MediaSource,
        }
    }
}

enumerate_modules!{then gen_modules!}
