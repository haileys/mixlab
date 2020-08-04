use derive_more::From;

use crate::project::ProjectBaseRef;
use crate::project::stream::{self, WriteStream};

pub struct UploadInfo {
    pub name: String,
    pub kind: String,
}

pub struct MediaUpload {
    pub base: ProjectBaseRef,
    pub stream: WriteStream,
    pub info: UploadInfo,
}

#[derive(From, Debug)]
pub enum UploadError {
    Database(sqlx::Error),
}

impl MediaUpload {
    pub async fn new(base: ProjectBaseRef, info: UploadInfo) -> Result<Self, UploadError> {
        let stream = stream::create(base.clone()).await?;

        Ok(MediaUpload {
            base,
            stream,
            info
        })
    }

    pub async fn receive_bytes(&mut self, bytes: &[u8]) -> Result<(), UploadError> {
        self.stream.write(bytes).await?;
        Ok(())
    }

    pub async fn finalize(self) -> Result<(), UploadError> {
        let stream_id = self.stream.finalize().await?;

        sqlx::query("INSERT INTO media (name, kind, stream_id) VALUES (?, ?, ?)")
            .bind(self.info.name)
            .bind(self.info.kind)
            .bind(stream_id.0)
            .execute(&self.base.database)
            .await?;

        // TODO announce new media over watch channel on base

        Ok(())
    }
}

pub struct MediaInfo {
    pub name: String,
    pub kind: String,
}
