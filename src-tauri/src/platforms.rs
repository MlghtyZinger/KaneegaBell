//! Built-in emulator and platform definitions.
//!
//! Launch flags and extensions were checked against each emulator's own source
//! (argument parser / loadable-extension lists) and RetroArch core .info files:
//! - RetroArch: `-L <core> -f <content>`
//! - Dolphin:   `-b` (batch, requires `-e`), `-e <file>`, `-C System.Section.Key=Value`;
//!              there is no fullscreen flag, so it's set via `-C Dolphin.Display.Fullscreen=True`
//! - DuckStation: `-batch -fullscreen -- <file>`
//! - PCSX2:     `-batch -nogui -fullscreen -- <file>`
//! - RPCS3:     `--no-gui --fullscreen <EBOOT.BIN>` (`--fullscreen` only valid with `--no-gui`)
//! - Azahar:    `-f <file>` (game path must be last)
//!
//! Defaults are re-applied on every start; user changes live in the `*_override`
//! columns and `exe_path`, which seeding never touches.

use rusqlite::{params, Connection};

use crate::error::Result;

pub struct EmulatorDef {
    pub id: &'static str,
    pub name: &'static str,
    pub template: &'static str,
    pub supports_m3u: bool,
}

pub struct PlatformDef {
    pub id: &'static str,
    pub name: &'static str,
    pub short_name: &'static str,
    pub emulator: &'static str,
    pub core: Option<&'static str>,
    pub extensions: &'static [&'static str],
    /// Folder on thumbnails.libretro.com with this platform's box art.
    pub libretro: Option<&'static str>,
    /// IGDB platform id (the `platforms` endpoint), for metadata searches.
    pub igdb: Option<u32>,
    /// RetroAchievements console id. Whether RA currently runs achievements
    /// for it is checked at runtime (its active-console list).
    pub ra: Option<u32>,
}

pub const EMULATORS: &[EmulatorDef] = &[
    EmulatorDef {
        id: "retroarch",
        name: "RetroArch",
        template: r#""{emulator}" -L "{core}" -f "{rom}""#,
        supports_m3u: true,
    },
    EmulatorDef {
        id: "dolphin",
        name: "Dolphin",
        template: r#""{emulator}" -b -C Dolphin.Interface.ConfirmStop=False -C Dolphin.Display.Fullscreen=True -e "{rom}""#,
        supports_m3u: true,
    },
    EmulatorDef {
        id: "duckstation",
        name: "DuckStation",
        template: r#""{emulator}" -batch -fullscreen -- "{rom}""#,
        supports_m3u: true,
    },
    EmulatorDef {
        id: "pcsx2",
        name: "PCSX2",
        template: r#""{emulator}" -batch -nogui -fullscreen -- "{rom}""#,
        supports_m3u: false,
    },
    EmulatorDef {
        id: "rpcs3",
        name: "RPCS3",
        template: r#""{emulator}" --no-gui --fullscreen "{rom}""#,
        supports_m3u: false,
    },
    EmulatorDef {
        id: "azahar",
        name: "Azahar",
        template: r#""{emulator}" -f "{rom}""#,
        supports_m3u: false,
    },
];

/// Listed in display order.
pub const PLATFORMS: &[PlatformDef] = &[
    PlatformDef {
        id: "snes",
        name: "Super Nintendo",
        short_name: "SNES",
        emulator: "retroarch",
        core: Some("snes9x"),
        extensions: &["sfc", "smc", "swc", "fig", "bs"],
        libretro: Some("Nintendo - Super Nintendo Entertainment System"),
        igdb: Some(19),
        ra: Some(3),
    },
    PlatformDef {
        id: "n64",
        name: "Nintendo 64",
        short_name: "N64",
        emulator: "retroarch",
        core: Some("mupen64plus_next"),
        extensions: &["z64", "n64", "v64", "ndd"],
        libretro: Some("Nintendo - Nintendo 64"),
        igdb: Some(4),
        ra: Some(2),
    },
    PlatformDef {
        id: "gba",
        name: "Game Boy Advance",
        short_name: "GBA",
        emulator: "retroarch",
        core: Some("mgba"),
        extensions: &["gba"],
        libretro: Some("Nintendo - Game Boy Advance"),
        igdb: Some(24),
        ra: Some(5),
    },
    PlatformDef {
        id: "gc",
        name: "GameCube",
        short_name: "GC",
        emulator: "dolphin",
        core: None,
        extensions: &["iso", "gcm", "tgc", "ciso", "gcz", "rvz", "wia", "m3u"],
        libretro: Some("Nintendo - GameCube"),
        igdb: Some(21),
        ra: Some(16),
    },
    PlatformDef {
        id: "nds",
        name: "Nintendo DS",
        short_name: "NDS",
        emulator: "retroarch",
        core: Some("melondsds"),
        extensions: &["nds", "dsi", "ids"],
        libretro: Some("Nintendo - Nintendo DS"),
        igdb: Some(20),
        ra: Some(18),
    },
    PlatformDef {
        id: "wii",
        name: "Wii",
        short_name: "Wii",
        emulator: "dolphin",
        core: None,
        extensions: &["iso", "wbfs", "ciso", "gcz", "rvz", "wia", "wad", "m3u"],
        libretro: Some("Nintendo - Wii"),
        igdb: Some(5),
        ra: Some(19),
    },
    PlatformDef {
        id: "3ds",
        name: "Nintendo 3DS",
        short_name: "3DS",
        emulator: "azahar",
        core: None,
        extensions: &["3ds", "cci", "cxi", "3dsx", "zcci", "zcxi", "z3dsx"],
        libretro: Some("Nintendo - Nintendo 3DS"),
        igdb: Some(37),
        ra: None,
    },
    PlatformDef {
        id: "segacd",
        name: "Sega CD",
        short_name: "Sega CD",
        emulator: "retroarch",
        core: Some("genesis_plus_gx"),
        extensions: &["cue", "chd", "iso", "m3u"],
        libretro: Some("Sega - Mega-CD - Sega CD"),
        igdb: Some(78),
        ra: Some(9),
    },
    PlatformDef {
        id: "ps1",
        name: "PlayStation",
        short_name: "PS1",
        emulator: "duckstation",
        core: None,
        extensions: &["cue", "chd", "pbp", "m3u", "ccd", "mds", "ecm", "img", "iso", "bin"],
        libretro: Some("Sony - PlayStation"),
        igdb: Some(7),
        ra: Some(12),
    },
    PlatformDef {
        id: "ps2",
        name: "PlayStation 2",
        short_name: "PS2",
        emulator: "pcsx2",
        core: None,
        extensions: &["iso", "chd", "cso", "zso", "gz", "bin", "img", "mdf"],
        libretro: Some("Sony - PlayStation 2"),
        igdb: Some(8),
        ra: Some(21),
    },
    PlatformDef {
        id: "psp",
        name: "PlayStation Portable",
        short_name: "PSP",
        emulator: "retroarch",
        core: Some("ppsspp"),
        extensions: &["iso", "cso", "pbp", "chd"],
        libretro: Some("Sony - PlayStation Portable"),
        igdb: Some(38),
        ra: Some(41),
    },
    PlatformDef {
        id: "ps3",
        name: "PlayStation 3",
        short_name: "PS3",
        emulator: "rpcs3",
        core: None,
        // PS3 games are folders (PS3_GAME/PARAM.SFO), not files; see scanner::ps3.
        extensions: &[],
        libretro: Some("Sony - PlayStation 3"),
        igdb: Some(9),
        ra: Some(82),
    },
];

/// Executable file names each emulator ships with (lowercase; `*` = any run of chars).
pub fn exe_patterns(emulator_id: &str) -> &'static [&'static str] {
    match emulator_id {
        "retroarch" => &["retroarch.exe"],
        "dolphin" => &["dolphin.exe"],
        "duckstation" => &["duckstation-qt*.exe", "duckstation*.exe"],
        "pcsx2" => &["pcsx2-qt.exe", "pcsx2*.exe"],
        "rpcs3" => &["rpcs3.exe"],
        "azahar" => &["azahar.exe"],
        _ => &[],
    }
}

fn glob_match(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((pre, post)) => name.len() >= pre.len() + post.len() && name.starts_with(pre) && name.ends_with(post),
    }
}

/// Finds emulator executables up to two folders below `root`, preferring the
/// shallowest match (so `RPCS3\rpcs3.exe` beats `RPCS3\rpcs3_old\rpcs3.exe`).
pub fn detect_emulators(root: &std::path::Path) -> Vec<(&'static str, std::path::PathBuf)> {
    let mut files: Vec<(usize, std::path::PathBuf)> = Vec::new();
    let mut dirs = vec![(0usize, root.to_path_buf())];
    while let Some((depth, dir)) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() && depth < 2 {
                dirs.push((depth + 1, e.path()));
            } else if ft.is_file() {
                files.push((depth, e.path()));
            }
        }
    }
    files.sort();
    EMULATORS
        .iter()
        .filter_map(|emu| {
            exe_patterns(emu.id).iter().find_map(|pat| {
                files
                    .iter()
                    .find(|(_, p)| {
                        p.file_name()
                            .map(|n| n.to_string_lossy().to_lowercase())
                            .is_some_and(|n| glob_match(pat, &n))
                    })
                    .map(|(_, p)| (emu.id, p.clone()))
            })
        })
        .collect()
}

pub fn seed(conn: &Connection) -> Result<()> {
    let mut emu = conn.prepare_cached(
        "INSERT INTO emulators (id, name, default_template, supports_m3u) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name,
             default_template = excluded.default_template, supports_m3u = excluded.supports_m3u",
    )?;
    for e in EMULATORS {
        emu.execute(params![e.id, e.name, e.template, e.supports_m3u])?;
    }

    let mut plat = conn.prepare_cached(
        "INSERT INTO platforms (id, name, short_name, sort_order, emulator_id, default_core, extensions, libretro_system,
                                igdb_platform, ra_console_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, short_name = excluded.short_name,
             sort_order = excluded.sort_order, emulator_id = excluded.emulator_id,
             default_core = excluded.default_core, extensions = excluded.extensions,
             libretro_system = excluded.libretro_system, igdb_platform = excluded.igdb_platform,
             ra_console_id = excluded.ra_console_id",
    )?;
    for (i, p) in PLATFORMS.iter().enumerate() {
        let exts = serde_json::to_string(p.extensions)?;
        plat.execute(params![p.id, p.name, p.short_name, i as i64, p.emulator, p.core, exts, p.libretro, p.igdb, p.ra])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_platform_references_a_known_emulator() {
        for p in PLATFORMS {
            assert!(EMULATORS.iter().any(|e| e.id == p.emulator), "{}", p.id);
        }
    }

    #[test]
    fn retroarch_platforms_have_cores() {
        for p in PLATFORMS.iter().filter(|p| p.emulator == "retroarch") {
            assert!(p.core.is_some(), "{} needs a default core", p.id);
        }
    }

    #[test]
    fn default_templates_parse_and_end_with_rom() {
        for e in EMULATORS {
            let toks = crate::launcher::template::tokenize(e.template).unwrap();
            assert_eq!(toks.first().map(String::as_str), Some("{emulator}"), "{}", e.id);
            assert_eq!(toks.last().map(String::as_str), Some("{rom}"), "{}", e.id);
        }
    }

    #[test]
    fn detects_emulators_like_the_real_install() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        for f in [
            r"Azahar\azahar-room.exe",
            r"Azahar\azahar.exe",
            r"Dolphin-x64\Dolphin.exe",
            r"Duckstation\duckstation-qt-x64-ReleaseLTCG.exe",
            r"PCSX2\pcsx2-qt.exe",
            r"RetroArch\retroarch.exe",
            r"RPCS3\rpcs3_old\rpcs3.exe",
            r"RPCS3\rpcs3.exe",
        ] {
            let p = r.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"MZ").unwrap();
        }
        let found: std::collections::HashMap<_, _> = detect_emulators(r).into_iter().collect();
        assert_eq!(found.len(), 6);
        assert_eq!(found["azahar"], r.join(r"Azahar\azahar.exe"));
        assert_eq!(found["rpcs3"], r.join(r"RPCS3\rpcs3.exe"));
        assert_eq!(found["duckstation"], r.join(r"Duckstation\duckstation-qt-x64-ReleaseLTCG.exe"));
    }

    #[test]
    fn seeding_is_idempotent_and_keeps_user_settings() {
        let conn = crate::db::test_db();
        conn.execute("UPDATE emulators SET exe_path = 'X:/ra.exe' WHERE id = 'retroarch'", []).unwrap();
        conn.execute("UPDATE platforms SET core_override = 'bsnes' WHERE id = 'snes'", []).unwrap();
        seed(&conn).unwrap();
        let exe: String = conn
            .query_row("SELECT exe_path FROM emulators WHERE id = 'retroarch'", [], |r| r.get(0))
            .unwrap();
        let core: String = conn
            .query_row("SELECT core_override FROM platforms WHERE id = 'snes'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(exe, "X:/ra.exe");
        assert_eq!(core, "bsnes");
    }
}
