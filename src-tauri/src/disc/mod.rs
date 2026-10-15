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
pub struct RawFile {
    file: File,
    sector_size: u64,
    data_offset: u64,
}

impl RawFile {
    pub fn open(path: &Path) -> Result<Self> {
        let (data_path, mode) = if ext(path) == "cue" {
            let text = std::fs::read_to_string(path)?;
            let dir = path.parent().unwrap_or(Path::new("."));
            let file = crate::scanner::cue_m3u::cue_files(&text, dir)
                .into_iter()
                .next()
                .ok_or_else(|| Error::msg("cue sheet lists no files"))?;
            (file, cue_track1_mode(&text))
        } else {
            (path.to_path_buf(), None)
        };
        let mut file = File::open(&data_path)?;
        let (sector_size, data_offset) = match mode.as_deref() {
            Some("MODE1/2048") | Some("MODE2/2048") => (2048, 0),
            Some("MODE1/2352") => (2352, 16),
            Some("MODE2/2352") => (2352, 24),
            _ => match raw_data_offset(&read_at(&mut file, 0, 16)?) {
                Some(offset) => (2352, offset),
                None => (2048, 0),
            },
        };
        Ok(Self { file, sector_size, data_offset })
    }
}

impl Sectors for RawFile {
    fn read(&mut self, lba: u32, len: usize) -> Result<Vec<u8>> {
        let at = u64::from(lba) * self.sector_size + self.data_offset;
        Ok(read_at(&mut self.file, at, len.min(2048))?)
    }
}

fn cue_track1_mode(cue: &str) -> Option<String> {
    let mut lines = cue.lines().map(str::trim);
    lines
        .find(|l| l.to_ascii_uppercase().starts_with("TRACK 01") || l.to_ascii_uppercase().starts_with("TRACK 1 "))
        .and_then(|l| l.split_whitespace().nth(2))
        .map(|m| m.to_ascii_uppercase())
}
