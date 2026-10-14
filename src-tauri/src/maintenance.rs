//! Settings → General → Storage: how much downloaded data KB keeps, clearing
//! it, and resetting KB to a fresh start.
//!
//! Both only ever delete inside KB's own data folder: game files, emulators
//! and their saves are never touched.

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::error::Result;

/// The copy of the library a reset leaves behind, in case it was a mistake.
pub const RESET_BACKUP: &str = "library.before-reset.db";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    /// Covers, backgrounds, logos, badges and cached web responses.
    pub downloaded_bytes: u64,
    pub data_dir: String,
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

fn remove_dir(dir: &Path) -> Result<()> {
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

pub fn storage(conn: &Connection, data_dir: &Path) -> Result<Storage> {
    let cached: i64 = conn.query_row("SELECT COALESCE(SUM(LENGTH(body)), 0) FROM api_cache", [], |r| r.get(0))?;
    Ok(Storage {
        downloaded_bytes: dir_size(&data_dir.join("art")) + dir_size(&data_dir.join("ra")) + cached.max(0) as u64,
        data_dir: data_dir.to_string_lossy().into_owned(),
    })
}

/// Everything downloaded is forgotten and looked up again, except what the
/// user chose themselves: covers, backgrounds and logos they picked, and
/// IGDB and LaunchBox matches they fixed (their details are fetched again).
/// Achievement fingerprints are KB's own work on the game files, so they
/// stay; which RetroAchievements game each one is gets looked up again.
const CLEAR_SQL: &str = "
    DELETE FROM api_cache;
    DELETE FROM artwork WHERE NOT (
           (kind = 'boxart' AND game_id IN (SELECT id FROM games WHERE art_manual = 1))
        OR (kind IN ('hero', 'logo') AND game_id IN (SELECT id FROM games WHERE extras_manual = 1)));
    UPDATE games SET art_status = 'none', art_checked_at = NULL, art_version = art_version + 1 WHERE art_manual = 0;
    UPDATE games SET sgdb_status = 'none', sgdb_checked_at = NULL, sgdb_id = NULL WHERE extras_manual = 0;
    DELETE FROM game_genres;
    DELETE FROM genres;
    DELETE FROM metadata;
    UPDATE games SET igdb_id = NULL, match_status = 'none', meta_checked_at = NULL WHERE match_status != 'manual';
    UPDATE games SET meta_checked_at = NULL WHERE match_status = 'manual';
    UPDATE games SET ra_game_id = NULL;
    DELETE FROM lb_names;
    DELETE FROM lb_games;
    DELETE FROM lb_images;
    UPDATE games SET lb_status = 'none', lb_id = NULL WHERE lb_status != 'manual';
    DELETE FROM settings WHERE key = 'launchbox';
";

/// Deletes `<id>.webp` files in `dir` except those of the `keep` games.
fn prune_images(dir: &Path, keep: &HashSet<i64>) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else { return Ok(()) };
    for e in entries.flatten() {
        let path = e.path();
        let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<i64>().ok());
        if id.is_some_and(|id| keep.contains(&id)) {
            continue;
        }
        match fs::remove_file(&path) {
            Err(e) if e.kind() != ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(())
}

fn ids(conn: &Connection, sql: &str) -> Result<HashSet<i64>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Clears the downloaded data (see `CLEAR_SQL`). Returns the bytes freed.
pub fn clear_downloaded(conn: &mut Connection, data_dir: &Path) -> Result<u64> {
    let before = storage(conn, data_dir)?.downloaded_bytes;
    let tx = conn.transaction()?;
    tx.execute_batch(CLEAR_SQL)?;
    tx.commit()?;

    let art = data_dir.join("art");
    let covers = ids(conn, "SELECT id FROM games WHERE art_manual = 1")?;
    let extras = ids(conn, "SELECT id FROM games WHERE extras_manual = 1")?;
    prune_images(&art.join("t"), &covers)?;
    prune_images(&art.join("f"), &covers)?;
    prune_images(&art.join("h"), &extras)?;
    prune_images(&art.join("l"), &extras)?;
    remove_dir(&data_dir.join("ra"))?;
    // Give the space back (best effort: another connection may be reading).
    if let Err(e) = conn.execute_batch("VACUUM") {
        log::info!("couldn't compact the library after clearing: {e}");
    }
    Ok(before.saturating_sub(storage(conn, data_dir)?.downloaded_bytes))
}

/// Back to a first start: no games, folders, play time, favorites, emulator
/// paths or settings. A copy of the library is kept as `RESET_BACKUP` in the
/// data folder first (replacing an older one). Saved keys are handled by the
/// caller.
pub fn reset(conn: &mut Connection, data_dir: &Path) -> Result<PathBuf> {
    let backup = data_dir.join(RESET_BACKUP);
    match fs::remove_file(&backup) {
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])?;

    let tx = conn.transaction()?;
    // Games first: their files, art, details and genres go with them.
    tx.execute_batch(
        "DELETE FROM games;
         DELETE FROM rom_sources;
         DELETE FROM genres;
         DELETE FROM api_cache;
         DELETE FROM lb_names;
         DELETE FROM lb_games;
         DELETE FROM lb_images;
         DELETE FROM settings;
         DELETE FROM platforms;
         DELETE FROM emulators;",
    )?;
    crate::platforms::seed(&tx)?;
    tx.commit()?;

    for dir in ["art", "ra", "playlists"] {
        remove_dir(&data_dir.join(dir))?;
    }
    if let Err(e) = conn.execute_batch("VACUUM") {
        log::info!("couldn't compact the library after the reset: {e}");
    }
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn add_game(conn: &Connection, title: &str) -> i64 {
        conn.execute(
            "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at, art_status,
                                match_status, igdb_id, ra_hash, ra_game_id, favorite, play_seconds)
             VALUES ('gba', ?1, ?1, ?1, ?1, 0, 'ok', 'ok', 5, 'abc', 9, 1, 3600)",
            [title],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        for kind in ["boxart", "hero", "logo"] {
            conn.execute(
                "INSERT INTO artwork (game_id, kind, source, remote_id, url, selected, fetched_at)
                 VALUES (?1, ?2, 'sgdb', ?2, 'https://x', 1, 0)",
                rusqlite::params![id, kind],
            )
            .unwrap();
        }
        conn.execute("INSERT INTO metadata (game_id, title, source, fetched_at) VALUES (?1, ?2, 'igdb', 0)", rusqlite::params![id, title])
            .unwrap();
        id
    }

    fn write(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"img").unwrap();
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn clearing_keeps_the_library_and_what_the_user_picked() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let mut conn = db::test_db();
        let auto = add_game(&conn, "Auto");
        let picked = add_game(&conn, "Picked");
        conn.execute("UPDATE games SET art_manual = 1, extras_manual = 1, match_status = 'manual' WHERE id = ?1", [picked])
            .unwrap();
        conn.execute("INSERT INTO api_cache (key, body, fetched_at) VALUES ('k', x'0102', 0)", []).unwrap();
        crate::db::queries::set_setting(&conn, "autoArt", &serde_json::json!(false)).unwrap();
        for id in [auto, picked] {
            for d in ["t", "f", "h", "l"] {
                write(&data.join("art").join(d).join(format!("{id}.webp")));
            }
        }
        write(&data.join("ra").join("badges").join("1.png"));

        let freed = clear_downloaded(&mut conn, data).unwrap();
        assert!(freed > 0);

        // The library, play time, favorites and settings are untouched.
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM games WHERE favorite = 1 AND play_seconds = 3600"), 2);
        assert_eq!(crate::db::queries::get_setting::<bool>(&conn, "autoArt").unwrap(), Some(false));
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM api_cache"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM metadata"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM games WHERE ra_game_id IS NOT NULL"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM games WHERE ra_hash = 'abc'"), 2, "fingerprints kept");

        // The automatic game is looked up from scratch...
        let (art, matched, igdb): (String, String, Option<i64>) = conn
            .query_row("SELECT art_status, match_status, igdb_id FROM games WHERE id = ?1", [auto], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((art.as_str(), matched.as_str(), igdb), ("none", "none", None));
        assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM artwork WHERE game_id = {auto}")), 0);
        for d in ["t", "f", "h", "l"] {
            assert!(!data.join("art").join(d).join(format!("{auto}.webp")).exists());
        }
        // ...while the user's picks stay, and the fixed match keeps its IGDB id.
        let (art, matched, igdb): (String, String, Option<i64>) = conn
            .query_row("SELECT art_status, match_status, igdb_id FROM games WHERE id = ?1", [picked], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((art.as_str(), matched.as_str(), igdb), ("ok", "manual", Some(5)));
        assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM artwork WHERE game_id = {picked}")), 3);
        for d in ["t", "f", "h", "l"] {
            assert!(data.join("art").join(d).join(format!("{picked}.webp")).exists());
        }
        assert!(!data.join("ra").exists());
    }

    #[test]
    fn reset_empties_everything_and_keeps_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let path = data.join("library.db");
        let mut conn = db::init(&path).unwrap();
        add_game(&conn, "Game");
        conn.execute("INSERT INTO rom_sources (path) VALUES ('E:\\Games')", []).unwrap();
        conn.execute("UPDATE emulators SET exe_path = 'E:\\RetroArch\\retroarch.exe'", []).unwrap();
        conn.execute("UPDATE platforms SET core_override = 'bsnes', enabled = 0 WHERE id = 'snes'", []).unwrap();
        crate::db::queries::set_setting(&conn, "tvMode", &serde_json::json!(true)).unwrap();
        write(&data.join("art").join("t").join("1.webp"));
        write(&data.join("playlists").join("game-1.m3u"));

        let backup = reset(&mut conn, data).unwrap();

        // Every table is empty except the built-in platforms and emulators,
        // which are back to their defaults.
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for t in &tables {
            let n = count(&conn, &format!("SELECT COUNT(*) FROM {t}"));
            match t.as_str() {
                "platforms" | "emulators" => assert!(n > 0, "{t} reseeded"),
                _ => assert_eq!(n, 0, "{t} emptied"),
            }
        }
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM emulators WHERE exe_path IS NOT NULL"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM platforms WHERE core_override IS NOT NULL OR enabled = 0"), 0);
        assert!(!data.join("art").exists() && !data.join("playlists").exists());

        // The backup still has the old library.
        let old = Connection::open(&backup).unwrap();
        assert_eq!(count(&old, "SELECT COUNT(*) FROM games"), 1);
        assert_eq!(count(&old, "SELECT COUNT(*) FROM rom_sources"), 1);
        drop(old);

        // A second reset replaces the backup.
        reset(&mut conn, data).unwrap();
        let old = Connection::open(&backup).unwrap();
        assert_eq!(count(&old, "SELECT COUNT(*) FROM games"), 0);
    }
}
