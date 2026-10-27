//! Game manuals (0.5.0).
//!
//! Sources, in order: the user's own PDF or CBZ (picked in the artwork studio,
//! read where it is, never copied), then on request downloads from Retrokit and
//! the Internet Archive's Console Manuals (see `sources`). Downloads are off by
//! default ("manualDownloads"); even when on, a manual downloads only when it's
//! opened.
//!
//! A downloaded PDF is kept only until the viewer has turned it into page
//! images (WebP, at most 2,400 px tall: `data\manuals\<game>\<n>.webp`), which
//! is 3-4 times smaller than the scans. A CBZ is turned into pages here. The
//! user's own files are never changed. An optional size cap ("manualCap", MB)
//! deletes the least recently opened downloads first.

pub mod sources;
pub mod zip;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::art::http::Fetch;
use crate::db::queries;
use crate::error::{Error, Result};
use sources::{Entry, GameKey};

/// A download bigger than this is refused (scans run 2-130 MB).
pub const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;
/// A page image the viewer hands back is never bigger than this.
const MAX_PAGE: usize = 12 * 1024 * 1024;
/// More pages than this is not a manual.
const MAX_PAGES: u32 = 2000;
/// "No manual found" is looked up again after this long.
const NONE_RETRY_SECS: i64 = sources::LIST_MAX_AGE_SECS;

pub fn dir(data_dir: &Path, game_id: i64) -> PathBuf {
    data_dir.join("manuals").join(game_id.to_string())
}

fn source_pdf(data_dir: &Path, game_id: i64) -> PathBuf {
    dir(data_dir, game_id).join("source.pdf")
}

pub fn page_path(data_dir: &Path, game_id: i64, n: u32) -> PathBuf {
    dir(data_dir, game_id).join(format!("{n}.webp"))
}

/// Downloads with the Internet Archive's limits in mind: one at a time, a
/// descriptive User-Agent, manuals up to 200 MB.
pub struct Downloader {
    agent: ureq::Agent,
}

impl Downloader {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(600)))
            .user_agent(concat!("KaneegaBell/", env!("CARGO_PKG_VERSION"), " (personal game library; manuals on request)"))
            .build()
            .into();
        Self { agent }
    }
}

impl Fetch for Downloader {
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>> {
        match self.agent.get(url).call() {
            Ok(mut resp) => {
                let body = resp.body_mut().with_config().limit(MAX_DOWNLOAD).read_to_vec().map_err(|e| {
                    Error::msg(if e.to_string().contains("limit") { "That manual is over 200 MB, so KB didn't keep it.".to_string() } else { format!("download failed: {e}") })
                })?;
                Ok(Some(body))
            }
            Err(ureq::Error::StatusCode(404)) => Ok(None),
            Err(ureq::Error::StatusCode(429)) => Err(Error::msg("The Internet Archive asked KB to slow down. Try again in a minute.")),
            Err(e) => Err(Error::msg(format!("couldn't reach the Internet Archive: {e}"))),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Row {
    source: String,
    url: Option<String>,
    manual: bool,
    pages: Option<u32>,
    outline: Option<String>,
    checked_at: i64,
}

fn row(conn: &Connection, game_id: i64) -> Result<Option<Row>> {
    Ok(conn
        .query_row(
            "SELECT source, url, manual, pages, outline, checked_at FROM game_manuals WHERE game_id = ?1",
            [game_id],
            |r| {
                Ok(Row {
                    source: r.get(0)?,
                    url: r.get(1)?,
                    manual: r.get::<_, i64>(2)? != 0,
                    pages: r.get(3)?,
                    outline: r.get(4)?,
                    checked_at: r.get(5)?,
                })
            },
        )
        .optional()?)
}

fn set_row(conn: &Connection, game_id: i64, source: &str, label: &str, url: Option<&str>, manual: bool) -> Result<()> {
    conn.execute(
        "INSERT INTO game_manuals (game_id, source, label, url, manual, pages, outline, bytes, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, 0, ?6)
         ON CONFLICT(game_id) DO UPDATE SET source = ?2, label = ?3, url = ?4, manual = ?5, pages = NULL, outline = NULL, bytes = 0, checked_at = ?6",
        rusqlite::params![game_id, source, label, url, manual as i64, queries::now()],
    )?;
    Ok(())
}

pub fn downloads_on(conn: &Connection) -> bool {
    queries::get_setting::<bool>(conn, "manualDownloads").ok().flatten() == Some(true)
}

fn remove_files(data_dir: &Path, game_id: i64) {
    match fs::remove_dir_all(dir(data_dir, game_id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => log::warn!("couldn't delete manual files of game {game_id}: {e}"),
        _ => {}
    }
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}

/// What a Manual button shows, from what's already known (never goes online).
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// "ready" (opens now), "available" (found, downloads when opened), "unknown" (looked up when
    /// opened), "none" (no manual found), "off" (downloads are off and the user has no file for it).
    pub state: &'static str,
    /// Where it comes from, for the label: "Your file: X.pdf", "Retrokit (Internet Archive)"...
    pub source: Option<String>,
}

fn source_label(r: &Row) -> String {
    match r.source.as_str() {
        "file" => format!("Your file: {}", file_name(r.url.as_deref().unwrap_or_default())),
        "retrokit" => "Retrokit (Internet Archive)".into(),
        _ if r.manual => "Internet Archive (your pick)".into(),
        _ => "Internet Archive: Console Manuals".into(),
    }
}

pub fn status(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<Status> {
    let on = downloads_on(conn);
    let Some(r) = row(conn, game_id)? else {
        return Ok(Status { state: if on { "unknown" } else { "off" }, source: None });
    };
    let source = Some(source_label(&r));
    let state = match r.source.as_str() {
        "file" if r.url.as_deref().is_some_and(|p| Path::new(p).is_file()) => "ready",
        "file" => return Ok(Status { state: if on { "unknown" } else { "off" }, source: None }),
        "none" if on && queries::now() - r.checked_at > NONE_RETRY_SECS => "unknown",
        "none" => "none",
        _ if r.pages.is_some() || source_pdf(data_dir, game_id).is_file() => "ready",
        _ if on => "available",
        _ => "off",
    };
    Ok(Status { state, source: if state == "none" { None } else { source } })
}

fn game_key(conn: &Connection, game_id: i64) -> Result<(String, GameKey)> {
    Ok(conn.query_row("SELECT platform_id, search_key, region FROM games WHERE id = ?1", [game_id], |r| {
        Ok((r.get(0)?, GameKey { search_key: r.get(1)?, region: r.get(2)? }))
    })?)
}

/// Looks the game's manual up in its platform's lists and records what was found
/// (or that nothing was). The user's own pick is never replaced.
pub fn lookup(conn: &Connection, fetch: &dyn Fetch, game_id: i64) -> Result<Option<Entry>> {
    let (platform, _) = game_key(conn, game_id)?;
    lookup_with(conn, &sources::Matcher::new(sources::lists(conn, fetch, &platform)), game_id)
}

fn lookup_with(conn: &Connection, matcher: &sources::Matcher, game_id: i64) -> Result<Option<Entry>> {
    if row(conn, game_id)?.is_some_and(|r| r.manual) {
        return Ok(None);
    }
    let (_, key) = game_key(conn, game_id)?;
    match matcher.best(&key).cloned() {
        Some(e) => {
            let source = if e.url.starts_with("ia:") { "ia" } else { "retrokit" };
            set_row(conn, game_id, source, &e.label, Some(&e.url), false)?;
            Ok(Some(e))
        }
        None => {
            set_row(conn, game_id, "none", "", None, false)?;
            Ok(None)
        }
    }
}

/// Matches every game that has no manual row yet (downloads on: run in the background
/// so the buttons know before they're pressed). Returns how many were found.
pub fn match_all(conn: &Connection, fetch: &dyn Fetch, cancel: &dyn Fn() -> bool) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.platform_id FROM games g LEFT JOIN game_manuals m ON m.game_id = g.id
         WHERE g.missing = 0 AND (m.game_id IS NULL OR (m.source = 'none' AND m.checked_at < ?1))
         ORDER BY g.platform_id",
    )?;
    let games: Vec<(i64, String)> = stmt.query_map([queries::now() - NONE_RETRY_SECS], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut found = 0;
    let mut current: Option<(String, sources::Matcher)> = None;
    for (id, platform) in games {
        if cancel() {
            break;
        }
        // One platform at a time (sorted), so its lists and indexes are built once.
        if current.as_ref().is_none_or(|(p, _)| *p != platform) {
            current = Some((platform.clone(), sources::Matcher::new(sources::lists(conn, fetch, &platform))));
        }
        if lookup_with(conn, &current.as_ref().unwrap().1, id)?.is_some() {
            found += 1;
        }
    }
    Ok(found)
}

/// What the viewer opens.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// "pages": page images (`/mp/<game>/<n>`); "pdf": the PDF's bytes from `manual_file`;
    /// "cbz": the user's own CBZ, one page at a time from `manual_page`.
    pub kind: &'static str,
    pub pages: Option<u32>,
    /// Contents saved when the pages were made (from the PDF's bookmarks): [{title, page, items?}].
    pub outline: Option<serde_json::Value>,
    pub source: String,
    /// Changes when the pages are made again (cache-busting for page URLs).
    pub version: i64,
    /// The viewer should turn the PDF into page images while it's open.
    pub convert: bool,
}

/// Gets the game's manual ready to read: the user's file, the pages made earlier,
/// or a download (looked up first when needed). `running`: a game is on (no converting then).
pub fn open(conn: &Connection, data_dir: &Path, fetch: &dyn Fetch, game_id: i64, running: bool) -> Result<Opened> {
    let mut r = row(conn, game_id)?;
    if let Some(own) = r.as_ref().filter(|r| r.source == "file") {
        let path = own.url.clone().unwrap_or_default();
        if Path::new(&path).is_file() {
            let source = source_label(own);
            touch(conn, game_id)?;
            return Ok(if is_pdf_path(&path) {
                Opened { kind: "pdf", pages: None, outline: None, source, version: 0, convert: false }
            } else {
                let zip = read_capped(Path::new(&path))?;
                let n = zip::pages(&zip)?.len() as u32;
                Opened { kind: "cbz", pages: Some(n), outline: None, source, version: 0, convert: false }
            });
        }
        if !downloads_on(conn) {
            return Err(Error::msg(format!("Your manual file isn't there any more ({path}). Pick it again with Artwork → Manual.")));
        }
        r = None;
    }
    if let Some(done) = r.as_ref().filter(|r| r.pages.is_some() && r.source != "none") {
        if page_path(data_dir, game_id, 0).is_file() {
            touch(conn, game_id)?;
            return Ok(Opened {
                kind: "pages",
                pages: done.pages,
                outline: done.outline.as_deref().and_then(|o| serde_json::from_str(o).ok()),
                source: source_label(done),
                version: done.checked_at,
                convert: false,
            });
        }
    }
    if let Some(have) = r.as_ref().filter(|r| r.source != "none" && source_pdf(data_dir, game_id).is_file()) {
        touch(conn, game_id)?;
        return Ok(Opened { kind: "pdf", pages: None, outline: None, source: source_label(have), version: have.checked_at, convert: !running });
    }
    if !downloads_on(conn) {
        return Err(Error::msg("Manual downloads are off. Turn them on in Settings → Game info, or pick a file of your own in Artwork → Manual."));
    }
    let wanted = match r.filter(|r| r.source != "none" && r.url.is_some()) {
        Some(r) => r,
        None => match lookup(conn, fetch, game_id)? {
            Some(_) => row(conn, game_id)?.ok_or_else(|| Error::msg("lost the manual's details"))?,
            None => return Err(Error::msg("No manual found for this game. You can search for one in Artwork → Manual.")),
        },
    };
    download(conn, data_dir, fetch, game_id, wanted.url.as_deref().unwrap_or_default())?;
    open(conn, data_dir, fetch, game_id, running)
}

fn is_pdf_path(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".pdf")
}

fn read_capped(path: &Path) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    fs::File::open(path)?.take(MAX_DOWNLOAD * 2).read_to_end(&mut out)?;
    Ok(out)
}

fn touch(conn: &Connection, game_id: i64) -> Result<()> {
    conn.execute("UPDATE game_manuals SET opened_at = ?2 WHERE game_id = ?1", rusqlite::params![game_id, queries::now()])?;
    Ok(())
}

/// Downloads a manual and checks it's really a PDF or CBZ. A CBZ becomes page images straight away.
fn download(conn: &Connection, data_dir: &Path, fetch: &dyn Fetch, game_id: i64, url: &str) -> Result<()> {
    if !sources::allowed_url(url) {
        return Err(Error::msg("KB only downloads manuals from the Internet Archive."));
    }
    let real = match url.strip_prefix("ia:") {
        Some(id) => sources::resolve_item(fetch, id)?,
        None => url.to_string(),
    };
    if !sources::allowed_url(&real) {
        return Err(Error::msg("KB only downloads manuals from the Internet Archive."));
    }
    let bytes = fetch.get(&real)?.ok_or_else(|| Error::msg("That manual isn't on the Internet Archive any more."))?;
    remove_files(data_dir, game_id);
    fs::create_dir_all(dir(data_dir, game_id))?;
    if bytes.starts_with(b"%PDF-") {
        fs::write(source_pdf(data_dir, game_id), &bytes)?;
        conn.execute("UPDATE game_manuals SET pages = NULL, outline = NULL, bytes = ?2 WHERE game_id = ?1", rusqlite::params![game_id, bytes.len() as i64])?;
    } else if bytes.starts_with(b"PK\x03\x04") {
        let n = cbz_to_pages(data_dir, game_id, &bytes)?;
        finish(conn, data_dir, game_id, n, None)?;
    } else {
        remove_files(data_dir, game_id);
        return Err(Error::msg("That download isn't a PDF or CBZ manual, so KB didn't keep it."));
    }
    enforce_cap(conn, data_dir, Some(game_id))
}

fn cbz_to_pages(data_dir: &Path, game_id: i64, zip_bytes: &[u8]) -> Result<u32> {
    let entries = zip::pages(zip_bytes)?;
    if entries.is_empty() || entries.len() > MAX_PAGES as usize {
        return Err(Error::msg("That archive has no manual pages in it."));
    }
    let mut n = 0;
    for e in &entries {
        match zip::read(zip_bytes, e).and_then(|b| crate::art::images::manual_page(&b)) {
            Ok(webp) => {
                fs::write(page_path(data_dir, game_id, n), webp)?;
                n += 1;
            }
            Err(err) => log::warn!("skipped page {} of a manual: {err}", e.name),
        }
    }
    if n == 0 {
        return Err(Error::msg("None of that archive's pages could be read."));
    }
    Ok(n)
}

/// The PDF to read (the user's own, or a download not yet turned into pages).
pub fn pdf_bytes(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<Vec<u8>> {
    let r = row(conn, game_id)?.ok_or_else(|| Error::msg("This game has no manual."))?;
    let path = match r.source.as_str() {
        "file" => PathBuf::from(r.url.unwrap_or_default()),
        _ => source_pdf(data_dir, game_id),
    };
    let bytes = read_capped(&path)?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(Error::msg("That manual file isn't a PDF."));
    }
    Ok(bytes)
}

/// Page `n` of the user's own CBZ, as it is in the archive.
pub fn cbz_page(conn: &Connection, game_id: i64, n: u32) -> Result<Vec<u8>> {
    let r = row(conn, game_id)?.filter(|r| r.source == "file").ok_or_else(|| Error::msg("This game has no CBZ manual."))?;
    let zip_bytes = read_capped(Path::new(r.url.as_deref().unwrap_or_default()))?;
    let entries = zip::pages(&zip_bytes)?;
    let e = entries.get(n as usize).ok_or_else(|| Error::msg("That page isn't in the manual."))?;
    zip::read(&zip_bytes, e)
}

/// Stores one page image the viewer made from a downloaded PDF.
pub fn save_page(conn: &Connection, data_dir: &Path, game_id: i64, n: u32, webp: &[u8]) -> Result<()> {
    if n >= MAX_PAGES || webp.len() > MAX_PAGE || !(webp.len() > 12 && &webp[..4] == b"RIFF" && &webp[8..12] == b"WEBP") {
        return Err(Error::msg("That isn't a manual page image."));
    }
    let r = row(conn, game_id)?.ok_or_else(|| Error::msg("This game has no manual."))?;
    if r.source == "file" || !source_pdf(data_dir, game_id).is_file() {
        return Err(Error::msg("Only downloaded manuals are turned into pages."));
    }
    fs::write(page_path(data_dir, game_id, n), webp)?;
    Ok(())
}

/// Every page is made: keep the pages and the contents, drop the PDF.
pub fn finish(conn: &Connection, data_dir: &Path, game_id: i64, pages: u32, outline: Option<&serde_json::Value>) -> Result<()> {
    if pages == 0 || pages > MAX_PAGES || !(0..pages).all(|n| page_path(data_dir, game_id, n).is_file()) {
        return Err(Error::msg("Some pages are missing; the manual stays as a PDF for now."));
    }
    let _ = fs::remove_file(source_pdf(data_dir, game_id));
    let bytes = dir_size(&dir(data_dir, game_id));
    conn.execute(
        "UPDATE game_manuals SET pages = ?2, outline = ?3, bytes = ?4, checked_at = ?5, opened_at = ?5 WHERE game_id = ?1",
        rusqlite::params![game_id, pages, outline.map(|o| o.to_string()), bytes as i64, queries::now()],
    )?;
    enforce_cap(conn, data_dir, Some(game_id))
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir).map(|it| it.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()).unwrap_or(0)
}

/// The size cap: the least recently opened downloads go first (they download again when opened).
/// `keep`: the manual just opened.
pub fn enforce_cap(conn: &Connection, data_dir: &Path, keep: Option<i64>) -> Result<()> {
    let cap_mb = queries::get_setting::<u64>(conn, "manualCap")?.unwrap_or(0);
    if cap_mb == 0 {
        return Ok(());
    }
    let cap = cap_mb * 1024 * 1024;
    let mut stmt = conn.prepare("SELECT game_id, bytes FROM game_manuals WHERE source IN ('retrokit', 'ia') AND bytes > 0 ORDER BY opened_at ASC")?;
    let rows: Vec<(i64, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut total: u64 = rows.iter().map(|(_, b)| *b as u64).sum();
    for (id, b) in rows {
        if total <= cap {
            break;
        }
        if Some(id) == keep {
            continue;
        }
        remove_files(data_dir, id);
        conn.execute("UPDATE game_manuals SET pages = NULL, outline = NULL, bytes = 0 WHERE game_id = ?1", [id])?;
        total -= b as u64;
    }
    Ok(())
}

/// The user's own file for a game's manual (always wins; downloads for it are deleted).
pub fn choose_file(conn: &Connection, data_dir: &Path, game_id: i64, path: &str) -> Result<()> {
    let lower = path.to_ascii_lowercase();
    if !(lower.ends_with(".pdf") || lower.ends_with(".cbz")) {
        return Err(Error::msg("Pick a PDF or CBZ file."));
    }
    let mut head = [0u8; 5];
    fs::File::open(path)?.read_exact(&mut head)?;
    let ok = if lower.ends_with(".pdf") { &head == b"%PDF-" } else { head.starts_with(b"PK\x03\x04") };
    if !ok {
        return Err(Error::msg("That file isn't a real PDF or CBZ."));
    }
    remove_files(data_dir, game_id);
    set_row(conn, game_id, "file", &file_name(path), Some(path), true)
}

/// A manual the user picked from the list (downloads when opened).
pub fn choose(conn: &Connection, data_dir: &Path, game_id: i64, url: &str, label: &str) -> Result<()> {
    if !sources::allowed_url(url) {
        return Err(Error::msg("KB only downloads manuals from the Internet Archive."));
    }
    remove_files(data_dir, game_id);
    let source = if url.starts_with("ia:") { "ia" } else { "retrokit" };
    set_row(conn, game_id, source, label, Some(url), true)
}

/// Forgets the game's manual (the user's pick included); it's looked up again when opened.
pub fn remove(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<()> {
    remove_files(data_dir, game_id);
    conn.execute("DELETE FROM game_manuals WHERE game_id = ?1", [game_id])?;
    Ok(())
}

/// Picker options: the lists' matches for the game (or the typed words), then an Internet Archive search.
pub fn candidates(conn: &Connection, fetch: &dyn Fetch, game_id: i64, query: Option<&str>) -> Result<Vec<Entry>> {
    let (platform, key) = game_key(conn, game_id)?;
    let typed = query.map(str::trim).filter(|q| !q.is_empty());
    let search_key = match typed {
        Some(q) => crate::scanner::normalize::parse(q).search_key,
        None => key.search_key.clone(),
    };
    let lists = sources::lists(conn, fetch, &platform);
    let mut out = sources::ranked(&lists, &search_key, key.region.as_deref(), 12);
    let title: String = match typed {
        Some(q) => q.to_string(),
        None => conn.query_row("SELECT title FROM games WHERE id = ?1", [game_id], |r| r.get(0))?,
    };
    match sources::search(fetch, &title) {
        Ok(found) => out.extend(found.into_iter().filter(|f| !out.iter().any(|o| o.url == f.url)).collect::<Vec<_>>()),
        Err(e) if out.is_empty() => return Err(e),
        Err(e) => log::warn!("Internet Archive search: {e}"),
    }
    Ok(out)
}

/// Downloaded manuals' size on disk (the user's own files aren't KB's).
pub fn bytes(data_dir: &Path) -> u64 {
    fn walk(d: &Path) -> u64 {
        fs::read_dir(d)
            .map(|it| {
                it.flatten()
                    .map(|e| match e.file_type() {
                        Ok(t) if t.is_dir() => walk(&e.path()),
                        _ => e.metadata().map(|m| m.len()).unwrap_or(0),
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    walk(&data_dir.join("manuals"))
}

/// Deletes every downloaded manual (they download again when opened). The user's own picks stay.
pub fn clear(conn: &Connection, data_dir: &Path) -> Result<()> {
    match fs::remove_dir_all(data_dir.join("manuals")) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    conn.execute("UPDATE game_manuals SET pages = NULL, outline = NULL, bytes = 0 WHERE source != 'file'", [])?;
    Ok(())
}

/// Folders of games that left the library.
pub fn prune(conn: &Connection, data_dir: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(data_dir.join("manuals")) else { return Ok(()) };
    for e in entries.flatten() {
        let id = e.file_name().to_str().and_then(|s| s.parse::<i64>().ok());
        let keep = match id {
            Some(id) => conn.query_row("SELECT COUNT(*) FROM games WHERE id = ?1", [id], |r| r.get::<_, i64>(0))? > 0,
            None => false,
        };
        if !keep {
            let _ = fs::remove_dir_all(e.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
