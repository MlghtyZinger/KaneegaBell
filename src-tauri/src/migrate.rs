//! One-time move of the library from the app's old data folder (from before
//! it was renamed KaneegaBell) to the new one.
//!
//! Copy, verify, then switch; the old folder is never modified or deleted.
//! The database is copied with `VACUUM INTO` (a consistent snapshot even with
//! a WAL file next to it), then checked: it must pass `integrity_check` and
//! hold the same games and schema version as the original. Only then does
//! the copy become the new folder. If anything fails, the app keeps using the
//! old folder for this run and says so.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::error::{Error, Result};

/// The data folder's name before the rename (Tauri's old identifier).
pub const LEGACY_IDENTIFIER: &str = "com.sultanmatoo.emufrontend";
const DB: &str = "library.db";

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to do: the new folder has a library, or there's no old one.
    NotNeeded,
    /// Copied and verified; the new folder is in use.
    Migrated { games: i64 },
}

fn snapshot(conn: &Connection) -> rusqlite::Result<(i64, i64)> {
    let games = conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?;
    let version = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    Ok((games, version))
}

/// Copies everything except the database files (art cache, achievement
/// badges, playlists...).
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(DB) {
            continue; // library.db, -wal, -shm: copied as one snapshot instead
        }
        let (src, dst) = (entry.path(), to.join(&name));
        if entry.file_type()?.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Moves `old` to `new` if `new` has no library yet and `old` has one.
pub fn migrate(old: &Path, new: &Path) -> Result<Outcome> {
    if new.join(DB).exists() || !old.join(DB).exists() {
        return Ok(Outcome::NotNeeded);
    }
    let staging = staging_dir(new);
    let _ = fs::remove_dir_all(&staging); // a half-done earlier attempt
    let result = (|| -> Result<i64> {
        copy_tree(old, &staging)?;
        let src = Connection::open_with_flags(old.join(DB), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let expected = snapshot(&src)?;
        let target = staging.join(DB);
        src.execute("VACUUM INTO ?1", [target.to_string_lossy()])?;
        drop(src);

        let copy = Connection::open_with_flags(&target, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let check: String = copy.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if check != "ok" {
            return Err(Error::msg(format!("the copied library failed its integrity check ({check})")));
        }
        let got = snapshot(&copy)?;
        if got != expected {
            return Err(Error::msg(format!(
                "the copied library doesn't match the original ({} vs {} games)",
                got.0, expected.0
            )));
        }
        drop(copy);

        fs::write(
            staging.join("migrated-from.txt"),
            format!("Copied from {} on first start of KaneegaBell. The old folder was left as it was.\n", old.display()),
        )?;
        // Switch: the staging folder becomes the data folder. A `new` folder
        // without a library (e.g. created empty earlier) is set aside first.
        if new.exists() {
            let aside = new.with_file_name(format!("{}.before-migration", file_name(new)));
            let _ = fs::remove_dir_all(&aside);
            fs::rename(new, &aside)?;
        }
        fs::rename(&staging, new)?;
        Ok(expected.0)
    })();
    match result {
        Ok(games) => Ok(Outcome::Migrated { games }),
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            Err(e)
        }
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn staging_dir(new: &Path) -> PathBuf {
    new.with_file_name(format!("{}.migrating", file_name(new)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An old data folder with a real (migrated) library holding `n` games, art and a WAL file.
    fn old_folder(root: &Path, n: usize) -> PathBuf {
        let old = root.join(LEGACY_IDENTIFIER);
        fs::create_dir_all(old.join("art").join("t")).unwrap();
        fs::write(old.join("art").join("t").join("1.webp"), b"thumb").unwrap();
        let conn = Connection::open(old.join(DB)).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        let mut migrated = conn;
        crate::db::migrate(&mut migrated).unwrap();
        crate::platforms::seed(&migrated).unwrap();
        for i in 0..n {
            migrated
                .execute(
                    "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at, play_seconds)
                     VALUES ('gba', ?1, ?1, ?1, ?1, 0, 3600)",
                    [format!("Game {i}")],
                )
                .unwrap();
        }
        // Keep the connection open while migrating, so the rows sit in the WAL.
        std::mem::forget(migrated);
        old
    }

    #[test]
    fn copies_verifies_and_leaves_the_old_folder_alone() {
        let t = tempfile::tempdir().unwrap();
        let old = old_folder(t.path(), 3);
        let new = t.path().join("com.kaneegabell.kb");
        assert_eq!(migrate(&old, &new).unwrap(), Outcome::Migrated { games: 3 });

        let conn = Connection::open(new.join(DB)).unwrap();
        let (games, secs): (i64, i64) =
            conn.query_row("SELECT COUNT(*), SUM(play_seconds) FROM games", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((games, secs), (3, 3 * 3600), "games and play time came along (from the WAL too)");
        assert_eq!(fs::read(new.join("art").join("t").join("1.webp")).unwrap(), b"thumb");
        assert!(new.join("migrated-from.txt").is_file());
        assert!(old.join(DB).is_file() && old.join("art").join("t").join("1.webp").is_file(), "old folder untouched");
        assert!(!staging_dir(&new).exists());

        // Second start: nothing to do.
        assert_eq!(migrate(&old, &new).unwrap(), Outcome::NotNeeded);
    }

    #[test]
    fn nothing_to_do_on_a_fresh_install() {
        let t = tempfile::tempdir().unwrap();
        let new = t.path().join("com.kaneegabell.kb");
        assert_eq!(migrate(&t.path().join(LEGACY_IDENTIFIER), &new).unwrap(), Outcome::NotNeeded);
        assert!(!new.exists());
    }

    #[test]
    fn an_empty_new_folder_is_set_aside_not_lost() {
        let t = tempfile::tempdir().unwrap();
        let old = old_folder(t.path(), 1);
        let new = t.path().join("com.kaneegabell.kb");
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join("stray.txt"), b"x").unwrap();
        assert_eq!(migrate(&old, &new).unwrap(), Outcome::Migrated { games: 1 });
        assert!(t.path().join("com.kaneegabell.kb.before-migration").join("stray.txt").is_file());
    }

    #[test]
    fn a_broken_old_library_fails_safely() {
        let t = tempfile::tempdir().unwrap();
        let old = t.path().join(LEGACY_IDENTIFIER);
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join(DB), b"this is not a database").unwrap();
        let new = t.path().join("com.kaneegabell.kb");
        assert!(migrate(&old, &new).is_err());
        assert!(!new.exists() && !staging_dir(&new).exists(), "no half-made folder left behind");
        assert_eq!(fs::read(old.join(DB)).unwrap(), b"this is not a database", "old folder untouched");
    }
}
