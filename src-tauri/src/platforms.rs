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
//! - Eden, Citron, Sudachi, Yuzu: `-f -g <file>` (Yuzu's Qt front end; no `--help`
//!   output, so checked in the owner's builds' argument parsers)
//! - Ryujinx, Ryubing: `--fullscreen <file>` (1.1.1403 and the owner's Ryubing; a bare path launches the game)
//! - xemu:      `-full-screen -dvd_path <file>` (checked in the owner's build); BIOS, MCPX and HDD
//!              come from xemu's own settings
//!
//! Defaults are re-applied on every start; user changes live in the `*_override`
//! columns and `exe_path`, which seeding never touches.

use std::path::{Path, PathBuf};

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
    /// Platform name in the LaunchBox Games Database (keyless covers, logos,
    /// backgrounds and info).
    pub launchbox: Option<&'static str>,
    /// Wikidata item of the platform (games list it as "platform", P400), for
    /// series and franchises.
    pub wikidata: Option<&'static str>,
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
    // Switch. Eden, Citron, Sudachi and Yuzu share Yuzu's Qt front end: `-f` fullscreen, `-g <file>`.
    EmulatorDef {
        id: "eden",
        name: "Eden",
        template: r#""{emulator}" -f -g "{rom}""#,
        supports_m3u: false,
    },
    // Ryubing is the maintained Ryujinx fork; its exe is also Ryujinx.exe (told apart by folder).
    EmulatorDef {
        id: "ryubing",
        name: "Ryubing",
        template: r#""{emulator}" --fullscreen "{rom}""#,
        supports_m3u: false,
    },
    EmulatorDef {
        id: "ryujinx",
        name: "Ryujinx",
        template: r#""{emulator}" --fullscreen "{rom}""#,
        supports_m3u: false,
    },
    EmulatorDef {
        id: "citron",
        name: "Citron",
        template: r#""{emulator}" -f -g "{rom}""#,
        supports_m3u: false,
    },
    EmulatorDef {
        id: "sudachi",
        name: "Sudachi",
        template: r#""{emulator}" -f -g "{rom}""#,
        supports_m3u: false,
    },
    EmulatorDef {
        id: "yuzu",
        name: "Yuzu (discontinued)",
        template: r#""{emulator}" -f -g "{rom}""#,
        supports_m3u: false,
    },
    EmulatorDef {
        id: "xemu",
        name: "xemu",
        template: r#""{emulator}" -full-screen -dvd_path "{rom}""#,
        supports_m3u: false,
    },
    // shortcut: flags from Xenia Canary's documented `--fullscreen` option, not checked in a local
    // build (the owner hasn't installed it yet); verify once it's there.
    EmulatorDef {
        id: "xenia",
        name: "Xenia Canary",
        template: r#""{emulator}" --fullscreen "{rom}""#,
        supports_m3u: false,
    },
];

/// Switch emulators besides the built-in Eden, addable to the Switch platform.
pub const SWITCH_ALTERNATIVES: &[&str] = &["ryubing", "ryujinx", "citron", "sudachi", "yuzu"];

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
        launchbox: Some("Super Nintendo Entertainment System"),
        wikidata: Some("Q183259"),
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
        launchbox: Some("Nintendo 64"),
        wikidata: Some("Q184839"),
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
        launchbox: Some("Nintendo Game Boy Advance"),
        wikidata: Some("Q188642"),
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
        launchbox: Some("Nintendo GameCube"),
        wikidata: Some("Q182172"),
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
        launchbox: Some("Nintendo DS"),
        wikidata: Some("Q170323"),
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
        launchbox: Some("Nintendo Wii"),
        wikidata: Some("Q8079"),
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
        launchbox: Some("Nintendo 3DS"),
        wikidata: Some("Q203597"),
        igdb: Some(37),
        ra: None,
    },
    PlatformDef {
        id: "switch",
        name: "Nintendo Switch",
        short_name: "Switch",
        emulator: "eden",
        core: None,
        // Update and DLC files are hidden by the scanner (is_switch_add_on).
        extensions: &["nsp", "xci"],
        libretro: None,
        launchbox: Some("Nintendo Switch"),
        wikidata: Some("Q19610114"),
        igdb: Some(130),
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
        launchbox: Some("Sega CD"),
        wikidata: Some("Q1047516"),
        igdb: Some(78),
        ra: Some(9),
    },
    PlatformDef {
        id: "saturn",
        name: "Sega Saturn",
        short_name: "Saturn",
        emulator: "retroarch",
        // Beetle Saturn: the most accurate core (docs.libretro.com/library/beetle_saturn).
        core: Some("mednafen_saturn"),
        extensions: &["cue", "chd", "ccd", "m3u"],
        libretro: Some("Sega - Saturn"),
        launchbox: Some("Sega Saturn"),
        wikidata: Some("Q200912"),
        igdb: Some(32),
        ra: Some(39),
    },
    PlatformDef {
        id: "ps1",
        name: "PlayStation",
        short_name: "PS1",
        emulator: "duckstation",
        core: None,
        extensions: &["cue", "chd", "pbp", "m3u", "ccd", "mds", "ecm", "img", "iso", "bin"],
        libretro: Some("Sony - PlayStation"),
        launchbox: Some("Sony Playstation"),
        wikidata: Some("Q10677"),
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
        launchbox: Some("Sony Playstation 2"),
        wikidata: Some("Q10680"),
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
        launchbox: Some("Sony PSP"),
        wikidata: Some("Q170325"),
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
        launchbox: Some("Sony Playstation 3"),
        wikidata: Some("Q10683"),
        igdb: Some(9),
        ra: Some(82),
    },
    PlatformDef {
        id: "xbox",
        name: "Xbox",
        short_name: "Xbox",
        emulator: "xemu",
        core: None,
        // XISO images (xemu's format); in mixed folders an .iso is Xbox by its
        // XDVDFS header (scanner::detect::xbox).
        extensions: &["iso", "xiso"],
        libretro: Some("Microsoft - Xbox"),
        launchbox: Some("Microsoft Xbox"),
        wikidata: Some("Q132020"),
        igdb: Some(11),
        ra: None,
    },
    PlatformDef {
        id: "xbox360",
        name: "Xbox 360",
        short_name: "360",
        emulator: "xenia",
        core: None,
        // Disc images; .xex folders, GOD and XBLA content aren't scanned yet.
        extensions: &["iso"],
        libretro: Some("Microsoft - Xbox 360"),
        launchbox: Some("Microsoft Xbox 360"),
        wikidata: Some("Q48263"),
        igdb: Some(12),
        ra: None,
    },
];

/// Whether an emulator is on RetroAchievements' list of supported emulators
/// (docs.retroachievements.org, "Emulator Support and Issues", checked
/// 2026-10): `Some(true)` listed, `Some(false)` not listed, `None` unknown
/// (an emulator the user added). Dolphin needs 2407-68+ for GameCube and
/// 2603+ for Wii there.
pub fn achievements(emulator_id: &str) -> Option<bool> {
    match emulator_id {
        "retroarch" | "duckstation" | "pcsx2" | "dolphin" => Some(true),
        "rpcs3" | "azahar" | "eden" | "ryubing" | "ryujinx" | "citron" | "sudachi" | "yuzu" | "xemu" | "xenia" => Some(false),
        _ => None,
    }
}

/// The libretro core suggested when RetroArch is added to a platform whose
/// built-in emulator is a standalone one (the user can change it).
pub fn retroarch_core_for(platform_id: &str) -> Option<&'static str> {
    match platform_id {
        "ps1" => Some("swanstation"),
        "ps2" => Some("pcsx2"),
        "gc" | "wii" => Some("dolphin"),
        "3ds" => Some("citra"),
        _ => None,
    }
}

/// RetroArch's system (BIOS) folder: `system_directory` in the retroarch.cfg
/// next to its exe (`:\…` is relative to RetroArch's folder), else its
/// `system` folder.
pub fn retroarch_system_dir(retroarch_exe: &Path) -> PathBuf {
    let dir = retroarch_exe.parent().unwrap_or(Path::new(".")).to_path_buf();
    let configured = std::fs::read_to_string(dir.join("retroarch.cfg")).ok().and_then(|cfg| {
        cfg.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "system_directory").then(|| v.trim().trim_matches('"').to_string())
        })
    });
    match configured.as_deref() {
        None | Some("") | Some("default") => dir.join("system"),
        Some(v) => match v.strip_prefix(":\\").or_else(|| v.strip_prefix(":/")) {
            Some(rel) => dir.join(rel),
            None => PathBuf::from(v),
        },
    }
}

/// Libretro cores that won't start a game without the console's BIOS: what's
/// missing from RetroArch's system folder, if it looks missing. Only a
/// warning (BIOS file names vary), never a reason to refuse a launch.
pub fn retroarch_bios_problem(core: &str, platform_id: &str, system_dir: &Path) -> Option<String> {
    let name = Path::new(core).file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let name = name.trim_end_matches("_libretro");
    let files = |dir: &Path| -> Vec<String> {
        std::fs::read_dir(dir)
            .map(|rd| rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().to_ascii_lowercase()).collect())
            .unwrap_or_default()
    };
    match name {
        "pcsx2" => {
            let bios = system_dir.join("pcsx2").join("bios");
            files(&bios).is_empty().then(|| format!("RetroArch's pcsx2 core needs a PS2 BIOS file in {}, which is empty.", bios.display()))
        }
        "swanstation" | "mednafen_psx" | "mednafen_psx_hw" => {
            let found = files(system_dir).iter().any(|f| (f.starts_with("scph") || f.starts_with("psxonpsp")) && f.ends_with(".bin"));
            (!found).then(|| format!("RetroArch's {name} core needs a PS1 BIOS (e.g. scph5501.bin) in {}.", system_dir.display()))
        }
        "mednafen_saturn" => {
            // sega_101.bin for Japanese games, mpr-17933.bin for US/European ones (names case-sensitive).
            let have = files(system_dir);
            let found = ["sega_101.bin", "mpr-17933.bin"].iter().any(|b| have.iter().any(|f| f == b));
            (!found).then(|| {
                format!("RetroArch's mednafen_saturn core needs a Saturn BIOS (mpr-17933.bin, or sega_101.bin for Japanese games) in {}.", system_dir.display())
            })
        }
        "genesis_plus_gx" if platform_id == "segacd" => {
            let found = files(system_dir).iter().any(|f| f.starts_with("bios_cd_") && f.ends_with(".bin"));
            (!found).then(|| format!("RetroArch's genesis_plus_gx core needs a Sega CD BIOS (bios_CD_U.bin) in {}.", system_dir.display()))
        }
        _ => None,
    }
}

/// Built-in emulators that can be added to a platform besides its own:
/// RetroArch (through a core; not for PS3, whose games are folders, or Switch,
/// which has no usable core), and the other Switch emulators for Switch.
/// The user's own emulators can be added to any platform.
pub fn can_add_builtin(emulator_id: &str, platform_id: &str, platform_builtin: &str) -> bool {
    match platform_id {
        "switch" => SWITCH_ALTERNATIVES.contains(&emulator_id),
        // PS3 games are folders; Xbox 360 has no RetroArch core.
        "ps3" | "xbox360" => false,
        _ => emulator_id == "retroarch" && platform_builtin != "retroarch",
    }
}

/// Executable file names each emulator ships with (lowercase; `*` = any run of
/// chars; `folder\file` also requires the file's folder name).
pub fn exe_patterns(emulator_id: &str) -> &'static [&'static str] {
    match emulator_id {
        "retroarch" => &["retroarch.exe"],
        "dolphin" => &["dolphin.exe"],
        "duckstation" => &["duckstation-qt*.exe", "duckstation*.exe"],
        "pcsx2" => &["pcsx2-qt.exe", "pcsx2*.exe"],
        "rpcs3" => &["rpcs3.exe"],
        "azahar" => &["azahar.exe"],
        "eden" => &["eden.exe"],
        // Both ship Ryujinx.exe: Ryubing's folder decides, and a file one
        // emulator claimed isn't given to the next (see detect_emulators).
        "ryubing" => &[r"ryubing*\ryujinx.exe", "ryubing*.exe"],
        "ryujinx" => &[r"ryujinx*\ryujinx.exe", "ryujinx.exe"],
        "citron" => &["citron.exe"],
        "sudachi" => &["sudachi.exe"],
        "yuzu" => &["yuzu.exe"],
        "xemu" => &["xemu.exe"],
        "xenia" => &["xenia_canary.exe", "xenia*.exe"],
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
    let name_of = |p: &std::path::Path| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut found: Vec<(&'static str, std::path::PathBuf)> = Vec::new();
    for emu in EMULATORS {
        let hit = exe_patterns(emu.id).iter().find_map(|pat| {
            files
                .iter()
                .filter(|(_, p)| !found.iter().any(|(_, f)| f == p))
                .find(|(_, p)| match pat.split_once('\\') {
                    Some((folder, file)) => glob_match(folder, &name_of(p.parent().unwrap_or(p))) && glob_match(file, &name_of(p)),
                    None => glob_match(pat, &name_of(p)),
                })
                .map(|(_, p)| p.clone())
        });
        if let Some(p) = hit {
            found.push((emu.id, p));
        }
    }
    found
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
                                igdb_platform, ra_console_id, launchbox_platform)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, short_name = excluded.short_name,
             sort_order = excluded.sort_order, emulator_id = excluded.emulator_id,
             default_core = excluded.default_core, extensions = excluded.extensions,
             libretro_system = excluded.libretro_system, igdb_platform = excluded.igdb_platform,
             ra_console_id = excluded.ra_console_id, launchbox_platform = excluded.launchbox_platform",
    )?;
    for (i, p) in PLATFORMS.iter().enumerate() {
        let exts = serde_json::to_string(p.extensions)?;
        plat.execute(params![p.id, p.name, p.short_name, i as i64, p.emulator, p.core, exts, p.libretro, p.igdb, p.ra, p.launchbox])?;
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
            r"Eden\eden.exe",
            r"Ryubing\Ryujinx.exe",
            r"Ryujinx\Ryujinx.exe",
            r"Citron\citron.exe",
            r"Citron\citron-cmd.exe",
        ] {
            let p = r.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"MZ").unwrap();
        }
        let found: std::collections::HashMap<_, _> = detect_emulators(r).into_iter().collect();
        assert_eq!(found.len(), 10);
        assert_eq!(found["azahar"], r.join(r"Azahar\azahar.exe"));
        assert_eq!(found["citron"], r.join(r"Citron\citron.exe"), "not the -cmd variant");
        // Both are Ryujinx.exe: each goes by its folder.
        assert_eq!(found["ryujinx"], r.join(r"Ryujinx\Ryujinx.exe"));
        assert_eq!(found["ryubing"], r.join(r"Ryubing\Ryujinx.exe"));

        // Only Ryubing installed: it isn't also taken for the original Ryujinx.
        let t2 = tempfile::tempdir().unwrap();
        let p = t2.path().join(r"Ryubing\Ryujinx.exe");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"MZ").unwrap();
        assert_eq!(detect_emulators(t2.path()), [("ryubing", p)]);
        assert_eq!(found["rpcs3"], r.join(r"RPCS3\rpcs3.exe"));
        assert_eq!(found["duckstation"], r.join(r"Duckstation\duckstation-qt-x64-ReleaseLTCG.exe"));
    }

    #[test]
    fn switch_emulators_are_addable_only_to_switch() {
        assert!(can_add_builtin("ryujinx", "switch", "eden"));
        assert!(can_add_builtin("yuzu", "switch", "eden"));
        assert!(!can_add_builtin("retroarch", "switch", "eden"), "no usable Switch core");
        assert!(!can_add_builtin("ryujinx", "ps1", "duckstation"));
        assert!(can_add_builtin("retroarch", "ps1", "duckstation"));
        assert!(!can_add_builtin("retroarch", "ps3", "rpcs3"));
        assert!(!can_add_builtin("retroarch", "snes", "retroarch"));
    }

    #[test]
    fn retroarch_system_folder_and_missing_bios() {
        let t = tempfile::tempdir().unwrap();
        let exe = t.path().join("retroarch.exe");
        // Default: RetroArch's own system folder.
        assert_eq!(retroarch_system_dir(&exe), t.path().join("system"));
        std::fs::write(t.path().join("retroarch.cfg"), "video_driver = \"vulkan\"\nsystem_directory = \":\\bios\"\n").unwrap();
        assert_eq!(retroarch_system_dir(&exe), t.path().join("bios"));
        std::fs::write(t.path().join("retroarch.cfg"), "system_directory = \"D:\\RA System\"\n").unwrap();
        assert_eq!(retroarch_system_dir(&exe), Path::new(r"D:\RA System"));

        let sys = t.path().join("system");
        std::fs::create_dir_all(sys.join("pcsx2").join("bios")).unwrap();
        // The owner's case: the PS2 BIOS folder exists but is empty.
        let ps2 = retroarch_bios_problem("pcsx2", "ps2", &sys).unwrap();
        assert!(ps2.contains("PS2 BIOS") && ps2.contains("empty"), "{ps2}");
        std::fs::write(sys.join("pcsx2").join("bios").join("SCPH-70012.bin"), b"x").unwrap();
        assert_eq!(retroarch_bios_problem(r"E:\RA\cores\pcsx2_libretro.dll", "ps2", &sys), None, "a core given as a path");
        assert!(retroarch_bios_problem("swanstation", "ps1", &sys).is_some());
        std::fs::write(sys.join("SCPH5501.BIN"), b"x").unwrap();
        assert_eq!(retroarch_bios_problem("swanstation", "ps1", &sys), None);
        assert!(retroarch_bios_problem("genesis_plus_gx", "segacd", &sys).is_some());
        assert_eq!(retroarch_bios_problem("genesis_plus_gx", "snes", &sys), None, "Genesis games need no BIOS");
        assert_eq!(retroarch_bios_problem("dolphin", "gc", &sys), None);
        // Saturn: either region's BIOS will do; the names are case-sensitive.
        assert!(retroarch_bios_problem("mednafen_saturn", "saturn", &sys).unwrap().contains("mpr-17933.bin"));
        std::fs::write(sys.join("mpr-17933.bin"), b"x").unwrap();
        assert_eq!(retroarch_bios_problem("mednafen_saturn", "saturn", &sys), None);
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
