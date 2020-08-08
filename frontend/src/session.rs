use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use yew::services::websocket::{WebSocketService, WebSocketStatus, WebSocketTask};
use yew::format::Binary;
use yew::Callback;

use mixlab_protocol::{ServerMessage, ServerUpdate, ClientMessage, ClientSequence, ModuleId, ModuleParams, WindowGeometry, InputId, OutputId, Indication, Terminal, WorkspaceOp, WorkspaceMessage};

use crate::util;
use crate::util::notify::{self, Notify};
use crate::Sequence;

#[derive(Debug)]
pub struct Session {
    websocket: RefCell<Option<WebSocketTask>>,
    state: RefCell<Option<WorkspaceStateRef>>,
    seq: RefCell<Seq>,
    notify: Notifiers,
}

#[derive(Debug)]
struct Seq {
    client: Sequence,
    server: Option<ClientSequence>,
}

#[derive(Debug)]
struct Notifiers {
    workspace: Notify<()>,
    performance: Notify<Rc<mixlab_protocol::PerformanceInfo>>,
    media: Notify<Rc<mixlab_protocol::MediaLibrary>>,
}

pub type SessionRef = Rc<Session>;

impl Session {
    pub fn new() -> SessionRef {
        let session = Rc::new(Session {
            websocket: RefCell::new(None),
            state: RefCell::new(None),
            seq: RefCell::new(Seq {
                client: Sequence::new(),
                server: None,
            }),
            notify: Notifiers {
                workspace: Notify::new(),
                performance: Notify::new(),
                media: Notify::new(),
            },
        });

        let websocket_url = util::websocket_origin() + "/session";

        let websocket = WebSocketService::connect_binary(&websocket_url,
            Callback::from({
                let session = session.clone();
                move |msg: Binary| {
                    match msg {
                        Ok(buff) => {
                            let msg = bincode::deserialize::<ServerMessage>(&buff)
                                .expect("bincode::deserialize");

                            session.on_server_message(msg);
                        }
                        Err(e) => {
                            crate::log!("websocket recv error: {:?}", e);
                        }
                    }
                }
            }),
            Callback::from(|status: WebSocketStatus| {
                crate::log!("websocket status: {:?}", status);
            }))
        .expect("websocket.connect_binary");

        *session.websocket.borrow_mut() = Some(websocket);

        session
    }

    fn sync(&self, new_client_seq: ClientSequence) {
        let mut seq = self.seq.borrow_mut();

        if Some(new_client_seq) <= seq.server {
            panic!("sequence number repeat, desync");
        }

        seq.server = Some(new_client_seq);
    }

    fn workspace_synced(&self) -> bool {
        // only re-render according to server state if all of
        // our changes have successfully round-tripped
        let seq = self.seq.borrow();
        let client_seq = seq.client.last().map(ClientSequence);

        if seq.server == client_seq {
            // server is up to date, re-render
            true
        } else if seq.server < client_seq {
            // server is behind, skip render
            false
        } else {
            panic!("server_seq > client_seq, desync")
        }
    }

    fn on_server_message(&self, msg: ServerMessage) {
        match msg {
            ServerMessage::WorkspaceState(state) => {
                *self.state.borrow_mut() = Some(Rc::new(RefCell::new(state.into())));
                self.notify.workspace.broadcast(());
            }
            ServerMessage::Sync(seq) => {
                self.sync(seq);

                // re-render if this Sync message caused us to consider
                // ourselves synced - there may be prior updates
                // waiting for render
                if self.workspace_synced() {
                    self.notify.workspace.broadcast(());
                }
            }
            ServerMessage::Update(op) => {
                {
                    let state = self.state.borrow().as_ref().cloned()
                        .expect("PROTOCOL VIOLATION: received Update before WorkspaceState");

                    let mut state = state.borrow_mut();

                    match op {
                        ServerUpdate::CreateModule { id, params, geometry, indication, inputs, outputs } => {
                            state.modules.insert(id, params);
                            state.geometry.insert(id, geometry);
                            state.indications.insert(id, indication);
                            state.inputs.insert(id, inputs);
                            state.outputs.insert(id, outputs);
                        }
                        ServerUpdate::UpdateModuleParams(id, new_params) => {
                            if let Some(params) = state.modules.get_mut(&id) {
                                *params = new_params;
                            }
                        }
                        ServerUpdate::UpdateWindowGeometry(id, new_geometry) => {
                            if let Some(geometry) = state.geometry.get_mut(&id) {
                                *geometry = new_geometry;
                            }
                        }
                        ServerUpdate::UpdateModuleIndication(id, new_indication) => {
                            if let Some(indication) = state.indications.get_mut(&id) {
                                *indication = new_indication;
                            }
                        }
                        ServerUpdate::DeleteModule(id) => {
                            state.modules.remove(&id);
                            state.geometry.remove(&id);
                            state.indications.remove(&id);
                            state.inputs.remove(&id);
                            state.outputs.remove(&id);
                        }
                        ServerUpdate::CreateConnection(input, output) => {
                            state.connections.insert(input, output);
                        }
                        ServerUpdate::DeleteConnection(input) => {
                            state.connections.remove(&input);
                        }
                    }
                }

                // only re-render according to server state if all of
                // our changes have successfully round-tripped
                if self.workspace_synced() {
                    self.notify.workspace.broadcast(());
                }
            }
            ServerMessage::Performance(perf_info) => {
                self.notify.performance.broadcast(
                    Rc::new(perf_info.into_owned()));
            }
            ServerMessage::MediaLibrary(library) => {
                crate::log!("Receiving media library!");
                self.notify.media.broadcast(Rc::new(library));
            }
        }
    }

    pub fn workspace(&self) -> Option<WorkspaceStateRef> {
        self.state.borrow().clone()
    }

    pub fn listen_workspace(&self, callback: Callback<()>) -> notify::Handle {
        self.notify.workspace.subscribe(callback)
    }

    pub fn update_workspace(&self, op: WorkspaceOp) {
        let msg = ClientMessage::Workspace(WorkspaceMessage {
            sequence: ClientSequence(self.seq.borrow_mut().client.next()),
            op: op,
        });

        self.send_message(msg);
    }

    pub fn listen_performance(&self, callback: Callback<Rc<mixlab_protocol::PerformanceInfo>>) -> notify::Handle {
        self.notify.performance.subscribe(callback)
    }

    pub fn listen_media(&self, callback: Callback<Rc<mixlab_protocol::MediaLibrary>>) -> notify::Handle {
        self.notify.media.subscribe(callback)
    }

    fn send_message(&self, msg: ClientMessage) {
        let packet = bincode::serialize(&msg)
            .expect("bincode::serialize");

        self.websocket.borrow_mut()
            .as_mut()
            .expect("tried to send session message because websocket connected")
            .send_binary(Ok(packet));
    }
}

pub type WorkspaceStateRef = Rc<RefCell<WorkspaceState>>;

#[derive(Debug, Clone)]
pub struct WorkspaceState {
    // modules uses BTreeMap for consistent iteration order:
    pub modules: BTreeMap<ModuleId, ModuleParams>,
    pub geometry: HashMap<ModuleId, WindowGeometry>,
    pub connections: HashMap<InputId, OutputId>,
    pub indications: HashMap<ModuleId, Indication>,
    pub inputs: HashMap<ModuleId, Vec<Terminal>>,
    pub outputs: HashMap<ModuleId, Vec<Terminal>>,
}

impl From<mixlab_protocol::WorkspaceState> for WorkspaceState {
    fn from(wstate: mixlab_protocol::WorkspaceState) -> WorkspaceState {
        WorkspaceState {
            modules: wstate.modules.into_iter().collect(),
            geometry: wstate.geometry.into_iter().collect(),
            indications: wstate.indications.into_iter().collect(),
            connections: wstate.connections.into_iter().collect(),
            inputs: wstate.inputs.into_iter().collect(),
            outputs: wstate.outputs.into_iter().collect(),
        }
    }
}
