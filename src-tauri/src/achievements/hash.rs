//! RetroAchievements game hashes, computed exactly as rcheevos does
//! (src/hash in github.com/RetroAchievements/rcheevos), so a dump is
//! recognized by the same MD5 the RetroAchievements database stores.
//!
//! Ported per console, quirks included (e.g. the ISO9660 lookup only reading
//! the first sector of a subdirectory), since matching depends on producing
//! byte-identical input to MD5:
//!
//! - SNES: whole file, minus a 512-byte copier header (size % 8 KB == 512).
//! - N64: whole file in big-endian (.z64) byte order.
//! - GBA: whole file.
//! - DS: header (0x160) + ARM9 code + ARM7 code + icon/title block (0xA00).
//! - GameCube / Wii: partition headers + apploader + main.dol segments
//!   (Wii: title metadata and the first 1024 encrypted clusters).
//! - Sega CD, Saturn: first 512 bytes of track 1.
//! - PS1 / PS2: boot executable's name + contents (from SYSTEM.CNF).
//! - PSP: PSP_GAME/PARAM.SFO + PSP_GAME/SYSDIR/EBOOT.BIN.
//! - PS3 (game folders): PARAM.SFO + USRDIR/EBOOT.BIN.
//!
//! Files over 64 MB are hashed up to 64 MB, as rcheevos does.
//!
//! Compressed images hash like the dumps they came from: CHD and CSO/ZSO are
//! read as the original sectors (`crate::disc`), RVZ/WIA/GCZ/CISO as the
//! original disc (`nod`).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::wbfs::Wbfs;
use crate::disc::{self, Sectors};
use crate::error::{Error, Result};

const MAX_BUFFER_SIZE: u64 = 64 * 1024 * 1024;

pub mod console {
    pub const N64: u32 = 2;
    pub const SNES: u32 = 3;
    pub const GBA: u32 = 5;
    pub const SEGA_CD: u32 = 9;
    pub const SATURN: u32 = 39;
    pub const PS1: u32 = 12;
    pub const GAMECUBE: u32 = 16;
    pub const NDS: u32 = 18;
    pub const WII: u32 = 19;
    pub const PS2: u32 = 21;
    pub const PSP: u32 = 41;
    pub const PS3: u32 = 82;
}

fn hex(digest: md5::Digest) -> String {
    format!("{digest:x}")
}

/// Reads up to `len` bytes at `offset` (fewer at end of file).
fn read_at<R: Read + Seek>(r: &mut R, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    r.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(len);
    r.take(len as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Like `read_at`, but always `len` bytes (zero-filled past the end), the way
/// rcheevos appends its fixed-size buffers.
fn read_exact_at<R: Read + Seek>(r: &mut R, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    let mut buf = read_at(r, offset, len)?;
    buf.resize(len, 0);
    Ok(buf)
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Dolphin's compressed formats (RVZ, WIA, GCZ, CISO, TGC) as the original
/// disc image. Wii partitions are stored decrypted in RVZ/WIA, so they're
/// encrypted again: the Wii hash reads encrypted clusters.
fn dolphin_disc(path: &Path) -> Result<nod::Disc> {
    let options = nod::OpenOptions { rebuild_encryption: true, validate_hashes: false };
    nod::Disc::new_with_options(path, &options).map_err(|e| Error::msg(format!("couldn't read the disc image: {e}")))
}

/// The hash of a game file for a RetroAchievements console, `None` when the
/// console or file format can't be hashed (3DS needs console keys).
pub fn hash_file(console: u32, path: &Path) -> Result<Option<String>> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if ext == "m3u" {
        // Playlists hash their first disc.
        let text = std::fs::read_to_string(path)?;
        let dir = path.parent().unwrap_or(Path::new("."));
        let Some(first) = crate::scanner::cue_m3u::m3u_entries(&text, dir).into_iter().next() else {
            return Ok(None);
        };
        return hash_file(console, &first);
    }
    Ok(match console {
        console::SNES => Some(snes(path)?),
        console::N64 => n64(path)?,
        console::GBA => Some(whole_file(path)?),
        console::NDS => Some(nds(path)?),
        console::GAMECUBE => match ext.as_str() {
            "iso" | "gcm" => gamecube(&mut File::open(path)?)?,
            "rvz" | "wia" | "gcz" | "ciso" | "tgc" => gamecube(&mut dolphin_disc(path)?)?,
            _ => None,
        },
        console::WII => match ext.as_str() {
            "iso" => wii(&mut File::open(path)?)?,
            "wbfs" => wii(&mut Wbfs::open(path)?)?,
            "rvz" | "wia" | "gcz" | "ciso" => wii(&mut dolphin_disc(path)?)?,
            _ => None,
        },
        // rcheevos hashes Saturn discs like Sega CD ones (rc_hash_sega_cd).
        console::SEGA_CD | console::SATURN => sega_cd(disc::open(path)?.as_mut())?,
        console::PS1 => psx(disc::open(path)?.as_mut())?,
        console::PS2 => ps2(disc::open(path)?.as_mut())?,
        console::PSP => psp(disc::open(path)?.as_mut())?,
        console::PS3 => ps3_folder(path)?,
        _ => None,
    })
}

// ---------------------------------------------------------------- cartridges

fn capped_file(path: &Path) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    File::open(path)?.take(MAX_BUFFER_SIZE).read_to_end(&mut buf)?;
    Ok(buf)
}

fn whole_file(path: &Path) -> Result<String> {
    Ok(hex(md5::compute(capped_file(path)?)))
}

fn snes(path: &Path) -> Result<String> {
    let buf = capped_file(path)?;
    let header = if buf.len() % 0x2000 == 512 { 512 } else { 0 };
    Ok(hex(md5::compute(&buf[header..])))
}

fn n64(path: &Path) -> Result<Option<String>> {
    let mut buf = capped_file(path)?;
    match buf.first() {
        Some(0x80) | Some(0xE8) | Some(0x22) => {} // z64 / ndd: as is
        Some(0x37) => buf.chunks_exact_mut(2).for_each(|c| c.swap(0, 1)), // v64: 16-bit swapped
        Some(0x40) => buf.chunks_exact_mut(4).for_each(|c| c.reverse()), // n64: little-endian words
        _ => return Ok(None),
    }
    Ok(Some(hex(md5::compute(&buf))))
}

fn nds(path: &Path) -> Result<String> {
    let mut f = File::open(path)?;
    let mut header = read_exact_at(&mut f, 0, 512)?;
    let mut offset = 0u64;
    // SuperCard header: skip it.
    if header[..4] == [0x2E, 0x00, 0x00, 0xEA] && header[0xB0..0xB4] == [0x44, 0x46, 0x96, 0x00] {
        offset = 512;
        header = read_exact_at(&mut f, offset, 512)?;
    }
    let (arm9_addr, arm9_size) = (le32(&header, 0x20), le32(&header, 0x2C));
    let (arm7_addr, arm7_size) = (le32(&header, 0x30), le32(&header, 0x3C));
    let icon_addr = le32(&header, 0x68);
    if u64::from(arm9_size) + u64::from(arm7_size) > 16 * 1024 * 1024 {
        return Err(Error::msg("not a DS ROM (code blocks over 16 MB)"));
    }
    let mut ctx = md5::Context::new();
    ctx.consume(&header[..0x160]);
    ctx.consume(read_exact_at(&mut f, u64::from(arm9_addr) + offset, arm9_size as usize)?);
    ctx.consume(read_exact_at(&mut f, u64::from(arm7_addr) + offset, arm7_size as usize)?);
    ctx.consume(read_exact_at(&mut f, u64::from(icon_addr) + offset, 0xA00)?);
    Ok(hex(ctx.finalize()))
}

// ---------------------------------------------------------------- Nintendo discs

/// Partition headers, apploader and the 18 main.dol segments
/// (rc_hash_nintendo_disc_partition). `shift` is 2 for Wii offsets.
fn nintendo_partition<R: Read + Seek>(ctx: &mut md5::Context, r: &mut R, part: u64, shift: u32) -> Result<()> {
    const BASE_HEADER_SIZE: u32 = 0x2440;
    const MAX_HEADER_SIZE: u32 = 1024 * 1024;
    let sizes = read_exact_at(r, part + u64::from(BASE_HEADER_SIZE) + 0x14, 8)?;
    let header_size = (BASE_HEADER_SIZE + 0x20)
        .wrapping_add(be32(&sizes, 0))
        .wrapping_add(be32(&sizes, 4))
        .min(MAX_HEADER_SIZE);
    let header = read_exact_at(r, part, header_size as usize)?;
    ctx.consume(&header);
    let dol_offset = u64::from(be32(&header, 0x420)) << shift;
    let table = read_exact_at(r, part + dol_offset, 0xD8)?;
    for ix in 0..18 {
        let seg_offset = u64::from(be32(&table, ix * 4)) << shift;
        let seg_size = u64::from(be32(&table, 0x90 + ix * 4)) << shift;
        if seg_size == 0 {
            continue;
        }
        r.seek(SeekFrom::Start(part + seg_offset))?;
        let mut remaining = seg_size;
        let mut chunk = vec![0u8; 1024 * 1024];
        while remaining > 0 {
            let n = remaining.min(chunk.len() as u64) as usize;
            let got = read_fill(r, &mut chunk[..n])?;
            chunk[got..n].fill(0);
            ctx.consume(&chunk[..n]);
            remaining -= n as u64;
        }
    }
    Ok(())
}

/// Reads as much of `buf` as the source has; returns the count.
fn read_fill<R: Read>(r: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match r.read(&mut buf[got..])? {
            0 => break,
            n => got += n,
        }
    }
    Ok(got)
}

fn gamecube<R: Read + Seek>(r: &mut R) -> Result<Option<String>> {
    if read_exact_at(r, 0x1C, 4)? != [0xC2, 0x33, 0x9F, 0x3D] {
        return Ok(None);
    }
    let mut ctx = md5::Context::new();
    nintendo_partition(&mut ctx, r, 0, 0)?;
    Ok(Some(hex(ctx.finalize())))
}

fn wii<R: Read + Seek>(r: &mut R) -> Result<Option<String>> {
    const MAIN_HEADER_SIZE: usize = 0x80;
    const REGION_CODE_ADDRESS: u64 = 0x4E000;
    const CLUSTER_SIZE: usize = 0x7C00;
    const MAX_CLUSTER_COUNT: u64 = 1024;

    if read_exact_at(r, 0x18, 4)? != [0x5D, 0x1C, 0x9E, 0xA3] {
        return Ok(None); // (WiiWare .wad isn't in the library's formats)
    }
    let encrypted = read_exact_at(r, 0x61, 1)?[0] == 0;
    let mut ctx = md5::Context::new();
    ctx.consume(read_exact_at(r, 0, MAIN_HEADER_SIZE)?);
    ctx.consume(read_exact_at(r, REGION_CODE_ADDRESS, 4)?);

    let info = read_exact_at(r, 0x40000, 32)?;
    let mut partitions: Vec<(u32, u32)> = Vec::new();
    for group in 0..4 {
        let count = be32(&info, group * 8);
        let table = u64::from(be32(&info, group * 8 + 4)) << 2;
        if count > 0 {
            let entries = read_exact_at(r, table, count as usize * 8)?;
            for e in entries.chunks_exact(8) {
                partitions.push((be32(e, 0), be32(e, 4)));
            }
        }
    }
    if partitions.is_empty() {
        return Err(Error::msg("no Wii partitions"));
    }
    for (offset, kind) in partitions {
        if kind == 1 {
            continue; // update partition
        }
        let base = u64::from(offset) << 2;
        let tmd = read_exact_at(r, base + 0x2A4, 8)?;
        let tmd_size = (be32(&tmd, 0) as usize).min(CLUSTER_SIZE);
        let tmd_offset = u64::from(be32(&tmd, 4)) << 2;
        ctx.consume(read_exact_at(r, base + tmd_offset, tmd_size)?);

        let part = read_exact_at(r, base + 0x2B8, 8)?;
        let part_offset = u64::from(be32(&part, 0)) << 2;
        let part_size = u64::from(be32(&part, 4)) << 2;
        if encrypted {
            let clusters = (part_size / 0x8000).min(MAX_CLUSTER_COUNT);
            for ix in 0..clusters {
                ctx.consume(read_exact_at(r, part_offset + ix * 0x8000 + 0x400, CLUSTER_SIZE)?);
            }
        } else {
            // rcheevos passes the partition offset as 32 bits here.
            nintendo_partition(&mut ctx, r, part_offset & 0xFFFF_FFFF, 2)?;
        }
    }
    Ok(Some(hex(ctx.finalize())))
}

// ---------------------------------------------------------------- CDs
//
// Track 1 of a disc image (a .cue's first track, a .bin/.iso, or a CHD/CSO),
// read as 2048-byte sectors of user data through `crate::disc`.

/// rc_cd_find_file_sector: the sector and size of `path` ("DIR\\FILE").
fn find_file_sector(cd: &mut dyn Sectors, path: &str) -> Result<Option<(u32, u32)>> {
    let path = path.strip_prefix('\\').unwrap_or(path);
    let (mut sector, mut num_sectors, name) = match path.rfind('\\') {
        Some(slash) => {
            let Some((dir, _)) = find_file_sector(cd, &path[..slash])? else { return Ok(None) };
            (dir, 0u32, &path[slash + 1..])
        }
        None => {
            let pvd = cd.read(16, 256)?;
            if pvd.len() < 256 {
                return Ok(None);
            }
            let root = u32::from(pvd[158]) | u32::from(pvd[159]) << 8 | u32::from(pvd[160]) << 16;
            let block = u32::from(pvd[128]) | u32::from(pvd[129]) << 8;
            let count = if block == 0 { 1 } else { le32(&pvd, 166) / block };
            (root, count, path)
        }
    };
    let name_bytes = name.as_bytes();
    let len = name_bytes.len();
    let mut buf = cd.read(sector, 2048)?;
    if buf.is_empty() {
        return Ok(None);
    }
    buf.resize(2048, 0);
    let mut at = 0usize;
    loop {
        if at >= buf.len() || buf[at] == 0 {
            // End of this directory block: keep going if the root spans more sectors.
            if num_sectors > 1 {
                num_sectors -= 1;
                sector += 1;
                let next = cd.read(sector, 2048)?;
                if !next.is_empty() {
                    buf = next;
                    buf.resize(2048, 0);
                    at = 0;
                    continue;
                }
            }
            return Ok(None);
        }
        let byte = |i: usize| buf.get(at + i).copied().unwrap_or(0);
        let rec_name_len = byte(32) as usize;
        let name_matches = (rec_name_len == len || byte(33 + len) == b';')
            && (0..len).all(|i| byte(33 + i).eq_ignore_ascii_case(&name_bytes[i]));
        if name_matches {
            let found = u32::from(byte(2)) | u32::from(byte(3)) << 8 | u32::from(byte(4)) << 16;
            let size = u32::from_le_bytes([byte(10), byte(11), byte(12), byte(13)]);
            return Ok(Some((found, size)));
        }
        at += buf[at] as usize;
    }
}

/// rc_hash_cd_file: appends `size` bytes of a file starting at `sector`.
fn hash_cd_file(ctx: &mut md5::Context, cd: &mut dyn Sectors, mut sector: u32, size: u32) -> Result<()> {
    let mut first = cd.read(sector, 2048)?;
    if first.len() < 2048 {
        return Err(Error::msg("could not read file on disc"));
    }
    let mut size = u64::from(size).min(MAX_BUFFER_SIZE);
    first.truncate(size.min(2048) as usize);
    let mut chunk = first;
    loop {
        ctx.consume(&chunk);
        if size <= chunk.len() as u64 {
            break;
        }
        size -= chunk.len() as u64;
        sector += 1;
        chunk = cd.read(sector, size.min(2048) as usize)?;
        if chunk.is_empty() {
            break;
        }
    }
    Ok(())
}

/// The boot executable named in SYSTEM.CNF (`key` "BOOT" / "BOOT2").
fn playstation_executable(cd: &mut dyn Sectors, key: &str, prefix: &str) -> Result<Option<(String, u32, u32)>> {
    let Some((cnf, _)) = find_file_sector(cd, "SYSTEM.CNF")? else { return Ok(None) };
    let text = cd.read(cnf, 2047)?;
    let text = &text[..text.iter().position(|&b| b == 0).unwrap_or(text.len())];
    for line in text.split(|&b| b == b'\n') {
        let Some(rest) = line.strip_prefix(key.as_bytes()) else { continue };
        let rest = trim_start(rest);
        let Some(rest) = rest.strip_prefix(b"=") else { continue };
        let mut rest = trim_start(rest);
        if let Some(r) = rest.strip_prefix(prefix.as_bytes()) {
            rest = r;
        }
        while let Some(r) = rest.strip_prefix(b"\\") {
            rest = r;
        }
        let end = rest.iter().position(|&b| b.is_ascii_whitespace() || b == b';').unwrap_or(rest.len());
        let name = String::from_utf8_lossy(&rest[..end.min(63)]).into_owned();
        return Ok(find_file_sector(cd, &name)?.map(|(sector, size)| (name, sector, size)));
    }
    Ok(None)
}

fn trim_start(b: &[u8]) -> &[u8] {
    let n = b.iter().take_while(|c| c.is_ascii_whitespace()).count();
    &b[n..]
}

fn psx(cd: &mut dyn Sectors) -> Result<Option<String>> {
    let found = match playstation_executable(cd, "BOOT", "cdrom:")? {
        Some(f) => Some(f),
        None => find_file_sector(cd, "PSX.EXE")?.map(|(s, z)| ("PSX.EXE".to_string(), s, z)),
    };
    let Some((name, sector, mut size)) = found else { return Ok(None) };
    let head = cd.read(sector, 32)?;
    if head.len() < 32 {
        return Err(Error::msg("could not read the PS1 executable"));
    }
    if &head[..7] == b"PS-X EX" {
        // Size from the PS-X EXE header (which it doesn't count) + the header.
        size = le32(&head, 28).wrapping_add(2048);
    }
    let mut ctx = md5::Context::new();
    ctx.consume(name.as_bytes());
    hash_cd_file(&mut ctx, cd, sector, size)?;
    Ok(Some(hex(ctx.finalize())))
}

fn ps2(cd: &mut dyn Sectors) -> Result<Option<String>> {
    let Some((name, sector, size)) = playstation_executable(cd, "BOOT2", "cdrom0:")? else { return Ok(None) };
    if cd.read(sector, 4)?.len() < 4 {
        return Err(Error::msg("could not read the PS2 executable"));
    }
    let mut ctx = md5::Context::new();
    ctx.consume(name.as_bytes());
    hash_cd_file(&mut ctx, cd, sector, size)?;
    Ok(Some(hex(ctx.finalize())))
}

fn psp(cd: &mut dyn Sectors) -> Result<Option<String>> {
    let Some((sfo, sfo_size)) = find_file_sector(cd, "PSP_GAME\\PARAM.SFO")? else { return Ok(None) };
    let mut ctx = md5::Context::new();
    hash_cd_file(&mut ctx, cd, sfo, sfo_size)?;
    let Some((eboot, eboot_size)) = find_file_sector(cd, "PSP_GAME\\SYSDIR\\EBOOT.BIN")? else {
        return Err(Error::msg("PSP disc without SYSDIR/EBOOT.BIN"));
    };
    hash_cd_file(&mut ctx, cd, eboot, eboot_size)?;
    Ok(Some(hex(ctx.finalize())))
}

fn sega_cd(cd: &mut dyn Sectors) -> Result<Option<String>> {
    let head = cd.read(0, 512)?;
    if head.len() < 512 || (&head[..16] != b"SEGADISCSYSTEM  " && &head[..16] != b"SEGA SEGASATURN ") {
        return Ok(None);
    }
    Ok(Some(hex(md5::compute(&head))))
}

// ---------------------------------------------------------------- PS3

/// Folder games: PARAM.SFO + EBOOT.BIN, with PARAM.SFO found the same way
/// rcheevos does (`PS3_GAME/USRDIR/EBOOT.BIN` -> `PS3_GAME/PARAM.SFO`,
/// installs `TITLE/USRDIR/EBOOT.BIN` -> `TITLE/PARAM.SFO`).
fn ps3_folder(eboot: &Path) -> Result<Option<String>> {
    let s = eboot.to_string_lossy();
    let dir_len = if let Some(i) = s.find("PS3_GAME/USRDIR/").or_else(|| s.find("PS3_GAME\\USRDIR\\")) {
        i + 9
    } else if let Some(i) = s.find("USRDIR/").or_else(|| s.find("USRDIR\\")) {
        i
    } else {
        s.len() - eboot.file_name().map_or(0, |n| n.len())
    };
    let sfo = PathBuf::from(format!("{}PARAM.SFO", &s[..dir_len]));
    let Ok(sfo_bytes) = std::fs::read(&sfo) else { return Ok(None) };
    let mut ctx = md5::Context::new();
    ctx.consume(&sfo_bytes);
    let mut f = File::open(eboot)?;
    let mut chunk = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        ctx.consume(&chunk[..n]);
    }
    Ok(Some(hex(ctx.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::File::create(&p).unwrap().write_all(data).unwrap();
        p
    }

    fn md5hex(data: &[u8]) -> String {
        hex(md5::compute(data))
    }

    /// Hashes real files: RA_PROBE_LIST names a file of `console|path` lines.
    #[test]
    #[ignore]
    fn real_library_hashes() {
        let list = std::fs::read_to_string(std::env::var("RA_PROBE_LIST").unwrap()).unwrap();
        for line in list.lines().filter(|l| !l.trim().is_empty()) {
            let (c, p) = line.split_once('|').unwrap();
            let t = std::time::Instant::now();
            let r = hash_file(c.trim().parse().unwrap(), Path::new(p.trim()));
            println!("{:>4} ms  {:?}  {}", t.elapsed().as_millis(), r, p.trim());
        }
    }

    #[test]
    fn cartridges() {
        let t = tempfile::tempdir().unwrap();
        let rom: Vec<u8> = (0..0x4000u32).map(|i| (i * 7) as u8).collect();
        // SNES: a 512-byte copier header is ignored.
        let mut headered = vec![0xAAu8; 512];
        headered.extend_from_slice(&rom);
        assert_eq!(hash_file(console::SNES, &write(t.path(), "a.smc", &headered)).unwrap(), Some(md5hex(&rom)));
        assert_eq!(hash_file(console::SNES, &write(t.path(), "b.sfc", &rom)).unwrap(), Some(md5hex(&rom)));
        // GBA: whole file.
        assert_eq!(hash_file(console::GBA, &write(t.path(), "c.gba", &rom)).unwrap(), Some(md5hex(&rom)));
        // N64: v64/n64 byte orders hash as their z64 equivalent.
        let mut z64 = rom.clone();
        z64[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]); // the real header magic
        let v64: Vec<u8> = z64.chunks(2).flat_map(|c| [c[1], c[0]]).collect();
        let n64le: Vec<u8> = z64.chunks(4).flat_map(|c| [c[3], c[2], c[1], c[0]]).collect();
        let want = Some(md5hex(&z64));
        assert_eq!(hash_file(console::N64, &write(t.path(), "d.z64", &z64)).unwrap(), want);
        assert_eq!(hash_file(console::N64, &write(t.path(), "e.v64", &v64)).unwrap(), want);
        assert_eq!(hash_file(console::N64, &write(t.path(), "f.n64", &n64le)).unwrap(), want);
    }

    #[test]
    fn nintendo_ds_hashes_header_code_and_icon() {
        let t = tempfile::tempdir().unwrap();
        let mut rom = vec![0u8; 0x8000];
        rom[0x20..0x24].copy_from_slice(&0x4000u32.to_le_bytes()); // arm9 at
        rom[0x2C..0x30].copy_from_slice(&0x100u32.to_le_bytes()); // arm9 size
        rom[0x30..0x34].copy_from_slice(&0x5000u32.to_le_bytes()); // arm7 at
        rom[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes()); // arm7 size
        rom[0x68..0x6C].copy_from_slice(&0x6000u32.to_le_bytes()); // icon at
        for (i, b) in rom.iter_mut().enumerate().skip(0x200) {
            *b = (i % 251) as u8;
        }
        let mut want = Vec::new();
        want.extend_from_slice(&rom[..0x160]);
        want.extend_from_slice(&rom[0x4000..0x4100]);
        want.extend_from_slice(&rom[0x5000..0x5080]);
        want.extend_from_slice(&rom[0x6000..0x6A00]);
        assert_eq!(hash_file(console::NDS, &write(t.path(), "g.nds", &rom)).unwrap(), Some(md5hex(&want)));
    }

    /// A tiny ISO9660 image: root dir with SYSTEM.CNF and an executable.
    fn iso(files: &[(&str, &[u8])], raw_mode2: bool) -> Vec<u8> {
        let mut sectors: Vec<Vec<u8>> = vec![vec![0u8; 2048]; 20];
        let mut root = Vec::new();
        let mut next = 20u32;
        for (name, data) in files {
            let lba = next;
            next += (data.len() as u32).div_ceil(2048).max(1);
            let ident = format!("{name};1");
            let len = 33 + ident.len() + (ident.len() + 1) % 2;
            let mut rec = vec![0u8; len];
            rec[0] = len as u8;
            rec[2..6].copy_from_slice(&lba.to_le_bytes());
            rec[10..14].copy_from_slice(&(data.len() as u32).to_le_bytes());
            rec[32] = ident.len() as u8;
            rec[33..33 + ident.len()].copy_from_slice(ident.as_bytes());
            root.extend_from_slice(&rec);
            for (i, chunk) in data.chunks(2048).enumerate() {
                let mut s = chunk.to_vec();
                s.resize(2048, 0);
                let at = (lba as usize) + i;
                if sectors.len() <= at {
                    sectors.resize(at + 1, vec![0u8; 2048]);
                }
                sectors[at] = s;
            }
        }
        root.resize(2048, 0);
        sectors[18] = root;
        let mut pvd = vec![0u8; 2048];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[128..130].copy_from_slice(&2048u16.to_le_bytes());
        pvd[156 + 2..156 + 6].copy_from_slice(&18u32.to_le_bytes());
        pvd[156 + 10..156 + 14].copy_from_slice(&2048u32.to_le_bytes());
        sectors[16] = pvd;
        if !raw_mode2 {
            return sectors.concat();
        }
        let mut out = Vec::new();
        for s in sectors {
            let mut raw = vec![0u8; 2352];
            raw[1..11].fill(0xFF);
            raw[15] = 2;
            raw[24..24 + 2048].copy_from_slice(&s);
            out.extend_from_slice(&raw);
        }
        out
    }

    #[test]
    fn playstation_hashes_the_boot_executable_name_and_contents() {
        let t = tempfile::tempdir().unwrap();
        // PS1: size comes from the PS-X EXE header (+ 2048 for the header itself).
        let mut exe = vec![0u8; 6000];
        exe[..8].copy_from_slice(b"PS-X EXE");
        exe[28..32].copy_from_slice(&3000u32.to_le_bytes());
        let cnf = b"BOOT = cdrom:\\SLUS_007.08;1\r\nTCB = 4\r\n";
        let img = iso(&[("SYSTEM.CNF", cnf), ("SLUS_007.08", &exe)], true);
        let bin = write(t.path(), "ff7.bin", &img);
        write(t.path(), "ff7.cue", b"FILE \"ff7.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n");
        let mut want = b"SLUS_007.08".to_vec();
        want.extend_from_slice(&exe[..5048]);
        assert_eq!(hash_file(console::PS1, &t.path().join("ff7.cue")).unwrap(), Some(md5hex(&want)));
        assert_eq!(hash_file(console::PS1, &bin).unwrap(), Some(md5hex(&want)), "bin without cue: sync detection");
        // The same disc with a 2-second pregap stored ahead of track 1 hashes the same.
        let mut with_pregap = vec![0u8; 150 * 2352];
        with_pregap.extend_from_slice(&img);
        write(t.path(), "pregap.bin", &with_pregap);
        let cue = write(
            t.path(),
            "pregap.cue",
            b"FILE \"pregap.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 00 00:00:00\r\n    INDEX 01 00:02:00\r\n",
        );
        assert_eq!(hash_file(console::PS1, &cue).unwrap(), Some(md5hex(&want)), "stored pregap skipped");

        // PS2: BOOT2 (cdrom0:), size from the directory record.
        let elf = [&b"\x7fELF"[..], &[7u8; 3000]].concat();
        let cnf2 = b"BOOT2 = cdrom0:\\SLUS_209.46;1\nVER = 1.00\n";
        let iso2 = write(t.path(), "game.iso", &iso(&[("SLUS_209.46", &elf), ("SYSTEM.CNF", cnf2)], false));
        let mut want2 = b"SLUS_209.46".to_vec();
        want2.extend_from_slice(&elf);
        assert_eq!(hash_file(console::PS2, &iso2).unwrap(), Some(md5hex(&want2)));
    }

    #[test]
    fn sega_cd_hashes_the_first_512_bytes() {
        let t = tempfile::tempdir().unwrap();
        let mut sector0 = vec![0u8; 2352 * 2];
        sector0[1..11].fill(0xFF);
        sector0[15] = 1;
        sector0[16..32].copy_from_slice(b"SEGADISCSYSTEM  ");
        let bin = write(t.path(), "snatcher.bin", &sector0);
        assert_eq!(hash_file(console::SEGA_CD, &bin).unwrap(), Some(md5hex(&sector0[16..16 + 512])));
        let not = write(t.path(), "other.bin", &vec![0u8; 4096]);
        assert_eq!(hash_file(console::SEGA_CD, &not).unwrap(), None);
        // Saturn discs hash the same way.
        let mut saturn = sector0.clone();
        saturn[16..32].copy_from_slice(b"SEGA SEGASATURN ");
        let bin = write(t.path(), "saturn.bin", &saturn);
        assert_eq!(hash_file(console::SATURN, &bin).unwrap(), Some(md5hex(&saturn[16..16 + 512])));
    }

    #[test]
    fn ps3_folder_hashes_param_sfo_then_eboot() {
        let t = tempfile::tempdir().unwrap();
        let game = t.path().join("Game (USA)").join("PS3_GAME");
        std::fs::create_dir_all(game.join("USRDIR")).unwrap();
        write(&game, "PARAM.SFO", b"SFO-DATA");
        let eboot = write(&game.join("USRDIR"), "EBOOT.BIN", b"EBOOT-DATA");
        assert_eq!(hash_file(console::PS3, &eboot).unwrap(), Some(md5hex(b"SFO-DATAEBOOT-DATA")));
        // Installed (PKG) layout: TITLE/USRDIR/EBOOT.BIN -> TITLE/PARAM.SFO.
        let inst = t.path().join("NPUB30024");
        std::fs::create_dir_all(inst.join("USRDIR")).unwrap();
        write(&inst, "PARAM.SFO", b"S2");
        let e2 = write(&inst.join("USRDIR"), "EBOOT.BIN", b"E2");
        assert_eq!(hash_file(console::PS3, &e2).unwrap(), Some(md5hex(b"S2E2")));
    }

    #[test]
    fn gamecube_hashes_apploader_and_dol_segments() {
        let mut disc = vec![0u8; 0x40000];
        disc[0x1C..0x20].copy_from_slice(&[0xC2, 0x33, 0x9F, 0x3D]);
        disc[0x2454..0x2458].copy_from_slice(&0x100u32.to_be_bytes()); // apploader body
        disc[0x2458..0x245C].copy_from_slice(&0x20u32.to_be_bytes()); // trailer
        disc[0x420..0x424].copy_from_slice(&0x30000u32.to_be_bytes()); // dol offset
        disc[0x30000..0x30004].copy_from_slice(&0x31000u32.to_be_bytes()); // segment 0 at
        disc[0x30090..0x30094].copy_from_slice(&0x200u32.to_be_bytes()); // segment 0 size
        for (i, b) in disc.iter_mut().enumerate().skip(0x31000).take(0x200) {
            *b = i as u8;
        }
        let header_size = 0x2440 + 0x20 + 0x100 + 0x20;
        let mut want = disc[..header_size].to_vec();
        want.extend_from_slice(&disc[0x31000..0x31200]);
        let h = gamecube(&mut std::io::Cursor::new(disc)).unwrap();
        assert_eq!(h, Some(md5hex(&want)));
    }
}
