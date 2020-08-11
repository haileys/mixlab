use std::fmt::{self, Debug};
use std::path::PathBuf;
use std::sync::Arc;

use derive_more::From;
use futures::stream::{Stream, StreamExt};
use rusqlite::{self, Connection, OptionalExtension};
use tokio::sync::watch;
use tokio::{fs, io, task, runtime};

use mixlab_protocol as protocol;
use mixlab_protocol::{WorkspaceState, PerformanceInfo};

use crate::db;
use crate::engine::{self, EngineHandle, EngineEvents, EngineError, EngineSession, WorkspaceEmbryo};
use crate::persist;

pub mod stream;
pub mod media;

#[derive(Clone)]
pub struct ProjectHandle {
    base: ProjectBaseRef,
    engine: EngineHandle,
    notify: NotifyRx,
}

pub struct ProjectBase {
    path: PathBuf,

    // must be Arc of a blocking Mutex because task::spawn_blocking requires
    // that the passed closure is 'static, so we are forced to lock the mutex
    // in the blocking context and pass it as an Arc rather than a reference
    database: Arc<std::sync::Mutex<Connection>>,

    notify: NotifyTx,
}

impl Debug for ProjectBase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ProjectBase({:?})", self.path)
    }
}

pub type ProjectBaseRef = Arc<ProjectBase>;

#[derive(From, Debug)]
pub enum OpenError {
    Io(io::Error),
    Json(serde_json::Error),
    Database(rusqlite::Error),
    NotDirectory,
}

impl ProjectBase {
    pub fn with_database_in_blocking_context<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        f(&mut self.database.lock().expect("lock sqlite connection"))
    }

    pub async fn with_database<T: Send + 'static>(&self, f: impl FnOnce(&mut Connection) -> T + Send + 'static) -> T {
        let conn = self.database.clone();
        task::spawn_blocking(move || {
            f(&mut conn.lock().expect("lock sqlite connection"))
        }).await.expect("blocking database section")
    }

    async fn attach(path: PathBuf, notify: NotifyTx) -> Result<Self, rusqlite::Error> {
        let mut sqlite_path = path.clone();
        sqlite_path.set_extension("mixlab");
        let database = db::attach(sqlite_path).await?;

        Ok(ProjectBase {
            path,
            database: Arc::new(std::sync::Mutex::new(database)),
            notify,
        })
    }

    async fn read_workspace(&self) -> Result<persist::Workspace, OpenError> {
        let serialized = self.with_database(|conn| -> Result<Option<Vec<u8>>, rusqlite::Error> {
            conn.query_row("SELECT serialized FROM workspace WHERE rowid = 1", rusqlite::NO_PARAMS,
                |row| row.get(0)).optional()
        }).await?;

        let workspace = match serialized {
            Some(serialized) => serde_json::from_slice(&serialized)?,
            None => persist::Workspace::default(),
        };

        Ok(workspace)
    }

    async fn write_workspace(&self, workspace: &persist::Workspace) -> Result<(), rusqlite::Error> {
        let serialized = serde_json::to_vec(workspace).expect("serde_json::to_vec");

        self.with_database(move |conn| -> Result<(), rusqlite::Error> {
            conn.execute(r"
                    INSERT INTO workspace (rowid, serialized) VALUES (1, ?)
                    ON CONFLICT (rowid) DO UPDATE SET serialized = excluded.serialized
                ",
                &[serialized])?;

            Ok(())
        }).await
    }
}

pub async fn open_or_create(path: PathBuf) -> Result<ProjectHandle, OpenError> {
    match fs::create_dir(&path).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            // TODO - this is racey! we need an atomic way of asserting that a directory exists
            match fs::metadata(&path).await {
                Ok(meta) if meta.is_dir() => {
                    // already exists!
                }
                Ok(_) => {
                    return Err(OpenError::NotDirectory);
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
        Err(e) => {
            return Err(e.into());
        }
    }

    let (notify_tx, notify_rx) = notify();
    let base = ProjectBase::attach(path, notify_tx).await?;
    let workspace = base.read_workspace().await?;

    let base = Arc::new(base);

    // start engine update thread
    let (embryo, mut persist_rx) = WorkspaceEmbryo::new(workspace);
    let engine = engine::start(runtime::Handle::current(), embryo, base.clone());

    task::spawn({
        let base = base.clone();
        async move {
            while let Some(workspace) = persist_rx.recv().await {
                match base.write_workspace(&workspace).await {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("project: could not persist workspace: {:?}", e);
                    }
                }
            }
        }
    });

    Ok(ProjectHandle {
        base,
        engine,
        notify: notify_rx,
    })
}

impl ProjectHandle {
    pub async fn connect_engine(&self) -> Result<(WorkspaceState, EngineEvents, EngineSession), EngineError> {
        self.engine.connect().await
    }

    pub fn notifications(&self) -> impl Stream<Item = Notification> {
        let perf_info = self.engine.performance_info().map(Notification::PerformanceInfo);
        let media = self.notify.media.clone().map(|()| Notification::MediaLibrary);
        futures::stream::select(perf_info, media)
    }

    pub async fn begin_media_upload(&self, info: media::UploadInfo) -> Result<media::MediaUpload, media::UploadError> {
        media::MediaUpload::new(self.base.clone(), info).await
    }

    pub async fn fetch_media_library(&self) -> Result<protocol::MediaLibrary, rusqlite::Error> {
        media::library(&self.base).await
    }
}

pub enum Notification {
    PerformanceInfo(Arc<PerformanceInfo>),
    MediaLibrary,
}

pub struct NotifyTx {
    media: watch::Sender<()>,
}

#[derive(Clone)]
pub struct NotifyRx {
    media: watch::Receiver<()>,
}

pub fn notify() -> (NotifyTx, NotifyRx) {
    let (media_tx, media_rx) = watch::channel(());

    let tx = NotifyTx {
        media: media_tx,
    };

    let rx = NotifyRx {
        media: media_rx,
    };

    (tx, rx)
}
