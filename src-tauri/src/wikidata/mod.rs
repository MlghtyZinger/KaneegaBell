//! Wikidata (keyless, CC0): each game's series and franchise, for the game
//! details and, later, automatic series collections.
//!
//! Wikidata's query service answers SPARQL; one query per platform lists every
//! game on it (titles, alternate titles, first release year, IGDB ID). KB
//! matches the library against that in memory and keeps only the matched
//! entries plus their series; the rest is never stored. A second, small query
//! fetches the series of the matched games only.
//!
//! A platform is asked again only when it has games never looked up, or games
//! still without a match and its last look is a month old, or on "Update now".
//! The service asks clients to name themselves (User-Agent with a contact) and
//! gives each one 60 s of query time per minute; KB's queries run one at a
//! time and take a few seconds.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::db::queries;
use crate::error::{Error, Result};
use crate::launchbox::matching::{Candidates, Name, Outcome};
use crate::platforms::PLATFORMS;

pub const ENDPOINT: &str = "https://query.wikidata.org/sparql";
/// The owner's public GitHub profile is the contact (their choice).
pub const USER_AGENT: &str = concat!("KaneegaBell/", env!("CARGO_PKG_VERSION"), " (https://github.com/MlghtyZinger; personal game library)");
/// A platform with unmatched games is asked again after this long.
const RECHECK_SECS: i64 = 30 * 24 * 3600;
/// Matched games per series query.
const BATCH: usize = 150;
/// Settings key: { platform id: last checked (unix seconds) }.
const STAMP_KEY: &str = "wikidata";

/// One result row: variable name -> value.
pub type Row = HashMap<String, String>;

/// Where query results come from (the network, or a test).
pub trait Source: Sync {
    fn select(&self, query: &str) -> Result<Vec<Row>>;
}

pub struct Web {
    agent: ureq::Agent,
}

impl Web {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            // The service stops queries after 60 s.
            .timeout_global(Some(Duration::from_secs(90)))
            .user_agent(USER_AGENT)
            .http_status_as_error(false)
            .build()
            .into();
        Self { agent }
    }
}

impl Source for Web {
    fn select(&self, query: &str) -> Result<Vec<Row>> {
        let err = |e: &dyn std::fmt::Display| Error::msg(format!("Wikidata: {e}"));
        for attempt in 0..2 {
            let mut resp = self
                .agent
                .get(ENDPOINT)
                .query("query", query)
                .header("Accept", "application/sparql-results+json")
                .call()
                .map_err(|e| err(&e))?;
            let status = resp.status().as_u16();
            if status == 429 && attempt == 0 {
                // Over the per-minute budget: wait as asked (capped), once.
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(30)
                    .min(120);
                std::thread::sleep(Duration::from_secs(wait));
                continue;
            }
            if matches!(status, 500 | 502 | 503) && attempt == 0 {
                // A busy moment on their side: one more try shortly.
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
            if status != 200 {
                return Err(Error::msg(format!("Wikidata answered {status}")));
            }
            let body = resp.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec().map_err(|e| err(&e))?;
            return parse_bindings(&body);
        }
        Err(Error::msg("Wikidata is busy; try again later"))
    }
}

/// SPARQL JSON results -> rows of plain values.
pub fn parse_bindings(body: &[u8]) -> Result<Vec<Row>> {
    let v: serde_json::Value = serde_json::from_slice(body)?;
    let Some(rows) = v.pointer("/results/bindings").and_then(|b| b.as_array()) else {
        return Err(Error::msg("Wikidata: unexpected answer"));
    };
    Ok(rows
        .iter()
        .filter_map(|r| r.as_object())
        .map(|r| {
            r.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.get("value")?.as_str()?.to_string())))
                .collect()
        })
        .collect())
}

/// `http://www.wikidata.org/entity/Q123` -> 123.
fn qid(uri: &str) -> Option<i64> {
    uri.rsplit('/').next()?.strip_prefix('Q')?.parse().ok()
}

/// The Wikidata item of a KB platform.
pub fn platform_item(platform_id: &str) -> Option<&'static str> {
    PLATFORMS.iter().find(|p| p.id == platform_id).and_then(|p| p.wikidata)
}

/// `'snes','n64',...`: the platforms Wikidata is asked about, for SQL.
fn platform_list() -> String {
    PLATFORMS.iter().filter(|p| p.wikidata.is_some()).map(|p| format!("'{}'", p.id)).collect::<Vec<_>>().join(",")
}

/// Every game on a platform with its English titles (`alt` = 1: an
/// alternate title). Kept apart from `info_query`: together they're too slow
/// for the service (PS2: ~5 s and ~2 s apart, a timeout combined).
pub fn names_query(platform_item: &str) -> String {
    format!(
        "SELECT ?g ?name ?alt WHERE {{
           ?g wdt:P400 wd:{platform_item} .
           {{ ?g rdfs:label ?name BIND(0 AS ?alt) }} UNION {{ ?g skos:altLabel ?name BIND(1 AS ?alt) }}
           FILTER(LANG(?name) = \"en\")
         }}"
    )
}

/// Every game on a platform with its first release year and IGDB ID.
pub fn info_query(platform_item: &str) -> String {
    format!(
        "SELECT ?g (MIN(YEAR(?d)) AS ?y) (SAMPLE(?i) AS ?igdb) WHERE {{
           ?g wdt:P400 wd:{platform_item} .
           OPTIONAL {{ ?g wdt:P577 ?d }}
           OPTIONAL {{ ?g wdt:P5794 ?i }}
         }} GROUP BY ?g"
    )
}

/// The series (P179, with the game's number in it: P1545) and franchises
/// (P8345) of the given games, with English names. The service's planner
/// times out on this unless told to keep the written order (a few seconds
/// with the hint).
pub fn series_query(qids: &[i64]) -> String {
    let values: Vec<String> = qids.iter().map(|q| format!("wd:Q{q}")).collect();
    format!(
        "SELECT ?g ?s ?sl ?kind ?ord WHERE {{
           hint:Query hint:optimizer \"None\" .
           VALUES ?g {{ {} }}
           {{ ?g p:P179 ?st . ?st ps:P179 ?s . OPTIONAL {{ ?st pq:P1545 ?ord }} BIND(\"series\" AS ?kind) }}
           UNION {{ ?g wdt:P8345 ?s BIND(\"franchise\" AS ?kind) }}
           ?s rdfs:label ?sl FILTER(LANG(?sl) = \"en\")
         }}",
        values.join(" ")
    )
}

/// The same without the hint (in case the service stops accepting it): its
/// label service names each series (or gives back the bare item id).
pub fn series_query_plain(qids: &[i64]) -> String {
    let values: Vec<String> = qids.iter().map(|q| format!("wd:Q{q}")).collect();
    format!(
        "SELECT ?g ?s ?sl ?kind ?ord WHERE {{
           VALUES ?g {{ {} }}
           {{ ?g p:P179 ?st . ?st ps:P179 ?s . OPTIONAL {{ ?st pq:P1545 ?ord }} BIND(\"series\" AS ?kind) }}
           UNION {{ ?g wdt:P8345 ?s BIND(\"franchise\" AS ?kind) }}
           SERVICE wikibase:label {{ bd:serviceParam wikibase:language \"en\". ?s rdfs:label ?sl . }}
         }}",
        values.join(" ")
    )
}

pub fn enabled(conn: &Connection) -> Result<bool> {
    Ok(queries::get_setting::<bool>(conn, "wdEnabled")?.unwrap_or(true))
}

fn stamps(conn: &Connection) -> Result<HashMap<String, i64>> {
    Ok(queries::get_setting::<HashMap<String, i64>>(conn, STAMP_KEY)?.unwrap_or_default())
}

/// A library game on a platform Wikidata is asked about, with what helps
/// matching it.
#[derive(Debug, Clone)]
struct Game {
    id: i64,
    platform_id: String,
    search_key: String,
    region: Option<String>,
    status: String,
    has_entry: bool,
    /// The LaunchBox match's name and release year (better titles than file names).
    lb_name: Option<String>,
    year: Option<i64>,
    /// The IGDB match's URL name: exact when Wikidata lists it.
    igdb: Option<String>,
}

fn library_games(conn: &Connection) -> Result<Vec<Game>> {
    let sql = format!(
        "SELECT g.id, g.platform_id, g.search_key, g.region, g.wd_status,
                EXISTS (SELECT 1 FROM wd_games w WHERE w.qid = g.wd_id),
                l.name, COALESCE(l.release_date, m.release_date), NULLIF(m.slug, '')
         FROM games g
         JOIN platforms p ON p.id = g.platform_id
         LEFT JOIN lb_games l ON l.db_id = g.lb_id AND g.lb_status IN ('ok', 'manual')
         LEFT JOIN metadata m ON m.game_id = g.id
         WHERE g.missing = 0 AND p.enabled = 1 AND g.platform_id IN ({})",
        platform_list()
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |r| {
        let released: Option<i64> = r.get(7)?;
        Ok(Game {
            id: r.get(0)?,
            platform_id: r.get(1)?,
            search_key: r.get(2)?,
            region: r.get(3)?,
            status: r.get(4)?,
            has_entry: r.get(5)?,
            lb_name: r.get(6)?,
            year: released.map(year_of),
            igdb: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Unix seconds -> calendar year (UTC).
fn year_of(secs: i64) -> i64 {
    // Days to civil date (Howard Hinnant's algorithm), year only.
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    yoe + era * 400 + i64::from(mp >= 10)
}

/// Platforms worth asking (see the module docs).
fn platforms_to_ask(games: &[Game], stamps: &HashMap<String, i64>, force: bool, now: i64) -> Vec<String> {
    let mut out: Vec<String> = games
        .iter()
        .filter(|g| {
            force
                || g.status == "none"
                || (g.status == "ok" && !g.has_entry)
                || (g.status != "ok" && now - stamps.get(&g.platform_id).copied().unwrap_or(0) >= RECHECK_SECS)
        })
        .map(|g| g.platform_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    out.sort_by_key(|p| PLATFORMS.iter().position(|d| d.id == p));
    out
}

/// Whether a pass has work (games never looked up, a matched game whose entry
/// was cleared, or a month-old platform with unmatched games).
pub fn wanted(conn: &Connection) -> Result<bool> {
    if !enabled(conn)? {
        return Ok(false);
    }
    Ok(!platforms_to_ask(&library_games(conn)?, &stamps(conn)?, false, queries::now()).is_empty())
}

/// An entry's main name, first release year and IGDB ID.
type Info = (Option<String>, Option<i64>, Option<String>);

/// One platform's entries, as returned by the names query.
struct Listing {
    names: Vec<Name>,
    info: HashMap<i64, Info>,
}

fn listing(name_rows: Vec<Row>, info_rows: Vec<Row>) -> Listing {
    let mut names = Vec::new();
    let mut info: HashMap<i64, Info> = HashMap::new();
    for r in name_rows {
        let (Some(q), Some(name)) = (r.get("g").and_then(|g| qid(g)), r.get("name")) else { continue };
        let is_alt = r.get("alt").map(String::as_str) == Some("1");
        if !is_alt {
            info.entry(q).or_default().0 = Some(name.clone());
        }
        names.push(Name { db_id: q, name: name.clone(), region: None, is_alt });
    }
    for r in info_rows {
        let Some(q) = r.get("g").and_then(|g| qid(g)) else { continue };
        let e = info.entry(q).or_default();
        e.1 = r.get("y").and_then(|y| y.parse().ok());
        e.2 = r.get("igdb").cloned();
    }
    Listing { names, info }
}

fn match_game(g: &Game, cands: &Candidates, list: &Listing) -> Outcome {
    if let Some(slug) = &g.igdb {
        let mut hits = list.info.iter().filter(|(_, i)| i.2.as_deref() == Some(slug.as_str())).map(|(q, _)| *q);
        if let (Some(q), None) = (hits.next(), hits.next()) {
            return Outcome::Match(q);
        }
    }
    let same_year = |q: i64| g.year.is_some() && list.info.get(&q).and_then(|i| i.1) == g.year;
    let lb_key = g.lb_name.as_deref().map(crate::scanner::normalize::search_key);
    let mut outcome = Outcome::NotFound;
    for key in lb_key.iter().chain(std::iter::once(&g.search_key)) {
        outcome = cands.best_with(key, g.region.as_deref(), &same_year);
        if outcome != Outcome::NotFound {
            break;
        }
    }
    outcome
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub platforms: usize,
    pub matched: usize,
    pub not_found: usize,
    pub ambiguous: usize,
    /// Matched games that are part of a series or franchise.
    pub with_series: usize,
    pub cancelled: bool,
}

/// One Wikidata pass (see the module docs). `force` asks about every platform
/// with games ("Update now").
pub fn run(conn: &mut Connection, source: &dyn Source, force: bool, progress: &dyn Fn(Progress), cancel: &AtomicBool) -> Result<Summary> {
    let mut s = Summary::default();
    let games = library_games(conn)?;
    let mut st = stamps(conn)?;
    let todo = platforms_to_ask(&games, &st, force, queries::now());
    let total = todo.len() as u64;
    for (i, platform) in todo.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            s.cancelled = true;
            break;
        }
        progress(Progress { done: i as u64, total });
        let Some(item) = platform_item(platform) else { continue };
        let list = listing(source.select(&names_query(item))?, source.select(&info_query(item))?);
        let cands = Candidates::new(list.names.clone());
        let mine: Vec<&Game> = games.iter().filter(|g| &g.platform_id == platform).collect();
        let outcomes: Vec<(i64, Outcome)> = mine.iter().map(|g| (g.id, match_game(g, &cands, &list))).collect();
        let matched: Vec<i64> = outcomes
            .iter()
            .filter_map(|(_, o)| if let Outcome::Match(q) = o { Some(*q) } else { None })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let mut series: Vec<Row> = Vec::new();
        for chunk in matched.chunks(BATCH) {
            if cancel.load(Ordering::Relaxed) {
                s.cancelled = true;
                break;
            }
            let rows = match source.select(&series_query(chunk)) {
                Ok(rows) => rows,
                Err(e) => {
                    log::info!("Wikidata series query: {e}; trying without the planner hint");
                    source.select(&series_query_plain(chunk))?
                }
            };
            series.extend(rows);
        }
        if s.cancelled {
            break;
        }
        store(conn, &outcomes, &matched, &list, &series)?;
        st.insert(platform.clone(), queries::now());
        queries::set_setting(conn, STAMP_KEY, &serde_json::to_value(&st)?)?;
        s.platforms += 1;
    }
    prune(conn)?;
    progress(Progress { done: total, total });
    let st = status(conn)?;
    (s.matched, s.not_found, s.ambiguous, s.with_series) = (st.matched as usize, st.not_found as usize, st.ambiguous as usize, st.with_series as usize);
    log::info!("Wikidata: {s:?}");
    Ok(s)
}

fn store(conn: &mut Connection, outcomes: &[(i64, Outcome)], matched: &[i64], list: &Listing, series: &[Row]) -> Result<()> {
    let tx = conn.transaction()?;
    let now = queries::now();
    {
        let mut game = tx.prepare(
            "INSERT INTO wd_games (qid, name, year, igdb, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(qid) DO UPDATE SET name = excluded.name, year = excluded.year, igdb = excluded.igdb,
                 fetched_at = excluded.fetched_at",
        )?;
        let mut clear = tx.prepare("DELETE FROM wd_game_series WHERE game_qid = ?1")?;
        for q in matched {
            let (name, year, igdb) = list.info.get(q).cloned().unwrap_or_default();
            let name = name.or_else(|| list.names.iter().find(|n| n.db_id == *q).map(|n| n.name.clone())).unwrap_or_default();
            game.execute(params![q, name, year, igdb, now])?;
            clear.execute([q])?;
        }
        let mut name = tx.prepare("INSERT INTO wd_series (qid, name) VALUES (?1, ?2) ON CONFLICT(qid) DO UPDATE SET name = excluded.name")?;
        let mut link = tx.prepare(
            "INSERT INTO wd_game_series (game_qid, series_qid, kind, ord) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(game_qid, series_qid, kind) DO UPDATE SET ord = COALESCE(ord, excluded.ord)",
        )?;
        for r in series {
            let (Some(g), Some(sq), Some(label), Some(kind)) =
                (r.get("g").and_then(|v| qid(v)), r.get("s").and_then(|v| qid(v)), r.get("sl"), r.get("kind"))
            else {
                continue;
            };
            // No English name (the label service then gives the item id).
            if qid(label) == Some(sq) || label.trim().is_empty() {
                continue;
            }
            name.execute(params![sq, label])?;
            let ord = r.get("ord").map(|o| o.trim()).filter(|o| !o.is_empty());
            link.execute(params![g, sq, kind, ord])?;
        }
        let mut set = tx.prepare("UPDATE games SET wd_status = ?2, wd_id = ?3 WHERE id = ?1")?;
        for (id, o) in outcomes {
            let (status, q) = match o {
                Outcome::Match(q) => ("ok", Some(*q)),
                Outcome::Ambiguous => ("ambiguous", None),
                Outcome::NotFound => ("notfound", None),
            };
            set.execute(params![id, status, q])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Keeps only the entries and series of the library's matched games.
fn prune(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DELETE FROM wd_games WHERE qid NOT IN (SELECT wd_id FROM games WHERE wd_id IS NOT NULL);
         DELETE FROM wd_game_series WHERE game_qid NOT IN (SELECT qid FROM wd_games);
         DELETE FROM wd_series WHERE qid NOT IN (SELECT series_qid FROM wd_game_series);",
    )?;
    Ok(())
}

/// A series or franchise a game belongs to, for the game details.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    /// Its collection key ("s:<item>"), for the logo the user picked.
    pub key: String,
    pub name: String,
    /// "series" | "franchise"
    pub kind: String,
    /// The game's number in it ("2"), when Wikidata has one.
    pub number: Option<String>,
    /// The library's games in it, this one included, in release order (then
    /// series number, then title); one per Wikidata entry.
    pub games: Vec<i64>,
}

/// A game's series, then franchises (no repeated names).
pub fn series_for(conn: &Connection, game_id: i64) -> Result<Vec<Series>> {
    let mut stmt = conn.prepare_cached(
        "SELECT gs.series_qid, s.name, gs.kind, gs.ord, g.wd_id FROM games g
         JOIN wd_game_series gs ON gs.game_qid = g.wd_id
         JOIN wd_series s ON s.qid = gs.series_qid
         WHERE g.id = ?1 AND g.wd_status = 'ok'
         ORDER BY gs.kind DESC, s.name",
    )?;
    let rows: Vec<(i64, String, String, Option<String>, i64)> = stmt
        .query_map([game_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut members = conn.prepare_cached(
        "SELECT g.id, g.wd_id, gs.ord, w.year, g.sort_title FROM wd_game_series gs
         JOIN games g ON g.wd_id = gs.game_qid AND g.wd_status = 'ok' AND g.missing = 0
         LEFT JOIN wd_games w ON w.qid = gs.game_qid
         WHERE gs.series_qid = ?1 AND gs.kind = ?2
         ORDER BY g.id",
    )?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (series_qid, name, kind, number, own_qid) in rows {
        if !seen.insert(name.to_lowercase()) {
            continue;
        }
        type Member = (i64, i64, Option<String>, Option<i64>, String);
        let list: Vec<Member> = members
            .query_map(params![series_qid, kind], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        // Two copies of one game (e.g. two regions): this one, else the first.
        let mut by_entry: HashMap<i64, Member> = HashMap::new();
        for m in list {
            if m.1 == own_qid && m.0 != game_id {
                continue;
            }
            by_entry.entry(m.1).or_insert(m);
        }
        let mut list: Vec<Member> = by_entry.into_values().collect();
        // Release order; the series number settles games from the same year.
        let num = |o: &Option<String>| o.as_deref().and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(f64::MAX);
        list.sort_by(|a, b| {
            a.3.unwrap_or(i64::MAX)
                .cmp(&b.3.unwrap_or(i64::MAX))
                .then(num(&a.2).total_cmp(&num(&b.2)))
                .then_with(|| a.4.cmp(&b.4))
        });
        out.push(Series { key: format!("s:{series_qid}"), name, kind, number, games: list.into_iter().map(|m| m.0).collect() });
    }
    Ok(out)
}

/// Forgets a game's match so the next pass looks it up again (e.g. its
/// LaunchBox match was fixed, which gives a better title).
pub fn relook(conn: &Connection, game_id: i64) -> Result<()> {
    conn.execute("UPDATE games SET wd_status = 'none', wd_id = NULL WHERE id = ?1", [game_id])?;
    Ok(())
}

/// Settings → Game info: what Wikidata has done.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub checked_at: Option<i64>,
    pub total: i64,
    pub matched: i64,
    pub with_series: i64,
    pub not_found: i64,
    pub ambiguous: i64,
    /// The games behind `not_found` and `ambiguous`, by name.
    pub unmatched: Vec<crate::db::queries::Unmatched>,
}

pub fn status(conn: &Connection) -> Result<Status> {
    let sql = format!(
        "SELECT COUNT(*),
                SUM(g.wd_status = 'ok'),
                SUM(g.wd_status = 'ok' AND EXISTS (SELECT 1 FROM wd_game_series gs WHERE gs.game_qid = g.wd_id)),
                SUM(g.wd_status = 'notfound'),
                SUM(g.wd_status = 'ambiguous')
         FROM games g JOIN platforms p ON p.id = g.platform_id
         WHERE g.missing = 0 AND p.enabled = 1 AND g.platform_id IN ({})",
        platform_list()
    );
    let n = |v: Option<i64>| v.unwrap_or(0);
    let (total, matched, with_series, not_found, ambiguous) = conn.query_row(&sql, [], |r| {
        Ok((r.get::<_, i64>(0)?, n(r.get(1)?), n(r.get(2)?), n(r.get(3)?), n(r.get(4)?)))
    })?;
    let checked_at = stamps(conn)?.values().copied().max();
    let unmatched = crate::db::queries::unmatched(conn, "wd_status", &format!("g.platform_id IN ({})", platform_list()))?;
    Ok(Status { enabled: enabled(conn)?, checked_at, total, matched, with_series, not_found, ambiguous, unmatched })
}

#[cfg(test)]
mod tests;
