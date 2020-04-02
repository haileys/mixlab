use serde_derive::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    WorkspaceState(WorkspaceState),
    ModelOp(ModelOp),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkspaceState {
    pub modules: Vec<(ModuleId, ModuleParams)>,
    pub connections: Vec<(InputId, OutputId)>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    CreateModule(ModuleParams, WindowGeometry),
    UpdateModuleParams(ModuleId, ModuleParams),
    UpdateWindowGeometry(ModuleId, WindowGeometry),
    DeleteModule(ModuleId),
    CreateConnection(InputId, OutputId),
    DeleteConnection(InputId),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ModelOp {
    CreateModule(ModuleId, ModuleParams, WindowGeometry),
    UpdateModuleParams(ModuleId, ModuleParams),
    UpdateWindowGeometry(ModuleId, WindowGeometry),
    DeleteModule(ModuleId),
    CreateConnection(InputId, OutputId),
    DeleteConnection(InputId),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct ModuleId(pub usize);

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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ModuleParams {
    SineGenerator(SineGeneratorParams),
    OutputDevice,
    Mixer2ch,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SineGeneratorParams {
    pub freq: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
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
