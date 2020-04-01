#![recursion_limit="256"]

mod util;
mod workspace;

use yew::{html, Component, ComponentLink, Html, ShouldRender};
use wasm_bindgen::prelude::*;
use web_sys::WebSocket;

use mixlab_protocol::{ClientMessage, WorkspaceState, ModuleId, ModuleParams, SineGeneratorParams};

use workspace::Workspace;

pub struct App {
    link: ComponentLink<Self>,
    websocket: WebSocket,
}

pub enum AppMsg {
    ClientUpdate(ClientMessage),
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let websocket = WebSocket::new("ws://localhost:8000/session")
            .expect("WebSocket::new");

        App { link, websocket }
    }

    fn destroy(&mut self) {
        self.websocket.close()
            .expect("WebSocket::close");
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            AppMsg::ClientUpdate(msg) => {
                crate::log!("sending client update");

                let packet = bincode::serialize(&msg)
                    .expect("bincode::serialize");

                crate::log!("serialized: {:?}", &packet);

                let resp = self.websocket.send_with_u8_array(&packet);

                crate::log!("sent! {:?}", resp);
            }
        }

        false
    }

    fn view(&self) -> Html {
        let modules = vec![
            (ModuleId(0), ModuleParams::SineGenerator(SineGeneratorParams { freq: 220.0 })),
            (ModuleId(1), ModuleParams::SineGenerator(SineGeneratorParams { freq: 295.0 })),
            (ModuleId(2), ModuleParams::OutputDevice),
            (ModuleId(3), ModuleParams::Mixer2ch),
        ];

        let state = WorkspaceState {
            modules,
            connections: vec![],
        };

        html! {
            <Workspace app={self.link.clone()} state={state} />
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
}

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => (crate::log_str(&format_args!($($t)*).to_string()))
}
