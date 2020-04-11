mod codec;
mod engine;
mod icecast;
mod module;
mod util;


use std::sync::Arc;
use std::net::SocketAddr;

use futures::{StreamExt, SinkExt, stream};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use warp::Filter;
use warp::reply::{self, Reply};
use warp::ws::{self, Ws, WebSocket};

use engine::EngineHandle;
use icecast::http::Disambiguation;

use mixlab_protocol::{ClientMessage, ServerMessage, ModelOp, LogPosition};

fn content(content_type: &str, reply: impl Reply) -> impl Reply {
    reply::with_header(reply, "content-type", content_type)
}

// #[get("/")]
fn index() -> impl Reply {
    #[cfg(not(debug_assertions))]
    static index_html: &str = include_str!("../frontend/static/index.html");
    #[cfg(debug_assertions)]
    let index_html = std::fs::read_to_string("frontend/static/index.html").expect("frontend built");
    content("text/html; charset=utf-8", index_html)
}

// #[get("/style.css")]
fn style() -> impl Reply {
    #[cfg(not(debug_assertions))]
    static style_css: &str = include_str!("../frontend/static/style.css");
    #[cfg(debug_assertions)]
    let style_css = std::fs::read_to_string("frontend/static/style.css").expect("frontend built");
    content("text/css; charset=utf-8", style_css)
}

// #[get("/app.js")]
fn js() -> impl Reply {
    #[cfg(not(debug_assertions))]
    static app_js: &str = include_str!("../frontend/pkg/frontend.js");
    #[cfg(debug_assertions)]
    let app_js = std::fs::read_to_string("frontend/pkg/frontend.js").expect("frontend built");
    content("text/javascript; charset=utf-8", app_js)
}

// #[get("/app.wasm")]
fn wasm() -> impl Reply {
    #[cfg(not(debug_assertions))]
    static app_wasm: &[u8] = include_bytes!("../frontend/pkg/frontend_bg.wasm");
    #[cfg(debug_assertions)]
    let app_wasm = std::fs::read("frontend/pkg/frontend_bg.wasm").expect("frontend built");
    content("application/wasm", app_wasm)
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
                let result = engine.update(msg);
                println!(" => {:?}", result);
            }
            Event::ModelOp(Err(broadcast::RecvError::Lagged(skipped))) => {
                println!("disconnecting client: lagged {} messages behind", skipped);
                return;
            }
            Event::ModelOp(Err(broadcast::RecvError::Closed)) => {
                // TODO we should tell the user that the engine has stopped
                unimplemented!()
            }
            Event::ModelOp(Ok((pos, op))) => {
                let msg = bincode::serialize(&ServerMessage::ModelOp(pos, op))
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

    let warp = warp::serve(routes);

    let listen_addr = "127.0.0.1:8000".parse::<SocketAddr>()
        .expect("parse SocketAddr");

    let mut listener = TcpListener::bind(&listen_addr).await
        .expect("TcpListener::bind");

    println!("Mixlab is now running at http://localhost:8000");

    let (incoming_tx, incoming_rx) = mpsc::channel::<Result<_, warp::Error>>(1);

    tokio::spawn(async move {
        let mut incoming = listener.incoming();

        while let Some(conn) = incoming.next().await {
            let conn = conn.and_then(|conn| {
                conn.set_nodelay(true)?;
                Ok(conn)
            });

            match conn {
                Ok(conn) => {
                    let mut incoming_tx = incoming_tx.clone();
                    tokio::spawn(async move {
                        match icecast::http::disambiguate(conn).await {
                            Ok(Disambiguation::Http(conn)) => {
                                // nothing we can do in case of error here
                                let _ = incoming_tx.send(Ok(conn)).await;
                            }
                            Ok(Disambiguation::Icecast(conn)) => {
                                tokio::spawn(icecast::accept(conn));
                            }
                            Err(e) => {
                                eprintln!("http: disambiguation error: {:?}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("http: accept error: {:?}", e);
                }
            }
        }
    });

    warp.run_incoming(incoming_rx).await;
}
