//! Game icons: one small square picture per game, for the sidebar, Recent and
//! collections.
//!
//! Sources, in order: the user's pick (SteamGridDB or an image file, kept for
//! good), then a square crop of the game's own cover. The crop is KB's own
//! work on a cached picture: no network, no key, and it follows the cover
//! when the cover changes (a manual pick never does).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rusqlite::Connection;

use super::images;
use crate::db::queries;
use crate::error::Result;

/// How long to rest between two crops, so a big backfill never eats a frame.
const PAUSE: Duration = Duration::from_millis(10);

/// Writes the automatic crop unless the user picked an icon.
/// Returns whether one was written.
pub fn auto(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<bool> {
    if queries::icon_manual(conn, game_id)? {
        return Ok(false);
    }
    if !images::store_icon_from_cover(data_dir, game_id)? {
        return Ok(false);
    }
    queries::bump_icon(conn, game_id)?;
    Ok(true)
}

/// Stores an icon the user picked (from the picker or an image file).
pub fn set_manual(conn: &Connection, data_dir: &Path, game_id: i64, bytes: &[u8]) -> Result<()> {
    images::store_icon(data_dir, game_id, bytes)?;
    queries::set_icon_manual(conn, game_id, true)?;
    queries::bump_icon(conn, game_id)?;
    Ok(())
}

/// "Use the cover": forgets a picked icon and crops the current cover instead.
pub fn reset_to_cover(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<()> {
    queries::set_icon_manual(conn, game_id, false)?;
    images::remove_icon(data_dir, game_id);
    if images::store_icon_from_cover(data_dir, game_id)? {
        queries::bump_icon(conn, game_id)?;
    } else {
        queries::clear_icon(conn, game_id)?;
    }
    Ok(())
}

/// A cover change (or its removal): the crop follows it, a picked icon stays.
pub fn cover_changed(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<()> {
    if queries::icon_manual(conn, game_id)? {
        return Ok(());
    }
    if images::store_icon_from_cover(data_dir, game_id)? {
        queries::bump_icon(conn, game_id)?;
    } else {
        images::remove_icon(data_dir, game_id);
        queries::clear_icon(conn, game_id)?;
    }
    Ok(())
}

/// The one-time backfill after an update, and new covers found later: crops
/// every game with a cover but no icon yet. Returns how many were written.
#[derive(Clone, serde::Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub written: usize,
    pub failed: usize,
}
#[derive(serde::Serialize)]
pub struct Summary {
    pub written: usize,
    pub failed: usize,
    pub cancelled: bool,
}
pub fn run(
    conn: &Connection,
    data_dir: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(Progress),
) -> Result<Summary> {
    let ids = queries::games_needing_icon(conn)?;
    let total = ids.len();
    let mut p = Progress {
        done: 0,
        total,
        written: 0,
        failed: 0,
    };
    progress(p.clone());
    for id in ids {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        match auto(conn, data_dir, id) {
            Ok(true) => p.written += 1,
            Ok(false) => {}
            Err(e) => {
                p.failed += 1;
                log::warn!("icon for game {id}: {e}");
            }
        }
        p.done += 1;
        progress(p.clone());
        std::thread::sleep(PAUSE);
    }
    log::info!("game icons: {} of {total} written", p.written);
    Ok(Summary {
        written: p.written,
        failed: p.failed,
        cancelled: cancel.load(Ordering::Acquire),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn game(conn: &Connection, art_status: &str) -> i64 {
        conn.execute(
            "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at, art_status)
             VALUES ('gba', ?2, ?2, ?2, ?3, 0, ?1)",
            rusqlite::params![art_status, format!("game-{art_status}-{}", conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get::<_, i64>(0)).unwrap()), format!("{art_status}-{}.gba", conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get::<_, i64>(0)).unwrap())],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn cover(data_dir: &Path, id: i64) {
        let p = images::process(&images::test_png(600, 900)).unwrap();
        images::store(data_dir, id, &p).unwrap();
    }

    fn version(conn: &Connection, id: i64) -> i64 {
        conn.query_row(
            "SELECT MAX(icon_version, 0) FROM games WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn a_cover_gets_an_icon_and_a_new_cover_moves_it_on() {
        let data = tempfile::tempdir().unwrap();
        let conn = db::test_db();
        let id = game(&conn, "ok");
        assert_eq!(version(&conn, id), 0);
        cover(data.path(), id);
        assert!(auto(&conn, data.path(), id).unwrap());
        let first = version(&conn, id);
        assert!(first > 0 && images::icon_path(data.path(), id).is_file());

        // The same cover again isn't re-cropped (the backfill skips it)...
        assert_eq!(queries::count_games_needing_icon(&conn).unwrap(), 0);
        // ...but a changed cover is.
        cover_changed(&conn, data.path(), id).unwrap();
        assert!(version(&conn, id) > first);
    }

    #[test]
    fn a_picked_icon_survives_a_new_cover_and_the_cover_comes_back_on_request() {
        let data = tempfile::tempdir().unwrap();
        let conn = db::test_db();
        let id = game(&conn, "ok");
        cover(data.path(), id);
        auto(&conn, data.path(), id).unwrap();
        set_manual(&conn, data.path(), id, &images::test_png(300, 300)).unwrap();
        let picked = version(&conn, id);
        assert!(queries::icon_manual(&conn, id).unwrap());

        // A new cover never replaces it.
        cover_changed(&conn, data.path(), id).unwrap();
        assert_eq!(version(&conn, id), picked);

        // Remove / "use the cover" goes back to the crop.
        reset_to_cover(&conn, data.path(), id).unwrap();
        assert!(!queries::icon_manual(&conn, id).unwrap());
        assert!(version(&conn, id) > picked);
        assert!(images::icon_path(data.path(), id).is_file());
    }

    #[test]
    fn the_backfill_only_takes_games_with_a_cover() {
        let data = tempfile::tempdir().unwrap();
        let conn = db::test_db();
        let with_cover = game(&conn, "ok");
        let without = game(&conn, "notfound");
        cover(data.path(), with_cover);
        assert_eq!(queries::games_needing_icon(&conn).unwrap(), [with_cover]);
        let seen = std::cell::RefCell::new(Vec::new());
        let result = run(&conn, data.path(), &AtomicBool::new(false), &|p| {
            seen.borrow_mut().push((p.done, p.total))
        })
        .unwrap();
        assert_eq!(result.written, 1);
        assert_eq!(*seen.borrow(), [(0, 1), (1, 1)]);
        assert!(!result.cancelled);
        let cancelled = run(&conn, data.path(), &AtomicBool::new(true), &|_| {}).unwrap();
        assert!(cancelled.cancelled);
        assert!(version(&conn, with_cover) > 0);
        assert_eq!(version(&conn, without), 0);
        assert!(queries::games_needing_icon(&conn).unwrap().is_empty());
    }

    #[test]
    fn cancelling_icon_backfill_reports_only_the_completed_items() {
        let data = tempfile::tempdir().unwrap();
        let conn = db::test_db();
        let first = game(&conn, "ok");
        let second = game(&conn, "ok");
        cover(data.path(), first);
        cover(data.path(), second);
        let cancel = AtomicBool::new(false);
        let seen = std::cell::RefCell::new(Vec::new());
        let summary = run(&conn, data.path(), &cancel, &|p| {
            seen.borrow_mut().push((p.done, p.total));
            if p.done == 1 {
                cancel.store(true, Ordering::Release);
            }
        })
        .unwrap();
        assert_eq!(*seen.borrow(), [(0, 2), (1, 2)]);
        assert_eq!(summary.written, 1);
        assert!(summary.cancelled);
        assert_eq!(queries::count_games_needing_icon(&conn).unwrap(), 1);
    }

    #[test]
    fn removing_the_cover_drops_an_automatic_icon_but_not_a_picked_one() {
        let data = tempfile::tempdir().unwrap();
        let conn = db::test_db();
        let auto_id = game(&conn, "ok");
        let picked_id = game(&conn, "ok");
        for id in [auto_id, picked_id] {
            cover(data.path(), id);
            auto(&conn, data.path(), id).unwrap();
        }
        set_manual(&conn, data.path(), picked_id, &images::test_png(300, 300)).unwrap();

        for id in [auto_id, picked_id] {
            std::fs::remove_file(images::thumb_path(data.path(), id)).unwrap();
            cover_changed(&conn, data.path(), id).unwrap();
        }
        assert_eq!(version(&conn, auto_id), 0);
        assert!(!images::icon_path(data.path(), auto_id).exists());
        assert!(queries::icon_manual(&conn, picked_id).unwrap());
        assert!(images::icon_path(data.path(), picked_id).exists());
    }
}
