//! Library scanning. Runs on a background thread with its own DB connection.
//!
//! Per ROM source:
//! 1. list files (extension allowlist, so saves/patches/readmes never appear)
//! 2. hide companions: files referenced by .m3u/.cue, .ccd/.mds sidecars
//! 3. resolve each file's platform (folder assignment, header sniffing)
//! 4. group multi-disc sets
//! 5. upsert into SQLite; games no longer found are flagged `missing`, never deleted
//!
//! Unchanged files (same size + mtime as the DB row) skip header sniffing, so
//! rescans of a large library touch only directory metadata.

pub mod cue_m3u;
pub mod detect;
pub mod multidisc;
pub mod normalize;
pub mod ps3;
pub mod sfo;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use rusqlite::Connection;
use serde::Serialize;

use crate::db::queries;
use crate::error::Result;
use multidisc::Item;
use normalize::ParsedName;

pub const DOLPHIN_PLATFORMS: &[&str] = &["gc", "wii"];
const MAX_DEPTH: usize = 8;
const SKIP_DIRS: &[&str] = &["$recycle.bin", "system volume information"];

#[derive(Debug, Clone)]
pub struct PlatformRule {
    pub id: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SourceSpec {
    pub id: i64,
    pub path: PathBuf,
    pub platform_id: Option<String>,
    pub recursive: bool,
    pub kind: String,
}

/// What the DB already knows about a launch path (keyed by [`path_key`]).
#[derive(Debug, Clone)]
pub struct Known {
    pub platform: String,
    pub serial: Option<String>,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRec {
    pub path: PathBuf,
    pub disc: Option<u32>,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub platform: String,
    pub launch_path: PathBuf,
    pub name: ParsedName,
    pub serial: Option<String>,
    pub files: Vec<FileRec>,
    pub total_size: u64,
    pub mtime: i64,
    pub is_multidisc: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: &'static str,
    pub source: String,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub added: usize,
    pub updated: usize,
    pub missing: usize,
    pub games_total: usize,
    pub duration_ms: u128,
    pub warnings: Vec<String>,
}

/// Case-insensitive identity for paths on Windows.
pub fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        s.to_lowercase().replace('/', "\\")
    } else {
        s.into_owned()
    }
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn mtime_secs(m: &fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

#[derive(Debug, Clone)]
struct Listed {
    path: PathBuf,
    ext: String,
    size: u64,
    mtime: i64,
}

fn list_files(root: &Path, recursive: bool, allowed: &HashSet<&str>, out: &mut Vec<Listed>, depth: usize) {
    let Ok(entries) = fs::read_dir(root) else {
        log::warn!("cannot read {}", root.display());
        return;
    };
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        if ft.is_dir() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if recursive && depth < MAX_DEPTH && !SKIP_DIRS.contains(&name.as_str()) {
                list_files(&path, true, allowed, out, depth + 1);
            }
        } else if ft.is_file() {
            let ext = ext_of(&path);
            if allowed.contains(ext.as_str()) {
                if let Ok(m) = e.metadata() {
                    out.push(Listed { path, ext, size: m.len(), mtime: mtime_secs(&m) });
                }
            }
        }
    }
}

/// Files a sheet/playlist pulls in (which therefore aren't games themselves).
fn referenced_files(l: &Listed) -> Vec<PathBuf> {
    let dir = l.path.parent().unwrap_or(Path::new(""));
    let read = || fs::read_to_string(&l.path).ok();
    match l.ext.as_str() {
        // Sheets are tiny; refuse to read anything that isn't.
        "cue" | "m3u" if l.size > 256 * 1024 => Vec::new(),
        "cue" => read().map(|c| cue_m3u::cue_files(&c, dir)).unwrap_or_default(),
        "m3u" => read().map(|c| cue_m3u::m3u_entries(&c, dir)).unwrap_or_default(),
        "ccd" | "mds" => cue_m3u::implied_companions(&l.path),
        _ => Vec::new(),
    }
}

struct Resolver<'a> {
    source_platform: Option<&'a str>,
    /// ext -> platforms accepting it (auto mode)
    ext_map: &'a HashMap<String, Vec<String>>,
    known: &'a HashMap<String, Known>,
}

impl Resolver<'_> {
    /// Returns (platform, serial) or a warning explaining why the file was skipped.
    fn resolve(&self, l: &Listed, refs: &[PathBuf]) -> std::result::Result<(String, Option<String>), String> {
        let candidates: Vec<&str> = match self.source_platform {
            Some(p) => vec![p],
            None => self.ext_map.get(&l.ext).map(|v| v.iter().map(String::as_str).collect()).unwrap_or_default(),
        };
        if candidates.is_empty() {
            return Err(format!("no platform accepts .{}: {}", l.ext, l.path.display()));
        }

        if let Some(k) = self.known.get(&path_key(&l.path)) {
            let compatible = candidates.contains(&k.platform.as_str())
                || (candidates.iter().any(|c| DOLPHIN_PLATFORMS.contains(c))
                    && DOLPHIN_PLATFORMS.contains(&k.platform.as_str()));
            if compatible && k.size == l.size && k.mtime == l.mtime {
                return Ok((k.platform.clone(), k.serial.clone()));
            }
        }

        // Playlists/sheets: sniff the first file they reference.
        let (probe, probe_ext) = match (l.ext.as_str(), refs.first()) {
            ("m3u" | "cue", Some(first)) => (first.clone(), ext_of(first)),
            _ => (l.path.clone(), l.ext.clone()),
        };
        let mut sniff = detect::sniff(&probe, &probe_ext);
        if sniff.platform.is_none() && matches!(probe_ext.as_str(), "bin" | "img" | "iso") {
            sniff.platform = detect::sniff_cd_track(&probe);
        }
        // An .m3u of .cue files: look one level further.
        if sniff.platform.is_none() && l.ext == "m3u" && probe_ext == "cue" {
            if let Some(bin) = fs::read_to_string(&probe)
                .ok()
                .and_then(|c| cue_m3u::cue_files(&c, probe.parent()?).into_iter().next())
            {
                sniff.platform = detect::sniff_cd_track(&bin);
            }
        }

        let platform = match (self.source_platform, sniff.platform) {
            // Folder assigned to GameCube but the disc is Wii (or vice versa).
            (Some(p), Some(s)) if DOLPHIN_PLATFORMS.contains(&p) && DOLPHIN_PLATFORMS.contains(&s) => s.to_string(),
            (Some(p), _) => p.to_string(),
            (None, _) if candidates.len() == 1 => candidates[0].to_string(),
            (None, Some(s)) if candidates.contains(&s) => s.to_string(),
            (None, _) => {
                return Err(format!(
                    "can't tell which platform ({}) this file is; assign a platform to its folder: {}",
                    candidates.join("/"),
                    l.path.display()
                ))
            }
        };
        Ok((platform, sniff.serial))
    }
}

/// Builds library candidates for one file-based source. Pure with respect to
/// the database: everything it needs is passed in.
pub fn scan_files(
    src: &SourceSpec,
    rules: &[PlatformRule],
    known: &HashMap<String, Known>,
    warnings: &mut Vec<String>,
) -> Vec<Candidate> {
    let mut ext_map: HashMap<String, Vec<String>> = HashMap::new();
    for r in rules {
        if src.platform_id.as_deref().is_none_or(|p| p == r.id) {
            for e in &r.extensions {
                ext_map.entry(e.clone()).or_default().push(r.id.clone());
            }
        }
    }
    // Wii discs in a folder assigned to GameCube (and vice versa) are fine.
    if let Some(p) = src.platform_id.as_deref() {
        if DOLPHIN_PLATFORMS.contains(&p) {
            for r in rules.iter().filter(|r| DOLPHIN_PLATFORMS.contains(&r.id.as_str())) {
                for e in &r.extensions {
                    let v = ext_map.entry(e.clone()).or_default();
                    if !v.contains(&p.to_string()) {
                        v.push(p.to_string());
                    }
                }
            }
        }
    }
    if ext_map.is_empty() {
        warnings.push(format!("no enabled platform matches source {}", src.path.display()));
        return Vec::new();
    }
    let allowed: HashSet<&str> = ext_map.keys().map(String::as_str).collect();

    let mut listed = Vec::new();
    list_files(&src.path, src.recursive, &allowed, &mut listed, 0);
    listed.sort_by(|a, b| a.path.cmp(&b.path));
    let by_key: HashMap<String, (u64, i64)> =
        listed.iter().map(|l| (path_key(&l.path), (l.size, l.mtime))).collect();

    let refs: Vec<Vec<PathBuf>> = listed.iter().map(referenced_files).collect();
    let hidden: HashSet<String> = refs.iter().flatten().map(|p| path_key(p)).collect();

    let resolver = Resolver { source_platform: src.platform_id.as_deref(), ext_map: &ext_map, known };
    let mut items = Vec::new();
    // path key -> (size incl. companions, mtime, sniffed serial)
    let mut meta: HashMap<String, (u64, i64, Option<String>)> = HashMap::new();
    for (l, r) in listed.iter().zip(refs) {
        if hidden.contains(&path_key(&l.path)) {
            continue;
        }
        match resolver.resolve(l, &r) {
            Ok((platform, serial)) => {
                let stem = l.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let parsed = normalize::parse(&stem);
                // Companion sizes count toward the game's size.
                let extra: u64 = r.iter().filter_map(|p| by_key.get(&path_key(p))).map(|(s, _)| s).sum();
                meta.insert(path_key(&l.path), (l.size + extra, l.mtime, serial));
                items.push(Item { platform, path: l.path.clone(), parsed });
            }
            Err(w) => warnings.push(w),
        }
    }

    multidisc::group(items)
        .into_iter()
        .map(|g| {
            let files: Vec<FileRec> = g
                .discs
                .iter()
                .map(|(p, disc)| {
                    let (size, mtime, _) = meta[&path_key(p)];
                    FileRec { path: p.clone(), disc: *disc, size, mtime }
                })
                .collect();
            let (_, _, serial) = &meta[&path_key(g.launch_path())];
            Candidate {
                platform: g.platform.clone(),
                launch_path: g.launch_path().clone(),
                serial: serial.clone().or_else(|| g.parsed.serial.clone()),
                total_size: files.iter().map(|f| f.size).sum(),
                mtime: files.iter().map(|f| f.mtime).max().unwrap_or(0),
                is_multidisc: g.is_multidisc(),
                name: g.parsed,
                files,
            }
        })
        .collect()
}

/// PS3 candidates: disc folders (kind "files") or installed games (kind "ps3_hdd0").
pub fn scan_ps3(src: &SourceSpec) -> Vec<Candidate> {
    let games = if src.kind == "ps3_hdd0" {
        ps3::find_hdd0_games(&src.path)
    } else {
        ps3::find_disc_games(&src.path, 3)
    };
    games
        .into_iter()
        .map(|g| {
            // SFO titles are often all-caps or oddly punctuated ("THE DARKNESS"),
            // while Redump-style folder names ("Darkness, The (USA)") are curated:
            // prefer the folder when it carries a region tag. dev_hdd0 folders
            // are bare title IDs, so those always use the SFO title.
            let folder = g.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let from_folder = normalize::parse(&folder);
            let name = if src.kind != "ps3_hdd0" && from_folder.region.is_some() {
                from_folder
            } else {
                normalize::parse(&g.title)
            };
            let (size, mtime) = fs::metadata(&g.eboot).map(|m| (m.len(), mtime_secs(&m))).unwrap_or((0, 0));
            Candidate {
                platform: "ps3".into(),
                launch_path: g.eboot.clone(),
                name,
                serial: g.title_id,
                files: vec![FileRec { path: g.eboot, disc: None, size, mtime }],
                total_size: size,
                mtime,
                is_multidisc: false,
            }
        })
        .collect()
}

/// Scans every configured source and writes the results.
pub fn run(conn: &mut Connection, progress: &dyn Fn(Progress)) -> Result<Summary> {
    let started = Instant::now();
    let mut summary = Summary::default();
    let rules = queries::platform_rules(conn)?;
    let sources = queries::sources(conn)?;
    let known = queries::known_games(conn)?;

    for (i, src) in sources.iter().enumerate() {
        let label = src.path.display().to_string();
        progress(Progress { phase: "scanning", source: label.clone(), done: i, total: sources.len() });
        if !src.path.is_dir() {
            summary.warnings.push(format!("source folder not found: {label}"));
            // Don't flag its games missing: the drive may simply be unplugged.
            continue;
        }
        let candidates = if src.platform_id.as_deref() == Some("ps3") || src.kind == "ps3_hdd0" {
            scan_ps3(src)
        } else {
            scan_files(src, &rules, &known, &mut summary.warnings)
        };
        let tx = conn.transaction()?;
        let (added, updated, missing) = queries::apply_scan(&tx, src.id, &candidates, &known)?;
        tx.commit()?;
        summary.added += added;
        summary.updated += updated;
        summary.missing += missing;
    }

    summary.games_total = queries::count_games(conn)?;
    summary.duration_ms = started.elapsed().as_millis();
    progress(Progress { phase: "done", source: String::new(), done: sources.len(), total: sources.len() });
    Ok(summary)
}

#[cfg(test)]
mod tests;
