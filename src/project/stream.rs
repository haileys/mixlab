use std::cmp;
use std::convert::TryFrom;
use std::mem;

use rusqlite::{params, OptionalExtension};

use crate::project::ProjectBaseRef;

const STREAM_BLOB_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct StreamId(pub i64);

pub async fn create(base: ProjectBaseRef) -> Result<WriteStream, rusqlite::Error> {
    let stream_id = base.with_database(|conn| -> Result<StreamId, rusqlite::Error> {
        conn.execute("INSERT INTO streams (size) VALUES (0)", rusqlite::NO_PARAMS)?;
        Ok(StreamId(conn.last_insert_rowid()))
    }).await?;

    Ok(WriteStream {
        base,
        id: stream_id,
        offset: 0,
        buff: Vec::with_capacity(STREAM_BLOB_SIZE),
    })
}

// TODO - automatically clean up write stream on drop if not explicitly finalized
pub struct WriteStream {
    base: ProjectBaseRef,
    id: StreamId,
    offset: i64,
    buff: Vec<u8>,
}

impl WriteStream {
    pub async fn write(&mut self, mut bytes: &[u8]) -> Result<(), rusqlite::Error> {
        while !bytes.is_empty() {
            let take = cmp::min(bytes.len(), STREAM_BLOB_SIZE - self.buff.len());

            let (this_chunk, remaining) = bytes.split_at(take);

            self.buff.extend(this_chunk);

            bytes = remaining;

            if self.buff.len() == STREAM_BLOB_SIZE {
                self.flush().await?;
            }
        }

        Ok(())
    }

    pub async fn finalize(mut self) -> Result<StreamId, rusqlite::Error> {
        self.flush().await?;
        Ok(self.id)
    }

    async fn flush(&mut self) -> Result<(), rusqlite::Error> {
        if self.buff.len() > 0 {
            let id = self.id;
            let offset = self.offset;
            let buff_len = i64::try_from(self.buff.len()).expect("buff.len as i64");
            let buff = mem::take(&mut self.buff);

            self.base.with_database(move |conn| -> Result<(), rusqlite::Error> {
                conn.execute(r"INSERT INTO blobs (stream_id, offset, data) VALUES (?, ?, ?)",
                    params![id.0, offset, &buff])?;

                conn.execute(r"UPDATE streams SET size = ? WHERE id = ?",
                    params![id.0, offset + buff_len])?;

                Ok(())
            }).await?;

            self.offset += buff_len;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ReadStream {
    base: ProjectBaseRef,
    stream_id: StreamId,
    offset: i64,
    size: i64,
}

impl ReadStream {
    pub async fn open(base: ProjectBaseRef, stream_id: StreamId) -> Result<Option<Self>, rusqlite::Error> {
        Ok(base.with_database(move |conn| {
            conn.query_row(
                r"SELECT size FROM streams WHERE rowid = ?",
                &[stream_id.0],
                |row| row.get(0)
            ).optional()
        }).await?.map(|size| {
            ReadStream {
                base,
                stream_id,
                offset: 0,
                size: size,
            }
        }))
    }

    #[allow(unused)]
    pub async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, rusqlite::Error> {
        let stream_id = self.stream_id;
        let offset = self.offset;

        self.base.with_database(move |conn| -> Result<Option<Vec<u8>>, rusqlite::Error> {
            conn.query_row(
                    "SELECT data FROM blobs WHERE stream_id = ? AND offset = ?",
                    params![stream_id.0, offset],
                    |data| data.get(0))
                .optional()
        }).await
    }
}
