#![feature(proc_macro_hygiene, decl_macro)]

mod engine;
mod util;

static INDEX_HTML: &str = include_str!("../frontend/static/index.html");
static STYLE_CSS: &str = include_str!("../frontend/static/style.css");
static APP_JS: &str = include_str!("../frontend/pkg/frontend.js");
static APP_WASM: &[u8] = include_bytes!("../frontend/pkg/frontend_bg.wasm");

use std::sync::Arc;

use futures::{StreamExt, SinkExt, stream};
use tokio::sync::broadcast;
use warp::Filter;
use warp::reply::{self, Reply};
use warp::ws::{self, Ws, WebSocket};

use engine::{EngineHandle, LogPosition};
use mixlab_protocol::{ClientMessage, ServerMessage, ModelOp};

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

async fn session(websocket: WebSocket, engine: Arc<EngineHandle>) {
    let (mut tx, rx) = websocket.split();

    let (state, model_ops, engine) = engine.connect().await
        .expect("engine.connect");

    let state_msg = bincode::serialize(&ServerMessage::WorkspaceState(state))
        .expect("bincode::serialize");

    tx.send(ws::Message::binary(state_msg))
        .await
        .expect("tx.send WorkspaceState");

    enum Event {
        ClientMessage(Result<ws::Message, warp::Error>),
        ModelOp(Result<(LogPosition, ModelOp), broadcast::RecvError>),
    }

    let mut events = stream::select(
        rx.map(Event::ClientMessage),
        model_ops.map(Event::ModelOp),
    );

    while let Some(event) = events.next().await {
        match event {
            Event::ClientMessage(Err(e)) => {
                println!("error reading from client: {:?}", e);
                return;
            }
            Event::ClientMessage(Ok(msg)) => {
                if !msg.is_binary() {
                    continue;
                }

                let msg = bincode::deserialize::<ClientMessage>(msg.as_bytes())
                    .expect("bincode::deserialize");

                println!("{:?}", msg);
                println!(" => {:?}", engine.update(msg));
            }
            Event::ModelOp(Err(broadcast::RecvError::Lagged(skipped))) => {
                println!("disconnecting client: lagged {} messages behind", skipped);
                return;
            }
            Event::ModelOp(Err(broadcast::RecvError::Closed)) => {
                // TODO we should tell the user that the engine has stopped
                unimplemented!()
            }
            Event::ModelOp(Ok((_pos, op))) => {
                let msg = bincode::serialize(&ServerMessage::ModelOp(op))
                    .expect("bincode::serialize");

                tx.send(ws::Message::binary(msg))
                    .await
                    .expect("tx.send ModelOp")
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let engine = Arc::new(engine::start());

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
        .map(move |ws: Ws| {
            let engine = engine.clone();
            ws.on_upgrade(move |websocket| {
                let engine = engine.clone();
                session(websocket, engine)
            })
        });

    let routes = static_content
        .or(websocket)
        .with(warp::log("mixlab-http"));

    warp::serve(routes)
        .run(([127, 0, 0, 1], 8000))
        .await;
}
