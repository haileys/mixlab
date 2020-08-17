use std::convert::TryInto;

use derive_more::From;
use mixlab_protocol::MediaId;
use mixlab_protocol as protocol;
use rusqlite::{params, OptionalExtension};

use crate::project::ProjectBaseRef;
use crate::project::stream::{self, ReadStream, WriteStream, StreamId};

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
    Database(rusqlite::Error),
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
        let info = self.info;

        self.base.with_database(move |conn| -> Result<(), rusqlite::Error> {
            conn.execute(
                    "INSERT INTO media (name, kind, stream_id) VALUES (?, ?, ?)",
                    params![info.name, info.kind, stream_id.0])?;

            Ok(())
        }).await?;

        let _ = self.base.notify.media.broadcast(());

        Ok(())
    }
}

pub async fn library(base: &ProjectBaseRef) -> Result<protocol::MediaLibrary, rusqlite::Error> {
    #[derive(Debug)]
    struct Item {
        id: i64,
        name: String,
        kind: String,
        size: i64,
    }

    let items = base.with_database(|conn| -> Result<Vec<protocol::MediaItem>, rusqlite::Error> {
        conn.prepare(r"
                SELECT media.id, media.name, media.kind, streams.size FROM media
                INNER JOIN streams ON streams.id = media.stream_id
                ORDER BY media.id DESC
            ")?
            .query_map(rusqlite::NO_PARAMS,
                |row| Ok(protocol::MediaItem {
                    id: protocol::MediaId(row.get(0)?),
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    size: row.get::<_, i64>(3)?.try_into().unwrap(),
                })
            )?
            .collect()
    }).await?;

    Ok(protocol::MediaLibrary { items })
}

pub async fn open(base: ProjectBaseRef, media_id: MediaId) -> Result<Option<ReadStream>, rusqlite::Error> {
    let stream_id = base.with_database(move |conn| -> Result<Option<StreamId>, rusqlite::Error> {
        conn.query_row(r"SELECT media.stream_id FROM media WHERE id = ?",
            params![media_id.0],
            |row| Ok(StreamId(row.get(0)?))
        ).optional()
    }).await?;

    match stream_id {
        Some(stream_id) => ReadStream::open(base, stream_id).await,
        None => Ok(None),
    }
}
