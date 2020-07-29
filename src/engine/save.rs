use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use mixlab_protocol::{ModuleId, ModuleParams, OutputId, WindowGeometry};

use crate::util::Sequence;

#[derive(Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub module_seq: Sequence,
    pub modules: HashMap<ModuleId, Module>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Module {
    pub params: ModuleParams,
    pub geometry: WindowGeometry,
    pub inputs: Vec<Option<OutputId>>,
}
