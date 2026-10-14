//! LaunchBox Games Database (keyless): box art, logos, backgrounds and game
//! info, used before KB's other sources (the owner's choice). The user's own
//! picks always win.
//!
//! LaunchBox publishes everything as one daily `Metadata.zip` (~100 MB) with no
//! per-game access, so the job downloads it, filters it while it streams in
//! (see `stream`) and keeps only:
//! - a name list (main and alternate titles) for the enabled platforms, ~2 MB,
//!   so new games and Fix match work without downloading again;
//! - details and image lists for the games in the library.
//!
//! It downloads only when there's something to gain: no name list yet,
//! matched games without details, or LaunchBox's file changed (checked at most
//! weekly with a tiny request) while some games still have no match; or when
//! the user asks ("Update now"). Images come from LaunchBox's image server on
//! demand, like the other sources' images.

pub mod matching;
pub mod stream;

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::art::http::Fetch;
use crate::art::images::{self, Extra};
use crate::db::queries;
use crate::error::{Error, Result};
use matching::{Candidates, Name, Outcome};
use stream::{LbGame, Record};

pub const ZIP_URL: &str = "https://gamesdb.launchbox-app.com/Metadata.zip";
pub const IMAGE_BASE: &str = "https://images.launchbox-app.com/";
/// How often LaunchBox's file is checked for changes (a HEAD request).
const CHECK_EVERY_SECS: i64 = 7 * 24 * 3600;
/// Image types kept (LaunchBox has ~30; KB shows these).
const IMAGE_KINDS: &[&str] = &["Box - Front", "Box - Front - Reconstructed", "Clear Logo", "Fanart - Background"];
const STAMP_KEY: &str = "launchbox";

/// What KB remembers about the last download (settings, `launchbox`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stamp {
    /// LaunchBox's `Last-Modified` / `ETag` for the imported file.
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub imported_at: Option<i64>,
    /// Last time the file was checked for changes.
    pub checked_at: Option<i64>,
}

/// Where the zip comes from (the network, or a test).
pub trait Source: Sync {
    /// LaunchBox's current `(Last-Modified, ETag, size)`.
    fn head(&self) -> Result<(Option<String>, Option<String>, Option<u64>)>;
    fn open(&self) -> Result<Box<dyn Read + Send>>;
}

pub struct Web {
    agent: ureq::Agent,
}

impl Web {
    pub fn new() -> Self {
        // No overall timeout: ~100 MB on a slow line can take a while. A
        // stalled connection still gives up eventually.
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .timeout_recv_body(Some(Duration::from_secs(45 * 60)))
            .user_agent(concat!("KaneegaBell/", env!("CARGO_PKG_VERSION"), " (personal game library)"))
            .build()
            .into();
        Self { agent }
    }
}

impl Source for Web {
    fn head(&self) -> Result<(Option<String>, Option<String>, Option<u64>)> {
        let resp = self.agent.head(ZIP_URL).call().map_err(|e| Error::msg(format!("LaunchBox: {e}")))?;
        let h = |k: &str| resp.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
        Ok((h("last-modified"), h("etag"), h("content-length").and_then(|v| v.parse().ok())))
    }
    fn open(&self) -> Result<Box<dyn Read + Send>> {
        let resp = self.agent.get(ZIP_URL).call().map_err(|e| Error::msg(format!("LaunchBox: {e}")))?;
        Ok(Box::new(resp.into_body().into_reader()))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// "checking" | "downloading" | "matching" | "images"
    pub phase: &'static str,
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub downloaded: bool,
    pub matched: usize,
    pub not_found: usize,
    pub ambiguous: usize,
    /// Games whose cover will now come from LaunchBox (an art pass follows).
    pub covers_queued: usize,
    pub heroes: usize,
    pub logos: usize,
    pub cancelled: bool,
}

/// A library game on a platform LaunchBox covers.
#[derive(Debug, Clone)]
struct Game {
    id: i64,
    platform_id: String,
    search_key: String,
    region: Option<String>,
    lb_id: Option<i64>,
    status: String,
}

fn library_games(conn: &Connection) -> Result<Vec<Game>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.platform_id, g.search_key, g.region, g.lb_id, g.lb_status
         FROM games g JOIN platforms p ON p.id = g.platform_id
         WHERE g.missing = 0 AND p.enabled = 1 AND p.launchbox_platform IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Game { id: r.get(0)?, platform_id: r.get(1)?, search_key: r.get(2)?, region: r.get(3)?, lb_id: r.get(4)?, status: r.get(5)? })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// LaunchBox platform name -> KB platform id, for the enabled platforms.
fn platform_map(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT launchbox_platform, id FROM platforms WHERE enabled = 1 AND launchbox_platform IS NOT NULL")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn matched(g: &Game) -> bool {
    g.lb_id.is_some() && (g.status == "ok" || g.status == "manual")
}

pub fn stamp(conn: &Connection) -> Result<Stamp> {
    Ok(queries::get_setting::<Stamp>(conn, STAMP_KEY)?.unwrap_or_default())
}

fn save_stamp(conn: &Connection, s: &Stamp) -> Result<()> {
    queries::set_setting(conn, STAMP_KEY, &serde_json::to_value(s)?)
}

pub fn enabled(conn: &Connection) -> Result<bool> {
    Ok(queries::get_setting::<bool>(conn, "lbEnabled")?.unwrap_or(true))
}

fn names_for(conn: &Connection, platform_id: &str) -> Result<Vec<Name>> {
    let mut stmt = conn.prepare_cached("SELECT db_id, name, region, is_alt FROM lb_names WHERE platform_id = ?1")?;
    let rows = stmt.query_map([platform_id], |r| Ok(Name { db_id: r.get(0)?, name: r.get(1)?, region: r.get(2)?, is_alt: r.get(3)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn has_details(conn: &Connection, db_id: i64) -> Result<bool> {
    Ok(conn.query_row("SELECT 1 FROM lb_games WHERE db_id = ?1", [db_id], |_| Ok(())).optional()?.is_some())
}

/// Whether a LaunchBox pass has work: games never tried, or matched games
/// whose details aren't stored yet. (The weekly change check is separate.)
pub fn wanted(conn: &Connection) -> Result<bool> {
    if !enabled(conn)? {
        return Ok(false);
    }
    Ok(conn
        .query_row(
            "SELECT 1 FROM games g JOIN platforms p ON p.id = g.platform_id
             WHERE g.missing = 0 AND p.enabled = 1 AND p.launchbox_platform IS NOT NULL
               AND (g.lb_status = 'none'
                    OR (g.lb_status IN ('ok', 'manual') AND NOT EXISTS (SELECT 1 FROM lb_games l WHERE l.db_id = g.lb_id)))
             LIMIT 1",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Whether the weekly "did LaunchBox's file change?" check is due (only
/// worth it while some games have no match).
pub fn check_due(conn: &Connection) -> Result<bool> {
    if !enabled(conn)? {
        return Ok(false);
    }
    let s = stamp(conn)?;
    let unmatched: bool = conn
        .query_row("SELECT 1 FROM games WHERE missing = 0 AND lb_status IN ('notfound', 'ambiguous') LIMIT 1", [], |_| Ok(()))
        .optional()?
        .is_some();
    Ok(unmatched && s.imported_at.is_some() && queries::now() - s.checked_at.unwrap_or(0) >= CHECK_EVERY_SECS)
}

fn set_match(conn: &Connection, game_id: i64, outcome: &Outcome) -> Result<()> {
    let (status, id) = match outcome {
        Outcome::Match(id) => ("ok", Some(*id)),
        Outcome::Ambiguous => ("ambiguous", None),
        Outcome::NotFound => ("notfound", None),
    };
    conn.execute(
        "UPDATE games SET lb_status = ?2, lb_id = ?3 WHERE id = ?1 AND lb_status != 'manual'",
        params![game_id, status, id],
    )?;
    Ok(())
}

/// Matches `games` against prepared candidates; returns the outcomes.
fn match_games(cands: &HashMap<String, Candidates>, games: &[&Game]) -> Vec<(i64, Outcome)> {
    games
        .iter()
        .filter_map(|g| {
            let c = cands.get(&g.platform_id)?;
            Some((g.id, if c.is_empty() { Outcome::NotFound } else { c.best(&g.search_key, g.region.as_deref()) }))
        })
        .collect()
}

/// Release date as unix seconds: the full date when given, else 1 January.
fn release_secs(g: &LbGame) -> Option<i64> {
    let from_ymd = |y: i32, m: u32, d: u32| -> Option<i64> {
        // Days from civil (Howard Hinnant's algorithm), UTC midnight.
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        let y = if m <= 2 { y - 1 } else { y } as i64;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m as i64 + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Some((era * 146_097 + doe - 719_468) * 86_400)
    };
    if let Some(d) = g.release_date.as_deref() {
        let mut p = d.get(..10)?.split('-');
        if let (Some(y), Some(m), Some(dd)) = (p.next(), p.next(), p.next()) {
            if let (Ok(y), Ok(m), Ok(dd)) = (y.parse(), m.parse(), dd.parse()) {
                return from_ymd(y, m, dd);
            }
        }
    }
    g.release_year.and_then(|y| from_ymd(y, 1, 1))
}

/// Everything one download keeps, written in one transaction.
#[derive(Default)]
struct Import {
    names: Vec<(String, Name)>,
    details: Vec<LbGame>,
    images: Vec<(i64, String, Option<String>, String)>,
    outcomes: Vec<(i64, Outcome)>,
    wanted: HashSet<i64>,
}

/// Downloads LaunchBox's file and filters it as it streams in.
fn download(
    conn: &Connection,
    source: &dyn Source,
    games: &[Game],
    progress: &dyn Fn(Progress),
    cancel: &AtomicBool,
) -> Result<(Import, (Option<String>, Option<String>))> {
    let (last_modified, etag, size) = source.head().unwrap_or((None, None, None));
    let platforms = platform_map(conn)?;
    let total = size.unwrap_or(0);
    let read = Arc::new(AtomicU64::new(0));
    let body = Counting { inner: source.open()?, read: read.clone() };
    let xml = stream::open_entry(body)?;

    let mut imp = Import::default();
    // Details of every game on the enabled platforms, held only until the
    // alternate titles are known (matching needs both); then trimmed to the
    // matched games before the images stream past.
    let mut buffered: HashMap<i64, LbGame> = HashMap::new();
    let mut platform_of: HashMap<i64, String> = HashMap::new();
    let mut resolved = false;
    let mut last_report = 0u64;
    let resolve = |imp: &mut Import, buffered: &mut HashMap<i64, LbGame>| {
        let mut cands: HashMap<String, Vec<Name>> = HashMap::new();
        for (p, n) in &imp.names {
            cands.entry(p.clone()).or_default().push(n.clone());
        }
        let cands: HashMap<String, Candidates> = cands.into_iter().map(|(p, n)| (p, Candidates::new(n))).collect();
        let todo: Vec<&Game> = games.iter().filter(|g| g.status != "manual").collect();
        imp.outcomes = match_games(&cands, &todo);
        imp.wanted = imp
            .outcomes
            .iter()
            .filter_map(|(_, o)| if let Outcome::Match(id) = o { Some(*id) } else { None })
            .chain(games.iter().filter(|g| g.status == "manual").filter_map(|g| g.lb_id))
            .collect();
        imp.details = imp.wanted.iter().filter_map(|id| buffered.remove(id)).collect();
        buffered.clear();
        buffered.shrink_to_fit();
    };
    stream::records(xml, |r| {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::msg("cancelled"));
        }
        let n = read.load(Ordering::Relaxed);
        if n - last_report > 1_000_000 {
            last_report = n;
            progress(Progress { phase: "downloading", done: n, total });
        }
        match r {
            Record::Game(g) => {
                if let Some(kb) = platforms.get(&g.platform) {
                    imp.names.push((kb.clone(), Name { db_id: g.db_id, name: g.name.clone(), region: None, is_alt: false }));
                    platform_of.insert(g.db_id, kb.clone());
                    buffered.insert(g.db_id, g);
                }
            }
            Record::AltName { db_id, name, region } => {
                if let Some(kb) = platform_of.get(&db_id) {
                    imp.names.push((kb.clone(), Name { db_id, name, region, is_alt: true }));
                }
            }
            Record::Image { db_id, file, kind, region } => {
                if !resolved {
                    resolve(&mut imp, &mut buffered);
                    resolved = true;
                }
                if imp.wanted.contains(&db_id) && IMAGE_KINDS.contains(&kind.as_str()) {
                    imp.images.push((db_id, kind, region, file));
                }
            }
        }
        Ok(())
    })?;
    if !resolved {
        resolve(&mut imp, &mut buffered);
    }
    progress(Progress { phase: "downloading", done: total.max(read.load(Ordering::Relaxed)), total });
    Ok((imp, (last_modified, etag)))
}

struct Counting<R> {
    inner: R,
    read: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

fn store(conn: &mut Connection, imp: &Import) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM lb_names", [])?;
    {
        let mut ins = tx.prepare("INSERT INTO lb_names (db_id, platform_id, name, region, is_alt) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for (p, n) in &imp.names {
            ins.execute(params![n.db_id, p, n.name, n.region, n.is_alt])?;
        }
    }
    // Details and images: exactly the library's matches.
    let keep: Vec<String> = imp.wanted.iter().map(i64::to_string).collect();
    let keep = keep.join(",");
    tx.execute(&format!("DELETE FROM lb_games WHERE db_id NOT IN ({keep})"), [])?;
    tx.execute(&format!("DELETE FROM lb_images WHERE db_id NOT IN ({keep})"), [])?;
    let now = queries::now();
    {
        let mut ins = tx.prepare(
            "INSERT INTO lb_games (db_id, name, release_date, overview, developer, publisher, genres, max_players, rating, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(db_id) DO UPDATE SET name = excluded.name, release_date = excluded.release_date,
                 overview = excluded.overview, developer = excluded.developer, publisher = excluded.publisher,
                 genres = excluded.genres, max_players = excluded.max_players, rating = excluded.rating,
                 fetched_at = excluded.fetched_at",
        )?;
        for g in &imp.details {
            ins.execute(params![g.db_id, g.name, release_secs(g), g.overview, g.developer, g.publisher, g.genres, g.max_players, g.rating, now])?;
        }
        let ids: Vec<i64> = imp.details.iter().map(|g| g.db_id).collect();
        let mut del = tx.prepare("DELETE FROM lb_images WHERE db_id = ?1")?;
        for id in &ids {
            del.execute([id])?;
        }
        let mut img = tx.prepare("INSERT OR IGNORE INTO lb_images (db_id, kind, region, file) VALUES (?1, ?2, ?3, ?4)")?;
        for (id, kind, region, file) in &imp.images {
            img.execute(params![id, kind, region, file])?;
        }
    }
    for (game_id, o) in &imp.outcomes {
        set_match(&tx, *game_id, o)?;
    }
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------- images

/// A LaunchBox image's URL.
pub fn image_url(file: &str) -> String {
    let encoded: String = file
        .split('/')
        .map(|s| percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string())
        .collect::<Vec<_>>()
        .join("/");
    format!("{IMAGE_BASE}{encoded}")
}

/// Image files of one kind for a LaunchBox game, best first: the game's own
/// region, then region-free/"World", then the rest. `kinds` are tried in order.
pub fn images_for(conn: &Connection, db_id: i64, kinds: &[&str], region: Option<&str>) -> Result<Vec<(String, Option<String>)>> {
    let mut stmt = conn.prepare_cached("SELECT kind, region, file FROM lb_images WHERE db_id = ?1")?;
    let rows: Vec<(String, Option<String>, String)> =
        stmt.query_map([db_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut out: Vec<(usize, u8, String, Option<String>)> = rows
        .into_iter()
        .filter_map(|(kind, reg, file)| {
            let k = kinds.iter().position(|x| *x == kind)?;
            let r = if matching::region_fits(region, reg.as_deref()) {
                0
            } else if reg.is_none() || reg.as_deref().is_some_and(|x| x.eq_ignore_ascii_case("world")) {
                1
            } else {
                2
            };
            Some((k, r, file, reg))
        })
        .collect();
    out.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
    Ok(out.into_iter().map(|(_, _, f, r)| (f, r)).collect())
}

const COVER_KINDS: &[&str] = &["Box - Front", "Box - Front - Reconstructed"];

/// Covers for a matched game as (file, URL), best first. A reconstructed box
/// (a clean re-creation) only comes after the real scans.
pub fn cover_urls(conn: &Connection, db_id: i64, region: Option<&str>) -> Result<Vec<(String, String)>> {
    Ok(images_for(conn, db_id, COVER_KINDS, region)?.into_iter().take(4).map(|(f, _)| {
        let url = image_url(&f);
        (f, url)
    }).collect())
}

/// Picker options from the game's LaunchBox match: covers (`extra` None),
/// backgrounds or logos.
pub fn picker(conn: &Connection, game_id: i64, extra: Option<Extra>) -> Result<Vec<crate::art::Candidate>> {
    let row: Option<(Option<i64>, String, Option<String>)> = conn
        .query_row("SELECT lb_id, lb_status, region FROM games WHERE id = ?1", [game_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?;
    let Some((Some(db_id), status, region)) = row else { return Ok(Vec::new()) };
    if status != "ok" && status != "manual" {
        return Ok(Vec::new());
    }
    let (kinds, label): (&[&str], &str) = match extra {
        None => (COVER_KINDS, "LaunchBox"),
        Some(Extra::Hero) => (&["Fanart - Background"], "LaunchBox background"),
        Some(Extra::Logo) => (&["Clear Logo"], "LaunchBox logo"),
    };
    Ok(images_for(conn, db_id, kinds, region.as_deref())?
        .into_iter()
        .map(|(file, reg)| {
            let url = image_url(&file);
            crate::art::Candidate {
                source: "launchbox".into(),
                name: file,
                preview: Some(url.clone()),
                url,
                label: match reg {
                    Some(r) => format!("{label} · {r}"),
                    None => label.to_string(),
                },
            }
        })
        .collect())
}

/// Stores a LaunchBox hero/logo unless the user chose theirs; true if stored.
fn fetch_extra(conn: &Connection, data_dir: &Path, fetch: &dyn Fetch, game_id: i64, db_id: i64, region: Option<&str>, extra: Extra) -> Result<bool> {
    let kind = match extra {
        Extra::Hero => "Fanart - Background",
        Extra::Logo => "Clear Logo",
    };
    let current: Option<String> = conn
        .query_row("SELECT source FROM artwork WHERE game_id = ?1 AND kind = ?2 AND selected = 1", params![game_id, extra.kind()], |r| r.get(0))
        .optional()?;
    if current.as_deref() == Some("launchbox") {
        return Ok(false);
    }
    for (file, _) in images_for(conn, db_id, &[kind], region)?.into_iter().take(2) {
        let url = image_url(&file);
        let Some(bytes) = fetch.get(&url)? else { continue };
        let (w, h) = images::store_extra(data_dir, extra, game_id, &bytes)?;
        queries::set_extra(conn, game_id, extra.kind(), "launchbox", &file, &url, w, h, queries::now())?;
        return Ok(true);
    }
    Ok(false)
}

// ---------------------------------------------------------------- the job

/// One LaunchBox pass (see the module docs). `force` downloads even when
/// nothing seems to need it ("Update now").
pub fn run(
    conn: &mut Connection,
    data_dir: &Path,
    source: &dyn Source,
    fetch: &dyn Fetch,
    force: bool,
    progress: &dyn Fn(Progress),
    cancel: &AtomicBool,
) -> Result<Summary> {
    let mut s = Summary::default();
    let mut games = library_games(conn)?;
    if games.is_empty() && !force {
        return Ok(s);
    }
    let mut st = stamp(conn)?;
    let have_names: bool = conn.query_row("SELECT 1 FROM lb_names LIMIT 1", [], |_| Ok(())).optional()?.is_some();

    // Offline first: new games against the stored name list.
    if have_names {
        progress(Progress { phase: "matching", done: 0, total: games.len() as u64 });
        let todo: Vec<&Game> = games.iter().filter(|g| g.status == "none").collect();
        if !todo.is_empty() {
            let mut cands = HashMap::new();
            for p in todo.iter().map(|g| g.platform_id.clone()).collect::<HashSet<_>>() {
                cands.insert(p.clone(), Candidates::new(names_for(conn, &p)?));
            }
            let tx = conn.transaction()?;
            for (id, o) in match_games(&cands, &todo) {
                set_match(&tx, id, &o)?;
            }
            tx.commit()?;
            games = library_games(conn)?;
        }
    }
    let missing_details = games.iter().filter(|g| matched(g)).try_fold(false, |acc, g| -> Result<bool> {
        Ok(acc || !has_details(conn, g.lb_id.unwrap_or_default())?)
    })?;
    let mut need = force || !have_names || missing_details;
    if !need && check_due(conn)? {
        progress(Progress { phase: "checking", done: 0, total: 1 });
        let (lm, etag, _) = source.head()?;
        st.checked_at = Some(queries::now());
        need = (lm.is_some() && lm != st.last_modified) || (etag.is_some() && etag != st.etag);
        save_stamp(conn, &st)?;
    }

    if need {
        log::info!("LaunchBox: downloading its game database to update {} games", games.len());
        let (imp, (lm, etag)) = match download(conn, source, &games, progress, cancel) {
            Err(_) if cancel.load(Ordering::Relaxed) => {
                s.cancelled = true;
                return Ok(s);
            }
            r => r?,
        };
        progress(Progress { phase: "matching", done: 0, total: games.len() as u64 });
        store(conn, &imp)?;
        let now = queries::now();
        st = Stamp { last_modified: lm, etag, imported_at: Some(now), checked_at: Some(now) };
        save_stamp(conn, &st)?;
        s.downloaded = true;
        games = library_games(conn)?;
    }

    // Apply: covers (queued for the art pass), genres, heroes and logos.
    // Every step is cheap when there's nothing to do (a cover or picture that
    // already is LaunchBox's, or the user's own, is left alone).
    let matched_games: Vec<&Game> = games.iter().filter(|g| matched(g)).collect();
    s.matched = matched_games.len();
    s.not_found = games.iter().filter(|g| g.status == "notfound").count();
    s.ambiguous = games.iter().filter(|g| g.status == "ambiguous").count();
    let total = matched_games.len() as u64;
    for (i, g) in matched_games.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            s.cancelled = true;
            break;
        }
        progress(Progress { phase: "images", done: i as u64, total });
        let db_id = g.lb_id.unwrap_or_default();
        apply_genres(conn, g.id, db_id)?;
        if queue_cover(conn, g.id, db_id, g.region.as_deref())? {
            s.covers_queued += 1;
        }
        let manual: bool = conn.query_row("SELECT extras_manual FROM games WHERE id = ?1", [g.id], |r| r.get(0))?;
        if !manual {
            for (extra, count) in [(Extra::Hero, &mut s.heroes), (Extra::Logo, &mut s.logos)] {
                match fetch_extra(conn, data_dir, fetch, g.id, db_id, g.region.as_deref(), extra) {
                    Ok(true) => *count += 1,
                    Ok(false) => {}
                    Err(e) => log::warn!("LaunchBox {} for game {}: {e}", extra.kind(), g.id),
                }
            }
        }
    }
    progress(Progress { phase: "images", done: total, total });
    log::info!("LaunchBox: {s:?}");
    Ok(s)
}

/// LaunchBox's genres become the game's genres (for the genre filter).
fn apply_genres(conn: &Connection, game_id: i64, db_id: i64) -> Result<()> {
    let genres: Option<String> =
        conn.query_row("SELECT genres FROM lb_games WHERE db_id = ?1", [db_id], |r| r.get(0)).optional()?.flatten();
    let Some(genres) = genres else { return Ok(()) };
    conn.execute("DELETE FROM game_genres WHERE game_id = ?1", [game_id])?;
    for g in genres.split(';').map(str::trim).filter(|g| !g.is_empty()) {
        conn.execute("INSERT OR IGNORE INTO genres (name) VALUES (?1)", [g])?;
        conn.execute(
            "INSERT OR IGNORE INTO game_genres (game_id, genre_id) SELECT ?1, id FROM genres WHERE name = ?2",
            params![game_id, g],
        )?;
    }
    Ok(())
}

/// Queues a LaunchBox cover for the art pass, unless the user chose the cover
/// or it already is LaunchBox's. Returns whether it was queued.
fn queue_cover(conn: &Connection, game_id: i64, db_id: i64, region: Option<&str>) -> Result<bool> {
    if cover_urls(conn, db_id, region)?.is_empty() {
        return Ok(false);
    }
    let n = conn.execute(
        "UPDATE games SET art_status = 'none', art_checked_at = NULL
         WHERE id = ?1 AND art_manual = 0
           AND NOT EXISTS (SELECT 1 FROM artwork a WHERE a.game_id = games.id AND a.kind = 'boxart'
                           AND a.selected = 1 AND a.source = 'launchbox')",
        [game_id],
    )?;
    Ok(n > 0)
}

// ---------------------------------------------------------------- Fix match

/// LaunchBox entries for a game's platform matching `query` (or its title).
pub fn search(conn: &Connection, game_id: i64, query: Option<&str>) -> Result<Vec<matching::Hit>> {
    let (platform, key): (String, String) =
        conn.query_row("SELECT platform_id, search_key FROM games WHERE id = ?1", [game_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let key = match query.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => crate::scanner::normalize::search_key(q),
        None => key,
    };
    Ok(Candidates::new(names_for(conn, &platform)?).search(&key, 40))
}

/// The user's pick: kept for good. Returns true when its details aren't
/// stored yet (a download fills them in).
pub fn set_manual(conn: &Connection, game_id: i64, db_id: i64) -> Result<bool> {
    conn.execute("UPDATE games SET lb_id = ?2, lb_status = 'manual' WHERE id = ?1", params![game_id, db_id])?;
    if has_details(conn, db_id)? {
        apply_genres(conn, game_id, db_id)?;
        let region: Option<String> = conn.query_row("SELECT region FROM games WHERE id = ?1", [game_id], |r| r.get(0))?;
        queue_cover(conn, game_id, db_id, region.as_deref())?;
        // Its own logo and background, not the previous match's.
        conn.execute("DELETE FROM artwork WHERE game_id = ?1 AND kind IN ('hero', 'logo') AND source = 'launchbox'", [game_id])?;
        return Ok(false);
    }
    Ok(true)
}

/// Settings → Game info: what LaunchBox has done.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub imported_at: Option<i64>,
    pub matched: i64,
    pub total: i64,
    pub not_found: i64,
    pub ambiguous: i64,
}

pub fn status(conn: &Connection) -> Result<Status> {
    let (total, matched, not_found, ambiguous) = conn.query_row(
        "SELECT COUNT(*),
                SUM(g.lb_status IN ('ok', 'manual')),
                SUM(g.lb_status = 'notfound'),
                SUM(g.lb_status = 'ambiguous')
         FROM games g JOIN platforms p ON p.id = g.platform_id
         WHERE g.missing = 0 AND p.enabled = 1 AND p.launchbox_platform IS NOT NULL",
        [],
        |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0), r.get::<_, Option<i64>>(2)?.unwrap_or(0), r.get::<_, Option<i64>>(3)?.unwrap_or(0))),
    )?;
    Ok(Status { enabled: enabled(conn)?, imported_at: stamp(conn)?.imported_at, matched, total, not_found, ambiguous })
}

#[cfg(test)]
mod tests;
