use std::borrow::Cow;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use futures::{StreamExt, SinkExt, stream};
use structopt::StructOpt;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::Filter;
use warp::reply::{self, Reply};
use warp::ws::{self, Ws, WebSocket};

use mixlab_protocol::{ClientMessage, ServerMessage, PerformanceInfo};

use crate::engine::EngineEvent;
use crate::listen::{self, Disambiguation};
use crate::project::{self, ProjectHandle};
use crate::{icecast, module, rtmp};

#[derive(StructOpt)]
pub struct RunOpts {
    #[structopt(short, long, default_value = "127.0.0.1:8000")]
    listen: SocketAddr,
    workspace_path: PathBuf,
}

pub async fn run(opts: RunOpts) {
    let project = project::open_or_create(opts.workspace_path).await
        .expect("create_or_open_project");

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
            let project = project.clone();
            ws.on_upgrade(move |websocket| {
                session(websocket, project.clone())
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

fn content(content_type: &str, reply: impl Reply) -> impl Reply {
    reply::with_header(reply, "content-type", content_type)
}

fn index() -> impl Reply {
    #[cfg(not(debug_assertions))]
    let index_html: &str = include_str!("../frontend/static/index.html");
    #[cfg(debug_assertions)]
    let index_html = std::fs::read_to_string("frontend/static/index.html").expect("frontend built");
    content("text/html; charset=utf-8", index_html)
}

fn style() -> impl Reply {
    #[cfg(not(debug_assertions))]
    let style_css: &str = include_str!("../frontend/static/style.css");
    #[cfg(debug_assertions)]
    let style_css = std::fs::read_to_string("frontend/static/style.css").expect("frontend built");
    content("text/css; charset=utf-8", style_css)
}

fn js() -> impl Reply {
    #[cfg(not(debug_assertions))]
    let app_js: &str = include_str!("../frontend/pkg/frontend.js");
    #[cfg(debug_assertions)]
    let app_js = std::fs::read_to_string("frontend/pkg/frontend.js").expect("frontend built");
    content("text/javascript; charset=utf-8", app_js)
}

fn wasm() -> impl Reply {
    #[cfg(not(debug_assertions))]
    let app_wasm: &[u8] = include_bytes!("../frontend/pkg/frontend_bg.wasm");
    #[cfg(debug_assertions)]
    let app_wasm = std::fs::read("frontend/pkg/frontend_bg.wasm").expect("frontend built");
    content("application/wasm", app_wasm)
}

async fn session(websocket: WebSocket, project: ProjectHandle) {
    let (mut tx, rx) = websocket.split();

    let perf_info = project.performance_info();

    let (state, engine_ops, engine) = project.connect_engine().await
        .expect("engine.connect");

    let state_msg = bincode::serialize(&ServerMessage::WorkspaceState(state))
        .expect("bincode::serialize");

    tx.send(ws::Message::binary(state_msg))
        .await
        .expect("tx.send WorkspaceState");

    enum Event {
        ClientMessage(Result<ws::Message, warp::Error>),
        Engine(Result<EngineEvent, broadcast::RecvError>),
        Performance(Arc<PerformanceInfo>),
    }

    let mut events = stream::select(
        rx.map(Event::ClientMessage),
        stream::select(
            engine_ops.map(Event::Engine),
            perf_info.map(Event::Performance)));

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
            Event::Engine(Err(broadcast::RecvError::Lagged(skipped))) => {
                println!("disconnecting client: lagged {} messages behind", skipped);
                return;
            }
            Event::Engine(Err(broadcast::RecvError::Closed)) => {
                // TODO we should tell the user that the engine has stopped
                unimplemented!()
            }
            Event::Engine(Ok(event)) => {
                // sequence is only applicable if it belongs to this session:
                let msg = match event {
                    EngineEvent::ServerUpdate(update) => Some(ServerMessage::Update(update)),
                    EngineEvent::Sync(clock) => {
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
            Event::Performance(perf_info) => {
                let msg = ServerMessage::Performance(Cow::Borrowed(&perf_info));

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
