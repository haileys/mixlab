#![recursion_limit="512"]

mod component;
mod control;
mod module;
mod service;
mod sidebar;
mod util;
mod workspace;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use gloo_events::EventListener;
use wasm_bindgen::prelude::*;
use web_sys::Element;
use yew::format::Binary;
use yew::services::websocket::{WebSocketService, WebSocketStatus, WebSocketTask};
use yew::{html, Component, ComponentLink, Html, ShouldRender};

use mixlab_protocol::{ClientMessage, WorkspaceState, ServerMessage, ModuleId, InputId, OutputId, ModuleParams, WindowGeometry, ServerUpdate, Indication, Terminal, ClientOp, ClientSequence, PerformanceInfo};

use sidebar::Sidebar;
use util::Sequence;
use workspace::Workspace;

pub struct App {
    link: ComponentLink<Self>,
    websocket: WebSocketTask,
    state: Option<Rc<RefCell<State>>>,
    client_seq: Sequence,
    server_seq: Option<ClientSequence>,
    root_element: Element,
    viewport_width: usize,
    viewport_height: usize,
    performance_info: Option<Rc<PerformanceInfo>>,
    // must be kept alive while app is running:
    _resize_listener: EventListener,
}

#[derive(Debug, Clone)]
pub struct State {
    // modules uses BTreeMap for consistent iteration order:
    modules: BTreeMap<ModuleId, ModuleParams>,
    geometry: HashMap<ModuleId, WindowGeometry>,
    connections: HashMap<InputId, OutputId>,
    indications: HashMap<ModuleId, Indication>,
    inputs: HashMap<ModuleId, Vec<Terminal>>,
    outputs: HashMap<ModuleId, Vec<Terminal>>,
}

impl From<WorkspaceState> for State {
    fn from(wstate: WorkspaceState) -> State {
        State {
            modules: wstate.modules.into_iter().collect(),
            geometry: wstate.geometry.into_iter().collect(),
            indications: wstate.indications.into_iter().collect(),
            connections: wstate.connections.into_iter().collect(),
            inputs: wstate.inputs.into_iter().collect(),
            outputs: wstate.outputs.into_iter().collect(),
        }
    }
}

#[derive(Debug)]
pub enum AppMsg {
    NoOp,
    WindowResize,
    ServerMessage(ServerMessage<'static>),
    ClientUpdate(ClientOp),
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let websocket_url = util::websocket_origin() + "/session";

        let websocket = WebSocketService::connect_binary(&websocket_url,
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

        let window = web_sys::window()
            .expect("window");

        let root_element = window.document()
            .and_then(|doc| doc.document_element())
            .expect("root element");

        let viewport_width = root_element.client_width() as usize;
        let viewport_height = root_element.client_height() as usize;

        let resize_listener = EventListener::new(&window, "resize", {
            let link = link.clone();
            move |_| { link.send_message(AppMsg::WindowResize) }
        });

        App {
            link,
            websocket,
            state: None,
            client_seq: Sequence::new(),
            server_seq: None,
            root_element,
            viewport_width,
            viewport_height,
            performance_info: None,
            _resize_listener: resize_listener,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            AppMsg::NoOp => false,
            AppMsg::WindowResize => {
                self.viewport_width = self.root_element.client_width() as usize;
                self.viewport_height = self.root_element.client_height() as usize;
                true
            }
            AppMsg::ServerMessage(msg) => {
                match msg {
                    ServerMessage::WorkspaceState(state) => {
                        self.state = Some(Rc::new(RefCell::new(state.into())));
                        true
                    }
                    ServerMessage::Sync(seq) => {
                        if Some(seq) <= self.server_seq {
                            panic!("sequence number repeat, desync");
                        }

                        self.server_seq = Some(seq);

                        // re-render if this Sync message caused us to consider
                        // ourselves synced - there may be prior updates
                        // waiting for render
                        self.synced()
                    }
                    ServerMessage::Update(op) => {
                        let mut state = self.state.as_ref()
                            .expect("server should always send a WorkspaceState before a ModelOp")
                            .borrow_mut();

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

                        // only re-render according to server state if all of
                        // our changes have successfully round-tripped
                        self.synced()
                    }
                    ServerMessage::Performance(perf_info) => {
                        self.performance_info = Some(Rc::new(perf_info.into_owned()));

                        // TODO we should rerender perf pane but not workspace even if not synced
                        self.synced()
                    }
                }
            }
            AppMsg::ClientUpdate(op) => {
                let msg = ClientMessage {
                    sequence: ClientSequence(self.client_seq.next()),
                    op: op,
                };

                let packet = bincode::serialize(&msg)
                    .expect("bincode::serialize");

                let _ = self.websocket.send_binary(Ok(packet));

                false
            }
        }
    }

    fn view(&self) -> Html {
        match &self.state {
            Some(state) => {
                html! {
                    <div class="app">
                        <Workspace
                            app={self.link.clone()}
                            state={state}
                        />

                        <Sidebar
                            state={state}
                            performance_info={self.performance_info.clone()}
                        />
                    </div>
                }
            }
            None => html! {}
        }
    }
}

impl App {
    fn synced(&self) -> bool {
        // only re-render according to server state if all of
        // our changes have successfully round-tripped

        let client_seq = self.client_seq.last().map(ClientSequence);

        if self.server_seq == client_seq {
            // server is up to date, re-render
            true
        } else if self.server_seq < client_seq {
            // server is behind, skip render
            false
        } else {
            panic!("server_seq > client_seq, desync")
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

    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn warn_str(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn error_str(s: &str);
}

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => (crate::log_str(&format_args!($($t)*).to_string()))
}

#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => (crate::warn_str(&format_args!($($t)*).to_string()))
}

#[macro_export]
macro_rules! error {
    ($($t:tt)*) => (crate::error_str(&format_args!($($t)*).to_string()))
}
