//! Directory listing for the in-app folder/file picker. The native Windows file
//! dialog can't be driven with a controller, so Settings uses its own browser.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// None = the drive list.
    pub path: Option<String>,
    /// None at a drive root (going up shows the drive list).
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
    /// True if the listing was cut off at the entry limit.
    pub truncated: bool,
}

const MAX_ENTRIES: usize = 3000;
const SKIP: &[&str] = &["$recycle.bin", "system volume information", "$windows.~bt", "$windows.~ws"];

/// Drive roots, without touching the drives (so empty card readers don't stall).
pub fn drives() -> Vec<Entry> {
    #[cfg(windows)]
    {
        let mask = unsafe { windows::Win32::Storage::FileSystem::GetLogicalDrives() };
        (0..26u8)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| {
                let root = format!("{}:\\", (b'A' + i) as char);
                Entry { name: root.clone(), path: root, is_dir: true }
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        vec![Entry { name: "/".into(), path: "/".into(), is_dir: true }]
    }
}

/// Lists `dir`: folders first, then files whose extension is in `extensions`
/// (lowercase, no dot). An empty filter lists folders only.
pub fn list(dir: Option<&Path>, extensions: &[String]) -> Result<Listing> {
    let Some(dir) = dir else {
        return Ok(Listing { path: None, parent: None, entries: drives(), truncated: false });
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut truncated = false;
    for e in std::fs::read_dir(dir)?.flatten() {
        if dirs.len() + files.len() >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let Ok(ft) = e.file_type() else { continue };
        let entry = Entry { path: e.path().to_string_lossy().into_owned(), name, is_dir: ft.is_dir() };
        if ft.is_dir() {
            if !SKIP.contains(&entry.name.to_lowercase().as_str()) {
                dirs.push(entry);
            }
        } else if !extensions.is_empty() {
            let ext = Path::new(&entry.name)
                .extension()
                .map(|x| x.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if extensions.contains(&ext) {
                files.push(entry);
            }
        }
    }
    let by_name = |a: &Entry, b: &Entry| a.name.to_lowercase().cmp(&b.name.to_lowercase());
    dirs.sort_by(by_name);
    files.sort_by(by_name);
    dirs.extend(files);
    Ok(Listing {
        path: Some(dir.to_string_lossy().into_owned()),
        parent: dir.parent().map(|p| p.to_string_lossy().into_owned()),
        entries: dirs,
        truncated,
    })
}

/// The deepest existing folder of `path` (for opening the browser where a
/// half-valid path points).
pub fn nearest_existing_dir(path: &str) -> Option<PathBuf> {
    let mut p = PathBuf::from(path.trim());
    if p.as_os_str().is_empty() {
        return None;
    }
    loop {
        if p.is_dir() {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_dirs_first_then_filtered_files() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join("zeta")).unwrap();
        std::fs::create_dir(t.path().join("Alpha")).unwrap();
        std::fs::create_dir(t.path().join("$RECYCLE.BIN")).unwrap();
        std::fs::write(t.path().join("retroarch.exe"), b"").unwrap();
        std::fs::write(t.path().join("readme.txt"), b"").unwrap();

        let l = list(Some(t.path()), &["exe".into()]).unwrap();
        let names: Vec<_> = l.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "zeta", "retroarch.exe"]);
        assert!(l.parent.is_some());

        let folders_only = list(Some(t.path()), &[]).unwrap();
        assert!(folders_only.entries.iter().all(|e| e.is_dir));
    }

    #[test]
    fn nearest_existing_dir_walks_up() {
        let t = tempfile::tempdir().unwrap();
        let missing = t.path().join("a").join("b.exe");
        assert_eq!(nearest_existing_dir(&missing.to_string_lossy()), Some(t.path().to_path_buf()));
        assert_eq!(nearest_existing_dir("  "), None);
    }

    #[cfg(windows)]
    #[test]
    fn drive_list_includes_system_drive() {
        let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        assert!(drives().iter().any(|d| d.path.starts_with(&sys)));
    }
}
