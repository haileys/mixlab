#![recursion_limit="256"]

mod util;
mod workspace;

use yew::{html, Component, ComponentLink, Html, ShouldRender};
use wasm_bindgen::prelude::*;

use workspace::Workspace;

struct App {
    link: ComponentLink<Self>,
}

enum Msg {
    // Click,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        App {
            link,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <Workspace />
        }
    }
}

#[wasm_bindgen]
pub fn start() {
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
