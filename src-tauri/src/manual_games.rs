//! Player-added entries can exist before their game files are available.
use crate::{
    db::queries,
    error::{Error, Result},
    scanner::normalize,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub fn contains(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM manual_games WHERE game_id=?1", [id], |_| {
            Ok(())
        })
        .optional()?
        .is_some())
}

pub fn require_file(conn: &Connection, id: i64, path: &str) -> Result<()> {
    if contains(conn, id)? && !Path::new(path).is_file() {
        return Err(Error::msg(
            "No game file is available. Choose a game file in Game files before playing.",
        ));
    }
    Ok(())
}

/// Reuse the same platform-specific name index as Fix match, without needing a library game.
pub fn search(
    conn: &Connection,
    platform: &str,
    query: &str,
) -> Result<Vec<crate::launchbox::matching::Hit>> {
    check_platform(conn, platform)?;
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    if query.chars().count() > 200 {
        return Err(Error::msg("Keep the search under 200 characters."));
    }
    crate::launchbox::search_platform(conn, platform, query)
}

fn check_platform(conn: &Connection, platform: &str) -> Result<()> {
    if crate::launchers::is_launcher(platform)
        || !conn
            .query_row(
                "SELECT enabled FROM platforms WHERE id=?1",
                [platform],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(false)
    {
        return Err(Error::msg("Choose an enabled game platform. Client games are added through their launcher connection."));
    }
    Ok(())
}

/// Everything is committed together; failed file validation never leaves a partial entry.
pub fn add(
    conn: &mut Connection,
    platform: &str,
    title: &str,
    db_id: Option<i64>,
    kind: &str,
    file: Option<&str>,
) -> Result<i64> {
    check_platform(conn, platform)?;
    if !matches!(kind, "emulator" | "port" | "recomp" | "decomp") {
        return Err(Error::msg("Choose Emulator, Port, Recomp or Decomp."));
    }
    let title = if let Some(db_id) = db_id {
        conn.query_row(
            "SELECT name FROM lb_names WHERE platform_id=?1 AND db_id=?2 AND is_alt=0 LIMIT 1",
            params![platform, db_id],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| Error::msg("That catalogue title is no longer available. Search again."))?
    } else {
        title.trim().to_string()
    };
    if title.is_empty() || title.chars().count() > 200 {
        return Err(Error::msg("Enter a game title (up to 200 characters)."));
    }
    let tx = conn.transaction()?;
    // NULL source makes this independent of all ROM folder scans. A temporary unique
    // path reserves the ID; it is replaced with the stable marker or selected file.
    tx.execute(
        "INSERT INTO games(platform_id,title,sort_title,search_key,launch_path,added_at)
        VALUES (?1,?2,?3,?4,lower(hex(randomblob(16))),?5)",
        params![
            platform,
            title,
            normalize::sort_title(&title),
            normalize::search_key(&title),
            queries::now()
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE games SET launch_path=?2 WHERE id=?1",
        params![id, format!("kb-manual:{id}")],
    )?;
    tx.execute("INSERT INTO manual_games(game_id) VALUES (?1)", [id])?;
    if let Some(file) = file.filter(|f| !f.trim().is_empty()) {
        if kind == "emulator" {
            set_file(&tx, id, file)?;
        } else {
            crate::direct_launch::set(
                &tx,
                id,
                Some(crate::direct_launch::DirectLaunch {
                    kind: kind.into(),
                    executable: file.into(),
                }),
            )?;
        }
    } else if kind != "emulator" {
        // A known type with no executable yet. Launch validation explains what to choose.
        tx.execute(
            "INSERT INTO direct_launch(game_id,kind,executable) VALUES (?1,?2,'')",
            params![id, kind],
        )?;
    }
    if let Some(db_id) = db_id {
        crate::launchbox::set_manual(&tx, id, db_id)?;
    }
    tx.commit()?;
    Ok(id)
}

pub fn set_file(conn: &Connection, id: i64, file: &str) -> Result<()> {
    if !contains(conn, id)? {
        return Err(Error::msg("This game is managed by its scanned folder."));
    }
    let path = Path::new(file);
    if !path.is_absolute() || !path.is_file() {
        return Err(Error::msg("Choose an existing game file."));
    }
    let path = std::fs::canonicalize(path)?;
    let metadata = std::fs::metadata(&path)?;
    let duplicate: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM games WHERE id!=?1 AND launch_path=?2)",
        params![id, path.to_string_lossy()],
        |r| r.get(0),
    )?;
    if duplicate {
        return Err(Error::msg("That game file is already in the library."));
    }
    conn.execute(
        "UPDATE games SET launch_path=?2,total_size=?3,missing=0 WHERE id=?1",
        params![id, path.to_string_lossy(), metadata.len() as i64],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, id: i64) -> Result<()> {
    if !contains(conn, id)? {
        return Err(Error::msg(
            "Only manually added entries can be removed here.",
        ));
    }
    conn.execute("DELETE FROM games WHERE id=?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_entries_need_no_files_and_survive_scan_cleanup() {
        let mut conn = crate::db::test_db();
        conn.execute("INSERT INTO lb_names(db_id,platform_id,name,is_alt) VALUES (123,'n64','Example Game',0)", []).unwrap();
        assert_eq!(search(&conn, "n64", "Example").unwrap()[0].db_id, 123);
        assert!(search(&conn, "n64", "").unwrap().is_empty());
        let id = add(&mut conn, "n64", "ignored", Some(123), "recomp", None).unwrap();
        let game = queries::game_detail(&conn, id).unwrap();
        assert_eq!(game.title, "Example Game");
        assert!(crate::direct_launch::get(&conn, id)
            .unwrap()
            .unwrap()
            .validate()
            .is_err());
        conn.execute("UPDATE games SET missing=1 WHERE id=?1", [id])
            .unwrap();
        queries::prune_orphans(&conn).unwrap();
        assert!(
            contains(&conn, id).unwrap(),
            "manual entry survives cleanup even with no history or file"
        );
        conn.execute("UPDATE games SET missing=0 WHERE id=?1", [id])
            .unwrap();
        crate::direct_launch::set(
            &conn,
            id,
            Some(crate::direct_launch::DirectLaunch {
                kind: "decomp".into(),
                executable: "".into(),
            }),
        )
        .unwrap();
        assert_eq!(
            crate::direct_launch::get(&conn, id).unwrap().unwrap().kind,
            "decomp"
        );
        crate::direct_launch::set(&conn, id, None).unwrap();
        assert!(require_file(&conn, id, &game.launch_path).is_err());
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Example.z64");
        std::fs::write(&rom, b"fixture").unwrap();
        set_file(&conn, id, rom.to_str().unwrap()).unwrap();
        let game = queries::game_detail(&conn, id).unwrap();
        require_file(&conn, id, &game.launch_path).unwrap();
        assert!(add(&mut conn, "steam", "Bad", None, "port", None).is_err());
        assert!(add(&mut conn, "n64", "Bad", Some(999), "port", None).is_err());
        let count = queries::count_games(&conn).unwrap();
        assert!(add(
            &mut conn,
            "n64",
            "Bad",
            None,
            "port",
            Some("C:/missing.exe")
        )
        .is_err());
        assert_eq!(queries::count_games(&conn).unwrap(), count);
        let custom = add(&mut conn, "n64", "Custom game", None, "emulator", None).unwrap();
        assert_ne!(custom, id);
        remove(&conn, id).unwrap();
        assert!(!contains(&conn, id).unwrap());
        assert!(rom.is_file(), "removing an entry never deletes its files");
    }
}
