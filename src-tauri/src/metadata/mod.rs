//! Optional game details from IGDB (description, release date, developer,
//! publisher, genres), for users who add their own free Twitch/IGDB key.
//!
//! The job runs in the background after scans (and on demand): games with a
//! known IGDB id are fetched directly; the rest are searched by title on their
//! platform, 10 per request, and matched by the same title rules as covers.
//! Matches are stored (IGDB id + details), so later runs are direct lookups
//! and the details work offline. A match the user picks ("Fix match") is
//! never replaced automatically.

pub mod igdb;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use rusqlite::Connection;
use serde::Serialize;

use crate::art::matcher::{title_similarity, FUZZY_MIN};
use crate::db::queries;
use crate::error::Result;
use crate::scanner::normalize::search_key;
use igdb::{Game, Igdb};

/// Games IGDB doesn't know are retried after this long.
pub const NOTFOUND_RETRY_SECS: i64 = 7 * 24 * 3600;
/// A search without the platform filter must match the title exactly.
const STRICT: f32 = 0.99;

#[derive(Debug, Clone)]
pub struct Target {
    pub id: i64,
    pub title: String,
    pub search_key: String,
    pub igdb_platform: Option<i64>,
    pub igdb_id: Option<i64>,
    /// The user picked this match; keep it.
    pub manual: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub matched: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub matched: usize,
    pub not_found: usize,
    pub failed: usize,
    /// Games that now have an IGDB cover to fetch (a cover pass should follow).
    pub covers: usize,
    pub cancelled: bool,
    pub duration_ms: u128,
}

/// How well an IGDB entry matches: its main name, or (slightly lower, so an
/// exact main name wins a tie) one of its alternative names. Mods (ROM hacks,
/// fan remakes) only count on an exact title.
fn score(key: &str, g: &Game) -> f32 {
    let main = title_similarity(key, &search_key(&g.name));
    let alt = g.alternative_names.iter().map(|n| title_similarity(key, &search_key(&n.name)) * 0.98).fold(0.0, f32::max);
    let s = main.max(alt);
    if g.is_mod() && s < 0.999 {
        0.0
    } else {
        s
    }
}

/// Other spellings to search when the title itself finds nothing (IGDB's
/// search is literal): "Killer 7" -> "Killer7", "DragonBall Z" -> "Dragon
/// Ball Z", "Dante's Inferno HDD" -> "Dante's Inferno".
pub fn search_variants(title: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if !s.is_empty() && s != title && !out.contains(&s) {
            out.push(s);
        }
    };
    // Split CamelCase words.
    let mut camel = String::new();
    let mut prev: Option<char> = None;
    for c in title.chars() {
        if prev.is_some_and(|p| p.is_lowercase()) && c.is_uppercase() {
            camel.push(' ');
        }
        camel.push(c);
        prev = Some(c);
    }
    push(camel);
    // "Word 7" -> "Word7" (two-word titles only; "Tenkaichi 3" stays apart).
    let words: Vec<&str> = title.split_whitespace().collect();
    if let [first, last] = words[..] {
        if last.len() <= 2 && last.bytes().all(|b| b.is_ascii_digit()) {
            push(format!("{first}{last}"));
        }
    }
    // Drop a trailing word that isn't a number (editions, "HD", "HDD").
    if words.len() >= 3 && !words.last().unwrap().bytes().all(|b| b.is_ascii_digit()) {
        push(words[..words.len() - 1].join(" "));
    }
    out.truncate(3);
    out
}

/// The best result at or above `min`; on a tie, the main game over an
/// edition, then IGDB's own relevance order.
pub fn best_match<'g>(key: &str, results: &'g [Game], min: f32) -> Option<&'g Game> {
    let mut best: Option<(&Game, f32)> = None;
    for g in results {
        let s = score(key, g);
        let better = match best {
            None => true,
            Some((b, bs)) => s > bs + 1e-6 || ((s - bs).abs() <= 1e-6 && b.version_parent.is_some() && g.version_parent.is_none()),
        };
        if s >= min && better {
            best = Some((g, s));
        }
    }
    best.map(|(g, _)| g)
}

struct Run<'a> {
    conn: &'a Connection,
    summary: Summary,
    done: usize,
    total: usize,
    progress: &'a dyn Fn(Progress),
}

impl Run<'_> {
    fn finish(&mut self, t: &Target, outcome: std::result::Result<Option<&Game>, ()>) -> Result<()> {
        let now = queries::now();
        match outcome {
            Ok(Some(g)) => {
                if queries::set_metadata(self.conn, t.id, g, if t.manual { "manual" } else { "ok" }, now)? {
                    self.summary.covers += 1;
                }
                self.summary.matched += 1;
            }
            Ok(None) => {
                queries::set_match_status(self.conn, t.id, "notfound", now)?;
                self.summary.not_found += 1;
            }
            Err(()) => {
                queries::set_match_status(self.conn, t.id, "error", now)?;
                self.summary.failed += 1;
            }
        }
        self.done += 1;
        (self.progress)(Progress { done: self.done, total: self.total, matched: self.summary.matched });
        Ok(())
    }
}

/// Is this error about the user's keys (stop the whole run) rather than one request?
fn is_auth_error(e: &crate::error::Error) -> bool {
    e.to_string().contains("Client ID / Client Secret")
}

/// Fetches details for every game that needs them.
pub fn run(conn: &mut Connection, igdb: &Igdb, progress: &dyn Fn(Progress), cancel: &AtomicBool) -> Result<Summary> {
    let started = Instant::now();
    let targets = queries::games_needing_metadata(conn, queries::now())?;
    let mut run = Run { conn, summary: Summary::default(), done: 0, total: targets.len(), progress };
    if targets.is_empty() {
        return Ok(run.summary);
    }
    log::info!("fetching IGDB details for {} games", targets.len());
    let (known, unknown): (Vec<_>, Vec<_>) = targets.into_iter().partition(|t| t.igdb_id.is_some());

    // 1. Already matched: direct lookups by id.
    for chunk in known.chunks(100) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let ids: Vec<i64> = chunk.iter().filter_map(|t| t.igdb_id).collect();
        match igdb.games(&igdb::ids_query(&ids)) {
            Ok(games) => {
                let by_id: HashMap<i64, &Game> = games.iter().map(|g| (g.id, g)).collect();
                for t in chunk {
                    run.finish(t, Ok(t.igdb_id.and_then(|id| by_id.get(&id).copied())))?;
                }
            }
            Err(e) if is_auth_error(&e) => return Err(e),
            Err(e) => {
                log::warn!("IGDB lookup failed: {e}");
                for t in chunk {
                    run.finish(t, Err(()))?;
                }
            }
        }
    }

    // 2. Search by title on the game's platform, then (IGDB's platform data
    // has gaps) without the filter, accepting only an exact title, then other
    // spellings of the title on the platform. One request per search: IGDB's
    // multiquery endpoint silently returns no results for `search` queries.
    for t in &unknown {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut passes: Vec<(String, Option<i64>, f32)> = vec![(t.title.clone(), t.igdb_platform, FUZZY_MIN)];
        if t.igdb_platform.is_some() {
            passes.push((t.title.clone(), None, STRICT));
        }
        passes.extend(search_variants(&t.title).into_iter().map(|v| (v, t.igdb_platform, FUZZY_MIN)));
        let mut outcome: std::result::Result<Option<Game>, ()> = Ok(None);
        for (query, platform, min) in passes {
            match igdb.games(&igdb::search_query(&query, platform, 10)) {
                Ok(results) => {
                    if let Some(g) = best_match(&t.search_key, &results, min) {
                        outcome = Ok(Some(g.clone()));
                        break;
                    }
                }
                Err(e) if is_auth_error(&e) => return Err(e),
                Err(e) => {
                    log::warn!("IGDB search for '{}' failed: {e}", t.title);
                    outcome = Err(());
                    break;
                }
            }
        }
        run.finish(t, outcome.as_ref().map(Option::as_ref).map_err(|_| ()))?;
    }

    run.summary.cancelled = cancel.load(Ordering::Relaxed);
    run.summary.duration_ms = started.elapsed().as_millis();
    log::info!("IGDB details: {:?}", run.summary);
    Ok(run.summary)
}

// ---------------------------------------------------------------- fix match

/// One search result in the "Fix match" screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub igdb_id: i64,
    pub name: String,
    pub year: Option<i32>,
    pub cover: Option<String>,
    /// Listed for the game's platform on IGDB.
    pub on_platform: bool,
    pub summary: Option<String>,
}

fn year(unix: Option<i64>) -> Option<i32> {
    // Days to civil year (Howard Hinnant's algorithm), good for any date IGDB has.
    let days = unix?.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    Some((yoe + era * 400 + i64::from(month <= 2)) as i32)
}

/// IGDB entries for `query`: the game's platform first, then everything else.
pub fn search(igdb: &Igdb, query: &str, platform: Option<i64>) -> Result<Vec<Candidate>> {
    let mut queries = vec![igdb::search_query(query, None, 20)];
    if let Some(p) = platform {
        queries.insert(0, igdb::search_query(query, Some(p), 20));
    }
    let mut out: Vec<Candidate> = Vec::new();
    for q in queries {
        let games = igdb.games(&q)?;
        for g in games {
            if out.iter().any(|c| c.igdb_id == g.id) {
                continue;
            }
            out.push(Candidate {
                igdb_id: g.id,
                year: year(g.first_release_date),
                cover: g.cover_url(),
                on_platform: platform.is_some_and(|p| g.platforms.contains(&p)),
                summary: g.summary.as_ref().map(|s| s.chars().take(240).collect()),
                name: g.name,
            });
        }
    }
    Ok(out)
}

/// Links a game to the IGDB entry the user picked and stores its details.
/// Returns true when that brought a cover to fetch.
pub fn apply_match(conn: &Connection, igdb: &Igdb, game_id: i64, igdb_id: i64) -> Result<bool> {
    let games = igdb.games(&igdb::ids_query(&[igdb_id]))?;
    let g = games.first().ok_or_else(|| crate::error::Error::msg("IGDB no longer has that game"))?;
    queries::set_metadata(conn, game_id, g, "manual", queries::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::http::fake::{json, FakePost};
    use serde_json::json as j;

    fn game(id: i64, name: &str) -> Game {
        serde_json::from_value(j!({"id": id, "name": name})).unwrap()
    }

    #[test]
    fn best_match_follows_the_title_rules() {
        let results = vec![game(1, "Final Fantasy X-2"), game(2, "Final Fantasy X"), game(3, "Final Fantasy X | X-2 HD Remaster")];
        assert_eq!(best_match("final fantasy x", &results, FUZZY_MIN).unwrap().id, 2);
        // Sequel numbers must agree: no match rather than the wrong game.
        assert!(best_match("final fantasy x 3", &results, FUZZY_MIN).is_none());
        // Alternative names count.
        let alt: Game = serde_json::from_value(
            j!({"id": 9, "name": "Fire Emblem", "alternative_names": [{"name": "Fire Emblem: The Blazing Blade"}]}),
        )
        .unwrap();
        assert_eq!(best_match("fire emblem the blazing blade", &[alt], FUZZY_MIN).unwrap().id, 9);
    }

    #[test]
    fn numbers_mods_and_alternative_names_dont_steal_matches() {
        let black9: Game = serde_json::from_value(j!({"id": 1, "name": "Black 9", "alternative_names": [{"name": "BLACK9"}]})).unwrap();
        let black: Game = serde_json::from_value(j!({"id": 2, "name": "Black"})).unwrap();
        assert_eq!(best_match("black", &[black9.clone(), black], FUZZY_MIN).unwrap().id, 2);
        assert!(best_match("black", &[black9], FUZZY_MIN).is_none());
        let fan: Game =
            serde_json::from_value(j!({"id": 3, "name": "Fire Emblem: Binding Blade Plus", "game_type": {"type": "Mod"}})).unwrap();
        assert!(best_match("fire emblem the binding blade", &[fan.clone()], FUZZY_MIN).is_none());
        // ...but a mod with exactly your title is fine (you have the hack).
        assert_eq!(best_match("fire emblem binding blade plus", &[fan], FUZZY_MIN).unwrap().id, 3);
    }

    #[test]
    fn search_variants_respell_titles_igdb_misses() {
        assert_eq!(search_variants("Killer 7"), ["Killer7"]);
        assert_eq!(search_variants("DragonBall Z: Budokai Tenkaichi 3"), ["Dragon Ball Z: Budokai Tenkaichi 3"]);
        assert_eq!(search_variants("Dante's Inferno HDD"), ["Dante's Inferno"]);
        assert!(search_variants("Okami").is_empty());
    }

    #[test]
    fn year_from_unix() {
        assert_eq!(year(Some(1_158_019_200)), Some(2006)); // 2006-09-12
        assert_eq!(year(Some(0)), Some(1970));
        assert_eq!(year(Some(-86_400)), Some(1969));
    }

    fn db() -> Connection {
        let conn = crate::db::test_db();
        for (title, platform) in [("Okami", "ps2"), ("Jak II", "ps2"), ("Homebrew Thing", "ps2"), ("Pokemon Emerald", "gba")] {
            conn.execute(
                "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at)
                 VALUES (?1, ?2, ?2, ?3, ?2, 0)",
                rusqlite::params![platform, title, search_key(title)],
            )
            .unwrap();
        }
        conn
    }

    #[test]
    fn run_matches_stores_details_and_then_skips_done_games() {
        let mut conn = db();
        let http = FakePost::new(|url: &str, _: &[(&str, &str)], body: &str| {
            if url.starts_with(igdb::TOKEN_URL) {
                return json(200, j!({"access_token": "t", "expires_in": 5_000_000}));
            }
            assert!(url.ends_with("/games"), "searches go to /games, one each: {url}");
            json(
                200,
                if body.contains("search \"Okami\"") {
                    j!([{"id": 11, "name": "Okami", "first_release_date": 1_158_019_200, "summary": "Wolf.",
                         "genres": [{"name": "Adventure"}, {"name": "Action"}],
                         "involved_companies": [{"company": {"name": "Clover Studio"}, "developer": true}]}])
                } else if body.contains("search \"Jak II\"") {
                    j!([{"id": 22, "name": "Jak II"}])
                } else if body.contains("search \"Pokemon Emerald\"") && body.contains("where platforms = (24)") {
                    j!([{"id": 33, "name": "Pokémon Emerald Version"}])
                } else {
                    j!([])
                },
            )
        });
        let client = Igdb::new(&http, igdb::Credentials { client_id: "c".into(), client_secret: "s".into() }, None);
        let s = run(&mut conn, &client, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((s.matched, s.not_found, s.failed), (3, 1, 0));
        // The unknown game was tried on its platform, then without the filter.
        let tries = http.requests.lock().unwrap().iter().filter(|r| r.2.contains("Homebrew Thing")).count();
        assert_eq!(tries, 2);

        let okami: i64 = conn.query_row("SELECT id FROM games WHERE title = 'Okami'", [], |r| r.get(0)).unwrap();
        let m = queries::game_metadata(&conn, okami).unwrap();
        assert_eq!(m.igdb_id, Some(11));
        assert_eq!(m.developer.as_deref(), Some("Clover Studio"));
        assert_eq!(m.genres, ["Action", "Adventure"]);
        assert_eq!(m.match_status, "ok");
        let (names, by_game) = queries::genres(&conn).unwrap();
        assert_eq!(names.len(), 2);
        assert_eq!(by_game[&okami].len(), 2);

        // Second run: nothing left to do, no requests.
        let before = http.requests.lock().unwrap().len();
        let s2 = run(&mut conn, &client, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(s2.matched + s2.not_found, 0);
        assert_eq!(http.requests.lock().unwrap().len(), before);
    }

    /// Live run against a *copy* of a real library.db with the keys saved in the app:
    /// `EMU_DB_COPY=copy.db cargo test real_meta_fetch -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_meta_fetch() {
        let db = std::env::var("EMU_DB_COPY").expect("EMU_DB_COPY");
        let mut conn = crate::db::open(std::path::Path::new(&db)).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        crate::platforms::seed(&conn).unwrap();
        let creds = igdb::Credentials {
            client_id: crate::secrets::get("igdb-client-id").unwrap().expect("keys"),
            client_secret: crate::secrets::get("igdb-client-secret").unwrap().expect("keys"),
        };
        let http = crate::art::http::Http::new();
        let client = Igdb::new(&http, creds, None);
        let s = run(&mut conn, &client, &|p| if p.done % 20 == 0 { println!("{}/{}", p.done, p.total) }, &AtomicBool::new(false))
            .unwrap();
        println!("summary: {s:?}");
        let mut stmt = conn
            .prepare(
                "SELECT g.platform_id, g.title, g.match_status, m.title, m.release_date
                 FROM games g LEFT JOIN metadata m ON m.game_id = g.id WHERE g.missing = 0 ORDER BY g.platform_id, g.sort_title",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<i64>>(4)?))
            })
            .unwrap();
        for row in rows {
            let (p, title, status, igdb, date) = row.unwrap();
            println!("{p:<6} {status:<8} {title:<50} -> {} ({})", igdb.unwrap_or_default(), year(date).map(|y| y.to_string()).unwrap_or_default());
        }
    }

    #[test]
    fn a_match_queues_its_cover_for_games_without_one_but_never_over_your_choice() {
        let conn = db();
        let id = |t: &str| -> i64 { conn.query_row("SELECT id FROM games WHERE title = ?1", [t], |r| r.get(0)).unwrap() };
        let (okami, jak) = (id("Okami"), id("Jak II"));
        conn.execute("UPDATE games SET art_status = 'notfound'", []).unwrap();
        conn.execute("UPDATE games SET art_manual = 1 WHERE id = ?1", [jak]).unwrap();
        let g: Game = serde_json::from_value(j!({"id": 11, "name": "Okami", "cover": {"image_id": "co1"}})).unwrap();
        assert!(queries::set_metadata(&conn, okami, &g, "ok", 0).unwrap());
        assert!(!queries::set_metadata(&conn, jak, &g, "ok", 0).unwrap(), "the user's choice stays");
        let status: String = conn.query_row("SELECT art_status FROM games WHERE id = ?1", [okami], |r| r.get(0)).unwrap();
        assert_eq!(status, "none");
        let target = queries::games_needing_art(&conn, queries::now()).unwrap().into_iter().find(|t| t.id == okami).unwrap();
        assert_eq!(target.igdb_cover.as_deref(), Some("https://images.igdb.com/igdb/image/upload/t_1080p/co1.jpg"));
    }

    #[test]
    fn a_manual_match_sticks() {
        let conn = db();
        let http = FakePost::new(|url: &str, _: &[(&str, &str)], body: &str| {
            if url.starts_with(igdb::TOKEN_URL) {
                return json(200, j!({"access_token": "t", "expires_in": 5_000_000}));
            }
            assert!(body.contains("where id = (55)"), "{body}");
            json(200, j!([{"id": 55, "name": "Homebrew Thing Deluxe"}]))
        });
        let client = Igdb::new(&http, igdb::Credentials { client_id: "c".into(), client_secret: "s".into() }, None);
        let id: i64 = conn.query_row("SELECT id FROM games WHERE title = 'Homebrew Thing'", [], |r| r.get(0)).unwrap();
        apply_match(&conn, &client, id, 55).unwrap();
        let m = queries::game_metadata(&conn, id).unwrap();
        assert_eq!((m.igdb_id, m.match_status.as_str()), (Some(55), "manual"));
        // Automatic failures never overwrite it.
        queries::set_match_status(&conn, id, "notfound", 0).unwrap();
        assert_eq!(queries::game_metadata(&conn, id).unwrap().match_status, "manual");
    }
}
