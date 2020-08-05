use std::borrow::Cow;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use bytes::Buf;
use derive_more::From;
use futures::sink::{Sink, SinkExt};
use futures::stream::{self, Stream, StreamExt};
use structopt::StructOpt;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::Filter;
use warp::reply::{self, Reply};
use warp::ws::{self, Ws, WebSocket};

use mixlab_protocol::{ClientMessage, ServerMessage};

use crate::engine::EngineEvent;
use crate::listen::{self, Disambiguation};
use crate::project::{self, ProjectHandle, Notification};
use crate::{icecast, module, rtmp};

#[derive(StructOpt)]
pub struct RunOpts {
    #[structopt(short, long, default_value = "127.0.0.1:8000")]
    listen: SocketAddr,
    workspace_path: PathBuf,
}

struct Server {
    project: ProjectHandle,
}

type ServerRef = Arc<Server>;

impl Server {
    pub fn new(project: ProjectHandle) -> Self {
        Server {
            project,
        }
    }
}

pub async fn run(opts: RunOpts) {
    let project = project::open_or_create(opts.workspace_path).await
        .expect("create_or_open_project");

    let server = Arc::new(Server::new(project));

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
        .map({
            let server = server.clone();
            move |ws: Ws| {
                let server = server.clone();
                ws.on_upgrade(move |websocket| {
                    session(websocket, server.clone())
                })
            }
        });

    let monitor_socket = warp::get()
        .and(warp::path!("_monitor" / Uuid))
        .and(warp::ws())
        .map(move |socket_id: Uuid, ws: Ws| {
            ws.on_upgrade(move |websocket| async move {
                let _ = module::monitor::stream(socket_id, websocket).await;
            })
        });

    let media_upload = warp::post()
        .and(warp::path!("_upload" / String))
        .and(warp::header::<String>("content-type"))
        .and(warp::filters::body::stream())
        .and_then({
            let server = server.clone();
            move |filename, kind, stream| {
                let server = server.clone();
                async move {
                    let params = UploadParams {
                        filename,
                        kind,
                    };

                    handle_upload(params, stream, server).await
                        .map(|()| warp::reply::reply())
                        .map_err(|e| {
                            eprintln!("upload failed: {:?}", e);
                            // TODO - internal server error?
                            warp::reject::not_found()
                        })
                }
            }
        });

    let routes = static_content
        .or(websocket)
        .or(monitor_socket)
        .or(media_upload)
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

async fn session(websocket: WebSocket, server: ServerRef) {
    let (tx, rx) = websocket.split();
    let mut tx = ClientTx(tx);

    let notifications = server.project.notifications();

    let (state, engine_ops, engine) = server.project.connect_engine().await
        .expect("connect engine");

    let library = server.project.fetch_media_library().await
        .expect("fetch_media_library");

    tx.send(ServerMessage::WorkspaceState(state))
        .await
        .expect("tx.send WorkspaceState");

    tx.send(ServerMessage::MediaLibrary(library))
        .await
        .expect("tx.send MediaLibrary");

    enum Event {
        ClientMessage(Result<ws::Message, warp::Error>),
        Engine(Result<EngineEvent, broadcast::RecvError>),
        Notification(Notification),
    }

    let mut events = stream::select(
        rx.map(Event::ClientMessage),
        stream::select(
            engine_ops.map(Event::Engine),
            notifications.map(Event::Notification)));

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

                match msg {
                    ClientMessage::Workspace(msg) => {
                        if let Err(e) = engine.update(msg) {
                            println!("Engine update failed: {:?}", e);
                        }
                    }
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
                    match tx.send(msg).await {
                        Ok(()) => {}
                        Err(_) => {
                            // client disconnected
                            return;
                        }
                    }
                }
            }
            Event::Notification(notif) => {
                let msg = match &notif {
                    Notification::PerformanceInfo(perf_info) => {
                        Some(ServerMessage::Performance(Cow::Borrowed(perf_info)))
                    }
                    Notification::MediaLibrary => {
                        match server.project.fetch_media_library().await {
                            Ok(library) => Some(ServerMessage::MediaLibrary(library)),
                            Err(e) => {
                                eprintln!("failed to query media library: {:?}", e);
                                None
                            }
                        }
                    }
                };

                if let Some(msg) = msg {
                    match tx.send(msg).await {
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

#[derive(From, Debug)]
enum UploadError {
    Warp(warp::Error),
    Upload(project::media::UploadError),
}

struct UploadParams {
    filename: String,
    kind: String,
}

async fn handle_upload(
    params: UploadParams,
    stream: impl Stream<Item = Result<impl Buf, warp::Error>>,
    server: ServerRef,
) -> Result<(), UploadError> {
    futures::pin_mut!(stream);

    let mut upload = server.project.begin_media_upload(project::media::UploadInfo {
        name: params.filename,
        kind: params.kind,
    }).await?;

    while let Some(buf) = stream.next().await {
        upload.receive_bytes(buf?.bytes()).await?;
    }

    upload.finalize().await?;

    Ok(())
}

#[derive(Debug, From)]
pub enum TxError {
    Warp(warp::Error),
    Bincode(bincode::Error),
}

pub struct ClientTx<S>(S);

impl<S: Sink<ws::Message, Error = warp::Error> + Unpin> ClientTx<S> {
    pub async fn send<'a>(&mut self, msg: ServerMessage<'a>) -> Result<(), TxError> {
        let msg = bincode::serialize(&msg)?;
        self.0.send(ws::Message::binary(msg)).await?;
        Ok(())
    }
}
