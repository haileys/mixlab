CREATE TABLE media (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    stream_id INTEGER NOT NULL,
    FOREIGN KEY (stream_id) REFERENCES streams (id)
);

CREATE TABLE streams (
    id INTEGER PRIMARY KEY NOT NULL,
    size INTEGER NOT NULL,
    CONSTRAINT non_negative_size CHECK (size >= 0)
);

CREATE TABLE blobs (
    stream_id INTEGER NOT NULL,
    offset INTEGER NOT NULL,
    data BLOB NOT NULL,
    FOREIGN KEY (stream_id) REFERENCES streams (id),
    CONSTRAINT non_negative_offset CHECK (offset >= 0)
);

CREATE UNIQUE INDEX blob_sequence_idx ON blobs (stream_id, offset);
