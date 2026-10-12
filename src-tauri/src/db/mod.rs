//! SQLite access. One connection per thread: the UI-command connection lives in
//! `AppState`, background jobs (scanner) open their own. WAL lets them coexist.

pub mod queries;

use rusqlite::Connection;
use std::path::Path;

use crate::error::Result;

/// Ordered migrations; index + 1 == the `user_version` after applying it.
const MIGRATIONS: &[&str] = &[include_str!("migrations/001_init.sql")];

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;",
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

pub fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as i64 + 1)?;
        tx.commit()?;
        log::info!("applied migration {}", i + 1);
    }
    Ok(())
}

/// Opens (or creates) the database, applies migrations and seeds built-in platforms.
pub fn init(path: &Path) -> Result<Connection> {
    let mut conn = open(path)?;
    migrate(&mut conn)?;
    crate::platforms::seed(&conn)?;
    queries::reconcile_interrupted_sessions(&conn)?;
    let pruned = queries::prune_orphans(&conn)?;
    if pruned > 0 {
        log::info!("removed {pruned} leftover entries from deleted folders");
    }
    Ok(conn)
}

#[cfg(test)]
pub fn test_db() -> Connection {
    let mut conn = open_in_memory().unwrap();
    migrate(&mut conn).unwrap();
    crate::platforms::seed(&conn).unwrap();
    conn
}
