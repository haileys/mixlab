use std::convert::TryInto;

use derive_more::From;
use mixlab_protocol as protocol;

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

        let _ = self.base.notify.media.broadcast(());

        Ok(())
    }
}

pub async fn library(base: ProjectBaseRef) -> Result<protocol::MediaLibrary, sqlx::Error> {
    #[derive(sqlx::FromRow, Debug)]
    struct Item {
        id: i64,
        name: String,
        kind: String,
        size: i64,
    }

    let items = sqlx::query_as::<_, Item>(r"
            SELECT media.id, media.name, media.kind, streams.size FROM media
            INNER JOIN streams ON streams.id = media.stream_id
            ORDER BY media.id DESC
        ")
        .fetch_all(&base.database)
        .await?;

    let items = items.into_iter().map(|item| {
        protocol::MediaItem {
            id: protocol::MediaId(item.id),
            name: item.name,
            kind: item.kind,
            size: item.size.try_into().expect("size is u64"),
        }
    }).collect();

    Ok(protocol::MediaLibrary { items })
}
