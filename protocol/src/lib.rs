use std::fmt;
use std::num::NonZeroUsize;

use serde_derive::{Deserialize, Serialize};

pub type Sample = f32;

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    WorkspaceState(WorkspaceState),
    Update(ServerUpdate),
    Sync(ClientSequence),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkspaceState {
    pub modules: Vec<(ModuleId, ModuleParams)>,
    pub geometry: Vec<(ModuleId, WindowGeometry)>,
    pub indications: Vec<(ModuleId, Indication)>,
    pub connections: Vec<(InputId, OutputId)>,
    pub inputs: Vec<(ModuleId, Vec<Terminal>)>,
    pub outputs: Vec<(ModuleId, Vec<Terminal>)>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientMessage {
    pub sequence: ClientSequence,
    pub op: ClientOp,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClientSequence(pub NonZeroUsize);

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientOp {
    CreateModule(ModuleParams, WindowGeometry),
    UpdateModuleParams(ModuleId, ModuleParams),
    UpdateWindowGeometry(ModuleId, WindowGeometry),
    DeleteModule(ModuleId),
    CreateConnection(InputId, OutputId),
    DeleteConnection(InputId),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerUpdate {
    CreateModule {
        id: ModuleId,
        params: ModuleParams,
        geometry: WindowGeometry,
        indication: Indication,
        inputs: Vec<Terminal>,
        outputs: Vec<Terminal>,
    },
    UpdateModuleParams(ModuleId, ModuleParams),
    UpdateWindowGeometry(ModuleId, WindowGeometry),
    UpdateModuleIndication(ModuleId, Indication),
    DeleteModule(ModuleId),
    CreateConnection(InputId, OutputId),
    DeleteConnection(InputId),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct ModuleId(pub NonZeroUsize);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum TerminalId {
    Input(InputId),
    Output(OutputId),
}

impl TerminalId {
    pub fn module_id(&self) -> ModuleId {
        match self {
            TerminalId::Input(input) => input.module_id(),
            TerminalId::Output(output) => output.module_id(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct InputId(pub ModuleId, pub usize);

impl InputId {
    pub fn module_id(&self) -> ModuleId {
        self.0
    }

    pub fn index(&self) -> usize {
        self.1
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct OutputId(pub ModuleId, pub usize);

impl OutputId {
    pub fn module_id(&self) -> ModuleId {
        self.0
    }

    pub fn index(&self) -> usize {
        self.1
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Terminal(Option<String>, LineType);

impl Terminal {
    pub fn label(&self) -> Option<&str> {
        self.0.as_ref().map(String::as_str)
    }

    pub fn line_type(&self) -> LineType {
        self.1
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Mono,
    Stereo,
}

impl LineType {
    pub fn labeled(self, label: &str) -> Terminal {
        Terminal(Some(label.to_string()), self)
    }

    pub fn unlabeled(self) -> Terminal {
        Terminal(None, self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ModuleParams {
    Amplifier(AmplifierParams),
    FmSine(FmSineParams),
    OutputDevice(OutputDeviceParams),
    Plotter(()),
    Oscillator(OscillatorParams),
    StereoPanner(()),
    StereoSplitter(()),
    Trigger(GateState),
    Envelope(EnvelopeParams),
    Mixer(MixerParams),
    IcecastInput(IcecastInputParams),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Indication {
    Amplifier(()),
    Envelope(()),
    FmSine(()),
    IcecastInput(()),
    Mixer(()),
    OutputDevice(OutputDeviceIndication),
    Plotter(PlotterIndication),
    Oscillator(()),
    StereoPanner(()),
    StereoSplitter(()),
    Trigger(()),
}


#[derive(Serialize, Deserialize, Clone, Debug, Copy, PartialEq)]
pub enum Waveform {
    On,
    Off,
    Sine,
    Square,
    Triangle,
    Saw,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Frequency {
    Hz(f64),
    BPM(f64),
}

impl Frequency {
    pub fn to_hz(&self) -> Self {
        match self {
            Self::Hz(_) => self.clone(),
            Self::BPM(bpm) => Self::Hz(bpm / 60.0),
        }
    }

    pub fn to_bpm(&self) -> Self {
        match self {
            Self::Hz(hz) => Self::BPM(hz * 60.0),
            Self::BPM(_) => self.clone(),
        }
    }

    pub fn value(&self) -> f64 {
        match self {
            Self::Hz(hz) => *hz,
            Self::BPM(bpm) => *bpm,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OscillatorParams {
    pub freq: Frequency,
    pub waveform: Waveform,
    pub pulse_width: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OutputDeviceParams {
    pub device: Option<String>,
    pub left: Option<usize>,
    pub right: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OutputDeviceIndication {
    pub clip: Option<OutputDeviceWarning>,
    pub lag: Option<OutputDeviceWarning>,
    pub default_device: Option<String>,
    pub devices: Option<Vec<(String, usize)>>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputDeviceWarning {
    Active,
    Recent,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlotterIndication {
    pub inputs: Vec<Vec<Sample>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FmSineParams {
    pub freq_lo: f64,
    pub freq_hi: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AmplifierParams {
    pub amplitude: f64,
    pub mod_depth: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GateState {
    Open,
    Closed
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EnvelopeParams {
    pub attack_ms: f64,
    pub decay_ms: f64,
    pub sustain_amplitude: f64,
    pub release_ms: f64,
}

impl Default for EnvelopeParams {
    fn default() -> EnvelopeParams {
        EnvelopeParams {
            attack_ms: 25.0,
            decay_ms: 500.0,
            sustain_amplitude: 0.8,
            release_ms: 200.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MixerParams {
    pub channels: Vec<MixerChannelParams>
}

impl MixerParams {
    pub fn with_channels(n: usize) -> MixerParams {
        MixerParams {
            channels: vec![MixerChannelParams::default(); n]
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MixerChannelParams {
    pub gain: Decibel,
    pub fader: f64,
    pub cue: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct IcecastInputParams {
    pub mountpoint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct IcecastSource {
    pub codec: String,
    pub kbps: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coords {
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WindowGeometry {
    pub position: Coords,
    pub z_index: usize,
}

impl Coords {
    pub fn add(&self, other: Coords) -> Coords {
        Coords {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    pub fn sub(&self, other: Coords) -> Coords {
        Coords {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq)]
pub struct Decibel(pub f64);

impl fmt::Display for Decibel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.2} dB", self.0)
    }
}

impl Decibel {
    pub fn from_linear(linear: f64) -> Self {
        Decibel(linear.log10() * 20.0)
    }

    pub fn to_linear(self) -> f64 {
        f64::powf(10.0, self.0 / 20.0)
    }
}

impl From<f64> for Decibel {
    fn from(db: f64) -> Decibel {
        Decibel(db)
    }
}

impl From<Decibel> for f64 {
    fn from(db: Decibel) -> f64 {
        db.0
    }
}
