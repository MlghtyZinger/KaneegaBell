//! Just enough zip reading for CBZ manuals: the page images listed in the
//! central directory, read one at a time. Entries are named by their index,
//! never written to disk under their own names, so odd paths can't escape.
//! Stored and deflated entries only (no zip64, no encryption).

use std::io::Read;

use flate2::read::DeflateDecoder;

use crate::error::{Error, Result};

/// One page image is never bigger than this unpacked.
const MAX_ENTRY: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    method: u16,
    compressed: u64,
    size: u64,
    offset: u64,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

pub fn is_image(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !lower.ends_with('/')
        && !lower.contains("__macosx")
        && [".jpg", ".jpeg", ".png", ".webp", ".gif", ".bmp"].iter().any(|e| lower.ends_with(e))
}

/// Compares names so "page2" sorts before "page10".
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        let mut out = Vec::new();
        let mut num = String::new();
        for c in s.to_ascii_lowercase().chars() {
            if c.is_ascii_digit() {
                num.push(c);
            } else {
                if !num.is_empty() {
                    out.push((0u8, format!("{:0>12}", num), ' '));
                    num.clear();
                }
                out.push((1u8, String::new(), c));
            }
        }
        if !num.is_empty() {
            out.push((0u8, format!("{:0>12}", num), ' '));
        }
        out
    };
    key(a).cmp(&key(b))
}

/// The archive's image entries, in reading order.
pub fn pages(zip: &[u8]) -> Result<Vec<Entry>> {
    let bad = || Error::msg("That file isn't a readable CBZ/zip archive.");
    // End of central directory: in the last 64 KB (after an optional comment).
    let tail = zip.len().saturating_sub(65_557);
    let eocd = (tail..zip.len().saturating_sub(21)).rev().find(|&i| zip[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]).ok_or_else(bad)?;
    let count = u16_at(zip, eocd + 10) as usize;
    let mut at = u32_at(zip, eocd + 16) as usize;
    let mut out = Vec::new();
    for _ in 0..count {
        let h = zip.get(at..at + 46).ok_or_else(bad)?;
        if h[..4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(bad());
        }
        let (name_len, extra_len, comment_len) = (u16_at(h, 28) as usize, u16_at(h, 30) as usize, u16_at(h, 32) as usize);
        let name = String::from_utf8_lossy(zip.get(at + 46..at + 46 + name_len).ok_or_else(bad)?).into_owned();
        let e = Entry { name, method: u16_at(h, 10), compressed: u32_at(h, 20) as u64, size: u32_at(h, 24) as u64, offset: u32_at(h, 42) as u64 };
        if is_image(&e.name) {
            out.push(e);
        }
        at += 46 + name_len + extra_len + comment_len;
    }
    out.sort_by(|a, b| natural(&a.name, &b.name));
    Ok(out)
}

/// One entry's bytes, unpacked.
pub fn read(zip: &[u8], e: &Entry) -> Result<Vec<u8>> {
    let bad = || Error::msg("A page in that archive can't be read.");
    if e.size > MAX_ENTRY {
        return Err(Error::msg("A page in that archive is too large."));
    }
    let at = e.offset as usize;
    let h = zip.get(at..at + 30).ok_or_else(bad)?;
    if h[..4] != [0x50, 0x4b, 0x03, 0x04] {
        return Err(bad());
    }
    let start = at + 30 + u16_at(h, 26) as usize + u16_at(h, 28) as usize;
    let data = zip.get(start..start + e.compressed as usize).ok_or_else(bad)?;
    match e.method {
        0 => Ok(data.to_vec()),
        8 => {
            let mut out = Vec::with_capacity(e.size as usize);
            DeflateDecoder::new(data).take(MAX_ENTRY + 1).read_to_end(&mut out)?;
            if out.len() as u64 > MAX_ENTRY {
                return Err(Error::msg("A page in that archive is too large."));
            }
            Ok(out)
        }
        _ => Err(Error::msg("That archive uses a compression KB can't read.")),
    }
}

#[cfg(test)]
pub fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
    // Stored entries only: enough for tests.
    let (mut body, mut central) = (Vec::new(), Vec::new());
    for (name, data) in files {
        let offset = body.len() as u32;
        let mut local = vec![0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        local.extend((data.len() as u32).to_le_bytes());
        local.extend((data.len() as u32).to_le_bytes());
        local.extend((name.len() as u16).to_le_bytes());
        local.extend(0u16.to_le_bytes());
        body.extend(local);
        body.extend(name.as_bytes());
        body.extend(*data);
        let mut c = vec![0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        c.extend((data.len() as u32).to_le_bytes());
        c.extend((data.len() as u32).to_le_bytes());
        c.extend((name.len() as u16).to_le_bytes());
        c.extend([0u8; 12]);
        c.extend(offset.to_le_bytes());
        c.extend(name.as_bytes());
        central.extend(c);
    }
    let cd_at = body.len() as u32;
    let cd_len = central.len() as u32;
    body.extend(central);
    body.extend([0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    body.extend((files.len() as u16).to_le_bytes());
    body.extend((files.len() as u16).to_le_bytes());
    body.extend(cd_len.to_le_bytes());
    body.extend(cd_at.to_le_bytes());
    body.extend(0u16.to_le_bytes());
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_images_in_reading_order_and_reads_them() {
        let zip = build(&[("p10.jpg", b"ten"), ("notes.txt", b"x"), ("p2.png", b"two"), ("__MACOSX/p1.jpg", b"junk"), ("p1.jpg", b"one")]);
        let pages = pages(&zip).unwrap();
        assert_eq!(pages.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["p1.jpg", "p2.png", "p10.jpg"]);
        assert_eq!(read(&zip, &pages[2]).unwrap(), b"ten");
    }

    #[test]
    fn junk_is_refused() {
        assert!(pages(b"PK\x03\x04 not really").is_err());
        assert!(pages(&[]).is_err());
    }
}
