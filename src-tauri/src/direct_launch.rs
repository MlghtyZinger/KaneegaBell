//! Direct executable overrides for ports, recompilations and decompilations.
use crate::{
    db::queries,
    error::{Error, Result},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DirectLaunch {
    pub kind: String,
    pub executable: PathBuf,
}

impl DirectLaunch {
    pub fn label(&self) -> &'static str {
        match self.kind.as_str() {
            "port" => "Port",
            "recomp" => "Recomp",
            _ => "Decomp",
        }
    }
    /// The SM64DS launcher can immediately hand off when its skip option is on.
    /// Observe the known game executable, never every executable in the folder.
    pub fn tracking_path(&self) -> Result<PathBuf> {
        let exe = self.validate()?;
        if exe.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("SM64DSLauncher.exe")) {
            let game = exe.with_file_name("walk_window.exe");
            if !game.is_file() {
                return Err(Error::msg("SM64DS game executable walk_window.exe was not found beside its launcher."));
            }
            return Ok(std::fs::canonicalize(game)?);
        }
        Ok(exe)
    }

    pub fn validate(&self) -> Result<PathBuf> {
        if !matches!(self.kind.as_str(), "port" | "recomp" | "decomp") {
            return Err(Error::msg("Choose Port, Recomp or Decomp."));
        }
        if !self.executable.is_absolute()
            || !self.executable.is_file()
            || !self
                .executable
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            return Err(Error::msg(
                "Choose an existing game .exe file. No emulator is used.",
            ));
        }
        Ok(std::fs::canonicalize(&self.executable)?)
    }
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<DirectLaunch>> {
    Ok(conn
        .query_row(
            "SELECT kind, executable FROM direct_launch WHERE game_id=?1",
            [id],
            |r| {
                Ok(DirectLaunch {
                    kind: r.get(0)?,
                    executable: PathBuf::from(r.get::<_, String>(1)?),
                })
            },
        )
        .optional()?)
}

pub fn set(conn: &Connection, id: i64, config: Option<DirectLaunch>) -> Result<()> {
    let game = queries::game_detail(conn, id)?;
    if crate::launchers::is_launcher(&game.platform_id) {
        return Err(Error::msg(
            "Launcher games must launch through their game client.",
        ));
    }
    if let Some(config) = config {
        let exe = if config.executable.as_os_str().is_empty()
            && matches!(config.kind.as_str(), "port" | "recomp" | "decomp")
            && crate::manual_games::contains(conn, id)?
        {
            config.executable.clone()
        } else {
            config.validate()?
        };
        conn.execute(
            "INSERT INTO direct_launch(game_id, kind, executable) VALUES (?1, ?2, ?3)
            ON CONFLICT(game_id) DO UPDATE SET kind=excluded.kind, executable=excluded.executable",
            params![id, config.kind, exe.to_string_lossy()],
        )?;
    } else {
        conn.execute("DELETE FROM direct_launch WHERE game_id=?1", [id])?;
    }
    Ok(())
}

/// Suppress only scanned copies of the same game bundled below its executable.
/// Rows and their history remain intact, and reappear when the override is removed.
pub fn bundled_roms(conn: &Connection) -> Result<std::collections::HashSet<i64>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.launch_path, d.executable FROM direct_launch d
         JOIN games owner ON owner.id=d.game_id
         JOIN games g ON g.platform_id=owner.platform_id AND g.id<>owner.id
         WHERE owner.missing=0 AND d.executable<>'' AND g.missing=0
           AND (g.lb_id=owner.lb_id OR (g.sort_title<>'' AND g.sort_title=owner.sort_title))
           AND g.id NOT IN (SELECT game_id FROM direct_launch)
           AND g.id NOT IN (SELECT game_id FROM manual_games)")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
    fn key(path: &str) -> String {
        let p = crate::scanner::path_key(std::path::Path::new(path)).replace('\\', "/");
        if let Some(rest) = p.strip_prefix("//?/unc/") { format!("//{rest}") }
        else { p.strip_prefix("//?/").unwrap_or(&p).to_owned() }
    }
    let mut hidden = std::collections::HashSet::new();
    for row in rows {
        let (id, rom, exe) = row?;
        let exe = key(&exe);
        if let Some((folder, _)) = exe.rsplit_once('/') {
            if !folder.is_empty() && key(&rom).starts_with(&format!("{folder}/")) { hidden.insert(id); }
        }
    }
    Ok(hidden)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sm64ds_skip_launcher_tracks_the_game_and_keeps_launch_target() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("SM64DSLauncher.exe");
        let game = dir.path().join("walk_window.exe");
        std::fs::write(&exe, b"fixture; never executed").unwrap();
        let config = DirectLaunch { kind: "port".into(), executable: exe.clone() };
        assert!(config.tracking_path().is_err());
        std::fs::write(&game, b"fixture; never executed").unwrap();
        assert_eq!(config.validate().unwrap(), std::fs::canonicalize(exe).unwrap());
        assert_eq!(config.tracking_path().unwrap(), std::fs::canonicalize(game).unwrap());
        let ordinary = dir.path().join("Other.exe");
        std::fs::write(&ordinary, b"fixture; never executed").unwrap();
        let other = DirectLaunch { kind: "port".into(), executable: ordinary.clone() };
        assert_eq!(other.tracking_path().unwrap(), std::fs::canonicalize(ordinary).unwrap());
    }

    #[test]
    fn bundled_rom_is_hidden_without_losing_history_and_returns_when_disabled() {
        let conn = crate::db::test_db();
        for (id, key, path) in [
            (1, "zelda", "kb-manual:1"),
            (2, "zelda", r"C:\Ports\Zelda\assets\zelda.z64"),
            (3, "zelda", r"C:\Ports\Zelda-other\zelda.z64"),
            (4, "mario", r"C:\Ports\Zelda\mario.z64"),
            (5, "zelda", r"C:\ROMs\zelda.z64"),
            (6, "zelda", "kb-manual:6"),
        ] {
            conn.execute("INSERT INTO games(id,platform_id,title,sort_title,search_key,launch_path,added_at,play_seconds,favorite)
                VALUES(?1,'n64',?2,?2,?2,?3,0,42,1)", params![id,key,path]).unwrap();
        }
        conn.execute("INSERT INTO manual_games(game_id) VALUES(1),(6)", []).unwrap();
        conn.execute("INSERT INTO direct_launch(game_id,kind,executable) VALUES(1,'port',?1)", [r"\\?\C:\Ports\Zelda\game.exe"]).unwrap();
        assert_eq!(bundled_roms(&conn).unwrap(), std::collections::HashSet::from([2]));
        assert!(!queries::library(&conn).unwrap().iter().any(|g| g.id==2));
        // A subsequent scan keeps the row present, without showing it twice.
        conn.execute("UPDATE games SET missing=0 WHERE id=2", []).unwrap();
        assert!(!queries::library(&conn).unwrap().iter().any(|g| g.id==2));
        set(&conn,1,None).unwrap();
        let restored = queries::library(&conn).unwrap();
        let rom=restored.iter().find(|g| g.id==2).unwrap();
        assert_eq!(rom.play_seconds,42);
        assert!(rom.favorite);
        assert_eq!(restored.len(),6);
    }

    #[test]
    fn direct_override_validates_and_preserves_game_identity() {
        let conn = crate::db::test_db();
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("My game.EXE");
        std::fs::write(&exe, b"fixture; never executed").unwrap();
        conn.execute("INSERT INTO games(id, platform_id, title, sort_title, search_key, launch_path, added_at, favorite, play_seconds)
            VALUES (1, 'n64', 'Original', 'original', 'original', 'original.z64', 0, 1, 42)", []).unwrap();
        for kind in ["port", "recomp", "decomp"] {
            set(
                &conn,
                1,
                Some(DirectLaunch {
                    kind: kind.into(),
                    executable: exe.clone(),
                }),
            )
            .unwrap();
            assert_eq!(get(&conn, 1).unwrap().unwrap().kind, kind);
        }
        assert!(set(
            &conn,
            1,
            Some(DirectLaunch {
                kind: "invalid".into(),
                executable: exe.clone()
            })
        )
        .is_err());
        assert!(set(
            &conn,
            1,
            Some(DirectLaunch {
                kind: "port".into(),
                executable: PathBuf::from("relative.exe")
            })
        )
        .is_err());
        let txt = dir.path().join("game.bat");
        std::fs::write(&txt, "fixture").unwrap();
        assert!(set(
            &conn,
            1,
            Some(DirectLaunch {
                kind: "port".into(),
                executable: txt
            })
        )
        .is_err());
        assert_eq!(
            get(&conn, 1).unwrap().unwrap().kind,
            "decomp",
            "invalid edits keep the saved choice"
        );
        let before = queries::game_detail(&conn, 1).unwrap();
        set(&conn, 1, None).unwrap();
        assert!(get(&conn, 1).unwrap().is_none());
        let after = queries::game_detail(&conn, 1).unwrap();
        assert_eq!(after.launch_path, before.launch_path);
        assert_eq!(after.play_seconds, 42);
        assert!(conn
            .query_row("SELECT favorite FROM games WHERE id=1", [], |r| r
                .get::<_, bool>(0))
            .unwrap());
        conn.execute("UPDATE games SET platform_id='steam' WHERE id=1", [])
            .unwrap();
        assert!(set(
            &conn,
            1,
            Some(DirectLaunch {
                kind: "port".into(),
                executable: exe.clone()
            })
        )
        .is_err());
        conn.execute("UPDATE games SET platform_id='n64' WHERE id=1", [])
            .unwrap();
        set(
            &conn,
            1,
            Some(DirectLaunch {
                kind: "port".into(),
                executable: exe.clone(),
            }),
        )
        .unwrap();
        std::fs::remove_file(exe).unwrap();
        assert!(get(&conn, 1).unwrap().unwrap().validate().is_err());
        conn.execute("DELETE FROM games WHERE id=1", []).unwrap();
        assert!(get(&conn, 1).unwrap().is_none());
    }
}
