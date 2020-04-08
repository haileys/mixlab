use mixlab_protocol::{ModuleParams, Indication, LineType};

pub mod amplifier;
pub mod envelope;
pub mod fm_sine;
pub mod icecast_input;
pub mod mixer_2ch;
pub mod mixer_4ch;
pub mod output_device;
pub mod plotter;
pub mod sine_generator;
pub mod stereo_panner;
pub mod stereo_splitter;
pub mod trigger;

use amplifier::Amplifier;
use envelope::Envelope;
use fm_sine::FmSine;
use icecast_input::IcecastInput;
use mixer_2ch::Mixer2ch;
use mixer_4ch::Mixer4ch;
use output_device::OutputDevice;
use plotter::Plotter;
use sine_generator::SineGenerator;
use stereo_panner::StereoPanner;
use stereo_splitter::StereoSplitter;
use trigger::Trigger;

use crate::engine::Sample;

pub trait ModuleT: Sized {
    type Params;
    type Indication;

    fn create(params: Self::Params) -> (Self, Self::Indication);
    fn params(&self) -> Self::Params;
    fn update(&mut self, new_params: Self::Params) -> Option<Self::Indication>;
    fn run_tick(&mut self, t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Self::Indication>;
    fn inputs(&self) -> &[LineType];
    fn outputs(&self) -> &[LineType];
}

#[derive(Debug)]
pub enum Module {
    Amplifier(Amplifier),
    Envelope(Envelope),
    FmSine(FmSine),
    IcecastInput(IcecastInput),
    Mixer2ch(Mixer2ch),
    Mixer4ch(Mixer4ch),
    OutputDevice(OutputDevice),
    Plotter(Plotter),
    SineGenerator(SineGenerator),
    StereoPanner(StereoPanner),
    StereoSplitter(StereoSplitter),
    Trigger(Trigger),
}

impl Module {
    pub fn create(params: ModuleParams) -> (Self, Indication) {
        macro_rules! gen {
            ($( $module:ident , )*) => {
                match params {
                    $(
                        ModuleParams::$module(params) => {
                            let (module, indication) = $module::create(params);
                            (Module::$module(module), Indication::$module(indication))
                        }
                    )*
                }
            }
        }

        gen! {
            Amplifier,
            Envelope,
            FmSine,
            IcecastInput,
            Mixer2ch,
            Mixer4ch,
            OutputDevice,
            Plotter,
            SineGenerator,
            StereoPanner,
            StereoSplitter,
            Trigger,
        }
    }

    pub fn params(&self) -> ModuleParams {
        macro_rules! gen {
            ($( $module:ident , )*) => {
                match self {
                    $(Module::$module(m) => ModuleParams::$module(m.params()),)*
                }
            }
        }

        gen! {
            Amplifier,
            Envelope,
            FmSine,
            IcecastInput,
            Mixer2ch,
            Mixer4ch,
            OutputDevice,
            Plotter,
            SineGenerator,
            StereoPanner,
            StereoSplitter,
            Trigger,
        }
    }

    pub fn update(&mut self, new_params: ModuleParams) -> Option<Indication> {
        macro_rules! gen {
            ($( $module:ident , )*) => {
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
        }

        gen! {
            Amplifier,
            Envelope,
            FmSine,
            IcecastInput,
            Mixer2ch,
            Mixer4ch,
            OutputDevice,
            SineGenerator,
            StereoPanner,
            StereoSplitter,
            Trigger,
        }
    }

    pub fn run_tick(&mut self, t: u64, inputs: &[Option<&[Sample]>], outputs: &mut [&mut [Sample]]) -> Option<Indication> {
        macro_rules! gen {
            ($( $module:ident , )*) => {
                match self {
                    $(
                        Module::$module(m) => m.run_tick(t, inputs, outputs).map(Indication::$module),
                    )*
                }
            }
        }

        gen! {
            Amplifier,
            Envelope,
            FmSine,
            IcecastInput,
            Mixer2ch,
            Mixer4ch,
            OutputDevice,
            Plotter,
            SineGenerator,
            StereoPanner,
            StereoSplitter,
            Trigger,
        }
    }

    pub fn inputs(&self) -> &[LineType] {
        macro_rules! gen {
            ($( $module:ident , )*) => {
                match self {
                    $(Module::$module(m) => m.inputs(),)*
                }
            }
        }

        gen! {
            Amplifier,
            Envelope,
            FmSine,
            IcecastInput,
            Mixer2ch,
            Mixer4ch,
            OutputDevice,
            Plotter,
            SineGenerator,
            StereoPanner,
            StereoSplitter,
            Trigger,
        }
    }

    pub fn outputs(&self) -> &[LineType] {
        macro_rules! gen {
            ($( $module:ident , )*) => {
                match self {
                    $(Module::$module(m) => m.outputs(),)*
                }
            }
        }

        gen! {
            Amplifier,
            Envelope,
            FmSine,
            IcecastInput,
            Mixer2ch,
            Mixer4ch,
            OutputDevice,
            Plotter,
            SineGenerator,
            StereoPanner,
            StereoSplitter,
            Trigger,
        }
    }
}
