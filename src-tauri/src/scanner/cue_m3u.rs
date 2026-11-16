//! Playlist/sheet parsing: which files does a .cue/.m3u/.ccd/.mds pull in? Those
//! referenced files are hidden from the library so a game appears exactly once.

use std::path::{Path, PathBuf};

/// Files referenced by a cue sheet (`FILE "Track 01.bin" BINARY`), resolved
/// against the cue's directory.
pub fn cue_files(content: &str, dir: &Path) -> Vec<PathBuf> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = strip_prefix_ci(line, "FILE")?;
            let rest = rest.trim_start();
            let name = if let Some(q) = rest.strip_prefix('"') {
                &q[..q.find('"')?]
            } else {
                // Unquoted: FILE name.bin BINARY -> the type is the last word.
                let end = rest.rfind(char::is_whitespace).unwrap_or(rest.len());
                rest[..end].trim()
            };
            (!name.is_empty()).then(|| dir.join(name))
        })
        .collect()
}

/// Entries of an .m3u playlist (comments and blank lines skipped).
pub fn m3u_entries(content: &str, dir: &Path) -> Vec<PathBuf> {
    content
        .lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}'))
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| dir.join(l))
        .collect()
}

/// Same-stem sidecar files implied by a sheet format, e.g. `game.ccd` ->
/// `game.img`, `game.sub`; `game.mds` -> `game.mdf`.
pub fn implied_companions(path: &Path) -> Vec<PathBuf> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let sidecars: &[&str] = match ext.as_str() {
        "ccd" => &["img", "sub"],
        "mds" => &["mdf"],
        _ => &[],
    };
    sidecars.iter().map(|e| path.with_extension(e)).collect()
}

/// Builds an .m3u body listing discs by absolute path (used when a multi-disc
/// set has no playlist of its own; written to the app's data dir, never next
/// to the user's files).
pub fn build_m3u(discs: &[PathBuf]) -> String {
    let mut s = String::new();
    for d in discs {
        s.push_str(&d.to_string_lossy());
        s.push_str("\r\n");
    }
    s
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    let rest = &s[prefix.len()..];
    (head.eq_ignore_ascii_case(prefix) && rest.starts_with(char::is_whitespace)).then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_track_cue() {
        let cue = "FILE \"Snatcher (USA) (Track 01).bin\" BINARY\r\n  TRACK 01 MODE1/2352\r\n    INDEX 01 00:00:00\r\nFILE \"Snatcher (USA) (Track 02).bin\" BINARY\r\n  TRACK 02 AUDIO\r\n";
        let dir = Path::new(r"E:\Sega\CD\Snatcher (USA)");
        assert_eq!(
            cue_files(cue, dir),
            [dir.join("Snatcher (USA) (Track 01).bin"), dir.join("Snatcher (USA) (Track 02).bin")]
        );
    }

    #[test]
    fn parses_unquoted_and_lowercase_file_lines() {
        let dir = Path::new("d");
        assert_eq!(cue_files("file game.bin BINARY", dir), [dir.join("game.bin")]);
        assert_eq!(cue_files("FILE my game.bin BINARY", dir), [dir.join("my game.bin")]);
        // FILENAME/REM lines are not FILE entries.
        assert!(cue_files("FILENAME x\nREM FILE y", dir).is_empty());
    }

    #[test]
    fn parses_m3u_with_comments_bom_and_blank_lines() {
        let dir = Path::new("d");
        let m3u = "\u{feff}#EXTM3U\r\nFF7 (Disc 1).cue\r\n\r\n# comment\r\nFF7 (Disc 2).cue\r\n";
        assert_eq!(
            m3u_entries(m3u, dir),
            [dir.join("FF7 (Disc 1).cue"), dir.join("FF7 (Disc 2).cue")]
        );
    }

    #[test]
    fn implied_sidecars() {
        assert_eq!(
            implied_companions(Path::new("a/Game.CCD")),
            [PathBuf::from("a/Game.img"), PathBuf::from("a/Game.sub")]
        );
        assert_eq!(implied_companions(Path::new("a/Game.mds")), [PathBuf::from("a/Game.mdf")]);
        assert!(implied_companions(Path::new("a/Game.iso")).is_empty());
    }

    #[test]
    fn builds_absolute_m3u() {
        let body = build_m3u(&[PathBuf::from(r"E:\a (Disc 1).iso"), PathBuf::from(r"E:\a (Disc 2).iso")]);
        assert_eq!(body, "E:\\a (Disc 1).iso\r\nE:\\a (Disc 2).iso\r\n");
    }
}
