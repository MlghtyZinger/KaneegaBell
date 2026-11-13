//! PARAM.SFO reader (PS3/PSP/Vita metadata). Format: 20-byte header, then
//! 16-byte index entries pointing into a key table and a data table.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfoValue {
    Str(String),
    Int(u32),
}

#[derive(Debug, Default, Clone)]
pub struct Sfo(pub HashMap<String, SfoValue>);

impl Sfo {
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.0.get(key)? {
            SfoValue::Str(s) => Some(s.as_str()),
            SfoValue::Int(_) => None,
        }
    }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

pub fn parse(b: &[u8]) -> Option<Sfo> {
    if b.get(..4)? != b"\0PSF" {
        return None;
    }
    let key_table = u32_at(b, 8)? as usize;
    let data_table = u32_at(b, 12)? as usize;
    let count = u32_at(b, 16)? as usize;
    if count > 512 {
        return None; // corrupt
    }
    let mut map = HashMap::with_capacity(count);
    for i in 0..count {
        let e = 20 + i * 16;
        let key_off = key_table + u16_at(b, e)? as usize;
        let fmt = u16_at(b, e + 2)?;
        let len = u32_at(b, e + 4)? as usize;
        let data_off = data_table + u32_at(b, e + 12)? as usize;
        let key = cstr(b.get(key_off..)?);
        let data = b.get(data_off..data_off.checked_add(len)?)?;
        let value = match fmt {
            0x0404 => SfoValue::Int(u32_at(data, 0)?),
            0x0204 | 0x0004 => SfoValue::Str(cstr(data)),
            _ => continue,
        };
        map.insert(key, value);
    }
    Some(Sfo(map))
}

pub fn read(path: &std::path::Path) -> Option<Sfo> {
    // Real SFOs are a few KB; refuse anything absurd.
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 1 << 20 {
        return None;
    }
    parse(&std::fs::read(path).ok()?)
}

/// Builds a valid SFO from string entries (tests in this and other modules).
#[cfg(test)]
pub fn build(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut keys = Vec::new();
    let mut data = Vec::new();
    let mut index = Vec::new();
    for (k, v) in entries {
        let key_off = keys.len() as u16;
        keys.extend_from_slice(k.as_bytes());
        keys.push(0);
        let data_off = data.len() as u32;
        let mut val = v.as_bytes().to_vec();
        val.push(0);
        let len = val.len() as u32;
        let max = len.div_ceil(4) * 4;
        val.resize(max as usize, 0);
        data.extend_from_slice(&val);
        index.extend_from_slice(&key_off.to_le_bytes());
        index.extend_from_slice(&0x0204u16.to_le_bytes());
        index.extend_from_slice(&len.to_le_bytes());
        index.extend_from_slice(&max.to_le_bytes());
        index.extend_from_slice(&data_off.to_le_bytes());
    }
    while keys.len() % 4 != 0 {
        keys.push(0);
    }
    let key_table = 20 + index.len() as u32;
    let data_table = key_table + keys.len() as u32;
    let mut out = b"\0PSF".to_vec();
    out.extend_from_slice(&0x0101u32.to_le_bytes());
    out.extend_from_slice(&key_table.to_le_bytes());
    out.extend_from_slice(&data_table.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    out.extend_from_slice(&index);
    out.extend_from_slice(&keys);
    out.extend_from_slice(&data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_strings() {
        let b = build(&[("CATEGORY", "DG"), ("TITLE", "Kane & Lynch 2: Dog Days"), ("TITLE_ID", "BLUS30383")]);
        let sfo = parse(&b).unwrap();
        assert_eq!(sfo.str("CATEGORY"), Some("DG"));
        assert_eq!(sfo.str("TITLE"), Some("Kane & Lynch 2: Dog Days"));
        assert_eq!(sfo.str("TITLE_ID"), Some("BLUS30383"));
        assert_eq!(sfo.str("MISSING"), None);
    }

    #[test]
    fn rejects_garbage_and_truncation() {
        assert!(parse(b"nope").is_none());
        let b = build(&[("TITLE", "X")]);
        assert!(parse(&b[..b.len() - 6]).is_none());
    }
}
