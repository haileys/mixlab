#![recursion_limit="512"]

mod util;
mod workspace;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use yew::{html, Component, ComponentLink, Html, ShouldRender};
use yew::format::Binary;
use yew::services::websocket::{WebSocketService, WebSocketStatus, WebSocketTask};
use wasm_bindgen::prelude::*;

use mixlab_protocol::{ClientMessage, WorkspaceState, ServerMessage, ModuleId, InputId, OutputId, ModuleParams, WindowGeometry, ModelOp, Indication};

use workspace::Workspace;

pub struct App {
    link: ComponentLink<Self>,
    websocket: WebSocketTask,
    state: Option<Rc<RefCell<State>>>,
    state_seq: usize,
}

#[derive(Debug, Clone)]
pub struct State {
    // modules uses BTreeMap for consistent iteration order:
    modules: BTreeMap<ModuleId, ModuleParams>,
    geometry: HashMap<ModuleId, WindowGeometry>,
    connections: HashMap<InputId, OutputId>,
    indications: HashMap<ModuleId, Indication>,
}

impl From<WorkspaceState> for State {
    fn from(wstate: WorkspaceState) -> State {
        State {
            modules: wstate.modules.into_iter().collect(),
            geometry: wstate.geometry.into_iter().collect(),
            indications: wstate.indications.into_iter().collect(),
            connections: wstate.connections.into_iter().collect(),
        }
    }
}

#[derive(Debug)]
pub enum AppMsg {
    NoOp,
    ServerMessage(ServerMessage),
    ClientUpdate(ClientMessage),
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut websocket = WebSocketService::new();

        let websocket = websocket.connect_binary("ws://localhost:8000/session",
            link.callback(|msg: Binary| {
                match msg {
                    Ok(buff) => {
                        let msg = bincode::deserialize::<ServerMessage>(&buff)
                            .expect("bincode::deserialize");

                        AppMsg::ServerMessage(msg)
                    }
                    Err(e) => {
                        crate::log!("websocket recv error: {:?}", e);
                        AppMsg::NoOp
                    }
                }
            }),
            link.callback(|status: WebSocketStatus| {
                crate::log!("websocket status: {:?}", status);
                AppMsg::NoOp
            }))
        .expect("websocket.connect_binary");

        App { link, websocket, state: None, state_seq: 0 }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            AppMsg::NoOp => false,
            AppMsg::ServerMessage(msg) => {
                match msg {
                    ServerMessage::WorkspaceState(state) => {
                        self.state = Some(Rc::new(RefCell::new(state.into())));
                        true
                    }
                    ServerMessage::ModelOp(_, op) => {
                        let mut state = self.state.as_ref()
                            .expect("server should always send a WorkspaceState before a ModelOp")
                            .borrow_mut();

                        match op {
                            ModelOp::CreateModule(id, module, geometry, indication) => {
                                state.modules.insert(id, module);
                                state.geometry.insert(id, geometry);
                                state.indications.insert(id, indication);
                            }
                            ModelOp::UpdateModuleParams(id, new_params) => {
                                if let Some(params) = state.modules.get_mut(&id) {
                                    *params = new_params;
                                }
                            }
                            ModelOp::UpdateWindowGeometry(id, new_geometry) => {
                                if let Some(geometry) = state.geometry.get_mut(&id) {
                                    *geometry = new_geometry;
                                }
                            }
                            ModelOp::DeleteModule(id) => {
                                state.modules.remove(&id);
                                state.geometry.remove(&id);
                                state.indications.remove(&id);
                            }
                            ModelOp::CreateConnection(input, output) => {
                                state.connections.insert(input, output);
                            }
                            ModelOp::DeleteConnection(input) => {
                                state.connections.remove(&input);
                            }
                        }

                        self.state_seq += 1;
                        true
                    }
                    ServerMessage::Indication(module_id, indication) => {
                        let mut state = self.state.as_ref()
                            .expect("server should always send a WorkspaceState before an Indication")
                            .borrow_mut();

                        state.indications.insert(module_id, indication);

                        self.state_seq += 1;
                        true
                    }
                }
            }
            AppMsg::ClientUpdate(msg) => {
                let packet = bincode::serialize(&msg)
                    .expect("bincode::serialize");

                let resp = self.websocket.send_binary(Ok(packet));
                crate::log!("send_binary: {:?}", resp);

                false
            }
        }
    }

    fn view(&self) -> Html {
        match &self.state {
            Some(state) => {
                html! { <Workspace app={self.link.clone()} state={state} state_seq={self.state_seq} /> }
            }
            None => html! {}
        }
    }
}

#[wasm_bindgen]
pub fn start() {
    console_error_panic_hook::set_once();

    yew::start_app::<App>();
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_str(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_val(v: &wasm_bindgen::JsValue);
}

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => (crate::log_str(&format_args!($($t)*).to_string()))
}
