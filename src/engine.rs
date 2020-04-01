use std::collections::{HashMap, HashSet, VecDeque};
use std::f32;
use std::fmt::{self, Debug};
use std::sync::mpsc::{self, SyncSender, Receiver, RecvTimeoutError, TrySendError};
use std::thread;
use std::time::{Instant, Duration};

use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use ringbuf::{RingBuffer, Producer, Consumer};
use tokio::sync::oneshot;

use mixlab_protocol::{ModuleId, InputId, OutputId, ModuleParams, SineGeneratorParams, ClientMessage, TerminalId, WorkspaceState};

use crate::util::Sequence;

pub type Sample = f32;

pub const CHANNELS: usize = 2;
pub const SAMPLE_RATE: usize = 44100;
pub const TICKS_PER_SECOND: usize = 10;
pub const SAMPLES_PER_TICK: usize = SAMPLE_RATE / TICKS_PER_SECOND;

pub struct OutputDeviceState {
    stream: cpal::Stream,
    tx: Producer<f32>,
    file: std::fs::File,
}

impl Debug for OutputDeviceState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OutputDeviceState")
    }
}

#[derive(Debug)]
pub enum Module {
    SineGenerator((), SineGeneratorParams),
    OutputDevice(OutputDeviceState, ()),
    Mixer2ch((), ()),
}

impl Module {
    fn create(params: ModuleParams) -> Self {
        match params {
            ModuleParams::SineGenerator(params) => {
                Module::SineGenerator((), params)
            }
            ModuleParams::OutputDevice => {
                let host = cpal::default_host();

                let device = host.default_output_device()
                    .expect("default_output_device");

                let config = device.default_output_config()
                    .expect("default_output_format");

                let (mut tx, mut rx) = RingBuffer::<f32>::new(SAMPLES_PER_TICK * 8).split();

                let stream = device.build_output_stream(
                        &config.config(),
                        move |data: &mut [f32]| {
                            let bytes = rx.pop_slice(data);

                            // zero-fill rest of buffer
                            for i in bytes..data.len() {
                                data[i] = 0.0;
                            }
                        },
                        |err| {
                            eprintln!("output stream error! {:?}", err);
                        })
                    .expect("build_output_stream");

                stream.play();

                println!("device: {:?}", device.name());
                println!("config: {:?}", config);

                let state = OutputDeviceState {
                    stream,
                    tx,
                    file: std::fs::File::create("/Users/charlie/Downloads/2ch.pcm").expect("File::create"),
                };

                Module::OutputDevice(state, ())
            }
            ModuleParams::Mixer2ch => {
                Module::Mixer2ch((), ())
            }
        }
    }

    fn destroy(&mut self) {}

    fn params(&self) -> ModuleParams {
        match self {
            Module::SineGenerator(_, params) => ModuleParams::SineGenerator(params.clone()),
            Module::OutputDevice(_, ()) => ModuleParams::OutputDevice,
            Module::Mixer2ch(_, ()) => ModuleParams::Mixer2ch,
        }
    }

    fn update(&mut self, new_params: ModuleParams) {
        match (self, &new_params) {
            (Module::SineGenerator(state, ref mut params), ModuleParams::SineGenerator(new_params)) => {
                *params = new_params.clone();
            }
            (module, new_params) => {
                *module = Self::create(new_params.clone());
            }
        }
    }

    fn run_tick(&mut self, t: u64, inputs: &[&[Sample]], outputs: &mut [&mut [Sample]]) {
        match self {
            Module::SineGenerator(state, params) => {
                let t = t as Sample * SAMPLES_PER_TICK as Sample;

                for i in 0..SAMPLES_PER_TICK {
                    let t = (t + i as Sample) / SAMPLE_RATE as Sample;
                    let x = Sample::sin(params.freq as f32 * t * 2.0 * f32::consts::PI);

                    for chan in 0..CHANNELS {
                        outputs[0][i * CHANNELS + chan] = x;
                    }
                }
            }
            Module::OutputDevice(state, params) => {
                state.tx.push_slice(inputs[0]);
                // use std::io::Write;
                // for sample in inputs[0] {
                //     state.file.write(&sample.to_le_bytes());
                // }
            }
            Module::Mixer2ch(state, params) => {
                for i in 0..SAMPLES_PER_TICK {
                    for chan in 0..CHANNELS {
                        let j = i * CHANNELS + chan;
                        outputs[0][j] = inputs[0][j] + inputs[1][j];
                    }
                }
            }
        }
    }

    fn input_count(&self) -> usize {
        match self {
            Module::SineGenerator(..) => 0,
            Module::OutputDevice(..) => 1,
            Module::Mixer2ch(..) => 2,
        }
    }

    fn output_count(&self) -> usize {
        match self {
            Module::SineGenerator(..) => 1,
            Module::OutputDevice(..) => 0,
            Module::Mixer2ch(..) => 1,
        }
    }
}

pub enum EngineMessage {
    DumpState(oneshot::Sender<WorkspaceState>),
    ClientMessage(ClientMessage),
}

pub struct EngineHandle {
    commands: SyncSender<EngineMessage>,
}

pub fn start() -> EngineHandle {
    let (tx, rx) = mpsc::sync_channel(8);

    thread::spawn(move || {
        let mut modules = HashMap::new();
        modules.insert(ModuleId(0), Module::create(ModuleParams::SineGenerator(SineGeneratorParams { freq: 220.0 })));
        modules.insert(ModuleId(1), Module::create(ModuleParams::SineGenerator(SineGeneratorParams { freq: 295.0 })));
        modules.insert(ModuleId(2), Module::create(ModuleParams::OutputDevice));
        modules.insert(ModuleId(3), Module::create(ModuleParams::Mixer2ch));

        let mut engine = Engine {
            commands: rx,
            modules: modules,
            module_seq: Sequence::new(),
            connections: HashMap::new(),
        };

        engine.run();
    });

    EngineHandle {
        commands: tx,
    }
}

#[derive(Debug)]
pub enum EngineError {
    Stopped,
    Busy,
}

impl EngineHandle {
    pub async fn dump_state(&self) -> Result<WorkspaceState, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.send_message(EngineMessage::DumpState(tx));
        rx.await.map_err(|_| EngineError::Stopped)
    }

    pub fn update(&self, msg: ClientMessage) -> Result<(), EngineError> {
        self.send_message(EngineMessage::ClientMessage(msg))
    }

    fn send_message(&self, msg: EngineMessage) -> Result<(), EngineError> {
        match self.commands.try_send(msg) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(EngineError::Busy),
            Err(TrySendError::Disconnected(_)) => Err(EngineError::Stopped),
        }
    }
}

pub struct Engine {
    commands: Receiver<EngineMessage>,
    modules: HashMap<ModuleId, Module>,
    module_seq: Sequence,
    connections: HashMap<InputId, OutputId>,
}

impl Engine {
    fn run(&mut self) {
        let start = Instant::now();
        let mut t = 0;

        loop {
            self.run_tick(t);
            t += 1;

            let sleep_until = start + Duration::from_millis(t * 1_000 / TICKS_PER_SECOND as u64);

            loop {
                let now = Instant::now();

                if now >= sleep_until {
                    break;
                }

                match self.commands.recv_timeout(sleep_until - now) {
                    Ok(EngineMessage::DumpState(tx)) => { tx.send(self.dump_state()); }
                    Ok(EngineMessage::ClientMessage(msg)) => { self.client_update(msg); }
                    Err(RecvTimeoutError::Timeout) => { break; }
                    Err(RecvTimeoutError::Disconnected) => { return; }
                }
            }
        }
    }

    fn dump_state(&self) -> WorkspaceState {
        let mut state = WorkspaceState {
            modules: Vec::new(),
            connections: Vec::new(),
        };

        for (module_id, module) in &self.modules {
            state.modules.push((*module_id, module.params()));
        }

        for (input, output) in &self.connections {
            state.connections.push((*input, *output));
        }

        state
    }

    fn client_update(&mut self, msg: ClientMessage) {
        match msg {
            ClientMessage::UpdateModuleParams(module_id, params) => {
                if let Some(module) = self.modules.get_mut(&module_id) {
                    module.update(params);
                }
            }
            ClientMessage::CreateConnection(input_id, output_id) => {
                // validate
                if !validate_terminal(self, TerminalId::Input(input_id)) {
                    return;
                }

                if !validate_terminal(self, TerminalId::Output(output_id)) {
                    return;
                }

                self.connections.insert(input_id, output_id);
            }
            ClientMessage::DeleteConnection(input_id) => {
                self.connections.remove(&input_id);
            }
        }

        fn validate_terminal(engine: &Engine, terminal: TerminalId) -> bool{
            if let Some(module) = engine.modules.get(&terminal.module_id()) {
                match terminal {
                    TerminalId::Input(input) => {
                        input.index() < module.input_count()
                    }
                    TerminalId::Output(output) => {
                        output.index() < module.output_count()
                    }
                }
            } else {
                // no such module
                false
            }
        }
    }

    fn add_module(&mut self, module: Module) -> ModuleId {
        let id = ModuleId(self.module_seq.next());
        self.modules.insert(id, module);
        id
    }

    fn remove_module(&mut self, id: ModuleId) {
        self.modules.remove(&id);
        self.connections.retain(|input, output| {
            input.module_id() != id && output.module_id() != id
        });
    }

    fn run_tick(&mut self, t: u64) {
        // find terminal modules - modules which do not send their output to
        // the input of any other module

        let mut terminal_modules = HashSet::new();

        for (id, _) in &self.modules {
            terminal_modules.insert(*id);
        }

        for (_, output) in &self.connections {
            terminal_modules.remove(&output.module_id());
        }

        // breadth-first-search modules out via their inputs, starting from
        // terminal modules

        let mut reverse_module_order = Vec::new();
        let mut visited_modules = HashSet::new();
        let mut module_queue = VecDeque::new();

        for id in terminal_modules.into_iter() {
            module_queue.push_back(id);
        }

        while let Some(module_id) = module_queue.pop_front() {
            let module = &self.modules[&module_id];

            // skip module if visited already
            if visited_modules.contains(&module_id) {
                continue;
            }

            // visit this module
            visited_modules.insert(module_id);
            reverse_module_order.push(module_id);

            // traverse input edges
            for i in 0..module.input_count() {
                let terminal_id = InputId(module_id, i);

                if let Some(output_id) = self.connections.get(&terminal_id) {
                    module_queue.push_back(output_id.module_id());
                }
            }
        }

        // run modules in dependency order according to BFS above

        let mut buffers = HashMap::<OutputId, Vec<Sample>>::new();

        for module_id in reverse_module_order.iter().rev() {
            let mut module = self.modules.get_mut(&module_id)
                .expect("module get_mut");

            let connections = &self.connections;

            let mut output_buffers = Vec::<Vec<Sample>>::new();

            for _ in 0..module.output_count() {
                output_buffers.push(vec![0.0; SAMPLES_PER_TICK * CHANNELS]);
            }

            {
                static ZERO_BUFFER: [Sample; SAMPLES_PER_TICK * CHANNELS] = [0.0; SAMPLES_PER_TICK * CHANNELS];

                let input_refs = (0..module.input_count())
                    .map(|i| InputId(*module_id, i))
                    .map(|input| connections.get(&input)
                        .map(|output| buffers[output].as_slice())
                        .unwrap_or(&ZERO_BUFFER))
                    .collect::<Vec<&[Sample]>>();

                let mut output_refs = output_buffers.iter_mut()
                    .map(|vec| vec.as_mut_slice())
                    .collect::<Vec<_>>();

                module.run_tick(t, &input_refs, &mut output_refs)
            }

            for (i, buffer) in output_buffers.into_iter().enumerate(){
                buffers.insert(OutputId(*module_id, i), buffer);
            }
        }
    }
}
