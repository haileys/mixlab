use mixlab_protocol::{ModuleParams, Indication, Terminal, LineType};

use crate::engine::{InputRef, OutputRef};

pub trait ModuleT: Sized {
    type Params;
    type Indication;

    fn create(params: Self::Params) -> (Self, Self::Indication);
    fn params(&self) -> Self::Params;
    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication>;
    fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Self::Indication>;
    fn inputs(&self) -> &[Terminal];
    fn outputs(&self) -> &[Terminal];
}

macro_rules! gen_modules {
    ($( $mod_name:ident::$module:ident , )*) => {
        $( pub mod $mod_name; )*
        $( use $mod_name::$module; )*

        #[derive(Debug)]
        pub enum Module {
            $( $module($module), )*
        }

        impl Module {
            pub fn create(params: ModuleParams) -> (Self, Indication) {
                match params {
                    $(
                        ModuleParams::$module(params) => {
                            let (module, indication) = $module::create(params);
                            (Module::$module(module), Indication::$module(indication))
                        }
                    )*
                }
            }

            pub fn params(&self) -> ModuleParams {
                match self {
                    $(Module::$module(m) => ModuleParams::$module(m.params()),)*
                }
            }

            pub fn update(&mut self, new_params: ModuleParams) -> Option<Indication> {
                match (self, new_params) {
                    $(
                        (Module::$module(m), ModuleParams::$module(ref new_params)) =>
                            m.update(new_params.clone()).map(Indication::$module),
                    )*
                    (module, new_params) => {
                        let (m, indic) = Self::create(new_params.clone());
                        *module = m;
                        Some(indic)
                    }
                }
            }

            pub fn run_tick(&mut self, t: u64, inputs: &[InputRef], outputs: &mut [OutputRef]) -> Option<Indication> {
                match self {
                    $(
                        Module::$module(m) => m.run_tick(t, inputs, outputs).map(Indication::$module),
                    )*
                }
            }

            pub fn inputs(&self) -> &[Terminal] {
                match self {
                    $(Module::$module(m) => m.inputs(),)*
                }
            }

            pub fn outputs(&self) -> &[Terminal] {
                match self {
                    $(Module::$module(m) => m.outputs(),)*
                }
            }
        }
    }
}

gen_modules!{
    amplifier::Amplifier,
    envelope::Envelope,
    eq_three::EqThree,
    fm_sine::FmSine,
    mixer::Mixer,
    monitor::Monitor,
    oscillator::Oscillator,
    output_device::OutputDevice,
    plotter::Plotter,
    stereo_panner::StereoPanner,
    stereo_splitter::StereoSplitter,
    stream_input::StreamInput,
    stream_output::StreamOutput,
    trigger::Trigger,
}
