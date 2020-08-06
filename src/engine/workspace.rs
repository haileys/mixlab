use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use tokio::sync::watch;

use mixlab_protocol::{ModuleId, InputId, OutputId, TerminalId, WindowGeometry, Indication, LineType};

use crate::module::ModuleE;
use crate::persist;
use crate::util::Sequence;

pub struct Workspace {
    pub(in crate::engine) module_seq: Sequence,
    pub(in crate::engine) modules: HashMap<ModuleId, ModuleE>,
    pub(in crate::engine) geometry: HashMap<ModuleId, WindowGeometry>,
    pub(in crate::engine) connections: HashMap<InputId, OutputId>,
    pub(in crate::engine) indications: HashMap<ModuleId, Indication>,
}

impl Workspace {
    pub fn from_persist(save: &persist::Workspace) -> Self {
        let mut modules = HashMap::new();
        let mut geometry = HashMap::new();
        let mut indications = HashMap::new();

        // load modules and geometry
        for (module_id, saved_module) in &save.modules {
            let (module, indication) = ModuleE::create(saved_module.params.clone());
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

    pub fn to_persist(&self) -> persist::Workspace {
        persist::Workspace {
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

                    (*module_id, persist::Module {
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

    pub fn disconnect(&mut self, input_id: InputId) -> Option<OutputId> {
        self.connections.remove(&input_id)
    }
}

pub enum ConnectError {
    NoInput,
    NoOutput,
    TypeMismatch,
}

pub struct WorkspaceEmbryo {
    workspace: persist::Workspace,
    persist_tx: watch::Sender<persist::Workspace>,
}

impl WorkspaceEmbryo {
    pub fn new(workspace: persist::Workspace) -> (WorkspaceEmbryo, watch::Receiver<persist::Workspace>) {
        let (persist_tx, persist_rx) = watch::channel(workspace.clone());
        (WorkspaceEmbryo { workspace, persist_tx }, persist_rx)
    }

    pub fn spawn(self) -> SyncWorkspace {
        let workspace = Workspace::from_persist(&self.workspace);

        SyncWorkspace {
            workspace,
            persist_tx: self.persist_tx,
        }
    }
}

pub struct SyncWorkspace {
    workspace: Workspace,
    persist_tx: watch::Sender<persist::Workspace>,
}

impl SyncWorkspace {
    // indications are not persisted, so we can hand out direct access
    pub fn indications_mut(&mut self) -> &mut HashMap<ModuleId, Indication> {
        &mut self.workspace.indications
    }

    pub fn borrow(&self) -> &Workspace {
        &self.workspace
    }

    pub fn borrow_mut<'a>(&'a mut self) -> WorkspaceBorrowMut<'a> {
        WorkspaceBorrowMut { sync: self }
    }

    pub fn borrow_mut_without_sync(&mut self) -> &mut Workspace {
        &mut self.workspace
    }
}

pub struct WorkspaceBorrowMut<'a> {
    sync: &'a mut SyncWorkspace,
}

impl<'a> Drop for WorkspaceBorrowMut<'a> {
    fn drop(&mut self) {
        let workspace = self.sync.workspace.to_persist();
        // nothing we can do if this fails
        let _ = self.sync.persist_tx.broadcast(workspace);
    }
}

impl<'a> Deref for WorkspaceBorrowMut<'a> {
    type Target = Workspace;

    fn deref(&self) -> &Workspace {
        &self.sync.workspace
    }
}

impl<'a> DerefMut for WorkspaceBorrowMut<'a> {
    fn deref_mut(&mut self) -> &mut Workspace {
        &mut self.sync.workspace
    }
}
