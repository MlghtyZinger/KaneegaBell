//! PS3 discovery. Two sources:
//! - Disc-dump folders: `<Game>/PS3_GAME/PARAM.SFO`, also nested one level deeper
//!   (`<Game>/<Game>/PS3_GAME`, as some dump tools produce).
//! - RPCS3's `dev_hdd0/game/<TITLEID>/`: only `CATEGORY == "HG"` entries are
//!   standalone games; `GD` entries are disc-game updates/patch data and are skipped.

use std::fs;
use std::path::{Path, PathBuf};

use super::sfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ps3Game {
    pub title: String,
    pub title_id: Option<String>,
    /// The folder the user sees (used for the region/title fallback and identity).
    pub root: PathBuf,
    /// `.../USRDIR/EBOOT.BIN`, passed to RPCS3.
    pub eboot: PathBuf,
}

/// Disc-format folders up to `max_depth` levels below `root`.
pub fn find_disc_games(root: &Path, max_depth: usize) -> Vec<Ps3Game> {
    let mut out = Vec::new();
    walk(root, root, max_depth, &mut out);
    out.sort_by(|a, b| a.root.cmp(&b.root));
    out
}

fn walk(dir: &Path, top: &Path, depth_left: usize, out: &mut Vec<Ps3Game>) {
    let game_dir = dir.join("PS3_GAME");
    if game_dir.join("PARAM.SFO").is_file() {
        if let Some(g) = disc_game(dir, top, &game_dir) {
            out.push(g);
        }
        return; // never look for games inside a game
    }
    if depth_left == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&e.path(), top, depth_left - 1, out);
        }
    }
}

fn disc_game(dir: &Path, top: &Path, game_dir: &Path) -> Option<Ps3Game> {
    let eboot = game_dir.join("USRDIR").join("EBOOT.BIN");
    if !eboot.is_file() {
        log::warn!("PS3 folder without EBOOT.BIN: {}", dir.display());
        return None;
    }
    let sfo = sfo::read(&game_dir.join("PARAM.SFO"));
    // For the Vimm-style nested layout, report the outer folder as the root.
    let root = match dir.parent() {
        Some(p) if p != top && p.file_name() == dir.file_name() => p.to_path_buf(),
        _ => dir.to_path_buf(),
    };
    let folder_name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Some(Ps3Game {
        title: sfo.as_ref().and_then(|s| s.str("TITLE")).map(clean_title).unwrap_or(folder_name),
        title_id: sfo.as_ref().and_then(|s| s.str("TITLE_ID")).map(str::to_string),
        root,
        eboot,
    })
}

/// Installed (PSN/HDD) games in a `dev_hdd0/game` directory.
pub fn find_hdd0_games(game_root: &Path) -> Vec<Ps3Game> {
    let Ok(entries) = fs::read_dir(game_root) else { return Vec::new() };
    let mut out: Vec<Ps3Game> = entries
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let sfo = sfo::read(&dir.join("PARAM.SFO"))?;
            if sfo.str("CATEGORY") != Some("HG") {
                return None;
            }
            let eboot = dir.join("USRDIR").join("EBOOT.BIN");
            if !eboot.is_file() {
                return None;
            }
            Some(Ps3Game {
                title: clean_title(sfo.str("TITLE")?),
                title_id: sfo.str("TITLE_ID").map(str::to_string),
                root: dir,
                eboot,
            })
        })
        .collect();
    out.sort_by(|a, b| a.root.cmp(&b.root));
    out
}

/// SFO titles can contain line breaks and trademark symbols.
fn clean_title(t: &str) -> String {
    let t: String = t
        .chars()
        .filter(|c| !matches!(c, '®' | '™' | '©'))
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `dev_hdd0/game` for an RPCS3 install, honoring a relocated dev_hdd0 in
/// `config/vfs.yml` (`/dev_hdd0/: $(EmulatorDir)dev_hdd0/` by default).
pub fn hdd0_game_dir(rpcs3_exe: &Path) -> Option<PathBuf> {
    let emu_dir = rpcs3_exe.parent()?;
    let default = emu_dir.join("dev_hdd0");
    let hdd0 = fs::read_to_string(emu_dir.join("config").join("vfs.yml"))
        .ok()
        .and_then(|yml| parse_vfs_hdd0(&yml, emu_dir))
        .unwrap_or(default);
    let game = hdd0.join("game");
    game.is_dir().then_some(game)
}

fn parse_vfs_hdd0(yml: &str, emu_dir: &Path) -> Option<PathBuf> {
    let line = yml.lines().find(|l| l.trim_start().starts_with("/dev_hdd0/:"))?;
    let value = line.split_once(':')?.1.trim().trim_matches('"');
    if value.is_empty() {
        return None;
    }
    let mut emu = emu_dir.to_string_lossy().into_owned();
    if !emu.ends_with(['/', '\\']) {
        emu.push('/');
    }
    Some(PathBuf::from(value.replace("$(EmulatorDir)", &emu)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_game(dir: &Path, sfo: &[(&str, &str)], with_eboot: bool, ps3_game: bool) {
        let base = if ps3_game { dir.join("PS3_GAME") } else { dir.to_path_buf() };
        fs::create_dir_all(base.join("USRDIR")).unwrap();
        fs::write(base.join("PARAM.SFO"), sfo::build(sfo)).unwrap();
        if with_eboot {
            fs::write(base.join("USRDIR").join("EBOOT.BIN"), b"SCE").unwrap();
        }
    }

    #[test]
    fn finds_flat_and_nested_disc_layouts_and_ignores_noise() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_game(
            &root.join("Darkness, The (USA) (En,Fr,De,Es,It)"),
            &[("TITLE", "The Darkness™"), ("TITLE_ID", "BLUS30035"), ("CATEGORY", "DG")],
            true,
            true,
        );
        let nested = root.join("Armored Core V (USA) (En,Fr,Es)");
        make_game(
            &nested.join("Armored Core V (USA) (En,Fr,Es)"),
            &[("TITLE", "ARMORED CORE V"), ("TITLE_ID", "BLUS30516"), ("CATEGORY", "DG")],
            true,
            true,
        );
        fs::create_dir_all(root.join("exdata")).unwrap();
        fs::write(root.join("exdata").join("x.rap"), b"").unwrap();
        fs::create_dir_all(root.join("PS3 DISC DUMPER").join("logs")).unwrap();
        make_game(&root.join("Broken"), &[("TITLE", "No Eboot")], false, true);

        let games = find_disc_games(root, 3);
        assert_eq!(games.len(), 2, "{games:#?}");
        let ac = games.iter().find(|g| g.title == "ARMORED CORE V").unwrap();
        assert_eq!(ac.root, nested, "nested layout reports the outer folder");
        assert!(ac.eboot.ends_with(r"PS3_GAME\USRDIR\EBOOT.BIN") || ac.eboot.ends_with("PS3_GAME/USRDIR/EBOOT.BIN"));
        let dk = games.iter().find(|g| g.title_id.as_deref() == Some("BLUS30035")).unwrap();
        assert_eq!(dk.title, "The Darkness");
    }

    #[test]
    fn hdd0_keeps_hg_and_skips_patches() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path();
        make_game(&game.join("NPUB30611"), &[("CATEGORY", "HG"), ("TITLE", "Tokyo Jungle"), ("TITLE_ID", "NPUB30611")], true, false);
        make_game(&game.join("BLUS30383"), &[("CATEGORY", "GD"), ("TITLE", "Kane & Lynch 2"), ("TITLE_ID", "BLUS30383")], true, false);
        make_game(&game.join("BCUS98107"), &[("CATEGORY", "GD"), ("TITLE", "Resistance")], false, false);
        make_game(&game.join("NPUB90126"), &[("CATEGORY", "HG"), ("TITLE", "No eboot")], false, false);
        fs::create_dir_all(game.join("$locks")).unwrap();

        let games = find_hdd0_games(game);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Tokyo Jungle");
        assert_eq!(games[0].title_id.as_deref(), Some("NPUB30611"));
    }

    #[test]
    fn vfs_yml_relocation() {
        let emu = Path::new(r"E:\Emulators\RPCS3");
        let yml = "/dev_flash/: $(EmulatorDir)dev_flash/\n/dev_hdd0/: $(EmulatorDir)dev_hdd0/\n";
        assert_eq!(parse_vfs_hdd0(yml, emu), Some(PathBuf::from(r"E:\Emulators\RPCS3/dev_hdd0/")));
        let yml = "/dev_hdd0/: \"D:/PS3/hdd0/\"\n";
        assert_eq!(parse_vfs_hdd0(yml, emu), Some(PathBuf::from("D:/PS3/hdd0/")));
        assert_eq!(parse_vfs_hdd0("/dev_hdd0/:\n", emu), None);
    }

    #[test]
    fn sfo_title_cleanup() {
        assert_eq!(clean_title("Uncharted®\nDrake's Fortune™"), "Uncharted Drake's Fortune");
    }
}
