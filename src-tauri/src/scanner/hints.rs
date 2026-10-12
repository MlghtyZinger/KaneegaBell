//! Platform hints from folder names ("PS2", "PSX", "GameCube - Wii", "Switch").
//!
//! Used to (1) break ties in Auto folders when file headers can't decide,
//! (2) preselect the platform when a folder is added in Settings, and (3) skip
//! folders, during Auto scans, that clearly belong to a platform the app isn't
//! set up for yet. The table deliberately includes platforms without a built-in
//! definition (Switch, PS4, Xbox...): once one is added to `platforms.rs`, its
//! folders are scanned with no scanner changes.

use super::normalize::search_key;

/// (platform id, names). Matched against a folder name's words, so "ps2"
/// matches "PS2", "PS2 Games" and "Sony - PS2", but not "ps2x".
const NAMES: &[(&str, &[&str])] = &[
    ("ps3", &["ps3", "playstation 3"]),
    ("ps2", &["ps2", "playstation 2"]),
    ("psp", &["psp", "playstation portable"]),
    ("ps4", &["ps4", "playstation 4"]),
    ("ps5", &["ps5", "playstation 5"]),
    ("vita", &["vita", "psvita", "ps vita", "playstation vita"]),
    ("ps1", &["ps1", "psx", "psone", "playstation 1", "playstation"]),
    ("3ds", &["3ds", "nintendo 3ds"]),
    ("nds", &["nds", "ds", "nintendo ds"]),
    ("gba", &["gba", "game boy advance", "gameboy advance"]),
    ("n64", &["n64", "nintendo 64"]),
    ("snes", &["snes", "super nintendo", "super famicom", "sfc"]),
    ("gc", &["gamecube", "game cube", "gc", "ngc"]),
    ("wiiu", &["wii u", "wiiu", "cemu"]),
    ("wii", &["wii"]),
    ("switch", &["switch", "nintendo switch", "yuzu", "ryujinx", "suyu", "citron"]),
    ("segacd", &["sega cd", "segacd", "mega cd", "megacd"]),
    ("saturn", &["saturn", "sega saturn"]),
    ("dreamcast", &["dreamcast"]),
    ("xbox360", &["xbox 360", "x360"]),
    ("xbox", &["xbox", "original xbox"]),
    // Not a game platform: NFC figure dumps (".bin") that would otherwise
    // look like disc images.
    ("amiibo", &["amiibo"]),
];

/// Every platform a folder name points at, in table order. "Gamecube - Wii"
/// yields both; "Emulator Games" yields none.
pub fn platforms_for_name(name: &str) -> Vec<&'static str> {
    let key = format!(" {} ", search_key(name));
    let has = |n: &str| key.contains(&format!(" {n} "));
    let mut out: Vec<&'static str> = Vec::new();
    for (platform, names) in NAMES {
        if !names.iter().any(|n| has(n)) || out.contains(platform) {
            continue;
        }
        // "PlayStation 2" also contains "playstation": that alone means PS1
        // only when no numbered PlayStation matched.
        if *platform == "ps1" && out.iter().any(|p| matches!(*p, "ps2" | "ps3" | "ps4" | "ps5" | "psp" | "vita")) && !has("ps1") && !has("psx") {
            continue;
        }
        // "Xbox 360" also contains "xbox".
        if *platform == "xbox" && out.contains(&"xbox360") && !has("original xbox") {
            continue;
        }
        // "Wii U" also contains "wii".
        if *platform == "wii" && out.contains(&"wiiu") {
            continue;
        }
        out.push(*platform);
    }
    out
}

/// The single platform a folder's own name suggests, if unambiguous.
pub fn suggest(folder_name: &str) -> Option<&'static str> {
    match platforms_for_name(folder_name).as_slice() {
        [one] => Some(one),
        _ => None,
    }
}

/// For a file in an Auto source: platforms hinted by the folders between the
/// source root and the file (nearest first), then by the source folder itself.
pub fn for_file(file: &std::path::Path, source_root: &std::path::Path) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut dir = file.parent();
    while let Some(d) = dir {
        if let Some(name) = d.file_name() {
            for p in platforms_for_name(&name.to_string_lossy()) {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
        if d == source_root {
            break;
        }
        dir = d.parent();
    }
    out
}

/// Auto scans skip a folder whose name only points at platforms that aren't
/// enabled ("Switch" today; scanned automatically once Switch is added).
pub fn skip_dir(name: &str, is_enabled: impl Fn(&str) -> bool) -> bool {
    let hinted = platforms_for_name(name);
    !hinted.is_empty() && !hinted.iter().any(|p| is_enabled(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn folder_names_from_the_real_library() {
        assert_eq!(suggest("PS2"), Some("ps2"));
        assert_eq!(suggest("PS1"), Some("ps1"));
        assert_eq!(suggest("PS3"), Some("ps3"));
        assert_eq!(suggest("PSP"), Some("psp"));
        assert_eq!(suggest("3DS"), Some("3ds"));
        assert_eq!(suggest("NDS"), Some("nds"));
        assert_eq!(suggest("GBA"), Some("gba"));
        assert_eq!(suggest("N64"), Some("n64"));
        assert_eq!(suggest("SNES"), Some("snes"));
        assert_eq!(suggest("CD"), None, "Sega\\CD: the parent says Sega, the folder alone is ambiguous");
        assert_eq!(platforms_for_name("Gamecube - Wii"), ["gc", "wii"]);
        assert_eq!(suggest("Gamecube - Wii"), None);
        assert_eq!(suggest("Emulator Games"), None);
        assert_eq!(suggest("Castlevania Romhacks"), None);
        // Platforms without a built-in definition yet are still recognized.
        assert_eq!(suggest("Switch"), Some("switch"));
        assert_eq!(suggest("Switch firmware"), Some("switch"));
        assert_eq!(suggest("yuzu keys"), Some("switch"));
        assert_eq!(suggest("PS4"), Some("ps4"));
        assert_eq!(suggest("Xbox"), Some("xbox"));
        assert_eq!(suggest("Xbox 360"), Some("xbox360"));
        assert_eq!(suggest("Saturn"), Some("saturn"));
        assert_eq!(suggest("Amiibo"), Some("amiibo"));
    }

    #[test]
    fn variants_and_word_boundaries() {
        assert_eq!(suggest("Sony - PlayStation 2"), Some("ps2"));
        assert_eq!(suggest("Sony PlayStation"), Some("ps1"));
        assert_eq!(suggest("PSX Games"), Some("ps1"));
        assert_eq!(suggest("Sega CD"), Some("segacd"));
        assert_eq!(suggest("Mega-CD"), Some("segacd"));
        assert_eq!(suggest("Nintendo DS"), Some("nds"));
        assert_eq!(suggest("Nintendo 3DS"), Some("3ds"));
        assert_eq!(suggest("Wii U"), Some("wiiu"));
        assert_eq!(suggest("PlayStation Vita"), Some("vita"));
        assert_eq!(suggest("dsiware"), None);
        assert_eq!(suggest("Wiiware"), None);
    }

    #[test]
    fn skips_only_platforms_that_are_not_enabled() {
        let today = |p: &str| ["gc", "wii", "ps2", "ps3"].contains(&p);
        assert!(skip_dir("Switch", today));
        assert!(skip_dir("Xbox 360", today));
        assert!(skip_dir("Amiibo", today));
        assert!(!skip_dir("Gamecube - Wii", today));
        assert!(!skip_dir("Nintendo", today), "no hint: keep walking");
        assert!(!skip_dir("Castlevania Romhacks", today));
        // Once Switch is added and enabled, its folders are scanned.
        let later = |p: &str| p == "switch";
        assert!(!skip_dir("Switch", later));
    }

    #[test]
    fn file_hints_walk_up_to_the_source() {
        let root = Path::new(r"E:\Games");
        let f = Path::new(r"E:\Games\PS2\Jak II (USA)\Jak II (USA).iso");
        assert_eq!(for_file(f, root), ["ps2"]);
        let f = Path::new(r"E:\Games\Mixed\Game.iso");
        assert!(for_file(f, root).is_empty());
        // The source folder's own name counts too.
        assert_eq!(for_file(Path::new(r"E:\PS2\x.iso"), Path::new(r"E:\PS2")), ["ps2"]);
    }
}
