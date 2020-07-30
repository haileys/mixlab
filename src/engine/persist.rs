use std::collections::HashMap;
use std::path::PathBuf;

use derive_more::From;
use serde::{Serialize, Deserialize};
use tokio::{fs, io};

use mixlab_protocol::{ModuleId, ModuleParams, OutputId, WindowGeometry};

use crate::util::Sequence;

pub struct Persist {
    base: PersistBase,
    workspace: Workspace,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Workspace {
    pub module_seq: Sequence,
    pub modules: HashMap<ModuleId, Module>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Module {
    pub params: ModuleParams,
    pub geometry: WindowGeometry,
    pub inputs: Vec<Option<OutputId>>,
}

struct PersistBase {
    path: PathBuf,
}

#[derive(From, Debug)]
pub enum ReadError {
    Io(io::Error),
    Json(serde_json::Error),
}

#[derive(From, Debug)]
pub enum OpenError {
    NotFound,
    NotDirectory,
    Read(ReadError),
    #[from(ignore)]
    Metadata(io::Error),
}

impl PersistBase {
    async fn read_workspace(&self) -> Result<Workspace, ReadError> {
        let workspace_path = self.path.join("workspace.json");

        let workspace = match fs::read(&workspace_path).await {
            Ok(serialized) => {
                serde_json::from_slice(&serialized)?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Workspace::default()
            }
            Err(e) => {
                return Err(e.into())
            }
        };

        Ok(workspace)
    }

    async fn write_workspace(&mut self, workspace: &Workspace) -> Result<(), io::Error> {
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

impl Persist {
    pub async fn create(path: PathBuf) -> Result<Persist, io::Error> {
        fs::create_dir(&path).await?;

        Ok(Persist {
            base: PersistBase { path },
            workspace: Workspace::default(),
        })
    }

    pub async fn open(path: PathBuf) -> Result<Persist, OpenError> {
        match fs::metadata(&path).await {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(OpenError::NotDirectory);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(OpenError::NotFound);
            }
            Err(e) => {
                return Err(OpenError::Metadata(e));
            }
        }

        let base = PersistBase { path };
        let workspace = base.read_workspace().await?;
        Ok(Persist {
            base,
            workspace,
        })
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub async fn update_workspace(&mut self, workspace: Workspace) -> Result<(), io::Error> {
        self.workspace = workspace;
        self.base.write_workspace(&self.workspace).await
    }
}
