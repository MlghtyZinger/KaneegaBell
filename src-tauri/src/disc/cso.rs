//! CSO / ZSO compressed ISOs (maxcso; read by PPSSPP and PCSX2).
//!
//! A 24-byte header (`CISO` or `ZISO`, image size, block size, version,
//! index alignment), then one 32-bit index entry per block plus one for the
//! end; each entry's low 31 bits, shifted by the alignment, are where the
//! block's data starts.
//! - CSO v1: raw deflate; the top bit marks a block stored uncompressed.
//! - CSO v2: a block as big as the block size is stored uncompressed;
//!   otherwise the top bit picks LZ4 over deflate.
//! - ZSO: LZ4; the top bit marks a block stored uncompressed.
//!
//! Compressed blocks may be followed by alignment padding, so decoding stops
//! once a block is full (as PPSSPP and PCSX2 do).

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use super::{read_at, Sectors};
use crate::error::{Error, Result};

const HEADER_SIZE: u64 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    CsoV1,
    CsoV2,
    Zso,
}

pub struct Cso {
    file: BufReader<File>,
    kind: Kind,
    total_bytes: u64,
    block_size: u64,
    align: u32,
    index: Vec<u32>,
    block: Vec<u8>,
    loaded: Option<u64>,
}

impl Cso {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = BufReader::new(File::open(path)?);
        let head = read_at(&mut file, 0, HEADER_SIZE as usize)?;
        if head.len() < HEADER_SIZE as usize {
            return Err(Error::msg("not a CSO/ZSO file"));
        }
        let version = head[20];
        let kind = match (&head[..4], version) {
            (b"CISO", 0 | 1) => Kind::CsoV1,
            (b"CISO", 2) => Kind::CsoV2,
            (b"ZISO", _) => Kind::Zso,
            _ => return Err(Error::msg("not a CSO/ZSO file (or an unsupported version)")),
        };
        let total_bytes = u64::from_le_bytes(head[8..16].try_into().unwrap());
        let block_size = u64::from(u32::from_le_bytes(head[16..20].try_into().unwrap()));
        if !(2048..=1 << 24).contains(&block_size) || !block_size.is_power_of_two() {
            return Err(Error::msg("CSO/ZSO file with an odd block size"));
        }
        let blocks = total_bytes.div_ceil(block_size);
        if blocks > 1 << 24 {
            return Err(Error::msg("CSO/ZSO file too large"));
        }
        let raw = read_at(&mut file, HEADER_SIZE, (blocks as usize + 1) * 4)?;
        if raw.len() != (blocks as usize + 1) * 4 {
            return Err(Error::msg("CSO/ZSO index is cut short"));
        }
        let align = u32::from(head[21]);
        if align > 31 {
            return Err(Error::msg("CSO/ZSO file with an odd index alignment"));
        }
        let index = raw.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        Ok(Self {
            file,
            kind,
            total_bytes,
            block_size,
            align,
            index,
            block: vec![0; block_size as usize],
            loaded: None,
        })
    }

    fn load(&mut self, n: u64) -> Result<()> {
        if self.loaded == Some(n) {
            return Ok(());
        }
        self.loaded = None;
        let (entry, next) = (self.index[n as usize], self.index[n as usize + 1]);
        let start = u64::from(entry & 0x7FFF_FFFF) << self.align;
        let end = u64::from(next & 0x7FFF_FFFF) << self.align;
        // A block never stores much more than its own size (plus alignment padding).
        if end.saturating_sub(start) > 2 * self.block_size + (1 << self.align) {
            return Err(Error::msg("damaged CSO/ZSO index"));
        }
        let stored = read_at(&mut self.file, start, end.saturating_sub(start) as usize)?;
        let top = entry & 0x8000_0000 != 0;
        let plain = match self.kind {
            Kind::CsoV1 | Kind::Zso => top,
            Kind::CsoV2 => stored.len() as u64 >= self.block_size,
        };
        let lz4 = match self.kind {
            Kind::Zso => true,
            Kind::CsoV2 => top,
            Kind::CsoV1 => false,
        };
        self.block.fill(0);
        if plain {
            let n = stored.len().min(self.block.len());
            self.block[..n].copy_from_slice(&stored[..n]);
        } else if lz4 {
            lz4_block(&stored, &mut self.block)?;
        } else {
            let mut d = flate2::read::DeflateDecoder::new(&stored[..]);
            let mut got = 0;
            while got < self.block.len() {
                match d.read(&mut self.block[got..])? {
                    0 => break,
                    n => got += n,
                }
            }
        }
        self.loaded = Some(n);
        Ok(())
    }
}

impl Sectors for Cso {
    fn read(&mut self, lba: u32, len: usize) -> Result<Vec<u8>> {
        let at = u64::from(lba) * 2048;
        if at >= self.total_bytes {
            return Ok(Vec::new());
        }
        let len = (len.min(2048) as u64).min(self.total_bytes - at) as usize;
        let n = at / self.block_size;
        self.load(n)?;
        let start = (at % self.block_size) as usize;
        Ok(self.block[start..start + len].to_vec())
    }
}

/// Decodes one LZ4 block into `out`, stopping once it's full.
fn lz4_block(src: &[u8], out: &mut [u8]) -> Result<()> {
    let bad = || Error::msg("damaged LZ4 block in a ZSO/CSO file");
    let (mut i, mut o) = (0usize, 0usize);
    let length = |i: &mut usize, mut n: usize| -> Result<usize> {
        if n == 15 {
            loop {
                let b = *src.get(*i).ok_or_else(bad)?;
                *i += 1;
                n += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        Ok(n)
    };
    while o < out.len() && i < src.len() {
        let token = src[i];
        i += 1;
        let literals = length(&mut i, (token >> 4) as usize)?;
        let take = literals.min(out.len() - o);
        let lit = src.get(i..i + take).ok_or_else(bad)?;
        out[o..o + take].copy_from_slice(lit);
        i += literals;
        o += take;
        if o >= out.len() || i >= src.len() {
            break; // the last sequence is literals only
        }
        let offset = u16::from_le_bytes([src[i], *src.get(i + 1).ok_or_else(bad)?]) as usize;
        i += 2;
        if offset == 0 || offset > o {
            return Err(bad());
        }
        let len = length(&mut i, (token & 15) as usize)? + 4;
        for _ in 0..len.min(out.len() - o) {
            out[o] = out[o - offset]; // byte by byte: matches may overlap
            o += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A CSO v1 image of `data` (2048-byte blocks), deflate-compressing the
    /// even blocks and storing the odd ones plain.
    fn cso(data: &[u8]) -> Vec<u8> {
        let blocks = data.len().div_ceil(2048);
        let mut body = Vec::new();
        let mut index = Vec::new();
        let base = HEADER_SIZE as usize + (blocks + 1) * 4;
        for (n, chunk) in data.chunks(2048).enumerate() {
            let at = (base + body.len()) as u32;
            if n % 2 == 0 {
                let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(chunk).unwrap();
                body.extend(e.finish().unwrap());
                index.push(at);
            } else {
                body.extend_from_slice(chunk);
                index.push(at | 0x8000_0000);
            }
        }
        index.push((base + body.len()) as u32);
        let mut out = b"CISO".to_vec();
        out.extend(24u32.to_le_bytes());
        out.extend((data.len() as u64).to_le_bytes());
        out.extend(2048u32.to_le_bytes());
        out.extend([1, 0, 0, 0]);
        for e in index {
            out.extend(e.to_le_bytes());
        }
        out.extend(body);
        out
    }

    #[test]
    fn cso_blocks_read_back_as_the_original_sectors() {
        let data: Vec<u8> = (0..5 * 2048u32).map(|i| (i % 251) as u8 ^ (i / 2048) as u8).collect();
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("a.cso");
        std::fs::write(&p, cso(&data)).unwrap();
        let mut d = Cso::open(&p).unwrap();
        for lba in 0..5u32 {
            let at = lba as usize * 2048;
            assert_eq!(d.read(lba, 2048).unwrap(), data[at..at + 2048], "sector {lba}");
        }
        assert_eq!(d.read(1, 10).unwrap(), data[2048..2058]);
        assert!(d.read(5, 2048).unwrap().is_empty(), "past the end");

        // A damaged index claiming a ~2 GB block is refused, not read into memory.
        let mut bad = cso(&data);
        bad[24 + 4..24 + 8].copy_from_slice(&0x7FFF_FFF0u32.to_le_bytes());
        std::fs::write(&p, bad).unwrap();
        assert!(Cso::open(&p).unwrap().read(0, 2048).is_err());
    }

    #[test]
    fn lz4_literals_overlapping_matches_and_padding() {
        // "abc" literals, then a 9-byte match at offset 3 ("abcabcabc"), then
        // the literals "XY" as the last sequence, then padding.
        let src = [0x35, b'a', b'b', b'c', 3, 0, 0x20, b'X', b'Y', 0, 0, 0];
        let mut out = [0u8; 14];
        lz4_block(&src, &mut out).unwrap();
        assert_eq!(&out, b"abcabcabcabcXY");
        // An offset pointing before the start is refused.
        assert!(lz4_block(&[0x10, b'a', 5, 0], &mut [0u8; 8]).is_err());
    }
}
