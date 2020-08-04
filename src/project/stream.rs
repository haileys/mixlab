use std::cmp;
use std::convert::{TryInto, TryFrom};

use derive_more::From;

use crate::project::ProjectBaseRef;

const STREAM_BLOB_SIZE: usize = 1024 * 1024;

pub struct StreamId(pub i64);

pub async fn create(base: ProjectBaseRef) -> Result<WriteStream, sqlx::Error> {
    let stream_id = StreamId(
        sqlx::query("INSERT INTO streams (size) VALUES (0)")
            .execute(&base.database)
            .await?
            .last_insert_rowid());

    Ok(WriteStream {
        base,
        id: stream_id,
        offset: 0,
        buff: Vec::with_capacity(STREAM_BLOB_SIZE),
    })
}

#[derive(From, Debug)]
pub enum OpenError {
    NoSuchStream,
    Database(sqlx::Error),
}

pub async fn open(base: ProjectBaseRef, stream_id: StreamId) -> Result<ReadStream, OpenError> {
    let (size,) = sqlx::query_as::<_, (i64,)>("SELECT size FROM streams WHERE rowid = ?")
        .bind(stream_id.0)
        .fetch_optional(&base.database)
        .await?
        .ok_or(OpenError::NoSuchStream)?;

    Ok(ReadStream {
        base,
        stream_id,
        offset: 0,
        size: size.try_into().expect("streams.size must not be negative"),
    })
}

// TODO - automatically clean up write stream on drop if not explicitly finalized
pub struct WriteStream {
    base: ProjectBaseRef,
    id: StreamId,
    offset: u64,
    buff: Vec<u8>,
}

impl WriteStream {
    pub async fn write(&mut self, mut bytes: &[u8]) -> Result<(), sqlx::Error> {
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

    pub async fn finalize(mut self) -> Result<StreamId, sqlx::Error> {
        self.flush().await?;
        Ok(self.id)
    }

    async fn flush(&mut self) -> Result<(), sqlx::Error> {
        if self.buff.len() > 0 {
            sqlx::query("INSERT INTO blobs (stream_id, offset, data) VALUES (?, ?, ?)")
                .bind(self.id.0)
                .bind(i64::try_from(self.offset).expect("offset as i64"))
                .bind(&self.buff)
                .execute(&self.base.database)
                .await?;

            self.offset += u64::try_from(self.buff.len()).expect("buff.len as u64");
            self.buff.truncate(0);
        }

        Ok(())
    }
}

pub struct ReadStream {
    base: ProjectBaseRef,
    stream_id: StreamId,
    offset: i64,
    size: i64,
}

impl ReadStream {
    pub async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let result = sqlx::query_as::<_, (Vec<u8>,)>("SELECT data FROM blobs WHERE stream_id = ? AND offset = ?")
            .bind(self.stream_id.0)
            .bind(self.offset)
            .fetch_optional(&self.base.database)
            .await?;

        match result {
            Some((data,)) => Ok(Some(data)),
            None => Ok(None),
        }
    }
}
