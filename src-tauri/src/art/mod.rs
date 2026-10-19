//! Artwork: keyless box art from the LaunchBox Games Database (first, for
//! matched games; see `launchbox`), libretro-thumbnails and GameTDB, cached
//! locally as WebP.
//!
//! The job runs in the background (after scans, or on demand): for each game
//! without art it matches the game against its platform's cached libretro
//! thumbnail index and/or looks it up on GameTDB by its ID, downloads the
//! cover, and stores a grid thumbnail plus a larger image for the details
//! view. Everything is cached (index listings in `api_cache`, images on disk),
//! so the library works fully offline afterwards.

pub mod color;
pub mod extras;
pub mod gametdb;
pub mod http;
pub mod icons;
pub mod images;
pub mod libretro;
pub mod matcher;
pub mod protocol;
pub mod sgdb;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use rusqlite::Connection;
use serde::Serialize;

use crate::db::queries;
use crate::error::Result;
use http::Fetch;

/// Thumbnail index listings are refreshed after this long (new covers appear upstream).
const INDEX_MAX_AGE_SECS: i64 = 14 * 24 * 3600;
/// Games with no match are retried after this long.
pub const NOTFOUND_RETRY_SECS: i64 = 7 * 24 * 3600;
/// Parallel downloads (the libretro server is a plain file server; be polite).
const WORKERS: usize = 4;

/// A game that needs art.
#[derive(Debug, Clone)]
pub struct Target {
    pub id: i64,
    pub platform_id: String,
    /// None for platforms libretro-thumbnails has no set for (e.g. Switch).
    pub libretro_system: Option<String>,
    pub launch_path: String,
    pub title: String,
    pub search_key: String,
    pub region: Option<String>,
    pub serial: Option<String>,
    /// Cover of the game's IGDB match, when it has one.
    pub igdb_cover: Option<String>,
    /// The game's LaunchBox match (its covers come first).
    pub lb_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub found: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub found: usize,
    pub not_found: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub duration_ms: u128,
}

/// Names worth trying verbatim against thumbnail names: the file stem, and for
/// folder-based games (PS3 EBOOT.BIN) the game folder's name.
fn candidate_names(launch_path: &str) -> Vec<String> {
    let p = Path::new(launch_path);
    let mut out = Vec::new();
    if let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().into_owned()) {
        if !stem.eq_ignore_ascii_case("eboot") {
            out.push(stem);
        }
    }
    // .../<Game (USA)>/PS3_GAME/USRDIR/EBOOT.BIN -> "<Game (USA)>"
    if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("EBOOT.BIN")) {
        if let Some(game_dir) = p.ancestors().nth(3) {
            if let Some(name) = game_dir.file_name() {
                out.push(name.to_string_lossy().into_owned());
            }
        }
    }
    out
}

/// The thumbnail index for a system: cached listing if fresh, else downloaded
/// (falling back to a stale cached copy when offline).
fn load_index(conn: &Connection, fetch: &dyn Fetch, system: &str) -> Result<Option<matcher::Index>> {
    let key = format!("libretro-index:{system}");
    let cached = queries::cache_get(conn, &key)?;
    let now = queries::now();
    let names: Vec<String> = match cached {
        Some((body, at)) if now - at < INDEX_MAX_AGE_SECS => serde_json::from_slice(&body)?,
        stale => match fetch.get(&libretro::index_url(system)) {
            Ok(Some(html)) => {
                let names = libretro::parse_index(&String::from_utf8_lossy(&html));
                queries::cache_put(conn, &key, &serde_json::to_vec(&names)?, now)?;
                names
            }
            Ok(None) => return Ok(None),
            Err(e) => match stale {
                Some((body, _)) => {
                    log::warn!("couldn't refresh {system} thumbnail index ({e}); using cached copy");
                    serde_json::from_slice(&body)?
                }
                None => return Err(e),
            },
        },
    };
    Ok(Some(matcher::Index::new(names)))
}

/// A PS3 game's `ICON0.PNG` (next to `USRDIR`, in disc folders and dev_hdd0
/// installs alike): the wide icon RPCS3 shows. Not a cover, so it's only
/// offered in the picker, never chosen automatically.
fn local_icon(launch_path: &str) -> Option<std::path::PathBuf> {
    let p = Path::new(launch_path);
    if !p.file_name()?.eq_ignore_ascii_case("EBOOT.BIN") {
        return None;
    }
    let icon = p.parent()?.parent()?.join("ICON0.PNG");
    icon.is_file().then_some(icon)
}

/// One place a cover might be; a job tries them in order until one exists.
#[derive(Clone)]
struct Try {
    source: &'static str,
    name: String,
    url: String,
}

#[derive(Clone)]
struct Job {
    id: i64,
    tries: Vec<Try>,
    /// The libretro index couldn't be loaded: if nothing else turns up, retry later.
    index_failed: bool,
}

fn gametdb_tries(platform: &str, serial: Option<&str>) -> Vec<Try> {
    gametdb::cover_urls(platform, serial)
        .into_iter()
        .map(|u| Try {
            source: "gametdb",
            name: format!("{} ({})", serial.unwrap_or_default().trim(), u.region),
            url: u.url,
        })
        .collect()
}

/// Where to look first: an exact ID lookup beats a name match for PS3
/// (libretro has few PS3 covers), except for PSN titles, where GameTDB only
/// has a generic "PlayStation Network" frame and the IGDB match's cover is
/// better; elsewhere libretro's sets are the most complete, GameTDB fills
/// gaps, and the IGDB match's cover comes last.
fn order_tries(platform: &str, serial: Option<&str>, libretro: Option<Try>, gametdb: Vec<Try>, igdb: Option<Try>) -> Vec<Try> {
    let psn = serial.is_some_and(|s| s.trim().to_ascii_uppercase().starts_with("NP"));
    match platform {
        "ps3" if psn => igdb.into_iter().chain(gametdb).chain(libretro).collect(),
        "ps3" => gametdb.into_iter().chain(igdb).chain(libretro).collect(),
        _ => libretro.into_iter().chain(gametdb).chain(igdb).collect(),
    }
}

fn igdb_try(url: Option<&str>) -> Option<Try> {
    url.map(|u| Try { source: "igdb", name: u.rsplit('/').next().unwrap_or(u).trim_end_matches(".jpg").to_string(), url: u.to_string() })
}

enum Outcome {
    Found { id: i64, source: &'static str, name: String, url: String, width: u32, height: u32 },
    NotFound { id: i64 },
    Failed { id: i64, error: String },
}

/// Fetches box art for every game that needs it.
pub fn run(
    conn: &mut Connection,
    data_dir: &Path,
    fetch: &dyn Fetch,
    progress: &dyn Fn(Progress),
    cancel: &AtomicBool,
) -> Result<Summary> {
    let started = Instant::now();
    let mut summary = Summary::default();
    let targets = queries::games_needing_art(conn, queries::now())?;
    if targets.is_empty() {
        return Ok(summary);
    }
    let total = targets.len();
    log::info!("fetching box art for {total} games");

    // Match everything first (fast, local), then download in parallel.
    let mut by_system: HashMap<Option<String>, Vec<Target>> = HashMap::new();
    for t in targets {
        by_system.entry(t.libretro_system.clone()).or_default().push(t);
    }
    let mut jobs: Vec<Job> = Vec::new();
    let mut outcomes: Vec<Outcome> = Vec::new();
    for (system, games) in &by_system {
        let index = match system {
            Some(system) => load_index(conn, fetch, system),
            None => Ok(None),
        };
        for g in games {
            let libretro = match (&index, system) {
                (Ok(Some(index)), Some(system)) => {
                    let q = matcher::Query {
                        names: candidate_names(&g.launch_path),
                        search_key: &g.search_key,
                        region: g.region.as_deref(),
                    };
                    index.best(&q).map(|m| {
                        if m.score < 1.0 {
                            log::info!("art: fuzzy match {:.2} '{}' -> '{}'", m.score, g.title, m.name);
                        }
                        Try { source: "libretro", name: m.name.to_string(), url: libretro::boxart_url(system, m.name) }
                    })
                }
                _ => None,
            };
            // LaunchBox first (the owner's choice), then the other sources.
            let launchbox = match g.lb_id {
                Some(db_id) => crate::launchbox::cover_urls(conn, db_id, g.region.as_deref())?,
                None => Vec::new(),
            };
            let tries: Vec<Try> = launchbox
                .into_iter()
                .map(|(name, url)| Try { source: "launchbox", name, url })
                .chain(order_tries(
                    &g.platform_id,
                    g.serial.as_deref(),
                    libretro,
                    gametdb_tries(&g.platform_id, g.serial.as_deref()),
                    igdb_try(g.igdb_cover.as_deref()),
                ))
                .collect();
            match (tries.is_empty(), &index) {
                (false, _) => jobs.push(Job { id: g.id, tries, index_failed: index.is_err() }),
                (true, Err(e)) => outcomes.push(Outcome::Failed { id: g.id, error: e.to_string() }),
                (true, _) => outcomes.push(Outcome::NotFound { id: g.id }),
            }
        }
    }

    // Record no-match / failure outcomes right away.
    let done = AtomicUsize::new(0);
    for o in outcomes.drain(..) {
        record(conn, data_dir, &o, &mut summary)?;
        done.fetch_add(1, Ordering::Relaxed);
    }
    progress(Progress { done: done.load(Ordering::Relaxed), total, found: summary.found });

    // Download + encode on workers; write results to the DB on this thread.
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<Outcome>();
    std::thread::scope(|s| -> Result<()> {
        for _ in 0..WORKERS.min(jobs.len()) {
            let tx = tx.clone();
            let (jobs, next) = (&jobs, &next);
            s.spawn(move || loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(i) else { break };
                let outcome = fetch_first(fetch, data_dir, job);
                if tx.send(outcome).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        for o in rx {
            record(conn, data_dir, &o, &mut summary)?;
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            progress(Progress { done: n, total, found: summary.found });
        }
        Ok(())
    })?;

    summary.cancelled = cancel.load(Ordering::Relaxed);
    summary.duration_ms = started.elapsed().as_millis();
    log::info!("box art: {summary:?}");
    Ok(summary)
}

/// Downloads the first cover that exists among a job's tries. A disc scan
/// filed as a box is passed over for the next try, and used only if nothing
/// else turns up.
fn fetch_first(fetch: &dyn Fetch, data_dir: &Path, job: &Job) -> Outcome {
    let id = job.id;
    let mut error = None;
    let mut disc: Option<(&Try, images::Processed)> = None;
    let keep = |t: &Try, p: images::Processed| match images::store(data_dir, id, &p) {
        Ok(()) => Outcome::Found { id, source: t.source, name: t.name.clone(), url: t.url.clone(), width: p.width, height: p.height },
        Err(e) => Outcome::Failed { id, error: e.to_string() },
    };
    for t in &job.tries {
        match fetch.get(&t.url) {
            Ok(Some(bytes)) => match images::process(&bytes) {
                Ok(p) if p.disc => {
                    log::info!("art: skipping a disc scan for game {id}: {}", t.name);
                    disc.get_or_insert((t, p));
                }
                Ok(p) => return keep(t, p),
                Err(e) => return Outcome::Failed { id, error: e.to_string() },
            },
            Ok(None) => {}
            Err(e) => error = Some(e.to_string()),
        }
    }
    if let Some((t, p)) = disc {
        return keep(t, p);
    }
    match error {
        Some(error) => Outcome::Failed { id, error },
        None if job.index_failed => Outcome::Failed { id, error: "cover index unavailable".into() },
        None => Outcome::NotFound { id },
    }
}

fn record(conn: &Connection, data_dir: &Path, o: &Outcome, summary: &mut Summary) -> Result<()> {
    let now = queries::now();
    match o {
        Outcome::Found { id, source, name, url, width, height } => {
            queries::set_boxart(conn, *id, source, name, url, *width, *height, now)?;
            icons::cover_changed(conn, data_dir, *id)?;
            summary.found += 1;
        }
        Outcome::NotFound { id } => {
            queries::set_art_status(conn, *id, "notfound", now)?;
            summary.not_found += 1;
        }
        Outcome::Failed { id, error } => {
            log::warn!("art for game {id} failed: {error}");
            queries::set_art_status(conn, *id, "error", now)?;
            summary.failed += 1;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- manual choice

/// What the picker needs to know about a game.
pub struct ArtInfo {
    pub platform_id: String,
    pub serial: Option<String>,
    pub igdb_cover: Option<String>,
    pub launch_path: String,
    pub search_key: String,
    pub region: Option<String>,
    pub libretro_system: Option<String>,
}

/// One option in the "Change cover" picker.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// "libretro" | "gametdb" | "igdb" | "sgdb" | "local"
    pub source: String,
    /// libretro name, or a file path for local images.
    pub name: String,
    /// What `set_cover` downloads/reads.
    pub url: String,
    /// Direct preview URL for the picker (remote covers only).
    pub preview: Option<String>,
    pub label: String,
}

/// Cover options for a game: GameTDB covers for its ID (one per region; the
/// picker hides regions that don't exist), libretro matches for its title (or
/// for `query`, when the user searches), SteamGridDB's covers (with a key),
/// and a PS3 game's own icon.
pub fn candidates(
    conn: &Connection,
    fetch: &dyn Fetch,
    sgdb: Option<&sgdb::Sgdb>,
    game_id: i64,
    query: Option<&str>,
) -> Result<Vec<Candidate>> {
    let t = queries::art_target(conn, game_id)?;
    let mut out = Vec::new();
    let searching = query.is_some_and(|q| !q.trim().is_empty());
    if !searching {
        // The LaunchBox match's covers first (one per region).
        out.extend(crate::launchbox::picker(conn, game_id, None)?);
        if let Some(url) = &t.igdb_cover {
            out.push(Candidate {
                source: "igdb".into(),
                name: igdb_try(Some(url)).map(|t| t.name).unwrap_or_default(),
                url: url.clone(),
                preview: Some(crate::metadata::igdb::preview_of(url)),
                label: "IGDB".into(),
            });
        }
        let mut seen = Vec::new();
        for u in gametdb::cover_urls(&t.platform_id, t.serial.as_deref()) {
            if seen.contains(&u.region) {
                continue;
            }
            seen.push(u.region);
            // Preview the medium size; `apply_choice` upgrades to the largest one.
            let preview = gametdb::preview_url(&u.url);
            out.push(Candidate {
                source: "gametdb".into(),
                name: format!("{} ({})", t.serial.as_deref().unwrap_or_default(), u.region),
                url: preview.clone(),
                preview: Some(preview),
                label: format!("GameTDB · {}", u.region),
            });
        }
    }
    if let Some(system) = t.libretro_system.as_deref() {
        if let Some(index) = load_index(conn, fetch, system)? {
            let key = match query.map(str::trim).filter(|q| !q.is_empty()) {
                Some(q) => crate::scanner::normalize::search_key(q),
                None => t.search_key.clone(),
            };
            for m in index.ranked(&key, t.region.as_deref(), 48) {
                let url = libretro::boxart_url(system, m.name);
                out.push(Candidate {
                    source: "libretro".into(),
                    name: m.name.to_string(),
                    preview: Some(url.clone()),
                    url,
                    label: m.name.to_string(),
                });
            }
        }
    }
    if let Some(sgdb) = sgdb {
        // Optional extra: an unreachable SteamGridDB doesn't empty the picker.
        match extras::candidates(conn, sgdb, game_id, sgdb::Kind::Grid, query) {
            Ok(c) => out.extend(c),
            Err(e) => log::warn!("SteamGridDB covers: {e}"),
        }
    }
    if !searching {
        if let Some(icon) = local_icon(&t.launch_path) {
            out.push(Candidate {
                source: "local".into(),
                name: icon.to_string_lossy().into_owned(),
                url: icon.to_string_lossy().into_owned(),
                preview: None,
                label: "The game's own icon".into(),
            });
        }
    }
    Ok(out)
}

/// Sets a game's cover from a picker choice or a user image file. Manual
/// choices are kept: automatic fetching only touches games without art.
pub fn apply_choice(conn: &Connection, data_dir: &Path, fetch: &dyn Fetch, game_id: i64, source: &str, name: &str, url: &str) -> Result<()> {
    let gone = || crate::error::Error::msg("that cover is no longer available");
    let bytes = match source {
        "libretro" | "igdb" | "sgdb" | "launchbox" => fetch.get(url)?.ok_or_else(gone)?,
        "gametdb" => match fetch.get(&gametdb::best_url(url))? {
            Some(b) => b,
            None => fetch.get(url)?.ok_or_else(gone)?,
        },
        "local" | "file" => std::fs::read(url)?,
        other => return Err(crate::error::Error::msg(format!("unknown cover source {other}"))),
    };
    let p = images::process(&bytes)?;
    images::store(data_dir, game_id, &p)?;
    queries::set_boxart(conn, game_id, source, name, url, p.width, p.height, queries::now())?;
    icons::cover_changed(conn, data_dir, game_id)?;
    queries::set_art_manual(conn, game_id)
}

/// Removes a game's cover and keeps it removed (the title card shows instead).
pub fn remove(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<()> {
    for p in [images::thumb_path(data_dir, game_id), images::full_path(data_dir, game_id)] {
        let _ = std::fs::remove_file(p);
    }
    queries::reset_art(conn, game_id)?;
    icons::cover_changed(conn, data_dir, game_id)?;
    queries::set_art_manual(conn, game_id)?;
    queries::set_art_status(conn, game_id, "removed", queries::now())
}

/// "Refresh" in the details: looks the cover up again, unless it's one the
/// user picked (or removed), which stays. Returns whether a lookup is due.
pub fn refresh(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<bool> {
    let manual: bool = conn.query_row("SELECT art_manual FROM games WHERE id = ?1", [game_id], |r| r.get(0))?;
    if manual {
        return Ok(false);
    }
    reset(conn, data_dir, game_id)?;
    Ok(true)
}

/// Forgets a game's art so the next job fetches it again.
pub fn reset(conn: &Connection, data_dir: &Path, game_id: i64) -> Result<()> {
    for p in [images::thumb_path(data_dir, game_id), images::full_path(data_dir, game_id)] {
        let _ = std::fs::remove_file(p);
    }
    queries::reset_art(conn, game_id)?;
    icons::cover_changed(conn, data_dir, game_id)
}

#[cfg(test)]
fn key_for(title: &str) -> String {
    crate::scanner::normalize::search_key(title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::fake::FakeHttp;

    const PS2: &str = "Sony - PlayStation 2";

    #[test]
    fn refresh_keeps_a_cover_the_user_picked() {
        let conn = db_with_games(&[("Picked", "a.iso", "USA"), ("Automatic", "b.iso", "USA")]);
        let data = tempfile::tempdir().unwrap();
        for id in [1, 2] {
            queries::set_boxart(&conn, id, "launchbox", "box", "https://img/box.png", 10, 14, 0).unwrap();
        }
        queries::set_art_manual(&conn, 1).unwrap();
        let status = |id: i64| -> String { conn.query_row("SELECT art_status FROM games WHERE id = ?1", [id], |r| r.get(0)).unwrap() };

        assert!(!refresh(&conn, data.path(), 1).unwrap(), "the user's pick stays");
        assert_eq!(status(1), "ok");
        assert!(refresh(&conn, data.path(), 2).unwrap(), "an automatic cover is looked up again");
        assert_eq!(status(2), "none");
    }

    fn db_with_games(games: &[(&str, &str, &str)]) -> Connection {
        let conn = crate::db::test_db();
        for (title, path, region) in games {
            conn.execute(
                "INSERT INTO games (platform_id, title, sort_title, search_key, region, launch_path, added_at)
                 VALUES ('ps2', ?1, ?1, ?2, ?3, ?4, 0)",
                rusqlite::params![title, key_for(title), region, path],
            )
            .unwrap();
        }
        conn
    }

    fn listing(names: &[&str]) -> Vec<u8> {
        names
            .iter()
            .map(|n| format!("<a href=\"{}.png\">x</a>\n", percent_encoding::utf8_percent_encode(n, percent_encoding::NON_ALPHANUMERIC)))
            .collect::<String>()
            .into_bytes()
    }

    fn status(conn: &Connection, title: &str) -> (String, i64) {
        conn.query_row("SELECT art_status, art_version FROM games WHERE title = ?1", [title], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
    }

    #[test]
    fn fetches_matches_caches_and_skips_done_games() {
        let mut conn = db_with_games(&[
            ("Jak II", r"E:\PS2\Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00).iso", "USA"),
            ("Shadow of the Colossus", r"E:\PS2\Shadow of the Colossus (USA).iso", "USA"),
            ("Homebrew Thing", r"E:\PS2\homebrew.iso", ""),
        ]);
        let data = tempfile::tempdir().unwrap();
        let mut http = FakeHttp::default();
        http.pages.insert(
            libretro::index_url(PS2),
            listing(&["Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00)", "Shadow of the Colossus (USA)"]),
        );
        http.pages.insert(libretro::boxart_url(PS2, "Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00)"), images::test_png(600, 850));
        http.pages.insert(libretro::boxart_url(PS2, "Shadow of the Colossus (USA)"), images::test_png(600, 850));

        let s = run(&mut conn, data.path(), &http, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((s.found, s.not_found, s.failed), (2, 1, 0));
        assert_eq!(status(&conn, "Jak II").0, "ok");
        assert!(status(&conn, "Jak II").1 > 0);
        assert_eq!(status(&conn, "Homebrew Thing").0, "notfound");
        let jak: i64 = conn.query_row("SELECT id FROM games WHERE title = 'Jak II'", [], |r| r.get(0)).unwrap();
        assert!(images::thumb_path(data.path(), jak).is_file());

        // Second run: nothing to do, and the index isn't downloaded again.
        let before = http.requests.lock().unwrap().len();
        let s2 = run(&mut conn, data.path(), &http, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((s2.found, s2.not_found), (0, 0));
        assert_eq!(http.requests.lock().unwrap().len(), before);
    }

    #[test]
    fn offline_with_no_cached_index_marks_errors_and_retries_later() {
        let mut conn = db_with_games(&[("Jak II", r"E:\PS2\Jak II (USA).iso", "USA")]);
        let data = tempfile::tempdir().unwrap();
        let http = FakeHttp { offline: true, ..Default::default() };
        let s = run(&mut conn, data.path(), &http, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(s.failed, 1);
        assert_eq!(status(&conn, "Jak II").0, "error");
        // 'error' games are picked up again on the next run.
        assert_eq!(queries::games_needing_art(&conn, queries::now()).unwrap().len(), 1);
    }

    #[test]
    fn stale_cached_index_is_used_when_offline() {
        let mut conn = db_with_games(&[("Jak II", r"E:\PS2\Jak II (USA).iso", "USA")]);
        queries::cache_put(&conn, &format!("libretro-index:{PS2}"), br#"["Jak II (USA)"]"#, 0).unwrap();
        let data = tempfile::tempdir().unwrap();
        let http = FakeHttp { offline: true, ..Default::default() };
        let s = run(&mut conn, data.path(), &http, &|_| {}, &AtomicBool::new(false)).unwrap();
        // Index came from the cache; only the image download failed.
        assert_eq!(s.failed, 1);
        let reqs = http.requests.lock().unwrap();
        assert!(reqs.iter().any(|u| u.ends_with("Jak%20II%20(USA).png")), "{reqs:?}");
    }

    /// Live run against a *copy* of a real library.db (network required):
    /// `EMU_DB_COPY=copy.db EMU_ART_DIR=tmpdir cargo test real_art_fetch -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_art_fetch() {
        let db = std::env::var("EMU_DB_COPY").expect("EMU_DB_COPY");
        let dir = std::env::var("EMU_ART_DIR").expect("EMU_ART_DIR");
        let mut conn = crate::db::open(Path::new(&db)).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        crate::platforms::seed(&conn).unwrap();
        let s = run(&mut conn, Path::new(&dir), &http::Http::new(), &|p| {
            if p.done % 20 == 0 || p.done == p.total {
                println!("progress {}/{} found {}", p.done, p.total, p.found);
            }
        }, &AtomicBool::new(false))
        .unwrap();
        println!("summary: {s:#?}");
        let mut stmt = conn
            .prepare("SELECT platform_id, title, art_status, (SELECT remote_id FROM artwork a WHERE a.game_id = g.id AND a.selected = 1) FROM games g WHERE missing = 0 ORDER BY platform_id, sort_title")
            .unwrap();
        for row in stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?)))
            .unwrap()
        {
            let (p, t, s, n) = row.unwrap();
            println!("{p:<6} {s:<8} {t:<45} -> {}", n.unwrap_or_default());
        }
    }

    /// A PS3 game in a folder with its ICON0.PNG; returns the EBOOT path.
    fn ps3_game(dir: &Path, folder: &str) -> String {
        let game = dir.join(folder).join("PS3_GAME");
        std::fs::create_dir_all(game.join("USRDIR")).unwrap();
        std::fs::write(game.join("USRDIR").join("EBOOT.BIN"), b"SCE").unwrap();
        std::fs::write(game.join("ICON0.PNG"), images::test_png(320, 176)).unwrap();
        game.join("USRDIR").join("EBOOT.BIN").to_string_lossy().into_owned()
    }

    fn insert_ps3(conn: &Connection, title: &str, serial: &str, eboot: &str) {
        conn.execute(
            "INSERT INTO games (platform_id, title, sort_title, search_key, serial, launch_path, added_at)
             VALUES ('ps3', ?1, ?1, ?2, ?3, ?4, 0)",
            rusqlite::params![title, key_for(title), serial, eboot],
        )
        .unwrap();
    }

    #[test]
    fn ps3_covers_come_from_gametdb_by_title_id_never_the_icon() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::test_db();
        insert_ps3(&conn, "Armored Core V", "BLUS30516", &ps3_game(dir.path(), "Armored Core V (USA)"));
        insert_ps3(&conn, "Bleach: Soul Resurreccion", "BLES01315", &ps3_game(dir.path(), "Bleach (Europe)"));
        insert_ps3(&conn, "Kane & Lynch 2", "BLUS30383", &ps3_game(dir.path(), "Kane & Lynch 2 (USA)"));
        let data = tempfile::tempdir().unwrap();
        let mut http = FakeHttp::default();
        http.pages.insert(libretro::index_url("Sony - PlayStation 3"), listing(&[]));
        // Only the medium US size exists for one, the big EN one for the other.
        http.pages.insert("https://art.gametdb.com/ps3/coverM/US/BLUS30516.jpg".into(), images::test_png(350, 408));
        http.pages.insert("https://art.gametdb.com/ps3/coverHQ/EN/BLES01315.jpg".into(), images::test_png(928, 1080));

        let s = run(&mut conn, data.path(), &http, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((s.found, s.not_found, s.failed), (2, 1, 0));
        let rows: Vec<(String, String)> = conn
            .prepare("SELECT source, remote_id FROM artwork ORDER BY remote_id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            rows,
            [("gametdb".to_string(), "BLES01315 (EN)".to_string()), ("gametdb".to_string(), "BLUS30516 (US)".to_string())]
        );
        // No cover anywhere: a title card, not the icon.
        assert_eq!(status(&conn, "Kane & Lynch 2").0, "notfound");

        // The picker still offers the icon, plus GameTDB regions.
        let id: i64 = conn.query_row("SELECT id FROM games WHERE title = 'Kane & Lynch 2'", [], |r| r.get(0)).unwrap();
        let c = candidates(&conn, &http, None, id, None).unwrap();
        assert_eq!(c[0].url, "https://art.gametdb.com/ps3/coverM/US/BLUS30383.jpg");
        assert_eq!(c.last().unwrap().source, "local");
    }

    #[test]
    fn a_platform_without_a_libretro_set_still_gets_launchbox_covers() {
        let mut conn = crate::db::test_db();
        conn.execute(
            "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at, lb_status, lb_id)
             VALUES ('switch', 'Bayonetta 2', 'bayonetta 00002', 'bayonetta 2', 'E:\\Switch\\Bayonetta 2.xci', 0, 'ok', 5)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO lb_images (db_id, kind, region, file) VALUES (5, 'Box - Front', 'North America', 'box.jpg')", []).unwrap();
        let mut http = FakeHttp::default();
        http.pages.insert(crate::launchbox::image_url("box.jpg"), images::test_png(300, 480));
        let data = tempfile::tempdir().unwrap();
        let s = run(&mut conn, data.path(), &http, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((s.found, s.failed), (1, 0));
        assert_eq!(status(&conn, "Bayonetta 2").0, "ok");
    }

    #[test]
    fn psn_titles_prefer_the_igdb_cover_over_gametdbs_generic_frame() {
        let igdb = igdb_try(Some("https://images.igdb.com/igdb/image/upload/t_1080p/co9.jpg"));
        let tdb = gametdb_tries("ps3", Some("NPUB30024"));
        let order: Vec<&str> =
            order_tries("ps3", Some("NPUB30024"), None, tdb.clone(), igdb.clone()).iter().map(|t| t.source).collect::<Vec<_>>();
        assert_eq!(order[0], "igdb");
        let disc: Vec<&str> = order_tries("ps3", Some("BLUS30443"), None, gametdb_tries("ps3", Some("BLUS30443")), igdb.clone())
            .iter()
            .map(|t| t.source)
            .collect();
        assert_eq!((disc[0], *disc.last().unwrap()), ("gametdb", "igdb"));
        assert_eq!(igdb.unwrap().name, "co9");
    }

    #[test]
    fn manual_choice_and_removal_stick() {
        let mut conn = db_with_games(&[("Pokemon: Radical Red", r"E:\GBA\Pokemon - Radical Red.gba", "")]);
        let id: i64 = conn.query_row("SELECT id FROM games", [], |r| r.get(0)).unwrap();
        let data = tempfile::tempdir().unwrap();
        let mut http = FakeHttp::default();
        http.pages.insert(
            libretro::index_url(PS2),
            listing(&["Pokemon - FireRed Version (USA)", "Pokemon - LeafGreen Version (USA)"]),
        );
        let url = libretro::boxart_url(PS2, "Pokemon - FireRed Version (USA)");
        http.pages.insert(url.clone(), images::test_png(500, 500));

        // Searching the set finds the base game for a romhack.
        let c = candidates(&conn, &http, None, id, Some("pokemon firered")).unwrap();
        assert_eq!(c[0].name, "Pokemon - FireRed Version (USA)");
        apply_choice(&conn, data.path(), &http, id, "libretro", &c[0].name, &c[0].url).unwrap();
        assert_eq!(status(&conn, "Pokemon: Radical Red").0, "ok");
        // Auto-fetch leaves a chosen cover alone.
        assert!(queries::games_needing_art(&conn, queries::now()).unwrap().is_empty());

        // Removing keeps it removed (no automatic re-fetch).
        remove(&conn, data.path(), id).unwrap();
        assert_eq!(status(&conn, "Pokemon: Radical Red").0, "removed");
        assert!(!images::thumb_path(data.path(), id).exists());
        let s = run(&mut conn, data.path(), &http, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(s.found + s.not_found, 0);

        // A user image file works too.
        let file = data.path().join("mine.png");
        std::fs::write(&file, images::test_png(300, 300)).unwrap();
        let path = file.to_string_lossy().into_owned();
        apply_choice(&conn, data.path(), &http, id, "file", &path, &path).unwrap();
        assert_eq!(status(&conn, "Pokemon: Radical Red").0, "ok");
    }

    #[test]
    fn ps3_folder_names_are_tried() {
        assert_eq!(
            candidate_names(r"E:\PS3\Darkness, The (USA)\PS3_GAME\USRDIR\EBOOT.BIN"),
            ["Darkness, The (USA)"]
        );
        assert_eq!(candidate_names(r"E:\PS1\FF7 (Disc 1).cue"), ["FF7 (Disc 1)"]);
    }
}
