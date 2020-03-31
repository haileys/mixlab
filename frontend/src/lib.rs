#![recursion_limit="256"]

mod workspace;
mod utils;

use wasm_bindgen::prelude::*;
use yew::{html, Component, ComponentLink, Html, ShouldRender};

use workspace::Workspace;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen]
    fn alert(s: &str);
}

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
