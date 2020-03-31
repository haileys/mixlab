#![feature(proc_macro_hygiene, decl_macro)]

static INDEX_HTML: &str = include_str!("../frontend/static/index.html");
static STYLE_CSS: &str = include_str!("../frontend/static/style.css");
static APP_JS: &str = include_str!("../frontend/pkg/frontend.js");
static APP_WASM: &[u8] = include_bytes!("../frontend/pkg/frontend_bg.wasm");

#[macro_use] extern crate rocket;

use rocket::http::ContentType;
use rocket::response::content::{Content, Css, Html, JavaScript};

#[get("/")]
fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

#[get("/style.css")]
fn style() -> Css<&'static str> {
    Css(STYLE_CSS)
}

#[get("/app.js")]
fn js() -> JavaScript<&'static str> {
    JavaScript(APP_JS)
}

#[get("/app.wasm")]
fn wasm() -> Content<&'static [u8]> {
    Content(ContentType::WASM, APP_WASM)
}

fn main() {
    rocket::ignite()
        .mount("/", routes![
            index,
            style,
            js,
            wasm,
        ])
        .launch();
}
