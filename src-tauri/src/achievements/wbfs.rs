//! Reads a Wii disc stored in a .wbfs file (as written by Wii Backup Manager
//! and similar), presenting it as the plain disc image it came from.
//!
//! Layout (libwbfs): a header sector holds "WBFS", the number of hard-disk
//! sectors, log2 of the hard-disk sector size (usually 512) and log2 of the
//! WBFS block size (usually 2 MB). The disc's info record starts at the next
//! hard-disk sector: a 0x100-byte copy of the disc header, then a table of
//! 16-bit big-endian block numbers, one per WBFS-block-sized slice of the
//! disc. Block 0 means "not stored" (scrubbed): it reads as zeros.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, Result};

/// Wii sectors on a dual-layer disc (the table's length is sized for this).
const WII_SECTORS_PER_DISC: u64 = 143_432 * 2;
const WII_SECTOR_SIZE: u64 = 0x8000;

pub struct Wbfs {
    file: File,
    block_size: u64,
    /// File block holding each disc block (0 = not stored).
    table: Vec<u16>,
    pos: u64,
}

impl Wbfs {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut head = [0u8; 12];
        file.read_exact(&mut head)?;
        if &head[..4] != b"WBFS" {
            return Err(Error::msg("not a WBFS file"));
        }
        let hd_sector = 1u64 << head[8];
        let block_size = 1u64 << head[9];
        if !(9..=16).contains(&head[8]) || !(15..=30).contains(&head[9]) {
            return Err(Error::msg("unexpected WBFS sector sizes"));
        }
        let blocks = WII_SECTORS_PER_DISC * WII_SECTOR_SIZE / block_size;
        file.seek(SeekFrom::Start(hd_sector + 0x100))?;
        let mut raw = vec![0u8; blocks as usize * 2];
        file.read_exact(&mut raw)?;
        let table = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        Ok(Self { file, block_size, table, pos: 0 })
    }
}

impl Read for Wbfs {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let block = (self.pos / self.block_size) as usize;
        let Some(&stored) = self.table.get(block) else { return Ok(0) };
        let within = self.pos % self.block_size;
        let n = (buf.len() as u64).min(self.block_size - within) as usize;
        if stored == 0 {
            buf[..n].fill(0);
        } else {
            self.file.seek(SeekFrom::Start(u64::from(stored) * self.block_size + within))?;
            let mut got = 0;
            while got < n {
                match self.file.read(&mut buf[got..n])? {
                    0 => break,
                    k => got += k,
                }
            }
            buf[got..n].fill(0);
        }
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for Wbfs {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let end = self.table.len() as u64 * self.block_size;
        self.pos = match to {
            SeekFrom::Start(p) => p,
            SeekFrom::Current(d) => self.pos.saturating_add_signed(d),
            SeekFrom::End(d) => end.saturating_add_signed(d),
        };
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn maps_disc_offsets_through_the_block_table() {
        // 512-byte sectors (2^9), 32 KB blocks (2^15) to keep the file small.
        let block = 1u64 << 15;
        let blocks = WII_SECTORS_PER_DISC * WII_SECTOR_SIZE / block;
        let mut img = vec![0u8; 512 + 0x100 + blocks as usize * 2];
        img[..4].copy_from_slice(b"WBFS");
        img[8] = 9;
        img[9] = 15;
        // Disc block 0 -> file block 20, disc block 2 -> file block 21; block 1
        // scrubbed. (With blocks this small the table spans the first ~18.)
        let table_at = 512 + 0x100;
        img[table_at..table_at + 2].copy_from_slice(&20u16.to_be_bytes());
        img[table_at + 4..table_at + 6].copy_from_slice(&21u16.to_be_bytes());
        img.resize(22 * block as usize, 0);
        img[20 * block as usize..20 * block as usize + 6].copy_from_slice(b"RMGE01");
        img[21 * block as usize + 10] = 0xAB;
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("game.wbfs");
        File::create(&p).unwrap().write_all(&img).unwrap();

        let mut w = Wbfs::open(&p).unwrap();
        let mut id = [0u8; 6];
        w.read_exact(&mut id).unwrap();
        assert_eq!(&id, b"RMGE01");
        let mut b = [0xFFu8; 4];
        w.seek(SeekFrom::Start(block + 100)).unwrap();
        w.read_exact(&mut b).unwrap();
        assert_eq!(b, [0; 4], "scrubbed blocks read as zeros");
        w.seek(SeekFrom::Start(2 * block + 10)).unwrap();
        w.read_exact(&mut b[..1]).unwrap();
        assert_eq!(b[0], 0xAB);
    }
}
