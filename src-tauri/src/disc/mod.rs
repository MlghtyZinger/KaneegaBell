//! Optical disc images read as 2048-byte sectors of user data, whatever the
//! container: a plain `.iso`/`.bin` (or a `.cue`'s first track), CHD (MAME's
//! compressed format, made by chdman) or CSO/ZSO (compressed ISOs, made by
//! maxcso). RetroAchievements hashing and the scanner's PS1/PS2/PSP/Sega CD
//! detection read discs through this.
//!
//! GameCube/Wii formats (RVZ, WIA, GCZ, CISO) are read by the `nod` crate
//! instead: their hashes need the raw disc, not ISO9660 sectors.

pub mod chd;
pub mod cso;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, Result};

/// Sector reads on a disc's first (data) track.
pub trait Sectors {
    /// Up to `len` (at most 2048) bytes of user data from sector `lba`: fewer,
    /// or none, past the end of the disc.
    fn read(&mut self, lba: u32, len: usize) -> Result<Vec<u8>>;
}

/// Opens a disc image by its extension: `.chd`, `.cso`/`.zso`, or a plain
/// image (`.cue` sheets resolve to their first track's file).
pub fn open(path: &Path) -> Result<Box<dyn Sectors>> {
    Ok(match ext(path).as_str() {
        "chd" => Box::new(chd::ChdDisc::open(path)?),
        "cso" | "zso" => Box::new(cso::Cso::open(path)?),
        _ => Box::new(RawFile::open(path)?),
    })
}

/// Whether `open` reads more than plain images for this extension.
pub fn is_compressed(ext: &str) -> bool {
    matches!(ext, "chd" | "cso" | "zso")
}

fn ext(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// Reads up to `len` bytes at `offset` (fewer at end of file).
pub(crate) fn read_at<R: Read + Seek>(r: &mut R, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    r.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(len);
    r.take(len as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Raw CD sectors start with this 12-byte sync pattern; byte 15 is the mode.
pub(crate) const SYNC: [u8; 12] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];

/// Where user data starts in a raw 2352-byte sector, from its header.
pub(crate) fn raw_data_offset(head: &[u8]) -> Option<u64> {
    (head.len() >= 16 && head[..12] == SYNC).then(|| if head[15] == 2 { 24 } else { 16 })
}

/// A plain image: 2048-byte sectors (`.iso`), or raw 2352-byte ones (`.bin`)
/// with a 16- or 24-byte header, picked from the cue sheet or the sector's
/// sync pattern and mode byte, like rcheevos' cdreader.
///
/// Sector 0 is where track 1's `INDEX 01` points: a pregap stored in the
/// file ahead of it (`INDEX 00`) is skipped. That's how emulators address a
/// disc (DuckStation, PCSX2, RetroArch), and how CHD images made from such a
/// cue are read (`chd.rs`), so both forms of a disc hash the same.
pub struct RawFile {
    file: File,
    sector_size: u64,
    data_offset: u64,
    /// Frames in the file before track 1's `INDEX 01`.
    first_frame: u64,
}

impl RawFile {
    pub fn open(path: &Path) -> Result<Self> {
        let (data_path, track) = if ext(path) == "cue" {
            let text = std::fs::read_to_string(path)?;
            let dir = path.parent().unwrap_or(Path::new("."));
            let file = crate::scanner::cue_m3u::cue_files(&text, dir)
                .into_iter()
                .next()
                .ok_or_else(|| Error::msg("cue sheet lists no files"))?;
            (file, cue_track1(&text))
        } else {
            (path.to_path_buf(), CueTrack1::default())
        };
        let mut file = File::open(&data_path)?;
        let first_frame = track.index01;
        let (sector_size, data_offset) = match track.mode.as_deref() {
            Some("MODE1/2048") | Some("MODE2/2048") => (2048, 0),
            Some("MODE1/2352") => (2352, 16),
            Some("MODE2/2352") => (2352, 24),
            _ => match raw_data_offset(&read_at(&mut file, first_frame * 2352, 16)?) {
                Some(offset) => (2352, offset),
                None => (2048, 0),
            },
        };
        Ok(Self { file, sector_size, data_offset, first_frame })
    }
}

impl Sectors for RawFile {
    fn read(&mut self, lba: u32, len: usize) -> Result<Vec<u8>> {
        let at = (self.first_frame + u64::from(lba)) * self.sector_size + self.data_offset;
        Ok(read_at(&mut self.file, at, len.min(2048))?)
    }
}

/// Track 1 of a cue sheet: its mode ("MODE2/2352") and where its `INDEX 01`
/// starts in the first file, in frames (75 per second).
#[derive(Debug, Default, PartialEq, Eq)]
struct CueTrack1 {
    mode: Option<String>,
    index01: u64,
}

fn cue_track1(cue: &str) -> CueTrack1 {
    let mut out = CueTrack1::default();
    let mut files = 0;
    let mut in_track1 = false;
    for line in cue.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.first().map(|w| w.to_ascii_uppercase()).as_deref() {
            Some("FILE") => {
                files += 1;
                in_track1 = false;
            }
            Some("TRACK") => {
                // Only track 1 in the first file: that's the file read above.
                in_track1 = files <= 1 && words.get(1).and_then(|n| n.parse::<u32>().ok()) == Some(1);
                if in_track1 {
                    out.mode = words.get(2).map(|m| m.to_ascii_uppercase());
                } else if out.mode.is_some() {
                    break;
                }
            }
            Some("INDEX") if in_track1 && words.get(1).and_then(|n| n.parse::<u32>().ok()) == Some(1) => {
                out.index01 = words.get(2).and_then(|t| msf_frames(t)).unwrap_or(0);
            }
            _ => {}
        }
    }
    out
}

/// "mm:ss:ff" -> frames.
fn msf_frames(t: &str) -> Option<u64> {
    let mut parts = t.split(':').map(|p| p.parse::<u64>().ok());
    let (m, s, f) = (parts.next()??, parts.next()??, parts.next()??);
    (s < 60 && f < 75).then_some((m * 60 + s) * 75 + f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_track1_mode_and_index01() {
        let redump = "FILE \"a.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    INDEX 00 10:00:00\n    INDEX 01 10:02:00\n";
        assert_eq!(cue_track1(redump), CueTrack1 { mode: Some("MODE2/2352".into()), index01: 0 });
        let pregap = "FILE \"a.bin\" BINARY\r\n  TRACK 01 MODE1/2352\r\n    INDEX 00 00:00:00\r\n    INDEX 01 00:02:00\r\n";
        assert_eq!(cue_track1(pregap), CueTrack1 { mode: Some("MODE1/2352".into()), index01: 150 });
        // Track 2's indexes (another file) never count for track 1.
        let split = "FILE \"1.bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\nFILE \"2.bin\" BINARY\n TRACK 02 AUDIO\n  INDEX 01 00:02:00\n";
        assert_eq!(cue_track1(split).index01, 0);
        assert_eq!(msf_frames("01:02:03"), Some(4653));
        assert_eq!(msf_frames("00:60:00"), None);
    }

    /// A raw MODE2 image whose user data in sector n is filled with `n + 1`
    /// (so it can't be mistaken for the zeroed pregap).
    fn raw_frames(count: u32) -> Vec<u8> {
        let mut out = Vec::new();
        for n in 0..count {
            let mut f = vec![0u8; 2352];
            f[..12].copy_from_slice(&SYNC);
            f[15] = 2;
            f[24..24 + 2048].fill(n as u8 + 1);
            out.extend_from_slice(&f);
        }
        out
    }

    #[test]
    fn a_stored_pregap_is_skipped_so_sector_0_is_index_01() {
        let t = tempfile::tempdir().unwrap();
        let mut bin = vec![0u8; 150 * 2352]; // pregap frames (no data track yet)
        bin.extend(raw_frames(20));
        std::fs::write(t.path().join("p.bin"), &bin).unwrap();
        let cue = t.path().join("p.cue");
        std::fs::write(&cue, "FILE \"p.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 00 00:00:00\n    INDEX 01 00:02:00\n").unwrap();
        let mut d = RawFile::open(&cue).unwrap();
        assert_eq!(d.read(0, 4).unwrap(), [1, 1, 1, 1]);
        assert_eq!(d.read(16, 4).unwrap(), [17, 17, 17, 17]);
        // No mode in the sheet: the sync pattern is looked for at INDEX 01 too.
        std::fs::write(&cue, "FILE \"p.bin\" BINARY\n  TRACK 01\n    INDEX 01 00:02:00\n").unwrap();
        assert_eq!(RawFile::open(&cue).unwrap().read(5, 2).unwrap(), [6, 6]);
    }
}
