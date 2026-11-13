//! Bounded readers for the two local Valve KeyValues encodings. No client DLLs or network access.
use crate::error::{Error, Result};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum Kv {
    Text(String),
    Object(BTreeMap<String, Kv>),
}
impl Kv {
    pub fn get(&self, name: &str) -> Option<&Kv> {
        match self {
            Kv::Object(o) => o.get(&name.to_ascii_lowercase()),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            Kv::Text(s) => Some(s),
            _ => None,
        }
    }
    pub fn entries(&self) -> impl Iterator<Item = (&String, &Kv)> {
        match self {
            Kv::Object(o) => Some(o),
            _ => None,
        }
        .into_iter()
        .flat_map(|o| o.iter())
    }
    pub fn value(&self, name: &str) -> Option<&str> {
        self.get(name)?.text()
    }
}
fn bad() -> Error {
    Error::msg("Steam metadata is malformed or uses an unsupported format. Refresh it in Steam and scan again.")
}

pub fn text(input: &str) -> Result<Kv> {
    if input.len() > 8 * 1024 * 1024 {
        return Err(bad());
    }
    let mut tokens = Vec::new();
    let mut chars = input.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' | '}' => tokens.push(c.to_string()),
            '"' => {
                let mut s = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        match chars.next().ok_or_else(bad)? {
                            '"' => s.push('"'),
                            '\\' => s.push('\\'),
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            c => {
                                s.push('\\');
                                s.push(c);
                            }
                        }
                    } else {
                        s.push(c);
                    }
                }
                if !closed {
                    return Err(bad());
                }
                // Prefix quoted tokens so a quoted brace cannot be mistaken for syntax.
                tokens.push(format!("={s}"));
            }
            _ => return Err(bad()),
        }
    }
    fn object(tokens: &[String], at: &mut usize, depth: usize, nested: bool) -> Result<Kv> {
        if depth > 32 {
            return Err(bad());
        }
        let mut out = BTreeMap::new();
        while let Some(token) = tokens.get(*at) {
            if token == "}" {
                if !nested {
                    return Err(bad());
                }
                *at += 1;
                return Ok(Kv::Object(out));
            }
            let key = token
                .strip_prefix('=')
                .ok_or_else(bad)?
                .to_ascii_lowercase();
            *at += 1;
            let value = tokens.get(*at).ok_or_else(bad)?;
            *at += 1;
            let value = if value == "{" {
                object(tokens, at, depth + 1, true)?
            } else {
                Kv::Text(value.strip_prefix('=').ok_or_else(bad)?.into())
            };
            if out.insert(key, value).is_some() {
                return Err(bad());
            }
        }
        if nested {
            Err(bad())
        } else {
            Ok(Kv::Object(out))
        }
    }
    object(&tokens, &mut 0, 0, false)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).ok_or_else(bad)?;
        let b = self.bytes.get(self.at..end).ok_or_else(bad)?;
        self.at = end;
        Ok(b)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| bad())?,
        ))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| bad())?,
        ))
    }
    fn string(&mut self) -> Result<String> {
        let n = self
            .bytes
            .get(self.at..)
            .ok_or_else(bad)?
            .iter()
            .take(1024 * 1024)
            .position(|&b| b == 0)
            .ok_or_else(bad)?;
        let s = std::str::from_utf8(self.take(n)?)
            .map_err(|_| bad())?
            .to_owned();
        self.take(1)?;
        Ok(s)
    }
    fn object(&mut self, strings: Option<&[String]>, depth: usize) -> Result<Kv> {
        if depth > 32 {
            return Err(bad());
        }
        let mut out = BTreeMap::new();
        loop {
            let tag = self.u8()?;
            if tag == 8 {
                return Ok(Kv::Object(out));
            }
            if out.len() > 100_000 {
                return Err(bad());
            }
            let key = match strings {
                Some(s) => s.get(self.u32()? as usize).ok_or_else(bad)?.clone(),
                None => self.string()?,
            }
            .to_ascii_lowercase();
            let value = match tag {
                0 => self.object(strings, depth + 1)?,
                1 => Kv::Text(self.string()?),
                2 => Kv::Text((self.u32()? as i32).to_string()),
                3 | 4 | 6 => {
                    self.take(4)?;
                    Kv::Text(String::new())
                }
                7 | 10 => {
                    self.take(8)?;
                    Kv::Text(String::new())
                }
                _ => return Err(bad()),
            };
            if out.insert(key, value).is_some() {
                return Err(bad());
            }
        }
    }
}

/// The public xPaw/SteamAppInfo format reference documents versions 39–41.
/// Only installed IDs are decoded; tokens and account-related fields are never returned.
pub fn appinfo(
    bytes: &[u8],
    installed: &std::collections::HashSet<u32>,
) -> Result<BTreeMap<u32, Kv>> {
    if bytes.len() > 512 * 1024 * 1024 {
        return Err(bad());
    }
    let mut r = Reader { bytes, at: 0 };
    let magic = r.u32()?;
    if magic >> 8 != 0x075644 || !(39..=41).contains(&(magic & 0xff)) || r.u32()? != 1 {
        return Err(bad());
    }
    let version = magic & 0xff;
    let mut strings = None;
    let mut entries_end = bytes.len();
    if version == 41 {
        let offset = usize::try_from(r.u64()?).map_err(|_| bad())?;
        if offset < r.at || offset >= bytes.len() {
            return Err(bad());
        }
        entries_end = offset;
        let mut pool = Reader { bytes, at: offset };
        let count = pool.u32()? as usize;
        if count > 2_000_000 || count > bytes.len().saturating_sub(pool.at) {
            return Err(bad());
        }
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(pool.string()?);
        }
        strings = Some(values);
    }
    let mut result = BTreeMap::new();
    loop {
        if r.at.checked_add(4).is_none_or(|end| end > entries_end) {
            return Err(bad());
        }
        let id = r.u32()?;
        if id == 0 {
            break;
        }
        let size = r.u32()? as usize;
        let end =
            r.at.checked_add(size)
                .filter(|&e| e <= entries_end)
                .ok_or_else(bad)?;
        let header = if version >= 40 { 60 } else { 40 };
        if size < header {
            return Err(bad());
        }
        if installed.contains(&id) {
            let data = bytes.get(r.at + header..end).ok_or_else(bad)?;
            let mut entry = Reader { bytes: data, at: 0 };
            let value = entry.object(strings.as_deref(), 0)?;
            // Some encoders add one root terminator after the named appinfo object.
            if entry.bytes[entry.at..].iter().any(|&b| b != 8 && b != 0) {
                return Err(bad());
            }
            if result.insert(id, value).is_some() {
                return Err(bad());
            }
        }
        r.at = end;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_text_preserves_paths_and_quoted_braces() {
        let v = text(
            "// comment\n\"LibraryFolders\" { \"0\" { \"path\" \"D:\\\\Steam\" \"label\" \"}\" } }",
        )
        .unwrap();
        assert_eq!(
            v.get("libraryfolders")
                .unwrap()
                .get("0")
                .unwrap()
                .value("path"),
            Some("D:\\Steam")
        );
        for bad in [
            "\"x\" {",
            "\"x\" \"a\" \"X\" \"b\"",
            "\"x\" \"unterminated",
            "}",
            "\"x\" y",
        ] {
            assert!(text(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn appinfo_current_string_table_and_bounds() {
        let mut entry = vec![0];
        entry.extend(0u32.to_le_bytes()); // appinfo root
        entry.push(0);
        entry.extend(1u32.to_le_bytes()); // common
        entry.push(1);
        entry.extend(2u32.to_le_bytes());
        entry.extend(b"game\0");
        entry.extend([8, 8, 8]);
        let mut b = Vec::new();
        b.extend(0x07564429u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend((16u64 + 8 + 60 + entry.len() as u64 + 4).to_le_bytes());
        b.extend(10u32.to_le_bytes());
        b.extend((60u32 + entry.len() as u32).to_le_bytes());
        b.extend([0u8; 60]);
        b.extend(entry);
        b.extend(0u32.to_le_bytes());
        b.extend(3u32.to_le_bytes());
        b.extend(b"appinfo\0common\0type\0");
        let info = appinfo(&b, &[10].into_iter().collect()).unwrap();
        assert_eq!(
            info[&10]
                .get("appinfo")
                .unwrap()
                .get("common")
                .unwrap()
                .value("type"),
            Some("game")
        );
        assert!(appinfo(&b[..20], &[10].into_iter().collect()).is_err());
        b[0] = 42;
        assert!(appinfo(&b, &[10].into_iter().collect()).is_err());
    }
}
