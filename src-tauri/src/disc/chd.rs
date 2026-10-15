//! CHD disc images (chdman `createcd` / `createdvd`).
//!
//! CD images store every frame as 2352 bytes of sector + 96 of subcode
//! (2448), in hunks of several frames; a track's data sits at the start of
//! its frames (cooked tracks) or after the sector header (raw tracks). DVD
//! images are plain 2048-byte sectors. Only the first track is read, which
//! is where every supported console's data lives.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use chd::metadata::{KnownMetadata, Metadata, MetadataTag};
use chd::Chd;

use super::{raw_data_offset, Sectors};
use crate::error::{Error, Result};

const CD_FRAME: u64 = 2448;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    /// Frame where track 1's first sector is stored, and where its user data
    /// starts inside a frame.
    Cd { first_frame: u64, data_offset: u64 },
    Dvd,
}

pub struct ChdDisc {
    chd: Chd<BufReader<File>>,
    layout: Layout,
    hunk: Vec<u8>,
    compressed: Vec<u8>,
    loaded: Option<u32>,
}

impl ChdDisc {
    pub fn open(path: &Path) -> Result<Self> {
        let mut chd = Chd::open(BufReader::new(File::open(path)?), None).map_err(chd_error)?;
        let metadata: Vec<Metadata> = chd.metadata_refs().try_into().map_err(chd_error)?;
        let layout = match metadata.iter().find(|m| KnownMetadata::is_cdrom(m.metatag())) {
            Some(_) => cd_layout(&metadata),
            None => Layout::Dvd,
        };
        let hunk = chd.get_hunksized_buffer();
        let mut disc = Self { chd, layout, hunk, compressed: Vec::new(), loaded: None };
        // A raw track's sector header says where its data starts, whatever
        // the metadata claims (as rcheevos' cdreader checks the sync pattern).
        if let Layout::Cd { first_frame, .. } = disc.layout {
            let head = disc.bytes(first_frame * CD_FRAME, 16)?;
            if let Some(offset) = raw_data_offset(&head) {
                disc.layout = Layout::Cd { first_frame, data_offset: offset };
            }
        }
        Ok(disc)
    }

    /// Up to `len` bytes at `at` in the uncompressed image, within one hunk.
    fn bytes(&mut self, at: u64, len: usize) -> Result<Vec<u8>> {
        let hunk_size = self.hunk.len() as u64;
        if hunk_size == 0 || at >= self.chd.header().logical_bytes() {
            return Ok(Vec::new());
        }
        let num = u32::try_from(at / hunk_size).map_err(|_| Error::msg("CHD offset out of range"))?;
        if num >= self.chd.header().hunk_count() {
            return Ok(Vec::new());
        }
        if self.loaded != Some(num) {
            self.loaded = None;
            self.chd.hunk(num).and_then(|mut h| h.read_hunk_in(&mut self.compressed, &mut self.hunk)).map_err(chd_error)?;
            self.loaded = Some(num);
        }
        let start = (at % hunk_size) as usize;
        let end = (start + len).min(self.hunk.len());
        Ok(self.hunk[start..end].to_vec())
    }
}

impl Sectors for ChdDisc {
    fn read(&mut self, lba: u32, len: usize) -> Result<Vec<u8>> {
        let len = len.min(2048);
        let at = match self.layout {
            Layout::Cd { first_frame, data_offset } => (first_frame + u64::from(lba)) * CD_FRAME + data_offset,
            Layout::Dvd => u64::from(lba) * 2048,
        };
        self.bytes(at, len)
    }
}

fn chd_error(e: chd::Error) -> Error {
    Error::msg(format!("couldn't read the CHD file: {e}"))
}

/// Track 1 from the CD track metadata (`TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE
/// FRAMES:... PREGAP:150 PGTYPE:VMODE2_RAW ...`). A pregap whose type starts
/// with "V" is stored in the file ahead of the track.
fn cd_layout(metadata: &[Metadata]) -> Layout {
    let track1 = metadata
        .iter()
        .filter(|m| {
            m.metatag() == KnownMetadata::CdRomTrack2.metatag() || m.metatag() == KnownMetadata::CdRomTrack.metatag()
        })
        .map(|m| {
            let end = m.value.iter().position(|&b| b == 0).unwrap_or(m.value.len());
            String::from_utf8_lossy(&m.value[..end]).into_owned()
        })
        .find(|text| field(text, "TRACK") == Some("1"));
    let Some(text) = track1 else { return Layout::Cd { first_frame: 0, data_offset: 0 } };
    track_layout(&text)
}

fn track_layout(text: &str) -> Layout {
    let data_offset = match field(text, "TYPE").unwrap_or("") {
        "MODE1_RAW" => 16,
        "MODE2_RAW" => 24,
        "MODE2" | "MODE2_FORM_MIX" => 8,
        _ => 0, // MODE1, MODE2_FORM1 (cooked 2048-byte data)
    };
    let stored_pregap = field(text, "PGTYPE").is_some_and(|t| t.starts_with('V'));
    let pregap: u64 = field(text, "PREGAP").and_then(|p| p.parse().ok()).unwrap_or(0);
    Layout::Cd { first_frame: if stored_pregap { pregap } else { 0 }, data_offset }
}

fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.split_whitespace().find_map(|kv| kv.strip_prefix(key)?.strip_prefix(':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_metadata_gives_the_data_layout() {
        assert_eq!(
            track_layout("TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE FRAMES:1000 PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0"),
            Layout::Cd { first_frame: 0, data_offset: 24 }
        );
        assert_eq!(
            track_layout("TRACK:1 TYPE:MODE1_RAW SUBTYPE:NONE FRAMES:1150 PREGAP:150 PGTYPE:VMODE1_RAW PGSUB:RW POSTGAP:0"),
            Layout::Cd { first_frame: 150, data_offset: 16 }
        );
        assert_eq!(track_layout("TRACK:1 TYPE:MODE1 SUBTYPE:NONE FRAMES:9"), Layout::Cd { first_frame: 0, data_offset: 0 });
        assert_eq!(field("TRACK:12 TYPE:AUDIO", "TRACK"), Some("12"));
    }
}
