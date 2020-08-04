use std::path::Path;

use sqlx::sqlite::{SqlitePool, SqliteConnectOptions};

mod migrations;

async fn schema_version(pool: &SqlitePool) -> Result<Option<i32>, sqlx::Error> {
    let result = sqlx::query_as::<_, (i32,)>("SELECT version FROM schema_migrations WHERE rowid = 1")
        .fetch_one(pool)
        .await;

    match result {
        Ok((schema_version,)) => Ok(Some(schema_version)),
        Err(sqlx::Error::Database(e)) if e.message() == "no such table: schema_migrations" => Ok(None),
        Err(e) => Err(e),
    }
}

async fn update_schema_version(pool: &SqlitePool, ver: i32) -> Result<(), sqlx::Error> {
    sqlx::query(r"
            INSERT INTO schema_migrations (rowid, version) VALUES (1, ?)
            ON CONFLICT (rowid) DO UPDATE SET version = excluded.version;
        ")
        .bind(ver)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn attach(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePool::connect_with(SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)).await?;

    let schema_version = schema_version(&pool).await?;

    let mut migrations = migrations::MIGRATIONS.to_vec();

    // migrations should already be sorted, but we should ensure it is anyway:
    migrations.sort_by_key(|(ver, _)| *ver);

    // retain only migrations yet to be performed on this database
    migrations.retain(|(ver, _)| Some(*ver) > schema_version);

    // run migrations to bring database up to date
    for (_, sql) in &migrations {
        sqlx::query(sql).execute(&pool).await?;
    }

    // update database schema version if migrations were performed
    if let Some((ver, _)) = migrations.last() {
        update_schema_version(&pool, *ver).await?;
    }

    Ok(pool)
}
