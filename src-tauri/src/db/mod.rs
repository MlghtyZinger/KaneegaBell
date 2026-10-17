//! SQLite access. One connection per thread: the UI-command connection lives in
//! `AppState`, background jobs (scanner) open their own. WAL lets them coexist.

pub mod queries;

use rusqlite::Connection;
use std::path::Path;

use crate::error::Result;

/// Ordered migrations; index + 1 == the `user_version` after applying it.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/001_init.sql"),
    include_str!("migrations/002_metadata.sql"),
    include_str!("migrations/003_gametdb.sql"),
    include_str!("migrations/004_igdb.sql"),
    include_str!("migrations/005_igdb_retry.sql"),
    include_str!("migrations/006_igdb_covers.sql"),
    include_str!("migrations/007_sgdb.sql"),
    include_str!("migrations/008_slugs.sql"),
    include_str!("migrations/009_launchbox.sql"),
    include_str!("migrations/010_wikidata.sql"),
    include_str!("migrations/011_series_order.sql"),
    include_str!("migrations/012_collections.sql"),
    include_str!("migrations/013_sgdb_names.sql"),
    include_str!("migrations/014_compressed_discs.sql"),
    include_str!("migrations/015_emulator_choice.sql"),
    include_str!("migrations/016_artwork_ids.sql"),
];

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
        // Their pictures and badges go in the startup cleanup.
        conn.execute("DELETE FROM settings WHERE key = 'leftoversSwept'", [])?;
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

#[cfg(test)]
mod tests {
    #[test]
    fn a_replaced_background_never_gets_the_old_version_back() {
        let conn = super::test_db();
        conn.execute(
            "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at) VALUES ('gba', 'A', 'a', 'a', 'a.gba', 0)",
            [],
        )
        .unwrap();
        let hero = || super::queries::library(&conn).unwrap()[0].hero;
        let mut seen = Vec::new();
        // The newest row is replaced each time, and removed once in between.
        for (i, remove) in [false, false, true, false].into_iter().enumerate() {
            if remove {
                super::queries::clear_extra(&conn, 1, "hero").unwrap();
            }
            super::queries::set_extra(&conn, 1, "hero", "file", &i.to_string(), "u", 1, 1, 0).unwrap();
            assert!(!seen.contains(&hero()), "version {} came back: {seen:?}", hero());
            seen.push(hero());
        }
    }
}
