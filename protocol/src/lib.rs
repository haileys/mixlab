use std::fmt;
use std::num::NonZeroUsize;
use std::borrow::Cow;

use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

use mixlab_mux::mp4::{self, Mp4Params};
use mixlab_util::time::MediaDuration;

pub type Sample = f32;

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage<'a> {
    WorkspaceState(WorkspaceState),
    Update(ServerUpdate),
    Sync(ClientSequence),
    Performance(Cow<'a, PerformanceInfo>),
    MediaLibrary(MediaLibrary),
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PerformanceInfo {
    pub realtime: bool,
    pub lag: Option<TemporalWarningStatus>,
    pub tick_rate: usize,
    pub tick_budget: Microseconds,
    pub accounts: Vec<(PerformanceAccount, PerformanceMetric)>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporalWarningStatus {
    Active,
    Recent,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum PerformanceAccount {
    Engine,
    Module(ModuleId),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PerformanceMetric {
    pub last: Microseconds,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct Microseconds(pub u64);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MediaLibrary {
    pub items: Vec<MediaItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MediaId(pub i64);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MediaItem {
    pub id: MediaId,
    pub name: String,
    pub kind: String,
    pub size: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Workspace(WorkspaceMessage),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClientSequence(pub NonZeroUsize);

#[derive(Serialize, Deserialize, Debug)]
pub struct WorkspaceMessage {
    pub sequence: ClientSequence,
    pub op: WorkspaceOp,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum WorkspaceOp {
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
    Video,
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
    Envelope(EnvelopeParams),
    EqThree(EqThreeParams),
    FmSine(FmSineParams),
    Mixer(MixerParams),
    Monitor(()),
    Oscillator(OscillatorParams),
    OutputDevice(OutputDeviceParams),
    Plotter(()),
    StereoPanner(()),
    StereoSplitter(()),
    StreamInput(StreamInputParams),
    StreamOutput(StreamOutputParams),
    Trigger(GateState),
    VideoMixer(VideoMixerParams),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Indication {
    Amplifier(()),
    Envelope(()),
    EqThree(()),
    FmSine(()),
    Mixer(()),
    Monitor(MonitorIndication),
    Oscillator(()),
    OutputDevice(OutputDeviceIndication),
    Plotter(PlotterIndication),
    StereoPanner(()),
    StereoSplitter(()),
    StreamInput(()),
    StreamOutput(StreamOutputIndication),
    Trigger(()),
    VideoMixer(()),
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
pub struct OscillatorParams {
    pub freq: f64,
    pub waveform: Waveform,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MonitorIndication {
    pub socket_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MonitorTransportPacket {
    Init {
        params: Mp4Params<'static>,
    },
    Frame {
        duration: MediaDuration,
        track_data: mp4::TrackData,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OutputDeviceParams {
    pub device: Option<String>,
    pub left: Option<usize>,
    pub right: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OutputDeviceIndication {
    pub clip: Option<TemporalWarningStatus>,
    pub lag: Option<TemporalWarningStatus>,
    pub default_device: Option<String>,
    pub devices: Option<Vec<(String, usize)>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlotterIndication {
    pub inputs: Vec<Vec<Sample>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct EqThreeParams {
    pub gain_lo: Decibel,
    pub gain_mid: Decibel,
    pub gain_hi: Decibel,
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
pub struct StreamInputParams {
    pub protocol: Option<StreamProtocol>,
    pub mountpoint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum StreamProtocol {
    Icecast,
    Rtmp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StreamOutputParams {
    // TODO this is an awful hack to encode one-time impulses into params
    // figure out a nicer way of doing this
    pub seq: u64,
    pub connect_seq: u64,
    pub disconnect_seq: u64,
    pub rtmp_url: String,
    pub rtmp_stream_key: String,
}

impl Default for StreamOutputParams {
    fn default() -> Self {
        Self {
            seq: 1,
            connect_seq: 0,
            disconnect_seq: 0,
            rtmp_url: "".to_owned(),
            rtmp_stream_key: "".to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StreamOutputIndication {
    pub live: StreamOutputLiveStatus,
    pub error: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamOutputLiveStatus {
    Offline,
    Connecting,
    Live,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StreamSource {
    pub codec: String,
    pub kbps: usize,
}

pub const VIDEO_MIXER_CHANNELS: usize = 4;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VideoMixerParams {
    pub a: Option<usize>,
    pub b: Option<usize>,
    pub fader: f64,
}

impl Default for VideoMixerParams {
    fn default() -> Self {
        VideoMixerParams {
            a: None,
            b: None,
            fader: 1.0, // start at A
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Coords {
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
