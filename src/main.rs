#![feature(proc_macro_hygiene, decl_macro)]

static INDEX_HTML: &str = include_str!("../frontend/static/index.html");
static STYLE_CSS: &str = include_str!("../frontend/static/style.css");
static APP_JS: &str = include_str!("../frontend/pkg/frontend.js");
static APP_WASM: &[u8] = include_bytes!("../frontend/pkg/frontend_bg.wasm");

use futures::{FutureExt, StreamExt, SinkExt};
use warp::Filter;
use warp::reply::{self, Reply};
use warp::ws::{self, Ws, WebSocket};

use mixlab_protocol::ClientMessage;

fn content(content_type: &str, reply: impl Reply) -> impl Reply {
    reply::with_header(reply, "content-type", content_type)
}

// #[get("/")]
fn index() -> impl Reply {
    content("text/html; charset=utf-8", INDEX_HTML)
}

// #[get("/style.css")]
fn style() -> impl Reply {
    content("text/css; charset=utf-8", STYLE_CSS)
}

// #[get("/app.js")]
fn js() -> impl Reply {
    content("text/javascript; charset=utf-8", APP_JS)
}

// #[get("/app.wasm")]
fn wasm() -> impl Reply {
    content("application/wasm", APP_WASM)
}

async fn session(websocket: WebSocket) {
    let (mut tx, mut rx) = websocket.split();

    tx.send(ws::Message::binary(b"hello world".to_owned()))
        .await
        .expect("tx.send");

    while let Some(msg) = rx.next().await.transpose().expect("rx.next") {
        if !msg.is_binary() {
            continue;
        }

        let msg = bincode::deserialize::<ClientMessage>(msg.as_bytes())
            .expect("bincode::deserialize");

        println!("{:?}", msg);
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let index = warp::path::end()
        .map(index);

    let style = warp::path!("style.css")
        .map(style);

    let js = warp::path!("app.js")
        .map(js);

    let wasm = warp::path!("app.wasm")
        .map(wasm);

    let static_content = warp::get()
        .and(index
            .or(style)
            .or(js)
            .or(wasm));

    let websocket = warp::get()
        .and(warp::path("session"))
        .and(warp::ws())
        .map(|ws: Ws| ws.on_upgrade(session));

    let routes = static_content
        .or(websocket)
        .with(warp::log("mixlab-http"));

    warp::serve(routes)
        .run(([127, 0, 0, 1], 8000))
        .await;
}
