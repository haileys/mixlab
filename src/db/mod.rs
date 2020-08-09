use std::path::PathBuf;

use rusqlite::{self, Connection, Row};
use tokio::task;

mod migrations;

fn schema_version(conn: &Connection) -> Result<Option<i64>, rusqlite::Error> {
    let result = conn.query_row::<i64, _, _>("SELECT version FROM schema_migrations WHERE rowid = 1", rusqlite::NO_PARAMS,
        |row: &Row| row.get::<_, i64>(0));

    match result {
        Ok(schema_version) => Ok(Some(schema_version)),
        // TODO double check what the rusqlite error here is
        // Err(sqlx::Error::Database(e)) if e.message() == "no such table: schema_migrations" => Ok(None),
        Err(e) => Err(e),
    }
}

fn update_schema_version(conn: &Connection, ver: i64) -> Result<(), rusqlite::Error> {
    conn.execute(r"
        INSERT INTO schema_migrations (rowid, version) VALUES (1, ?)
        ON CONFLICT (rowid) DO UPDATE SET version = excluded.version;
    ", &[ver])?;

    Ok(())
}

fn attach_blocking(path: PathBuf) -> Result<Connection, rusqlite::Error> {
    let mut conn = Connection::open(&path)?;

    {
        let mut txn = conn.transaction()?;

        let schema_version = schema_version(&mut txn)?;

        let mut migrations = migrations::MIGRATIONS.to_vec();

        // migrations should already be sorted, but we should ensure it is anyway:
        migrations.sort_by_key(|(ver, _)| *ver);

        // retain only migrations yet to be performed on this database
        migrations.retain(|(ver, _)| Some(*ver) > schema_version);

        // run migrations to bring database up to date
        for (_, sql) in &migrations {
            txn.execute(sql, rusqlite::NO_PARAMS)?;
        }

        // update database schema version if migrations were performed
        if let Some((ver, _)) = migrations.last() {
            update_schema_version(&mut txn, *ver)?;
        }

        txn.commit()?;
    }

    Ok(conn)
}

pub async fn attach(path: PathBuf) -> Result<Connection, rusqlite::Error> {
    task::spawn_blocking(|| attach_blocking(path)).await
        .expect("join blocking task")
}
