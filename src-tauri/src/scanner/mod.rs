//! Library scanning. Runs on a background thread with its own DB connection.
//!
//! Per ROM source:
//! 1. list files (extension allowlist, so saves/patches/readmes never appear)
//! 2. hide companions: files referenced by .m3u/.cue, .ccd/.mds sidecars
//! 3. resolve each file's platform (folder assignment, header sniffing)
//! 4. group multi-disc sets
//! 5. upsert into SQLite; games no longer found are flagged `missing` if they
//!    carry user data (play time, favorite), otherwise deleted
//!
//! Unchanged files (same size + mtime as the DB row) skip header sniffing, so
//! rescans of a large library touch only directory metadata.

pub mod cue_m3u;
pub mod detect;
pub mod hints;
pub mod multidisc;
pub mod normalize;
pub mod ps3;
pub mod sfo;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, UNIX_EPOCH};

use rusqlite::Connection;
use serde::Serialize;

use crate::db::queries;
use crate::error::Result;
use multidisc::Item;
use normalize::ParsedName;

pub const DOLPHIN_PLATFORMS: &[&str] = &["gc", "wii"];
/// Bump when platform detection changes, so the next scan re-sniffs files the
/// old logic classified instead of trusting the size/mtime cache.
/// 2: PS1 vs PS2 via SYSTEM.CNF (v1 called every "PLAYSTATION" disc PS1).
/// 3: Saturn ("SEGA SEGASATURN"), compressed discs and cues with a stored pregap read by content.
const DETECT_VERSION: i64 = 3;
const MAX_DEPTH: usize = 8;

/// Folder names that never hold games: emulator caches, saves, firmware,
/// logs... Matched case-insensitively against the whole folder name.
const SKIP_DIRS: &[&str] = &[
    "$recycle.bin", "system volume information", "cache", "caches", "shadercache", "shader cache", "shaders",
    "pipelinecache", "logs", "log", "screenshots", "captures", "recordings", "savestates", "save states",
    "states", "saves", "savedata", "save data", "memcards", "memory cards", "cheats", "textures", "config",
    "configs", "bios", "firmware", "patches", "updates", "dlc", "exdata", "packages", "temp", "tmp",
    "sce_sys", "dev_flash", "dev_flash2", "dev_flash3", "dev_bdvd", "dev_usb000", "dev_hdd1", "icons",
    "overlays", "thumbnails", "playlists", "assets", "autoconfig", "database", "system", "cores", "info",
    "downloads", "nand", "sysdata", "keys",
];

/// Disc-image extensions. In Auto folders, files under 1 MiB with these
/// extensions are ignored (real disc images are far larger; tiny .bin files
/// are Amiibo dumps, save data and the like).
const DISC_EXTS: &[&str] = &[
    "iso", "bin", "img", "chd", "cso", "zso", "gz", "mdf", "rvz", "wbfs", "gcz", "wia", "ciso", "gcm", "pbp",
    "ecm", "tgc",
];
const MIN_DISC_BYTES: u64 = 1 << 20;

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
    /// Sources finished / total.
    pub done: usize,
    pub total: usize,
    /// Files examined in the current source / total listed.
    pub files_done: usize,
    pub files_total: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub added: usize,
    pub updated: usize,
    /// Gone from disk but kept (they have play time or are favorites).
    pub missing: usize,
    /// Gone from disk and deleted (nothing worth keeping).
    pub removed: usize,
    pub cancelled: bool,
    pub games_total: usize,
    pub duration_ms: u128,
    pub warnings: Vec<String>,
    /// Games added, removed or moved between platforms, for the scan report.
    pub changes: Vec<queries::GameChange>,
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

/// Whether a file with an allowed extension should be considered at all.
fn accept_file(path: &Path, ext: &str, size: u64, auto: bool) -> bool {
    // PCSX2 reads gzip-compressed images, but only "name.iso.gz" is one;
    // plain .gz files are logs and caches (RPCS3 writes thousands).
    if ext == "gz" {
        let inner = path.file_stem().map(|s| ext_of(Path::new(s))).unwrap_or_default();
        if !matches!(inner.as_str(), "iso" | "bin" | "img") {
            return false;
        }
    }
    !(auto && DISC_EXTS.contains(&ext) && size < MIN_DISC_BYTES)
}

/// Directory walk for one source. Besides game files it collects PS3 game
/// folders and RPCS3 `dev_hdd0/game` folders (Auto sources only), and never
/// descends into them. Auto walks also skip folders named for a platform that
/// isn't enabled ("Switch" until Switch is added; see `hints::skip_dir`).
struct Walk<'a> {
    allowed: &'a HashSet<&'a str>,
    enabled: &'a HashSet<&'a str>,
    auto: bool,
    want_ps3: bool,
    cancel: &'a AtomicBool,
    files: Vec<Listed>,
    ps3_discs: Vec<PathBuf>,
    ps3_hdd0: Vec<PathBuf>,
}

impl Walk<'_> {
    fn dir(&mut self, dir: &Path, recursive: bool, depth: usize) {
        if self.cancel.load(Ordering::Relaxed) {
            return;
        }
        if self.want_ps3 && dir.join("PS3_GAME").join("PARAM.SFO").is_file() {
            self.ps3_discs.push(dir.to_path_buf());
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            log::warn!("cannot read {}", dir.display());
            return;
        };
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            if ft.is_dir() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if self.want_ps3 && name == "dev_hdd0" {
                    let games = path.join("game");
                    if games.is_dir() {
                        self.ps3_hdd0.push(games);
                    }
                    continue;
                }
                let not_enabled = self.auto && hints::skip_dir(&name, |p| self.enabled.contains(p));
                if SKIP_DIRS.contains(&name.as_str()) || not_enabled {
                    continue;
                }
                if recursive && depth < MAX_DEPTH {
                    self.dir(&path, true, depth + 1);
                }
            } else if ft.is_file() {
                let ext = ext_of(&path);
                if !self.allowed.contains(ext.as_str()) {
                    continue;
                }
                if let Ok(m) = e.metadata() {
                    if accept_file(&path, &ext, m.len(), self.auto) {
                        self.files.push(Listed { path, ext, size: m.len(), mtime: mtime_secs(&m) });
                    }
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
    source_root: &'a Path,
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

        // Playlists/sheets: sniff the first file they reference; an .m3u of
        // .cue files goes one level further, to the first track.
        let (mut probe, mut probe_ext) = match (l.ext.as_str(), refs.first()) {
            ("m3u" | "cue", Some(first)) => (first.clone(), ext_of(first)),
            _ => (l.path.clone(), l.ext.clone()),
        };
        // The cue sheet behind the probed track, if any (a .cue, or an .m3u of them).
        let mut cue = (l.ext == "cue").then(|| l.path.clone());
        if probe_ext == "cue" {
            if let Some(track) = fs::read_to_string(&probe)
                .ok()
                .and_then(|c| cue_m3u::cue_files(&c, probe.parent()?).into_iter().next())
            {
                cue = Some(std::mem::replace(&mut probe, track));
                probe_ext = ext_of(&probe);
            }
        }
        let mut sniff = detect::sniff(&probe, &probe_ext);
        // Optical discs: Sega CD / PSP / PS1 vs PS2 (via SYSTEM.CNF) + serial.
        if sniff.platform.is_none() && matches!(probe_ext.as_str(), "bin" | "img" | "iso") {
            // Through the cue when there is one: a pregap stored ahead of
            // track 1 moves the file system to where its INDEX 01 points.
            sniff = detect::sniff_sectors(cue.as_ref().unwrap_or(&probe));
        } else if sniff.platform.is_none() && crate::disc::is_compressed(&probe_ext) {
            sniff = detect::sniff_sectors(&probe);
        }

        let platform = match (self.source_platform, sniff.platform) {
            // Folder assigned to GameCube but the disc is Wii (or vice versa).
            (Some(p), Some(s)) if DOLPHIN_PLATFORMS.contains(&p) && DOLPHIN_PLATFORMS.contains(&s) => s.to_string(),
            (Some(p), _) => p.to_string(),
            (None, _) if candidates.len() == 1 => candidates[0].to_string(),
            (None, Some(s)) if candidates.contains(&s) => s.to_string(),
            (None, _) => {
                // Headers couldn't decide: fall back to folder names ("PS2", "PSX"...).
                match hints::for_file(&l.path, self.source_root).into_iter().find(|h| candidates.contains(h)) {
                    Some(h) => h.to_string(),
                    None => {
                        return Err(format!(
                            "can't tell which platform ({}) this file is; assign a platform to its folder: {}",
                            candidates.join("/"),
                            l.path.display()
                        ))
                    }
                }
            }
        };
        Ok((platform, sniff.serial))
    }
}

/// [`scan_files_with`] without progress reporting or cancellation.
#[cfg(test)]
pub fn scan_files(
    src: &SourceSpec,
    rules: &[PlatformRule],
    known: &HashMap<String, Known>,
    warnings: &mut Vec<String>,
) -> Vec<Candidate> {
    scan_files_with(src, rules, known, warnings, &|_, _| {}, &AtomicBool::new(false))
}

/// Builds library candidates for one file-based source. Pure with respect to
/// the database: everything it needs is passed in. `progress(done, total)`
/// reports files examined; `cancel` stops early (returning what was found).
pub fn scan_files_with(
    src: &SourceSpec,
    rules: &[PlatformRule],
    known: &HashMap<String, Known>,
    warnings: &mut Vec<String>,
    progress: &dyn Fn(usize, usize),
    cancel: &AtomicBool,
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

    let auto = src.platform_id.is_none();
    let enabled: HashSet<&str> = rules.iter().map(|r| r.id.as_str()).collect();
    let mut walk = Walk {
        allowed: &allowed,
        enabled: &enabled,
        auto,
        want_ps3: auto && rules.iter().any(|r| r.id == "ps3"),
        cancel,
        files: Vec::new(),
        ps3_discs: Vec::new(),
        ps3_hdd0: Vec::new(),
    };
    walk.dir(&src.path, src.recursive, 0);
    let mut listed = std::mem::take(&mut walk.files);
    listed.sort_by(|a, b| a.path.cmp(&b.path));

    // PS3 folders found inside an Auto source.
    let mut ps3_found = Vec::new();
    for root in &walk.ps3_discs {
        ps3_found.extend(ps3_candidates(ps3::find_disc_games(root, 0), false));
    }
    for games in &walk.ps3_hdd0 {
        ps3_found.extend(ps3_candidates(ps3::find_hdd0_games(games), true));
    }
    let by_key: HashMap<String, (u64, i64)> =
        listed.iter().map(|l| (path_key(&l.path), (l.size, l.mtime))).collect();

    let refs: Vec<Vec<PathBuf>> = listed.iter().map(referenced_files).collect();
    let hidden: HashSet<String> = refs.iter().flatten().map(|p| path_key(p)).collect();

    let resolver = Resolver { source_platform: src.platform_id.as_deref(), source_root: &src.path, ext_map: &ext_map, known };
    let mut items = Vec::new();
    // path key -> (size incl. companions, mtime, sniffed serial)
    let mut meta: HashMap<String, (u64, i64, Option<String>)> = HashMap::new();
    let total = listed.len();
    for (n, (l, r)) in listed.iter().zip(refs).enumerate() {
        if n % 25 == 0 {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            progress(n, total);
        }
        if hidden.contains(&path_key(&l.path)) {
            continue;
        }
        if matches!(l.ext.as_str(), "nsp" | "xci") && is_switch_add_on(&l.path.file_stem().unwrap_or_default().to_string_lossy()) {
            continue;
        }
        // A cue sheet whose disc images aren't there (e.g. a translation patch
        // that hasn't been applied yet) isn't a game that can start.
        if l.ext == "cue" && !r.is_empty() && r.iter().all(|p| !p.exists()) {
            warnings.push(format!("skipped {}: the disc image it lists isn't there ({})", l.path.display(), r[0].display()));
            continue;
        }
        match resolver.resolve(l, &r) {
            Ok((platform, serial)) => {
                let mut stem = l.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                if l.ext == "gz" {
                    // "Game (USA).iso.gz" -> "Game (USA)"
                    stem = Path::new(&stem).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(stem);
                }
                let parsed = normalize::parse(&stem);
                // Companion sizes count toward the game's size.
                let extra: u64 = r.iter().filter_map(|p| by_key.get(&path_key(p))).map(|(s, _)| s).sum();
                meta.insert(path_key(&l.path), (l.size + extra, l.mtime, serial));
                items.push(Item { platform, path: l.path.clone(), parsed });
            }
            Err(w) => warnings.push(w),
        }
    }

    progress(total, total);

    let mut out: Vec<Candidate> = multidisc::group(items)
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
        .collect();
    out.extend(ps3_found);
    out
}

/// Switch updates and DLC are .nsp files of their own next to the game: told
/// apart by the title ID in the name (a game's ends in 13 zero bits; an update
/// adds 0x800, DLC 0x1000 + n), or without one by an [UPD]/[Update]/[DLC] tag.
fn is_switch_add_on(stem: &str) -> bool {
    stem.split(['[', ']', '(', ')']).map(str::trim).any(|t| match u64::from_str_radix(t, 16) {
        Ok(id) if t.len() == 16 && t.starts_with("01") => id & 0x1FFF != 0,
        _ => matches!(t.to_ascii_lowercase().as_str(), "upd" | "update" | "dlc"),
    })
}

/// PS3 candidates: disc folders (kind "files") or installed games (kind "ps3_hdd0").
pub fn scan_ps3(src: &SourceSpec) -> Vec<Candidate> {
    let hdd0 = src.kind == "ps3_hdd0";
    let games = if hdd0 { ps3::find_hdd0_games(&src.path) } else { ps3::find_disc_games(&src.path, 3) };
    ps3_candidates(games, hdd0)
}

fn ps3_candidates(games: Vec<ps3::Ps3Game>, hdd0: bool) -> Vec<Candidate> {
    games
        .into_iter()
        .map(|g| {
            // SFO titles are often all-caps or oddly punctuated ("THE DARKNESS"),
            // while Redump-style folder names ("Darkness, The (USA)") are curated:
            // prefer the folder when it carries a region tag. dev_hdd0 folders
            // are bare title IDs, so those always use the SFO title.
            let folder = g.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let from_folder = normalize::parse(&folder);
            let name = if !hdd0 && from_folder.region.is_some() {
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

/// Scans every configured source and writes the results. A cancelled scan
/// writes the sources it finished and leaves the rest untouched.
pub fn run(conn: &mut Connection, progress: &dyn Fn(Progress), cancel: &AtomicBool) -> Result<Summary> {
    let started = Instant::now();
    let mut summary = Summary::default();
    let rules = queries::platform_rules(conn)?;
    let sources = queries::sources(conn)?;
    let known = queries::known_games(conn)?;
    // After a detector upgrade, don't reuse cached platform decisions.
    let redetect = queries::get_setting::<i64>(conn, "detectVersion")? != Some(DETECT_VERSION);
    let empty = HashMap::new();
    let sniff_cache = if redetect { &empty } else { &known };

    for (i, src) in sources.iter().enumerate() {
        let label = src.path.display().to_string();
        let report = |files_done: usize, files_total: usize| {
            progress(Progress {
                phase: "scanning",
                source: label.clone(),
                done: i,
                total: sources.len(),
                files_done,
                files_total,
            })
        };
        report(0, 0);
        if !src.path.is_dir() {
            summary.warnings.push(format!("source folder not found: {label}"));
            // Don't flag its games missing: the drive may simply be unplugged.
            continue;
        }
        let candidates = if src.platform_id.as_deref() == Some("ps3") || src.kind == "ps3_hdd0" {
            scan_ps3(src)
        } else {
            scan_files_with(src, &rules, sniff_cache, &mut summary.warnings, &report, cancel)
        };
        if cancel.load(Ordering::Relaxed) {
            // A partial listing would wrongly drop games; keep this source as it was.
            summary.cancelled = true;
            break;
        }
        let tx = conn.transaction()?;
        let r = queries::apply_scan(&tx, src.id, &candidates, &known)?;
        tx.commit()?;
        summary.added += r.added;
        summary.updated += r.updated;
        summary.missing += r.missing;
        summary.removed += r.removed;
        summary.changes.extend(r.changes);
    }

    if redetect && !summary.cancelled {
        queries::set_setting(conn, "detectVersion", &DETECT_VERSION.into())?;
    }
    summary.games_total = queries::count_games(conn)?;
    summary.duration_ms = started.elapsed().as_millis();
    progress(Progress {
        phase: "done",
        source: String::new(),
        done: sources.len(),
        total: sources.len(),
        files_done: 0,
        files_total: 0,
    });
    Ok(summary)
}

#[cfg(test)]
mod tests;
