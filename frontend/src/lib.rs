#![recursion_limit="512"]

mod component;
mod control;
mod library;
mod module;
mod service;
mod sidebar;
mod util;
mod workspace;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::rc::Rc;

use derive_more::Display;
use wasm_bindgen::prelude::*;
use yew::format::Binary;
use yew::services::websocket::{WebSocketService, WebSocketStatus, WebSocketTask};
use yew::{html, Component, ComponentLink, Html, ShouldRender, Callback, Properties};

use mixlab_protocol::{ClientMessage, WorkspaceMessage, WorkspaceState, ServerMessage, ModuleId, InputId, OutputId, ModuleParams, WindowGeometry, ServerUpdate, Indication, Terminal, WorkspaceOp, ClientSequence, PerformanceInfo};

use library::MediaLibrary;
use sidebar::Sidebar;
use util::Sequence;
use workspace::Workspace;

pub struct App {
    link: ComponentLink<Self>,
    websocket: WebSocketTask,
    state: Option<Rc<RefCell<State>>>,
    client_seq: Sequence,
    server_seq: Option<ClientSequence>,
    performance_info: Option<Rc<PerformanceInfo>>,
    selected_tab: Tab,
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

#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Tab {
    #[display(fmt = "Workspace")]
    Workspace,
    #[display(fmt = "Media Library")]
    MediaLibrary,
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
    ServerMessage(ServerMessage<'static>),
    ClientUpdate(WorkspaceOp),
    ChangeTab(Tab),
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

        App {
            link,
            websocket,
            state: None,
            client_seq: Sequence::new(),
            server_seq: None,
            performance_info: None,
            selected_tab: Tab::Workspace,
        }
    }

    fn change(&mut self, _: ()) -> ShouldRender {
        false
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
                let msg = ClientMessage::Workspace(WorkspaceMessage {
                    sequence: ClientSequence(self.client_seq.next()),
                    op: op,
                });

                let packet = bincode::serialize(&msg)
                    .expect("bincode::serialize");

                let _ = self.websocket.send_binary(Ok(packet));

                false
            }
            AppMsg::ChangeTab(tab) => {
                self.selected_tab = tab;
                true
            }
        }
    }

    fn view(&self) -> Html {
        match &self.state {
            Some(state) => {
                html! {
                    <div class="app">
                        <Sidebar
                            state={state}
                            performance_info={self.performance_info.clone()}
                        />

                        <div class="main">
                            <TabBar<Tab>
                                current={self.selected_tab.clone()}
                                tabs={vec![
                                    Tab::Workspace,
                                    Tab::MediaLibrary,
                                ]}
                                onchange={self.link.callback(AppMsg::ChangeTab)}
                            />

                            { match self.selected_tab {
                                Tab::Workspace => html! {
                                    <Workspace
                                        app={self.link.clone()}
                                        state={state}
                                    />
                                },
                                Tab::MediaLibrary => html! {
                                    <MediaLibrary />
                                },
                            } }
                        </div>
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

#[derive(Properties, Clone, Debug)]
pub struct TabBarProps<T: Clone> {
    current: T,
    tabs: Vec<T>,
    onchange: Callback<T>,
}

struct TabBar<T: Clone> {
    props: TabBarProps<T>
}

impl<T: Display + Clone + PartialEq + 'static> Component for TabBar<T> {
    type Properties = TabBarProps<T>;
    type Message = ();

    fn create(props: TabBarProps<T>, _: ComponentLink<Self>) -> Self {
        TabBar { props }
    }

    fn change(&mut self, props: TabBarProps<T>) -> ShouldRender {
        self.props = props;
        true
    }

    fn update(&mut self, _: ()) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <div class="tab-bar">
                { for self.props.tabs.iter().map(|tab| {
                    let class = if tab == &self.props.current {
                        "tab-bar-tab tab-bar-active"
                    } else {
                        "tab-bar-tab"
                    };

                    html! {
                        <div
                            class={class}
                            onclick={self.props.onchange.reform({
                                let tab = tab.clone();
                                move |_| tab.clone()
                            })}
                        >
                            {tab.to_string()}
                        </div>
                    }
                }) }
            </div>
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
