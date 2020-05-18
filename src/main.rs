mod engine;
mod icecast;
mod listen;
mod module;
mod rtmp;
mod source;
mod throttle;
mod util;
mod video;

use std::net::SocketAddr;
use std::sync::Arc;

use futures::{StreamExt, SinkExt, stream};
use structopt::StructOpt;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::Filter;
use warp::reply::{self, Reply};
use warp::ws::{self, Ws, WebSocket};

use engine::{EngineHandle, EngineOp};
use listen::Disambiguation;

use mixlab_protocol::{ClientMessage, ServerMessage};

#[derive(StructOpt)]
struct Opts {
    #[structopt(short, long, default_value = "127.0.0.1:8000")]
    listen: SocketAddr,
}

fn content(content_type: &str, reply: impl Reply) -> impl Reply {
    reply::with_header(reply, "content-type", content_type)
}

// #[get("/")]
fn index() -> impl Reply {
    #[cfg(not(debug_assertions))]
    let index_html: &str = include_str!("../frontend/static/index.html");
    #[cfg(debug_assertions)]
    let index_html = std::fs::read_to_string("frontend/static/index.html").expect("frontend built");
    content("text/html; charset=utf-8", index_html)
}

// #[get("/style.css")]
fn style() -> impl Reply {
    #[cfg(not(debug_assertions))]
    let style_css: &str = include_str!("../frontend/static/style.css");
    #[cfg(debug_assertions)]
    let style_css = std::fs::read_to_string("frontend/static/style.css").expect("frontend built");
    content("text/css; charset=utf-8", style_css)
}

// #[get("/app.js")]
fn js() -> impl Reply {
    #[cfg(not(debug_assertions))]
    let app_js: &str = include_str!("../frontend/pkg/frontend.js");
    #[cfg(debug_assertions)]
    let app_js = std::fs::read_to_string("frontend/pkg/frontend.js").expect("frontend built");
    content("text/javascript; charset=utf-8", app_js)
}

// #[get("/app.wasm")]
fn wasm() -> impl Reply {
    #[cfg(not(debug_assertions))]
    let app_wasm: &[u8] = include_bytes!("../frontend/pkg/frontend_bg.wasm");
    #[cfg(debug_assertions)]
    let app_wasm = std::fs::read("frontend/pkg/frontend_bg.wasm").expect("frontend built");
    content("application/wasm", app_wasm)
}

async fn session(websocket: WebSocket, engine: Arc<EngineHandle>) {
    let (mut tx, rx) = websocket.split();

    let (state, engine_ops, engine) = engine.connect().await
        .expect("engine.connect");

    let state_msg = bincode::serialize(&ServerMessage::WorkspaceState(state))
        .expect("bincode::serialize");

    tx.send(ws::Message::binary(state_msg))
        .await
        .expect("tx.send WorkspaceState");

    enum Event {
        ClientMessage(Result<ws::Message, warp::Error>),
        EngineOp(Result<EngineOp, broadcast::RecvError>),
    }

    let mut events = stream::select(
        rx.map(Event::ClientMessage),
        engine_ops.map(Event::EngineOp),
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

                if let Err(e) = engine.update(msg) {
                    println!("Engine update failed: {:?}", e);
                }
            }
            Event::EngineOp(Err(broadcast::RecvError::Lagged(skipped))) => {
                println!("disconnecting client: lagged {} messages behind", skipped);
                return;
            }
            Event::EngineOp(Err(broadcast::RecvError::Closed)) => {
                // TODO we should tell the user that the engine has stopped
                unimplemented!()
            }
            Event::EngineOp(Ok(op)) => {
                // sequence is only applicable if it belongs to this session:
                let msg = match op {
                    EngineOp::ServerUpdate(update) => Some(ServerMessage::Update(update)),
                    EngineOp::Sync(clock) => {
                        if clock.0 == engine.session_id() {
                            Some(ServerMessage::Sync(clock.1))
                        } else {
                            None
                        }
                    }
                };

                if let Some(msg) = msg {
                    let msg = bincode::serialize(&msg)
                        .expect("bincode::serialize");

                    match tx.send(ws::Message::binary(msg)).await {
                        Ok(()) => {}
                        Err(_) => {
                            // client disconnected
                            return;
                        }
                    }
                }
            }
        }
    }
}

async fn async_main(opts: Opts) {
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

    let monitor_socket = warp::get()
        .and(warp::path!("_monitor" / Uuid))
        .and(warp::ws())
        .map(move |socket_id: Uuid, ws: Ws| {
            ws.on_upgrade(move |websocket| async move {
                let _ = module::monitor::stream(socket_id, websocket).await;
            })
        });

    let routes = static_content
        .or(websocket)
        .or(monitor_socket)
        .with(warp::log("mixlab-http"));

    let warp = warp::serve(routes);

    let mut listener = listen::start(opts.listen).await
        .expect("listen::start");

    println!("Mixlab is now running at http://{}", listener.local_addr);

    let (mut incoming_tx, incoming_rx) = mpsc::channel::<Result<_, warp::Error>>(1);

    tokio::spawn(async move {
        while let Some(conn) = listener.incoming.next().await {
            match conn {
                Disambiguation::Http(conn) => {
                    match incoming_tx.send(Ok(conn)).await {
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                Disambiguation::Icecast(conn) => {
                    tokio::spawn(icecast::accept(conn));
                }
                Disambiguation::Rtmp(conn) => {
                    tokio::spawn(async move {
                        match rtmp::accept(conn).await {
                            Ok(()) => {}
                            Err(e) => { eprintln!("rtmp: {:?}", e); }
                        }
                    });
                }
            }
        }
    });

    warp.run_incoming(incoming_rx).await;
}

fn main() {
    let opts = Opts::from_args();

    let mut runtime = tokio::runtime::Builder::new()
        .enable_all()
        .threaded_scheduler()
        .build()
        .unwrap();

    runtime.block_on(async_main(opts));
}
