use std::cmp;
use std::convert::TryFrom;
use std::io::SeekFrom;
use std::mem;

use rusqlite::{params, OptionalExtension, types::ValueRef};
use mixlab_codec::ffmpeg;

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
                    params![offset + buff_len, id.0])?;

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
}

impl ffmpeg::IoReader for ReadStream {
    type Error = rusqlite::Error;
    const BUFFER_SIZE: usize = STREAM_BLOB_SIZE;

    fn read(&mut self, out: &mut [u8]) -> Result<usize, Self::Error> {
        let mut read_bytes = 0;

        self.base.with_database_in_blocking_context(|conn| -> Result<(), rusqlite::Error> {
            let mut stmt = conn.prepare(r"
                SELECT offset, data FROM blobs
                WHERE stream_id = ? AND offset <= ?
                ORDER BY offset DESC
                LIMIT 1
            ")?;

            while out.len() > read_bytes {
                let cursor_offset = self.offset + read_bytes as i64;
                let mut rows = stmt.query(params![self.stream_id.0, cursor_offset])?;

                match rows.next()? {
                    Some(row) => {
                        // pull offset and data out of row
                        let blob_offset = row.get::<_, i64>(0)?;
                        let blob_data = match row.get_raw(1) {
                            ValueRef::Blob(data) => data,
                            _ => unreachable!("data column is always blob"),
                        };

                        // seek blob from db
                        let blob_data_offset = usize::try_from(cursor_offset - blob_offset)
                            .expect("cursor_offset > blob_offset");

                        let data = match blob_data.get(blob_data_offset..) {
                            Some([]) | None => {
                                // reached end of stream
                                break;
                            }
                            Some(data) => data,
                        };

                        // copy to out
                        let out = &mut out[read_bytes..];
                        let copy_len = cmp::min(out.len(), data.len());
                        out[..copy_len].copy_from_slice(&data[0..copy_len]);

                        // advance
                        read_bytes += copy_len;
                    }
                    None => {
                        // reached end of stream
                        break;
                    }
                }
            }

            Ok(())
        })?;

        self.offset += read_bytes as i64;
        Ok(read_bytes)
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        let new_offset = match pos {
            SeekFrom::Start(pos) => i64::try_from(pos).expect("seek position too large"),
            SeekFrom::Current(offset) => self.offset + offset,
            SeekFrom::End(offset) => self.size + offset,
        };

        self.offset = cmp::max(new_offset, 0);

        Ok(self.offset as u64)
    }

    fn size(&mut self) -> Result<u64, Self::Error> {
        Ok(self.size as u64)
    }
}
