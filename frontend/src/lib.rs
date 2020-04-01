#![recursion_limit="256"]

mod util;
mod workspace;

use yew::{html, Component, ComponentLink, Html, ShouldRender};
use yew::format::Binary;
use yew::services::websocket::{WebSocketService, WebSocketStatus, WebSocketTask};
use wasm_bindgen::prelude::*;

use mixlab_protocol::{ClientMessage, WorkspaceState, ServerMessage};

use workspace::Workspace;

pub struct App {
    link: ComponentLink<Self>,
    websocket: WebSocketTask,
    state: Option<WorkspaceState>,
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

        App { link, websocket, state: None }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            AppMsg::NoOp => false,
            AppMsg::ServerMessage(msg) => {
                match msg {
                    ServerMessage::WorkspaceState(state) => {
                        self.state = Some(state);
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
                html! { <Workspace app={self.link.clone()} state={state} /> }
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
