use std::collections::HashMap;

use mixlab_protocol::{ModuleId, InputId, OutputId, TerminalId, WindowGeometry, Indication, LineType};

use crate::module::Module;
use crate::util::Sequence;
use crate::engine::save;

pub struct Workspace {
    pub(in crate::engine) module_seq: Sequence,
    pub(in crate::engine) modules: HashMap<ModuleId, Module>,
    pub(in crate::engine) geometry: HashMap<ModuleId, WindowGeometry>,
    pub(in crate::engine) connections: HashMap<InputId, OutputId>,
    pub(in crate::engine) indications: HashMap<ModuleId, Indication>,
}

impl Workspace {
    pub fn new() -> Self {
        Workspace {
            module_seq: Sequence::new(),
            modules: HashMap::new(),
            geometry: HashMap::new(),
            connections: HashMap::new(),
            indications: HashMap::new(),
        }
    }

    #[allow(unused)]
    pub fn load(save: &save::Workspace) -> Self {
        let mut modules = HashMap::new();
        let mut geometry = HashMap::new();
        let mut indications = HashMap::new();

        // load modules and geometry
        for (module_id, saved_module) in &save.modules {
            let (module, indication) = Module::create(saved_module.params.clone());
            modules.insert(*module_id, module);
            geometry.insert(*module_id, saved_module.geometry.clone());
            indications.insert(*module_id, indication);
        }

        let mut workspace = Workspace {
            module_seq: save.module_seq.clone(),
            modules,
            geometry,
            connections: HashMap::new(),
            indications,
        };

        // load connections after loading all modules
        for (module_id, saved_module) in &save.modules {
            for (input_idx, output_id) in saved_module.inputs.iter().enumerate() {
                let input_id = InputId(*module_id, input_idx);

                if let Some(output_id) = output_id {
                    // ignore workspace connect error for now... should we log?
                    let _ = workspace.connect(input_id, *output_id);
                }
            }
        }

        workspace
    }

    #[allow(unused)]
    pub fn save(&self) -> save::Workspace {
        save::Workspace {
            module_seq: self.module_seq.clone(),
            modules: self.modules.iter()
                .map(|(module_id, module)| {
                    let params = module.params();

                    let geometry = self.geometry.get(&module_id)
                        .cloned()
                        .unwrap_or_default();

                    let inputs = (0..module.inputs().len())
                        .map(|idx| InputId(*module_id, idx))
                        .map(|input_id| self.connections.get(&input_id).cloned())
                        .collect();

                    (*module_id, save::Module {
                        params,
                        geometry,
                        inputs,
                    })
                })
                .collect()
        }
    }

    fn terminal_type(&self, terminal: TerminalId) -> Option<LineType> {
        self.modules.get(&terminal.module_id()).and_then(|module| {
            match terminal {
                TerminalId::Input(input) => {
                    module.inputs().get(input.index()).map(|terminal| terminal.line_type())
                }
                TerminalId::Output(output) => {
                    module.outputs().get(output.index()).map(|terminal| terminal.line_type())
                }
            }
        })
    }

    pub fn connect(&mut self, input_id: InputId, output_id: OutputId) -> Result<Option<OutputId>, ConnectError> {
        let input_type = match self.terminal_type(TerminalId::Input(input_id)) {
            Some(ty) => ty,
            None => return Err(ConnectError::NoInput),
        };

        let output_type = match self.terminal_type(TerminalId::Output(output_id)) {
            Some(ty) => ty,
            None => return Err(ConnectError::NoOutput),
        };

        if input_type == output_type {
            Ok(self.connections.insert(input_id, output_id))
        } else {
            // type mismatch, don't connect
            Err(ConnectError::TypeMismatch)
        }
    }
}

pub enum ConnectError {
    NoInput,
    NoOutput,
    TypeMismatch,
}
