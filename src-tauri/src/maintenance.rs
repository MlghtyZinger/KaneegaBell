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
    /// Covers, game icons, backgrounds, logos, badges and cached web responses.
    pub downloaded_bytes: u64,
    pub data_dir: String,
    /// Size of the copy of the library the last reset kept, if there is one.
    pub reset_backup_bytes: Option<u64>,
    /// WebView2's disk cache (picker previews); capped, rebuilt as needed.
    pub web_cache_bytes: u64,
    /// LaunchBox's kept game database file, if it has been downloaded.
    pub launchbox_bytes: Option<u64>,
}

/// Where WebView2 keeps its disk cache inside KB's folder.
pub fn web_cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("webview").join("EBWebView").join("Default").join("Cache")
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
        reset_backup_bytes: reset_backup_bytes(data_dir),
        web_cache_bytes: dir_size(&web_cache_dir(data_dir)),
        launchbox_bytes: crate::launchbox::zip_bytes(data_dir),
    })
}

/// Everything downloaded is forgotten and looked up again, except what the
/// user chose themselves: covers, backgrounds and logos they picked, and
/// IGDB and LaunchBox matches they fixed (their details are fetched again).
/// Achievement fingerprints are KB's own work on the game files, so they
/// stay; which RetroAchievements game each one is gets looked up again.
/// WebView2's web cache is in use while KB runs, so it's emptied on the next
/// start (see `trim_web_cache`).
const CLEAR_SQL: &str = "
    DELETE FROM api_cache;
    DELETE FROM artwork WHERE NOT (
           (kind = 'boxart' AND game_id IN (SELECT id FROM games WHERE art_manual = 1))
        OR (kind IN ('hero', 'logo') AND game_id IN (SELECT id FROM games WHERE extras_manual = 1)));
    UPDATE games SET art_status = 'none', art_checked_at = NULL, art_version = art_version + 1 WHERE art_manual = 0;
    UPDATE games SET icon_version = -ABS(icon_version) WHERE icon_manual = 0;
    UPDATE games SET sgdb_status = 'none', sgdb_checked_at = NULL, sgdb_id = NULL WHERE extras_manual = 0;
    DELETE FROM game_genres;
    DELETE FROM genres;
    DELETE FROM metadata;
    DELETE FROM igdb_game_series;
    DELETE FROM igdb_series;
    UPDATE games SET igdb_id = NULL, match_status = 'none', meta_checked_at = NULL WHERE match_status != 'manual';
    UPDATE games SET meta_checked_at = NULL WHERE match_status = 'manual';
    UPDATE games SET ra_game_id = NULL;
    DELETE FROM lb_names;
    DELETE FROM lb_games;
    DELETE FROM lb_images;
    UPDATE games SET lb_status = 'none', lb_id = NULL WHERE lb_status != 'manual';
    DELETE FROM settings WHERE key = 'launchbox';
    DELETE FROM wd_games;
    DELETE FROM wd_series;
    DELETE FROM wd_game_series;
    UPDATE games SET wd_status = 'none', wd_id = NULL;
    DELETE FROM settings WHERE key = 'wikidata';
    DELETE FROM settings WHERE key = 'webCacheTrimmed';
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

/// Games whose files are gone. KB keeps them hidden when they carry play
/// time, a favorite, a collection or one of the user's picks (so that comes
/// back with the file); the others are deleted by the scan.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
    pub count: i64,
}

pub fn removed(conn: &Connection) -> Result<Removed> {
    Ok(Removed { count: conn.query_row("SELECT COUNT(*) FROM games WHERE missing = 1", [], |r| r.get(0))? })
}

/// Forgets the removed (hidden) games for good: their play time, favorites,
/// collection memberships, details and pictures. Returns how many.
pub fn forget_removed(conn: &Connection, data_dir: &Path) -> Result<usize> {
    // Not one that's running right now (its session is still being counted).
    let n = conn.execute("DELETE FROM games WHERE missing = 1 AND running_since IS NULL", [])?;
    prune_leftovers(conn, data_dir)?;
    Ok(n)
}

/// Deletes what games that are no longer in the library left behind: their
/// pictures, and their RetroAchievements lists, badges and icons (those no
/// remaining game uses). Hidden games keep everything.
pub fn prune_leftovers(conn: &Connection, data_dir: &Path) -> Result<()> {
    let keep = ids(conn, "SELECT id FROM games")?;
    for d in ["t", "f", "h", "l", "i", "d", "b", "s", "q"] {
        prune_images(&data_dir.join("art").join(d), &keep)?;
    }
    crate::achievements::prune_removed(conn, data_dir)?;
    Ok(())
}

/// Shortly after startup, once: cleans up what removed games left behind
/// (from versions before 0.2.9, or rows the startup housekeeping deleted).
pub fn sweep_leftovers_once(conn: &Connection, data_dir: &Path) {
    if crate::db::queries::get_setting::<bool>(conn, "leftoversSwept").ok().flatten() == Some(true) {
        return;
    }
    match prune_leftovers(conn, data_dir) {
        Ok(()) => {
            let _ = crate::db::queries::set_setting(conn, "leftoversSwept", &serde_json::Value::Bool(true));
        }
        Err(e) => log::warn!("couldn't clean up after removed games: {e}"),
    }
}

/// Whether the copy of the library a reset leaves behind exists, and its size.
pub fn reset_backup_bytes(data_dir: &Path) -> Option<u64> {
    fs::metadata(data_dir.join(RESET_BACKUP)).ok().filter(|m| m.is_file()).map(|m| m.len())
}

/// "Delete the reset backup".
pub fn delete_reset_backup(data_dir: &Path) -> Result<()> {
    match fs::remove_file(data_dir.join(RESET_BACKUP)) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
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
    let icons = ids(conn, "SELECT id FROM games WHERE icon_manual = 1")?;
    prune_images(&art.join("t"), &covers)?;
    prune_images(&art.join("f"), &covers)?;
    prune_images(&art.join("h"), &extras)?;
    prune_images(&art.join("l"), &extras)?;
    prune_images(&art.join("i"), &icons)?;
    prune_images(&art.join("q"), &HashSet::new())?;
    // Discs, backs and spines: the downloaded ones (the user's picks stay).
    crate::art::images::clear_case_art(data_dir);
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
         DELETE FROM wd_games;
         DELETE FROM wd_series;
         DELETE FROM wd_game_series;
         DELETE FROM igdb_series;
         DELETE FROM collections;
         DELETE FROM collection_looks;
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
        crate::db::queries::set_setting(&conn, "webCacheTrimmed", &serde_json::json!(true)).unwrap();
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
        assert_eq!(crate::db::queries::get_setting::<bool>(&conn, "webCacheTrimmed").unwrap(), None, "web cache emptied next start");
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
    fn removed_games_can_be_forgotten_with_their_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let conn = db::test_db();
        let kept = add_game(&conn, "Still here");
        let gone = add_game(&conn, "Gone but played");
        let running = add_game(&conn, "Gone, running");
        conn.execute("UPDATE games SET missing = 1 WHERE id IN (?1, ?2)", [gone, running]).unwrap();
        conn.execute("UPDATE games SET running_since = 1 WHERE id = ?1", [running]).unwrap();
        let col = crate::collections::create(&conn, "Mine", Some(gone)).unwrap();
        for id in [kept, gone] {
            for d in ["t", "f", "h", "l"] {
                write(&data.join("art").join(d).join(format!("{id}.webp")));
            }
        }
        // A leftover picture of a game deleted earlier.
        write(&data.join("art").join("t").join("999.webp"));
        assert_eq!(removed(&conn).unwrap(), Removed { count: 2 });

        assert_eq!(forget_removed(&conn, data).unwrap(), 1);
        assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM games WHERE id = {gone}")), 0);
        assert_eq!(count(&conn, &format!("SELECT COUNT(*) FROM games WHERE id = {running}")), 1, "not while it runs");
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM collection_games"), 0, "left its collection");
        assert!(crate::collections::list(&conn).unwrap().iter().any(|c| c.key == col), "the collection stays");
        for d in ["t", "f", "h", "l"] {
            assert!(!data.join("art").join(d).join(format!("{gone}.webp")).exists());
            assert!(data.join("art").join(d).join(format!("{kept}.webp")).exists());
        }
        assert!(!data.join("art").join("t").join("999.webp").exists());
    }

    fn progress(badges: &[&str], icon: &str) -> Vec<u8> {
        let list: serde_json::Map<String, serde_json::Value> = badges
            .iter()
            .enumerate()
            .map(|(i, b)| (i.to_string(), serde_json::json!({"ID": i, "Title": "a", "BadgeName": b})))
            .collect();
        serde_json::to_vec(&serde_json::json!({"Title": "g", "ImageIcon": format!("/Images/{icon}.png"), "Achievements": list}))
            .unwrap()
    }

    #[test]
    fn removed_games_leave_no_badges_behind() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let conn = db::test_db();
        add_game(&conn, "Kept"); // ra_game_id 9
        let hidden = add_game(&conn, "Hidden");
        conn.execute("UPDATE games SET ra_game_id = 7, missing = 1 WHERE id = ?1", [hidden]).unwrap();
        // Game 5 left the library; it shares badge 300 with game 9.
        for (id, badges, icon) in [(9, ["100", "300"], "900"), (7, ["200", "201"], "700"), (5, ["500", "300"], "555")] {
            conn.execute(
                "INSERT INTO api_cache (key, body, fetched_at) VALUES (?1, ?2, 0)",
                rusqlite::params![format!("ra:progress:{id}:me"), progress(&badges, icon)],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO api_cache (key, body, fetched_at) VALUES (?1, x'00', 0)",
                [format!("ra:gamehashes:{id}")],
            )
            .unwrap();
            for b in badges {
                write(&data.join("ra").join("badges").join(format!("{b}.png")));
                write(&data.join("ra").join("badges").join(format!("{b}_lock.png")));
            }
            write(&data.join("ra").join("icons").join(format!("{icon}.png")));
        }
        // A badge no cached list mentions is left alone.
        write(&data.join("ra").join("badges").join("999.png"));

        prune_leftovers(&conn, data).unwrap();

        let badge = |n: &str| data.join("ra").join("badges").join(format!("{n}.png")).exists();
        for n in ["100", "100_lock", "300", "300_lock", "200", "201_lock", "999"] {
            assert!(badge(n), "{n} kept");
        }
        assert!(!badge("500") && !badge("500_lock"), "only the removed game used 500");
        assert!(!data.join("ra").join("icons").join("555.png").exists());
        assert!(data.join("ra").join("icons").join("700.png").exists(), "hidden games keep theirs");
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM api_cache WHERE key LIKE '%:5:%' OR key LIKE '%:5'"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM api_cache WHERE key LIKE 'ra:%'"), 4);
    }

    #[test]
    fn leftovers_are_swept_once_and_again_after_startup_pruning() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let conn = db::test_db();
        write(&data.join("art").join("h").join("42.webp"));
        sweep_leftovers_once(&conn, data);
        assert!(!data.join("art").join("h").join("42.webp").exists());

        write(&data.join("art").join("h").join("43.webp"));
        sweep_leftovers_once(&conn, data);
        assert!(data.join("art").join("h").join("43.webp").exists(), "only once");

        conn.execute("DELETE FROM settings WHERE key = 'leftoversSwept'", []).unwrap();
        sweep_leftovers_once(&conn, data);
        assert!(!data.join("art").join("h").join("43.webp").exists());
    }

    #[test]
    fn the_reset_backup_can_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        assert_eq!(reset_backup_bytes(data), None);
        write(&data.join(RESET_BACKUP));
        assert_eq!(reset_backup_bytes(data), Some(3));
        delete_reset_backup(data).unwrap();
        assert_eq!(reset_backup_bytes(data), None);
        delete_reset_backup(data).unwrap();
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
