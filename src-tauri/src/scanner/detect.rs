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

/// Sector layout of an optical disc image: bytes per sector and where the 2048
/// bytes of user data start inside each sector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectorLayout {
    pub size: u64,
    pub data: u64,
}

/// Cooked ISO, raw MODE1 (16-byte header), raw MODE2 form 1 (24-byte header).
const LAYOUTS: [SectorLayout; 3] = [
    SectorLayout { size: 2048, data: 0 },
    SectorLayout { size: 2352, data: 16 },
    SectorLayout { size: 2352, data: 24 },
];

/// Finds the ISO9660 primary volume descriptor (sector 16) in `head`.
fn find_pvd(head: &[u8]) -> Option<(SectorLayout, usize)> {
    LAYOUTS.into_iter().find_map(|l| {
        let at = (16 * l.size + l.data) as usize;
        (head.get(at + 1..at + 6) == Some(b"CD001")).then_some((l, at))
    })
}

/// PS1 vs PS2 from SYSTEM.CNF (`BOOT2 = cdrom0:\SLUS_203.12;1` is PS2,
/// `BOOT = cdrom:\SCUS_944.55;1` is PS1), plus the serial from the boot file.
pub fn parse_system_cnf(text: &str) -> Option<(&'static str, Option<String>)> {
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let platform = match key.trim().to_ascii_uppercase().as_str() {
            "BOOT2" => "ps2",
            "BOOT" => "ps1",
            _ => continue,
        };
        // "cdrom0:\SLUS_203.12;1" -> "SLUS_203.12" -> "SLUS-20312"
        let file = value.trim().rsplit(['\\', ':', '/']).next().unwrap_or("");
        let file = file.split(';').next().unwrap_or("");
        let serial = file.replace('_', "-").replace('.', "").to_ascii_uppercase();
        let valid = serial.len() >= 8 && serial.contains('-') && serial.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-');
        return Some((platform, valid.then_some(serial)));
    }
    None
}

/// Looks up `name` in an ISO9660 directory's data (records never span sectors).
fn find_dir_entry(dir: &[u8], name: &str) -> Option<(u32, u32)> {
    let mut i = 0usize;
    while i < dir.len() {
        let len = dir[i] as usize;
        if len == 0 {
            i = (i / 2048 + 1) * 2048; // padding to the next sector
            continue;
        }
        let rec = dir.get(i..i + len)?;
        let name_len = *rec.get(32)? as usize;
        let rec_name = std::str::from_utf8(rec.get(33..33 + name_len)?).ok()?;
        if rec_name.split(';').next()?.eq_ignore_ascii_case(name) {
            let extent = u32::from_le_bytes(rec.get(2..6)?.try_into().ok()?);
            let size = u32::from_le_bytes(rec.get(10..14)?.try_into().ok()?);
            return Some((extent, size));
        }
        i += len;
    }
    None
}

/// Reads `bytes` of user data starting at sector `lba`.
fn read_sectors(f: &mut File, layout: SectorLayout, lba: u32, bytes: u32) -> std::io::Result<Vec<u8>> {
    let bytes = bytes.min(64 * 1024); // root dirs and SYSTEM.CNF are tiny
    if layout.size == 2048 {
        return read_at(f, u64::from(lba) * 2048, bytes as usize);
    }
    let mut out = Vec::with_capacity(bytes as usize);
    let mut sector = u64::from(lba);
    while (out.len() as u32) < bytes {
        let chunk = read_at(f, sector * layout.size + layout.data, 2048)?;
        if chunk.is_empty() {
            break;
        }
        out.extend_from_slice(&chunk);
        sector += 1;
    }
    out.truncate(bytes as usize);
    Ok(out)
}

/// Optical disc images (.iso/.bin/.img, or the first track behind a .cue):
/// Sega CD ("SEGADISCSYSTEM"), PSP ("PSP GAME"), and PS1 vs PS2 via SYSTEM.CNF,
/// with the PlayStation serial when present.
pub fn sniff_disc(path: &Path) -> Sniff {
    let run = || -> std::io::Result<Sniff> {
        let mut f = File::open(path)?;
        let head = read_at(&mut f, 0, 16 * 2352 + 24 + 2048)?;
        for at in [0usize, 0x10] {
            if head.get(at..at + 14) == Some(b"SEGADISCSYSTEM") {
                return Ok(Sniff { platform: Some("segacd"), serial: None });
            }
        }
        let Some((layout, pvd)) = find_pvd(&head) else { return Ok(Sniff::default()) };
        match ascii_field(&head, pvd + 8, 32).as_deref() {
            Some("PSP GAME") => return Ok(Sniff { platform: Some("psp"), serial: None }),
            Some("PLAYSTATION") => {}
            _ => return Ok(Sniff::default()),
        }
        let root = head.get(pvd + 156..pvd + 190).unwrap_or(&[]);
        let (Some(root_lba), Some(root_len)) = (
            root.get(2..6).and_then(|b| b.try_into().ok()).map(u32::from_le_bytes),
            root.get(10..14).and_then(|b| b.try_into().ok()).map(u32::from_le_bytes),
        ) else {
            return Ok(Sniff::default());
        };
        let dir = read_sectors(&mut f, layout, root_lba, root_len)?;
        let Some((cnf_lba, cnf_len)) = find_dir_entry(&dir, "SYSTEM.CNF") else { return Ok(Sniff::default()) };
        let cnf = read_sectors(&mut f, layout, cnf_lba, cnf_len)?;
        Ok(match parse_system_cnf(&String::from_utf8_lossy(&cnf)) {
            Some((platform, serial)) => Sniff { platform: Some(platform), serial },
            None => Sniff::default(),
        })
    };
    run().unwrap_or_default()
}

/// The same checks as `sniff_disc`, reading sectors through `crate::disc`:
/// compressed images (.chd, .cso/.zso), and cue sheets whose track 1 starts
/// after a stored pregap.
pub fn sniff_sectors(path: &Path) -> Sniff {
    let run = || -> crate::error::Result<Sniff> {
        let mut disc = crate::disc::open(path)?;
        let mut read = |lba: u32, bytes: u32| -> crate::error::Result<Vec<u8>> {
            let mut out = Vec::new();
            let bytes = bytes.min(64 * 1024) as usize; // root dirs and SYSTEM.CNF are tiny
            let mut sector = lba;
            while out.len() < bytes {
                let chunk = disc.read(sector, 2048)?;
                if chunk.is_empty() {
                    break;
                }
                out.extend_from_slice(&chunk);
                sector += 1;
            }
            out.truncate(bytes);
            Ok(out)
        };
        if read(0, 16)?.starts_with(b"SEGADISCSYSTEM") {
            return Ok(Sniff { platform: Some("segacd"), serial: None });
        }
        let pvd = read(16, 2048)?;
        if pvd.get(1..6) != Some(b"CD001") {
            return Ok(Sniff::default());
        }
        match ascii_field(&pvd, 8, 32).as_deref() {
            Some("PSP GAME") => return Ok(Sniff { platform: Some("psp"), serial: None }),
            Some("PLAYSTATION") => {}
            _ => return Ok(Sniff::default()),
        }
        let (Some(root_lba), Some(root_len)) = (le32(&pvd, 158), le32(&pvd, 166)) else { return Ok(Sniff::default()) };
        let dir = read(root_lba, root_len)?;
        let Some((cnf_lba, cnf_len)) = find_dir_entry(&dir, "SYSTEM.CNF") else { return Ok(Sniff::default()) };
        let cnf = read(cnf_lba, cnf_len)?;
        Ok(match parse_system_cnf(&String::from_utf8_lossy(&cnf)) {
            Some((platform, serial)) => Sniff { platform: Some(platform), serial },
            None => Sniff::default(),
        })
    };
    run().unwrap_or_default()
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
pub(crate) mod tests {
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

    /// Builds a minimal ISO9660 image: PVD at sector 16, root directory at 18
    /// holding SYSTEM.CNF (if given) at sector 19.
    pub fn build_disc(layout: SectorLayout, system_id: &str, cnf: Option<&str>) -> Vec<u8> {
        let mut sectors = vec![vec![0u8; 2048]; 20];
        let pvd = &mut sectors[16];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        let mut sys = [b' '; 32];
        sys[..system_id.len()].copy_from_slice(system_id.as_bytes());
        pvd[8..40].copy_from_slice(&sys);
        pvd[156] = 34;
        pvd[158..162].copy_from_slice(&18u32.to_le_bytes());
        pvd[166..170].copy_from_slice(&2048u32.to_le_bytes());
        if let Some(text) = cnf {
            let name = b"SYSTEM.CNF;1";
            let dir = &mut sectors[18];
            // "." and ".." entries first, like real discs.
            dir[0] = 34;
            dir[32] = 1;
            dir[34] = 34;
            dir[34 + 32] = 1;
            let at = 68;
            let len = 33 + name.len() + (name.len() + 1) % 2;
            dir[at] = len as u8;
            dir[at + 2..at + 6].copy_from_slice(&19u32.to_le_bytes());
            dir[at + 10..at + 14].copy_from_slice(&(text.len() as u32).to_le_bytes());
            dir[at + 32] = name.len() as u8;
            dir[at + 33..at + 33 + name.len()].copy_from_slice(name);
            sectors[19][..text.len()].copy_from_slice(text.as_bytes());
        }
        let mut out = Vec::new();
        for s in sectors {
            let mut raw = vec![0u8; layout.size as usize];
            raw[layout.data as usize..layout.data as usize + 2048].copy_from_slice(&s);
            out.extend_from_slice(&raw);
        }
        out
    }

    fn sniff_bytes(bytes: &[u8]) -> Sniff {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("disc.iso");
        std::fs::write(&p, bytes).unwrap();
        sniff_disc(&p)
    }

    #[test]
    fn ps2_dvd_iso_is_not_mistaken_for_ps1() {
        let iso = build_disc(LAYOUTS[0], "PLAYSTATION", Some("BOOT2 = cdrom0:\\SLUS_203.12;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n"));
        assert_eq!(sniff_bytes(&iso), Sniff { platform: Some("ps2"), serial: Some("SLUS-20312".into()) });
    }

    #[test]
    fn ps1_raw_bin_track_with_serial() {
        let bin = build_disc(LAYOUTS[2], "PLAYSTATION", Some("BOOT = cdrom:\\SCUS_944.55;1\r\nTCB = 4\r\n"));
        assert_eq!(sniff_bytes(&bin), Sniff { platform: Some("ps1"), serial: Some("SCUS-94455".into()) });
        let mode1 = build_disc(LAYOUTS[1], "PLAYSTATION", Some("BOOT=cdrom:SLES_012.34;1"));
        assert_eq!(sniff_bytes(&mode1).platform, Some("ps1"));
    }

    #[test]
    fn playstation_without_system_cnf_stays_ambiguous() {
        let iso = build_disc(LAYOUTS[0], "PLAYSTATION", None);
        assert_eq!(sniff_bytes(&iso), Sniff::default());
    }

    #[test]
    fn psp_and_sega_cd_discs() {
        assert_eq!(sniff_bytes(&build_disc(LAYOUTS[0], "PSP GAME", None)).platform, Some("psp"));
        let mut sega = build_disc(LAYOUTS[2], "", None);
        sega[0x10..0x1E].copy_from_slice(b"SEGADISCSYSTEM");
        assert_eq!(sniff_bytes(&sega).platform, Some("segacd"));
        assert_eq!(sniff_bytes(&[0u8; 64]), Sniff::default());
    }

    #[test]
    fn system_cnf_parsing() {
        assert_eq!(parse_system_cnf("boot2 = cdrom0:\\SLPM_662.34;1"), Some(("ps2", Some("SLPM-66234".into()))));
        assert_eq!(parse_system_cnf("BOOT = cdrom:\\PSX.EXE;1"), Some(("ps1", None)), "homebrew boot file, no serial");
        assert_eq!(parse_system_cnf("VMODE = PAL"), None);
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
