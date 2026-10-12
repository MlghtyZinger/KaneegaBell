//! Platform hints from folder names ("PS2", "PSX", "GameCube - Wii", "Sega CD").
//! Used to break ties in Auto folders when file headers can't decide, and to
//! preselect the platform when a folder is added in Settings.

use super::normalize::search_key;

/// (platform, names). Matched against a folder name's words, so "ps2" matches
/// "PS2", "PS2 Games" and "Sony - PS2", but not "ps2x".
const NAMES: &[(&str, &[&str])] = &[
    ("ps3", &["ps3", "playstation 3"]),
    ("ps2", &["ps2", "playstation 2"]),
    ("psp", &["psp", "playstation portable"]),
    ("ps1", &["ps1", "psx", "psone", "playstation 1", "playstation"]),
    ("3ds", &["3ds", "nintendo 3ds"]),
    ("nds", &["nds", "ds", "nintendo ds"]),
    ("gba", &["gba", "game boy advance", "gameboy advance"]),
    ("n64", &["n64", "nintendo 64"]),
    ("snes", &["snes", "super nintendo", "super famicom", "sfc"]),
    ("gc", &["gamecube", "game cube", "gc", "ngc"]),
    ("wii", &["wii"]),
    ("segacd", &["sega cd", "segacd", "mega cd", "megacd"]),
];

/// Every platform a folder name points at, in table order. "Gamecube - Wii"
/// yields both; "Emulator Games" yields none.
pub fn platforms_for_name(name: &str) -> Vec<&'static str> {
    let key = format!(" {} ", search_key(name));
    let mut out = Vec::new();
    for (platform, names) in NAMES {
        // "playstation" alone means PS1 only if no numbered PlayStation matched.
        if names.iter().any(|n| key.contains(&format!(" {n} "))) && !out.contains(platform) {
            if *platform == "ps1" && out.iter().any(|p| matches!(*p, "ps2" | "ps3" | "psp")) && !key.contains(" ps1 ") && !key.contains(" psx ") {
                continue;
            }
            out.push(*platform);
        }
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
        assert_eq!(suggest("dsiware"), None);
        assert_eq!(suggest("Wiiware"), None);
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
