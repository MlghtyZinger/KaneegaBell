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

#[cfg(test)]
mod tests {
    use super::*;
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
