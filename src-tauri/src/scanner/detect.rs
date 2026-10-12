//! Cheap header sniffing: which platform a disc image belongs to (GameCube vs
//! Wii vs PSP vs Sega CD for `.iso`) and the game ID when it sits in a fixed
//! header field. Reads at most a few KB per file, and only for new/changed files.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sniff {
    /// Platform id the header proves, if any.
    pub platform: Option<&'static str>,
    pub serial: Option<String>,
}

const GC_MAGIC: u32 = 0xC233_9F3D; // big-endian at 0x1C
const WII_MAGIC: u32 = 0x5D1C_9EA3; // big-endian at 0x18
const GCZ_MAGIC: u32 = 0xB10B_C001; // little-endian at 0
const TGC_MAGIC: u32 = 0xAE0F_38A2; // big-endian at 0

/// Enough to cover a CISO's disc header at 0x8000 and an ISO9660 PVD at 0x8000.
const HEAD_LEN: usize = 0x8100;

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Printable-ASCII field, trailing NULs/spaces trimmed. None if empty or binary.
fn ascii_field(b: &[u8], at: usize, len: usize) -> Option<String> {
    let raw = b.get(at..at + len)?;
    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    let s = std::str::from_utf8(&raw[..end]).ok()?.trim();
    (!s.is_empty() && s.bytes().all(|c| (0x20..0x7f).contains(&c))).then(|| s.to_string())
}

fn game_id(b: &[u8], at: usize) -> Option<String> {
    ascii_field(b, at, 6).filter(|s| s.len() == 6 && s.bytes().all(|c| c.is_ascii_alphanumeric()))
}

/// GameCube/Wii raw disc header at `at`.
fn raw_dolphin(b: &[u8], at: usize) -> Option<Sniff> {
    let platform = if be32(b, at + 0x18) == Some(WII_MAGIC) {
        "wii"
    } else if be32(b, at + 0x1C) == Some(GC_MAGIC) {
        "gc"
    } else {
        return None;
    };
    Some(Sniff { platform: Some(platform), serial: game_id(b, at) })
}

/// Recognizes every Dolphin container format from its first bytes.
pub fn dolphin(b: &[u8]) -> Option<Sniff> {
    match b.get(..4)? {
        b"WBFS" => {
            // Disc header copy sits at one "hd sector" (1 << byte 8).
            let sec = 1usize.checked_shl(u32::from(*b.get(8)?)).filter(|&s| s < b.len())?;
            Some(Sniff { platform: Some("wii"), serial: game_id(b, sec) })
        }
        b"CISO" => raw_dolphin(b, 0x8000),
        b"WIA\x01" | b"RVZ\x01" => {
            let platform = match be32(b, 0x48)? {
                1 => "gc",
                2 => "wii",
                _ => return None,
            };
            Some(Sniff { platform: Some(platform), serial: game_id(b, 0x58) })
        }
        _ if le32(b, 0) == Some(GCZ_MAGIC) => {
            let platform = if le32(b, 4)? == 1 { "wii" } else { "gc" };
            Some(Sniff { platform: Some(platform), serial: None })
        }
        _ if be32(b, 0) == Some(TGC_MAGIC) => Some(Sniff { platform: Some("gc"), serial: None }),
        _ => raw_dolphin(b, 0),
    }
}

/// Non-Nintendo `.iso` hints: PSP (ISO9660 system id "PSP GAME") and Sega CD
/// ("SEGADISCSYSTEM" at 0). PS1/PS2 both say "PLAYSTATION", so they stay ambiguous.
pub fn other_iso(b: &[u8]) -> Option<Sniff> {
    if b.get(..14) == Some(b"SEGADISCSYSTEM") {
        return Some(Sniff { platform: Some("segacd"), serial: None });
    }
    if b.get(0x8001..0x8006) == Some(b"CD001") && ascii_field(b, 0x8008, 32).as_deref() == Some("PSP GAME") {
        return Some(Sniff { platform: Some("psp"), serial: None });
    }
    None
}

/// First data track of a CD (the .bin a .cue points at): Sega CD has
/// "SEGADISCSYSTEM" in sector 0; PS1 says "PLAYSTATION" in the ISO9660 PVD at
/// sector 16. Handles cooked 2048-byte and raw 2352-byte (MODE1/MODE2) sectors.
pub fn cd_track(b: &[u8]) -> Option<&'static str> {
    for at in [0usize, 0x10] {
        if b.get(at..at + 14) == Some(b"SEGADISCSYSTEM") {
            return Some("segacd");
        }
    }
    // PVD start: cooked, raw MODE1 (16-byte header), raw MODE2 form 1 (24 bytes).
    for pvd in [0x8000usize, 16 * 2352 + 16, 16 * 2352 + 24] {
        if b.get(pvd + 1..pvd + 6) == Some(b"CD001")
            && ascii_field(b, pvd + 8, 32).as_deref() == Some("PLAYSTATION")
        {
            return Some("ps1");
        }
    }
    None
}

pub fn sniff_cd_track(path: &Path) -> Option<&'static str> {
    let mut f = File::open(path).ok()?;
    cd_track(&read_at(&mut f, 0, 16 * 2352 + 24 + 0x40).ok()?)
}

/// NDS: 12-byte title at 0, 4-char game code at 0x0C.
pub fn nds(b: &[u8]) -> Option<String> {
    ascii_field(b, 0x0C, 4).filter(|c| c.len() == 4 && c.bytes().all(|x| x.is_ascii_alphanumeric()))
}

/// GBA: fixed value 0x96 at 0xB2, 4-char game code at 0xAC.
pub fn gba(b: &[u8]) -> Option<String> {
    if b.get(0xB2) != Some(&0x96) {
        return None;
    }
    ascii_field(b, 0xAC, 4).filter(|c| c.len() == 4 && c.bytes().all(|x| x.is_ascii_alphanumeric()))
}

fn read_at(f: &mut File, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(len);
    f.take(len as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// 3DS product code ("CTR-P-AMGE") from an NCSD (.3ds/.cci) or NCCH (.cxi) image.
/// The NCCH header is not encrypted, so this works for any dump.
fn n3ds(f: &mut File) -> std::io::Result<Option<String>> {
    let head = read_at(f, 0, 0x200)?;
    let ncch_at = match head.get(0x100..0x104) {
        Some(b"NCSD") => match le32(&head, 0x120) {
            Some(units) => u64::from(units) * 0x200,
            None => return Ok(None),
        },
        Some(b"NCCH") => 0,
        _ => return Ok(None),
    };
    let ncch = read_at(f, ncch_at, 0x160)?;
    Ok(ascii_field(&ncch, 0x150, 16).filter(|c| c.starts_with("CTR-") || c.starts_with("KTR-")))
}

/// Sniffs a file whose lowercase extension is `ext`. I/O errors yield an empty
/// result; the scanner then falls back to extension/folder rules.
pub fn sniff(path: &Path, ext: &str) -> Sniff {
    let run = || -> std::io::Result<Sniff> {
        let mut f = File::open(path)?;
        Ok(match ext {
            "wad" => Sniff { platform: Some("wii"), serial: None },
            "wbfs" | "rvz" | "wia" | "gcz" | "ciso" | "gcm" | "tgc" => {
                dolphin(&read_at(&mut f, 0, HEAD_LEN)?).unwrap_or_default()
            }
            "iso" => {
                let b = read_at(&mut f, 0, HEAD_LEN)?;
                dolphin(&b).or_else(|| other_iso(&b)).unwrap_or_default()
            }
            "nds" | "dsi" | "ids" => Sniff { platform: None, serial: nds(&read_at(&mut f, 0, 0x10)?) },
            "gba" => Sniff { platform: None, serial: gba(&read_at(&mut f, 0, 0xC0)?) },
            "3ds" | "cci" | "cxi" => Sniff { platform: None, serial: n3ds(&mut f)? },
            _ => Sniff::default(),
        })
    };
    run().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disc_header(id: &[u8; 6], wii: bool) -> Vec<u8> {
        let mut b = vec![0u8; HEAD_LEN];
        b[..6].copy_from_slice(id);
        if wii {
            b[0x18..0x1C].copy_from_slice(&WII_MAGIC.to_be_bytes());
        } else {
            b[0x1C..0x20].copy_from_slice(&GC_MAGIC.to_be_bytes());
        }
        b
    }

    #[test]
    fn raw_gamecube_and_wii_iso() {
        let gc = dolphin(&disc_header(b"GLME01", false)).unwrap();
        assert_eq!((gc.platform, gc.serial.as_deref()), (Some("gc"), Some("GLME01")));
        let wii = dolphin(&disc_header(b"RMGE01", true)).unwrap();
        assert_eq!((wii.platform, wii.serial.as_deref()), (Some("wii"), Some("RMGE01")));
    }

    #[test]
    fn wbfs_is_wii_with_id_at_hd_sector() {
        let mut b = vec![0u8; HEAD_LEN];
        b[..4].copy_from_slice(b"WBFS");
        b[8] = 9; // 512-byte hd sectors
        b[0x200..0x206].copy_from_slice(b"RFEE01");
        let s = dolphin(&b).unwrap();
        assert_eq!((s.platform, s.serial.as_deref()), (Some("wii"), Some("RFEE01")));
    }

    #[test]
    fn rvz_wia_ciso_gcz_tgc() {
        let mut rvz = vec![0u8; 0x100];
        rvz[..4].copy_from_slice(b"RVZ\x01");
        rvz[0x48..0x4C].copy_from_slice(&1u32.to_be_bytes());
        rvz[0x58..0x5E].copy_from_slice(b"GZLE01");
        assert_eq!(dolphin(&rvz).unwrap(), Sniff { platform: Some("gc"), serial: Some("GZLE01".into()) });
        rvz[..4].copy_from_slice(b"WIA\x01");
        rvz[0x48..0x4C].copy_from_slice(&2u32.to_be_bytes());
        assert_eq!(dolphin(&rvz).unwrap().platform, Some("wii"));

        let mut ciso = disc_header(b"XXXXXX", false);
        ciso[..0x20].fill(0);
        ciso[..4].copy_from_slice(b"CISO");
        let hdr = disc_header(b"G8ME01", false);
        ciso[0x8000..0x8020].copy_from_slice(&hdr[..0x20]);
        assert_eq!(dolphin(&ciso).unwrap().serial.as_deref(), Some("G8ME01"));

        let mut gcz = vec![0u8; 32];
        gcz[..4].copy_from_slice(&GCZ_MAGIC.to_le_bytes());
        gcz[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(dolphin(&gcz).unwrap().platform, Some("wii"));

        let mut tgc = vec![0u8; 32];
        tgc[..4].copy_from_slice(&TGC_MAGIC.to_be_bytes());
        assert_eq!(dolphin(&tgc).unwrap().platform, Some("gc"));
    }

    #[test]
    fn non_dolphin_iso_is_not_misdetected() {
        let mut b = vec![0u8; HEAD_LEN];
        assert_eq!(dolphin(&b), None);
        b[0x8001..0x8006].copy_from_slice(b"CD001");
        b[0x8008..0x8010].copy_from_slice(b"PSP GAME");
        assert_eq!(other_iso(&b).unwrap().platform, Some("psp"));
        b[0x8008..0x8013].copy_from_slice(b"PLAYSTATION");
        assert_eq!(other_iso(&b), None);
        b[..14].copy_from_slice(b"SEGADISCSYSTEM");
        assert_eq!(other_iso(&b).unwrap().platform, Some("segacd"));
    }

    #[test]
    fn cd_track_sega_vs_playstation() {
        let mut raw = vec![0u8; 16 * 2352 + 24 + 0x40];
        raw[0x10..0x1E].copy_from_slice(b"SEGADISCSYSTEM");
        assert_eq!(cd_track(&raw), Some("segacd"));
        raw[0x10..0x1E].fill(0);
        let pvd = 16 * 2352 + 24;
        raw[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        raw[pvd + 8..pvd + 19].copy_from_slice(b"PLAYSTATION");
        assert_eq!(cd_track(&raw), Some("ps1"));
        assert_eq!(cd_track(&[0u8; 64]), None);
    }

    #[test]
    fn nds_and_gba_codes() {
        let mut nds_h = vec![0u8; 0x10];
        nds_h[..12].copy_from_slice(b"CHRONOTRIGGE");
        nds_h[0x0C..0x10].copy_from_slice(b"YQUE");
        assert_eq!(nds(&nds_h).as_deref(), Some("YQUE"));

        let mut gba_h = vec![0u8; 0xC0];
        gba_h[0xAC..0xB0].copy_from_slice(b"AGSE");
        assert_eq!(gba(&gba_h), None, "missing fixed byte");
        gba_h[0xB2] = 0x96;
        assert_eq!(gba(&gba_h).as_deref(), Some("AGSE"));
    }

    #[test]
    fn n3ds_product_code_from_cci_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("game.cci");
        let mut b = vec![0u8; 0x4000 + 0x200];
        b[0x100..0x104].copy_from_slice(b"NCSD");
        b[0x120..0x124].copy_from_slice(&0x20u32.to_le_bytes()); // partition 0 at 0x4000
        b[0x4100..0x4104].copy_from_slice(b"NCCH");
        b[0x4150..0x415A].copy_from_slice(b"CTR-P-AMGE");
        std::fs::write(&p, &b).unwrap();
        assert_eq!(sniff(&p, "cci").serial.as_deref(), Some("CTR-P-AMGE"));
        // Missing file: empty result rather than an error.
        assert_eq!(sniff(&dir.path().join("nope.cci"), "cci"), Sniff::default());
    }
}
