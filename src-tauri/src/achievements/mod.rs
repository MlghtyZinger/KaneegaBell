//! RetroAchievements: identify games by RA's own hash (rcheevos method),
//! then show their achievement lists and the user's progress.
//!
//! Per game: `ra_hash` NULL = not hashed yet, "" = no hash possible (unknown
//! layout, unreadable); `ra_game_id` NULL = not looked up, 0 = RA doesn't
//! know the hash, else RA's game id. A file whose size/mtime changes is
//! hashed again (see `apply_scan`). "Not known" is retried whenever the
//! console's hash list is refreshed (weekly).

pub mod hash;
pub mod ra;
pub mod wbfs;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;
use serde::Serialize;

use crate::db::queries::{self, now};
use crate::error::Result;
use ra::Ra;

/// Console list and per-console hash lists change rarely.
pub const LIST_TTL_SECS: i64 = 7 * 24 * 3600;
/// Unlocks are re-fetched when a game's details are opened this long after the last fetch.
pub const PROGRESS_TTL_SECS: i64 = 10 * 60;

const CONSOLES_KEY: &str = "ra:consoles";

fn list_key(console: u32) -> String {
    format!("ra:hashes:{console}")
}

fn progress_key(ra_game_id: i64, user: &str) -> String {
    format!("ra:progress:{ra_game_id}:{}", user.to_lowercase())
}

/// Returns the cached body when fresh; otherwise fetches it, falling back to a
/// stale copy when offline. The bool says whether the body is stale.
fn cached(conn: &Connection, key: &str, ttl: i64, fetch: impl FnOnce() -> Result<Vec<u8>>) -> Result<(Vec<u8>, i64, bool)> {
    let old = queries::cache_get(conn, key)?;
    if let Some((body, at)) = &old {
        if now() - at < ttl {
            return Ok((body.clone(), *at, false));
        }
    }
    match fetch() {
        Ok(body) => {
            let at = now();
            queries::cache_put(conn, key, &body, at)?;
            Ok((body, at, false))
        }
        Err(e) => match old {
            Some((body, at)) => {
                log::warn!("RetroAchievements: using cached {key}: {e}");
                Ok((body, at, true))
            }
            None => Err(e),
        },
    }
}

/// Console ids RA currently runs achievements for. Without a client, only
/// the cached list (None if there's none).
pub fn active_consoles(conn: &Connection, ra: Option<&Ra>) -> Result<Option<HashSet<u32>>> {
    let body = match ra {
        Some(ra) => Some(cached(conn, CONSOLES_KEY, LIST_TTL_SECS, || ra.active_consoles())?.0),
        None => queries::cache_get(conn, CONSOLES_KEY)?.map(|(b, _)| b),
    };
    let Some(body) = body else { return Ok(None) };
    Ok(Some(ra::parse_consoles(&body)?.into_iter().filter(|c| c.active).map(|c| c.id).collect()))
}

pub struct Target {
    pub id: i64,
    pub console: u32,
    pub launch_path: PathBuf,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub done: usize,
    pub total: usize,
    pub matched: usize,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub hashed: usize,
    pub matched: usize,
    pub unknown: usize,
    pub cancelled: bool,
}

/// RA's hash for a game's files ("" when there's none to be had).
fn compute_hash(console: u32, launch_path: &Path) -> String {
    match hash::hash_file(console, launch_path) {
        Ok(Some(h)) => h,
        Ok(None) => String::new(),
        Err(e) => {
            log::warn!("RetroAchievements hash of {}: {e}", launch_path.display());
            String::new()
        }
    }
}

/// Hash index per console, loaded once per pass.
struct Lists<'a> {
    ra: &'a Ra<'a>,
    loaded: HashMap<u32, HashMap<String, i64>>,
}

impl Lists<'_> {
    fn lookup(&mut self, conn: &Connection, console: u32, hash: &str) -> Result<i64> {
        if !self.loaded.contains_key(&console) {
            let (body, _, _) = cached(conn, &list_key(console), LIST_TTL_SECS, || self.ra.hash_list(console))?;
            self.loaded.insert(console, ra::hash_index(&body)?);
        }
        Ok(self.loaded[&console].get(hash).copied().unwrap_or(0))
    }
}

/// Hashes games that need it and looks them up in RA's hash lists.
pub fn run(conn: &mut Connection, ra: &Ra, progress: &dyn Fn(&JobProgress), cancel: &AtomicBool) -> Result<Summary> {
    let Some(active) = active_consoles(conn, Some(ra))? else { return Ok(Summary::default()) };
    queries::reopen_ra_unknown(conn, now() - LIST_TTL_SECS)?;
    let targets = queries::games_needing_ra(conn, &active)?;
    let mut lists = Lists { ra, loaded: HashMap::new() };
    let mut p = JobProgress { done: 0, total: targets.len(), matched: 0 };
    let mut s = Summary::default();
    progress(&p);
    for t in targets {
        if cancel.load(Ordering::Acquire) {
            s.cancelled = true;
            break;
        }
        let hash = match t.hash {
            Some(h) => h,
            None => {
                let h = compute_hash(t.console, &t.launch_path);
                queries::set_ra_hash(conn, t.id, &h)?;
                s.hashed += 1;
                h
            }
        };
        if !hash.is_empty() {
            let id = lists.lookup(conn, t.console, &hash)?;
            queries::set_ra_game(conn, t.id, id)?;
            if id > 0 {
                s.matched += 1;
                p.matched += 1;
            } else {
                s.unknown += 1;
            }
        }
        p.done += 1;
        progress(&p);
    }
    Ok(s)
}

/// A badge (`/ra/{name}`) or game icon (`/rai/{name}`) image, from the disk
/// cache or downloaded from RA's media server on first use. Kept forever:
/// RA gives changed images new names.
pub fn media(data_dir: &Path, http: &dyn crate::art::http::Fetch, kind: &str, name: &str) -> Option<Vec<u8>> {
    let name = name.trim_end_matches(".png");
    if !ra::is_media_name(name) {
        return None;
    }
    let (folder, remote) = match kind {
        "ra" => ("badges", "Badge"),
        "rai" => ("icons", "Images"),
        _ => return None,
    };
    let file = data_dir.join("ra").join(folder).join(format!("{name}.png"));
    if let Ok(bytes) = std::fs::read(&file) {
        return Some(bytes);
    }
    let bytes = http.get(&format!("{}/{remote}/{name}.png", ra::MEDIA)).ok()??;
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = file.with_extension("part");
    if std::fs::write(&tmp, &bytes).is_ok() {
        let _ = std::fs::rename(&tmp, &file);
    }
    Some(bytes)
}

/// What the game details screen shows.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAchievements {
    /// "unsupported" (RA has no achievements for this console, or RA isn't
    /// set up: the UI shows nothing), "nohash" (the files couldn't be
    /// hashed), "unknown" (RA has achievements for this game, but not for
    /// this version: see `accepted`), "noset" (RA has no achievements for a
    /// game by this title on this console), "ok".
    pub status: &'static str,
    pub hash: Option<String>,
    pub progress: Option<ra::Progress>,
    pub fetched_at: Option<i64>,
    /// Shown from the cache because RA couldn't be reached.
    pub stale: bool,
    /// For "unknown": the RA game with this title, and the versions it accepts.
    pub ra_title: Option<String>,
    pub ra_game_id: Option<i64>,
    pub accepted: Vec<ra::AcceptedDump>,
}

impl GameAchievements {
    fn status(status: &'static str, hash: Option<String>) -> Self {
        Self {
            status,
            hash,
            progress: None,
            fetched_at: None,
            stale: false,
            ra_title: None,
            ra_game_id: None,
            accepted: Vec::new(),
        }
    }
}

/// The RA game (with achievements) whose title matches one of `titles`, by
/// the same title rules as covers and IGDB (sequel numbers must agree).
/// Hacks, homebrew etc. ("~Hack~ ...") and subsets are never suggested.
pub fn title_match<'a>(list: &'a [ra::ListedGame], titles: &[String]) -> Option<&'a ra::ListedGame> {
    use crate::art::matcher::{title_similarity, FUZZY_MIN};
    use crate::scanner::normalize::search_key;
    let keys: Vec<String> = titles.iter().map(|t| search_key(t)).filter(|k| !k.is_empty()).collect();
    let mut best: Option<(f32, &ra::ListedGame)> = None;
    for g in list.iter().filter(|g| !g.title.starts_with('~') && !g.title.contains("[Subset")) {
        // RA writes alternative names as "Name A | Name B".
        for alt in g.title.split(" | ") {
            let k = search_key(alt);
            for q in &keys {
                let s = title_similarity(q, &k);
                if s >= FUZZY_MIN && best.is_none_or(|(b, _)| s > b) {
                    best = Some((s, g));
                }
            }
        }
    }
    best.map(|(_, g)| g)
}

/// Why RA didn't recognize a hash: another version of a game it has
/// ("unknown", with the versions it accepts), or no such game ("noset").
fn explain_unknown(conn: &Connection, ra: &Ra, game_id: i64, console: u32, hash: String) -> Result<GameAchievements> {
    let (body, _, _) = cached(conn, &list_key(console), LIST_TTL_SECS, || ra.hash_list(console))?;
    let list = ra::parse_list(&body)?;
    let titles = queries::game_titles(conn, game_id)?;
    let Some(g) = title_match(&list, &titles) else { return Ok(GameAchievements::status("noset", Some(hash))) };
    let accepted = cached(conn, &format!("ra:gamehashes:{}", g.id), LIST_TTL_SECS, || ra.game_hashes(g.id))
        .and_then(|(b, _, _)| ra::parse_game_hashes(&b))
        .unwrap_or_else(|e| {
            log::warn!("RetroAchievements versions of game {}: {e}", g.id);
            Vec::new()
        });
    Ok(GameAchievements {
        ra_title: Some(g.title.clone()),
        ra_game_id: Some(g.id),
        accepted,
        ..GameAchievements::status("unknown", Some(hash))
    })
}

/// A game's achievements and the user's unlocks. Identifies the game first
/// if the background pass hasn't got to it yet. `refresh` skips the cache.
pub fn for_game(conn: &Connection, ra: Option<&Ra>, game_id: i64, refresh: bool) -> Result<GameAchievements> {
    let Some(ra) = ra else { return Ok(GameAchievements::status("unsupported", None)) };
    let (console, launch_path, mut hash, mut ra_id) = queries::ra_state(conn, game_id)?;
    let Some(console) = console else { return Ok(GameAchievements::status("unsupported", None)) };
    let active = active_consoles(conn, Some(ra))?.unwrap_or_default();
    if !active.contains(&console) {
        return Ok(GameAchievements::status("unsupported", None));
    }
    let h = match hash.take() {
        Some(h) => h,
        None => {
            let h = compute_hash(console, Path::new(&launch_path));
            queries::set_ra_hash(conn, game_id, &h)?;
            ra_id = None;
            h
        }
    };
    if h.is_empty() {
        return Ok(GameAchievements::status("nohash", None));
    }
    let id = match ra_id {
        Some(id) => id,
        None => {
            let id = Lists { ra, loaded: HashMap::new() }.lookup(conn, console, &h)?;
            queries::set_ra_game(conn, game_id, id)?;
            id
        }
    };
    if id <= 0 {
        return explain_unknown(conn, ra, game_id, console, h);
    }
    let ttl = if refresh { 0 } else { PROGRESS_TTL_SECS };
    let (body, at, stale) = cached(conn, &progress_key(id, ra.user()), ttl, || ra.game_progress(id))?;
    Ok(GameAchievements {
        progress: Some(ra::parse_progress(id, &body)?),
        fetched_at: Some(at),
        stale,
        ra_game_id: Some(id),
        ..GameAchievements::status("ok", Some(h))
    })
}

/// Progress at a glance, for the game details sidebar.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Glance {
    pub earned: usize,
    pub total: usize,
    pub points_earned: i64,
    pub points_total: i64,
}

/// The user's progress in a game from what's already cached: no hashing and
/// no requests, so it's cheap enough to ask for on every selection. None when
/// the game hasn't been identified or its progress was never fetched.
pub fn glance(conn: &Connection, user: &str, game_id: i64) -> Result<Option<Glance>> {
    let (_, _, _, ra_id) = queries::ra_state(conn, game_id)?;
    let Some(id) = ra_id.filter(|&id| id > 0) else { return Ok(None) };
    let Some((body, _)) = queries::cache_get(conn, &progress_key(id, user))? else { return Ok(None) };
    let p = ra::parse_progress(id, &body)?;
    if p.total == 0 {
        return Ok(None);
    }
    Ok(Some(Glance { earned: p.earned, total: p.total, points_earned: p.points_earned, points_total: p.points_total }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::http::fake::{json, FakePost};
    use crate::art::http::Response;
    use crate::db;
    use serde_json::json as j;
    use std::io::Write;

    fn setup(dir: &Path) -> (Connection, i64, String) {
        let conn = db::test_db();
        // A GBA game: hashed as the whole file.
        let rom: Vec<u8> = (0..4096u32).map(|i| (i * 13) as u8).collect();
        let path = dir.join("game.gba");
        std::fs::File::create(&path).unwrap().write_all(&rom).unwrap();
        conn.execute(
            "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, total_size, mtime, added_at)
             VALUES ('gba', 'Game', 'game', 'game', ?1, 4096, 1, 0)",
            [path.to_string_lossy()],
        )
        .unwrap();
        (conn, 1, format!("{:x}", md5::compute(&rom)))
    }

    fn server(hash: String, earned: bool) -> impl Fn(&str, &[(&str, &str)], &str) -> Response + Sync {
        move |url: &str, _: &[(&str, &str)], _: &str| {
            if url.contains("API_GetConsoleIDs") {
                json(200, j!([{"ID": 5, "Name": "Game Boy Advance", "Active": true},
                              {"ID": 2, "Name": "Nintendo 64", "Active": true}]))
            } else if url.contains("API_GetGameList") && url.contains("i=5") {
                json(200, j!([{"ID": 777, "Title": "Game", "Hashes": [hash.to_uppercase()]},
                              {"ID": 778, "Title": "~Hack~ Other Game", "Hashes": []}]))
            } else if url.contains("API_GetGameHashes") && url.contains("i=777") {
                json(200, j!({"Results": [{"Name": "Game (Europe)", "MD5": hash, "Labels": ["nointro"]}]}))
            } else if url.contains("API_GetGameInfoAndUserProgress") && url.contains("g=777") {
                let date = if earned { j!("2024-01-02 03:04:05") } else { j!(null) };
                json(200, j!({"Title": "Game", "NumDistinctPlayers": 10, "Achievements": {
                    "1": {"ID": 1, "Title": "Start", "Points": 5, "BadgeName": "100", "DisplayOrder": 1,
                          "NumAwarded": 10, "DateEarned": date}}}))
            } else {
                Response { status: 404, body: vec![] }
            }
        }
    }

    fn creds() -> ra::Credentials {
        ra::Credentials { user: "me".into(), key: "key".into() }
    }

    #[test]
    fn pass_hashes_and_matches_then_details_show_progress() {
        let t = tempfile::tempdir().unwrap();
        let (mut conn, id, hash) = setup(t.path());
        let http = FakePost::new(server(hash.clone(), true));
        let client = Ra::new(&http, creds());
        let s = run(&mut conn, &client, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((s.hashed, s.matched, s.unknown), (1, 1, 0));
        let (_, _, h, ra_id) = queries::ra_state(&conn, id).unwrap();
        assert_eq!((h.as_deref(), ra_id), (Some(hash.as_str()), Some(777)));

        // A second pass has nothing to do and makes no requests (lists cached).
        let before = http.requests.lock().unwrap().len();
        let s = run(&mut conn, &client, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((s.hashed, s.matched), (0, 0));
        assert_eq!(http.requests.lock().unwrap().len(), before);

        let a = for_game(&conn, Some(&client), id, false).unwrap();
        assert_eq!(a.status, "ok");
        let p = a.progress.unwrap();
        assert_eq!((p.total, p.earned, p.points_earned), (1, 1, 5));

        // The sidebar's glance reads that cache and asks nothing.
        let before = http.requests.lock().unwrap().len();
        let g = glance(&conn, "Me", id).unwrap();
        assert_eq!(g, Some(Glance { earned: 1, total: 1, points_earned: 5, points_total: 5 }));
        assert_eq!(http.requests.lock().unwrap().len(), before);
        assert_eq!(glance(&conn, "someone else", id).unwrap(), None, "another user's progress was never fetched");
    }

    #[test]
    fn details_work_offline_from_the_cache_and_identify_on_demand() {
        let t = tempfile::tempdir().unwrap();
        let (conn, id, hash) = setup(t.path());
        let http = FakePost::new(server(hash, false));
        let client = Ra::new(&http, creds());
        // Never hashed by a pass: details identify it themselves.
        let a = for_game(&conn, Some(&client), id, false).unwrap();
        assert_eq!((a.status, a.stale), ("ok", false));

        let offline = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| Response { status: 503, body: vec![] });
        let client = Ra::new(&offline, creds());
        let a = for_game(&conn, Some(&client), id, true).unwrap();
        assert_eq!((a.status, a.stale), ("ok", true));
        assert_eq!(a.progress.unwrap().earned, 0);
    }

    /// What the details screen says for each unrecognized game, against a
    /// copy of a real library.db and the saved RA login:
    /// `RA_DB=copy.db cargo test real_unknown_explanations -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_unknown_explanations() {
        let conn = db::open(Path::new(&std::env::var("RA_DB").unwrap())).unwrap();
        let creds = ra::Credentials {
            user: crate::secrets::get("ra-user").unwrap().unwrap(),
            key: crate::secrets::get("ra-web-api-key").unwrap().unwrap(),
        };
        let http = crate::art::http::Http::new();
        let client = Ra::new(&http, creds);
        let ids: Vec<(i64, String)> = conn
            .prepare("SELECT id, title FROM games WHERE ra_game_id = 0 AND missing = 0")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for (id, title) in ids {
            let a = for_game(&conn, Some(&client), id, false).unwrap();
            let names: Vec<String> =
                a.accepted.iter().map(|d| format!("{}{}", d.name, if d.patched { " [patch]" } else { "" })).collect();
            println!("{title}: {} {:?} {names:?}", a.status, a.ra_title);
        }
    }

    #[test]
    fn titles_match_by_the_usual_rules() {
        let list: Vec<ra::ListedGame> = serde_json::from_value(j!([
            {"ID": 1, "Title": "Castlevania: The Dracula X Chronicles"},
            {"ID": 2, "Title": "Fairy Tail: Portable Guild"},
            {"ID": 3, "Title": "Pocket Monsters Gin | Pokemon Silver"},
            {"ID": 4, "Title": "Jak II [Subset - Bonus]"}
        ]))
        .unwrap();
        let find = |t: &str| title_match(&list, &[t.to_string()]).map(|g| g.id);
        assert_eq!(find("Castlevania Dracula X Chronicles"), Some(1));
        assert_eq!(find("Fairy Tail Portable Guild 2"), None, "sequel numbers must agree");
        assert_eq!(find("Pokémon Silver"), Some(3));
        assert_eq!(find("Jak II"), None, "subsets aren't the game");
        assert_eq!(find("BloodRayne"), None);
    }

    #[test]
    fn unknown_dumps_and_unsupported_consoles() {
        let t = tempfile::tempdir().unwrap();
        let (conn, id, _) = setup(t.path());
        let http = FakePost::new(server("0000".into(), false));
        let client = Ra::new(&http, creds());
        // RA has "Game", just not this version: say which versions it takes.
        let a = for_game(&conn, Some(&client), id, false).unwrap();
        assert_eq!((a.status, a.ra_title.as_deref(), a.ra_game_id), ("unknown", Some("Game"), Some(777)));
        assert_eq!(a.accepted[0].name, "Game (Europe)");
        assert_eq!(for_game(&conn, None, id, false).unwrap().status, "unsupported");

        // No RA game with achievements by this title (hacks aren't suggested).
        conn.execute("UPDATE games SET title = 'Other Game'", []).unwrap();
        assert_eq!(for_game(&conn, Some(&client), id, false).unwrap().status, "noset");

        // GBA missing from the active list: hidden, and nothing is hashed.
        conn.execute("DELETE FROM api_cache", []).unwrap();
        conn.execute("UPDATE games SET ra_hash = NULL, ra_game_id = NULL", []).unwrap();
        let n64_only = FakePost::new(|url: &str, _: &[(&str, &str)], _: &str| {
            if url.contains("API_GetConsoleIDs") {
                json(200, j!([{"ID": 2, "Name": "Nintendo 64", "Active": true}]))
            } else {
                Response { status: 404, body: vec![] }
            }
        });
        let client = Ra::new(&n64_only, creds());
        assert_eq!(for_game(&conn, Some(&client), id, false).unwrap().status, "unsupported");
        assert_eq!(queries::ra_state(&conn, id).unwrap().2, None);
    }
}
