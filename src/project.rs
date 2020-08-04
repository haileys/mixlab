use std::path::PathBuf;
use std::sync::Arc;

use derive_more::From;
use futures::stream::Stream;
use sqlx::SqlitePool;
use tokio::{fs, io, task, runtime};

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
}

pub struct ProjectBase {
    path: PathBuf,
    database: SqlitePool,
}

type ProjectBaseRef = Arc<ProjectBase>;

#[derive(From, Debug)]
pub enum OpenError {
    Io(io::Error),
    Json(serde_json::Error),
    Database(sqlx::Error),
    NotDirectory,
}

impl ProjectBase {
    async fn attach(path: PathBuf) -> Result<Self, sqlx::Error> {
        let mut sqlite_path = path.clone();
        sqlite_path.set_extension("mixlab");

        let database = db::attach(&sqlite_path).await?;

        Ok(ProjectBase {
            path,
            database,
        })
    }

    async fn read_workspace(&self) -> Result<persist::Workspace, io::Error> {
        let workspace_path = self.path.join("workspace.json");

        let workspace = match fs::read(&workspace_path).await {
            Ok(serialized) => {
                serde_json::from_slice(&serialized)?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                persist::Workspace::default()
            }
            Err(e) => {
                return Err(e)
            }
        };

        Ok(workspace)
    }

    async fn write_workspace(&self, workspace: &persist::Workspace) -> Result<(), io::Error> {
        let workspace_tmp_path = self.path.join(".workspace.json.tmp");
        let workspace_path = self.path.join("workspace.json");

        let serialized = serde_json::to_vec(workspace).expect("serde_json::to_vec");

        // write to temporary file and rename into place. this is atomic on unix,
        // maybe it is on windows too?
        fs::write(&workspace_tmp_path, &serialized).await?;
        fs::rename(&workspace_tmp_path, &workspace_path).await?;

        Ok(())
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

    let base = ProjectBase::attach(path).await?;
    let workspace = base.read_workspace().await?;

    // start engine update thread
    let (embryo, mut persist_rx) = WorkspaceEmbryo::new(workspace);
    let engine = engine::start(runtime::Handle::current(), embryo);

    let base = Arc::new(base);

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
    })
}

impl ProjectHandle {
    pub async fn connect_engine(&self) -> Result<(WorkspaceState, EngineEvents, EngineSession), EngineError> {
        self.engine.connect().await
    }

    pub fn performance_info(&self) -> impl Stream<Item = Arc<PerformanceInfo>> {
        self.engine.performance_info()
    }

    pub async fn begin_media_upload(&self, info: media::UploadInfo) -> Result<media::MediaUpload, media::UploadError> {
        media::MediaUpload::new(self.base.clone(), info).await
    }
}
