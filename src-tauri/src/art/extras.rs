//! SteamGridDB pass (optional key): finds each game on SteamGridDB by its
//! names (its title, IGDB's and LaunchBox's names),
//! then stores its best-voted hero (wide background) and logo for TV Mode,
//! and a cover for games no keyless source had one for. Everything is cached
//! on disk; each game is looked up once (not found: retried weekly).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::Connection;
use serde::Serialize;

use super::http::Fetch;
use super::images::{self, Extra};
use super::sgdb::{self, Kind, Sgdb};
use super::Candidate;
use crate::db::queries;
use crate::error::{Error, Result};

pub const NOTFOUND_RETRY_SECS: i64 = 7 * 24 * 3600;

#[derive(Debug, Clone)]
pub struct Target {
    pub id: i64,
    pub sgdb_id: Option<i64>,
    /// No cover from any other source (and the user didn't remove it).
    pub needs_cover: bool,
    pub has_hero: bool,
    pub has_logo: bool,
    /// The user picked the hero/logo: leave them alone.
    pub extras_manual: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub matched: usize,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub matched: usize,
    pub not_found: usize,
    pub failed: usize,
    pub heroes: usize,
    pub logos: usize,
    pub covers: usize,
    pub cancelled: bool,
}

/// Finds a game on SteamGridDB by its titles.
fn find(sgdb: &Sgdb, titles: &[String]) -> Result<Option<i64>> {
    for t in titles {
        let results = sgdb.search(t)?;
        if let Some(g) = sgdb::best_match(&results, titles) {
            return Ok(Some(g.id));
        }
    }
    Ok(None)
}

/// Downloads and stores the best-voted image of a kind. Returns whether one existed.
fn fetch_extra(conn: &Connection, data_dir: &Path, sgdb: &Sgdb, fetch: &dyn Fetch, game_id: i64, sgdb_id: i64, extra: Extra) -> Result<bool> {
    let kind = match extra {
        Extra::Hero => Kind::Hero,
        Extra::Logo => Kind::Logo,
    };
    for img in sgdb.images(kind, sgdb_id)?.iter().take(3) {
        let Some(bytes) = fetch.get(&img.url)? else { continue };
        let (w, h) = images::store_extra(data_dir, extra, game_id, &bytes)?;
        queries::set_extra(conn, game_id, extra.kind(), "sgdb", &img.id.to_string(), &img.url, w, h, queries::now())?;
        return Ok(true);
    }
    Ok(false)
}

fn fetch_cover(conn: &Connection, data_dir: &Path, sgdb: &Sgdb, fetch: &dyn Fetch, game_id: i64, sgdb_id: i64) -> Result<bool> {
    for img in sgdb.images(Kind::Grid, sgdb_id)?.iter().take(3) {
        let Some(bytes) = fetch.get(&img.url)? else { continue };
        let p = images::process(&bytes)?;
        images::store(data_dir, game_id, &p)?;
        queries::set_boxart(conn, game_id, "sgdb", &img.id.to_string(), &img.url, p.width, p.height, queries::now())?;
        super::icons::cover_changed(conn, data_dir, game_id)?;
        return Ok(true);
    }
    Ok(false)
}

/// One game: match it (unless already matched), then fill in what's missing.
fn process(conn: &Connection, data_dir: &Path, sgdb: &Sgdb, fetch: &dyn Fetch, t: &Target, s: &mut Summary) -> Result<bool> {
    let sgdb_id = match t.sgdb_id {
        Some(id) => id,
        None => {
            let titles = queries::sgdb_titles(conn, t.id)?;
            match find(sgdb, &titles)? {
                Some(id) => id,
                None => return Ok(false),
            }
        }
    };
    queries::set_sgdb(conn, t.id, "ok", Some(sgdb_id), queries::now())?;
    if !t.extras_manual {
        if !t.has_hero && fetch_extra(conn, data_dir, sgdb, fetch, t.id, sgdb_id, Extra::Hero)? {
            s.heroes += 1;
        }
        if !t.has_logo && fetch_extra(conn, data_dir, sgdb, fetch, t.id, sgdb_id, Extra::Logo)? {
            s.logos += 1;
        }
    }
    if t.needs_cover && fetch_cover(conn, data_dir, sgdb, fetch, t.id, sgdb_id)? {
        s.covers += 1;
    }
    Ok(true)
}

pub fn run(
    conn: &mut Connection,
    data_dir: &Path,
    sgdb: &Sgdb,
    fetch: &dyn Fetch,
    progress: &dyn Fn(&Progress),
    cancel: &AtomicBool,
) -> Result<Summary> {
    let targets = queries::games_needing_sgdb(conn, queries::now())?;
    let mut s = Summary::default();
    let mut p = Progress { done: 0, total: targets.len(), matched: 0 };
    progress(&p);
    for t in &targets {
        if cancel.load(Ordering::Acquire) {
            s.cancelled = true;
            break;
        }
        match process(conn, data_dir, sgdb, fetch, t, &mut s) {
            Ok(true) => {
                s.matched += 1;
                p.matched += 1;
            }
            Ok(false) => {
                queries::set_sgdb(conn, t.id, "notfound", None, queries::now())?;
                s.not_found += 1;
            }
            Err(e) => {
                log::warn!("SteamGridDB for game {}: {e}", t.id);
                queries::set_sgdb(conn, t.id, "error", t.sgdb_id, queries::now())?;
                s.failed += 1;
                // A bad key fails every game the same way: stop here.
                if e.to_string().contains("API key") {
                    return Err(e);
                }
            }
        }
        p.done += 1;
        progress(&p);
    }
    Ok(s)
}

// ---------------------------------------------------------------- manual choice

/// The game's SteamGridDB id: stored, or searched for now. A `query` from the
/// picker's search box searches that instead (and isn't stored).
fn resolve(conn: &Connection, sgdb: &Sgdb, game_id: i64, query: Option<&str>) -> Result<Option<i64>> {
    if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
        // The user typed it: take SteamGridDB's top result.
        return Ok(sgdb.search(q)?.first().map(|g| g.id));
    }
    if let Some(id) = queries::sgdb_id(conn, game_id)? {
        return Ok(Some(id));
    }
    let titles = queries::sgdb_titles(conn, game_id)?;
    let found = find(sgdb, &titles)?;
    queries::set_sgdb(conn, game_id, if found.is_some() { "ok" } else { "notfound" }, found, queries::now())?;
    Ok(found)
}

/// Picker options from SteamGridDB: covers, heroes or logos.
pub fn candidates(conn: &Connection, sgdb: &Sgdb, game_id: i64, kind: Kind, query: Option<&str>) -> Result<Vec<Candidate>> {
    let Some(id) = resolve(conn, sgdb, game_id, query)? else { return Ok(Vec::new()) };
    let label = match kind {
        Kind::Grid => "SteamGridDB",
        Kind::Hero => "SteamGridDB background",
        Kind::Logo => "SteamGridDB logo",
        Kind::Icon => "SteamGridDB icon",
    };
    Ok(sgdb
        .images(kind, id)?
        .into_iter()
        .map(|img| Candidate {
            source: "sgdb".into(),
            name: img.id.to_string(),
            preview: Some(img.thumb.clone()),
            url: img.url,
            label: match (img.width, img.height) {
                (Some(w), Some(h)) => format!("{label} · {w}×{h}"),
                _ => label.to_string(),
            },
        })
        .collect())
}

/// Logos or backgrounds for a collection: SteamGridDB's first 5 games for the words, each one's images.
pub fn search_images(sgdb: &Sgdb, kind: Kind, query: &str) -> Result<Vec<Candidate>> {
    let mut out = Vec::new();
    let (mut asked, mut last_err) = (0, None);
    for game in sgdb.search(query)?.into_iter().take(5) {
        asked += 1;
        match sgdb.images(kind, game.id) {
            Ok(images) => out.extend(images.into_iter().map(|img| Candidate {
                source: "sgdb".into(),
                name: img.id.to_string(),
                preview: Some(img.thumb.clone()),
                url: img.url,
                label: match (img.width, img.height) {
                    (Some(w), Some(h)) => format!("{} · {w}×{h}", game.name),
                    _ => game.name.clone(),
                },
            })),
            Err(e) => {
                log::warn!("SteamGridDB images for {}: {e}", game.name);
                last_err = Some(e);
            }
        }
    }
    match last_err {
        Some(e) if out.is_empty() && asked > 0 => Err(e),
        _ => Ok(out),
    }
}

/// Sets a hero or logo the user picked (kept: never replaced automatically).
pub fn apply(conn: &Connection, data_dir: &Path, fetch: &dyn Fetch, game_id: i64, extra: Extra, source: &str, name: &str, url: &str) -> Result<()> {
    let bytes = fetch.get(url)?.ok_or_else(|| Error::msg("that image is no longer available"))?;
    let (w, h) = images::store_extra(data_dir, extra, game_id, &bytes)?;
    queries::set_extra(conn, game_id, extra.kind(), source, name, url, w, h, queries::now())?;
    queries::set_extras_manual(conn, game_id)
}

/// Removes a hero or logo for good (TV Mode falls back to the cover).
pub fn remove(conn: &Connection, data_dir: &Path, game_id: i64, extra: Extra) -> Result<()> {
    images::remove_extra(data_dir, extra, game_id);
    queries::clear_extra(conn, game_id, extra.kind())?;
    queries::set_extras_manual(conn, game_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::http::fake::{json, FakeHttp, FakePost};
    use crate::art::http::Response;
    use serde_json::json as j;

    fn api(url: &str, _: &[(&str, &str)], _: &str) -> Response {
        if url.contains("/search/autocomplete/Okami") {
            json(200, j!({"success": true, "data": [{"id": 77, "name": "Okami", "verified": true}]}))
        } else if url.contains("/search/") {
            json(200, j!({"success": true, "data": [{"id": 1, "name": "Something Else"}]}))
        } else if url.contains("/heroes/game/77") {
            json(200, j!({"success": true, "data": [{"id": 501, "url": "https://cdn/hero.png", "thumb": "https://cdn/hero_t.png"}]}))
        } else if url.contains("/logos/game/77") {
            json(200, j!({"success": true, "data": [{"id": 601, "url": "https://cdn/logo.png", "thumb": "https://cdn/logo_t.png"}]}))
        } else if url.contains("/grids/game/77") {
            json(200, j!({"success": true, "data": [{"id": 701, "url": "https://cdn/grid.png", "thumb": "https://cdn/grid_t.png"}]}))
        } else {
            Response { status: 404, body: vec![] }
        }
    }

    fn add(conn: &Connection, title: &str, art_status: &str) -> i64 {
        conn.execute(
            "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at, art_status)
             VALUES ('ps2', ?1, ?1, ?1, ?1, 0, ?2)",
            rusqlite::params![title, art_status],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn matches_by_title_and_stores_hero_logo_and_a_missing_cover() {
        let mut conn = crate::db::test_db();
        let okami = add(&conn, "Okami", "notfound");
        let other = add(&conn, "Unknown Thing", "ok");
        let data = tempfile::tempdir().unwrap();
        let api = FakePost::new(api);
        let sgdb = Sgdb::new(&api, "key".into());
        let mut cdn = FakeHttp::default();
        for f in ["hero", "logo", "grid"] {
            cdn.pages.insert(format!("https://cdn/{f}.png"), images::test_png(900, 400));
        }
        let s = run(&mut conn, data.path(), &sgdb, &cdn, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((s.matched, s.not_found, s.heroes, s.logos, s.covers), (1, 1, 1, 1, 1));
        assert!(images::extra_path(data.path(), Extra::Hero, okami).is_file());
        assert!(images::extra_path(data.path(), Extra::Logo, okami).is_file());
        let (status, sgdb_id): (String, Option<i64>) =
            conn.query_row("SELECT sgdb_status, sgdb_id FROM games WHERE id = ?1", [okami], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((status.as_str(), sgdb_id), ("ok", Some(77)));
        let art: String = conn.query_row("SELECT art_status FROM games WHERE id = ?1", [okami], |r| r.get(0)).unwrap();
        assert_eq!(art, "ok");
        let st: String = conn.query_row("SELECT sgdb_status FROM games WHERE id = ?1", [other], |r| r.get(0)).unwrap();
        assert_eq!(st, "notfound");

        // Nothing left to do on the next pass.
        let before = api.requests.lock().unwrap().len();
        let s = run(&mut conn, data.path(), &sgdb, &cdn, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(s.matched + s.not_found, 0);
        assert_eq!(api.requests.lock().unwrap().len(), before);

        // The library row carries the hero/logo versions for TV Mode.
        let row = queries::library(&conn).unwrap().into_iter().find(|g| g.id == okami).unwrap();
        assert!(row.hero > 0 && row.logo > 0);
    }

    #[test]
    fn launchbox_names_find_games_the_file_name_misses() {
        // SteamGridDB knows ".hack//Infection"; the file says "Dot Hack Part 1: Infection".
        let api = FakePost::new(|url: &str, _: &[(&str, &str)], _: &str| {
            if url.contains("/search/autocomplete/Dot") {
                json(200, j!({"success": true, "data": [{"id": 1, "name": "Dot"}]}))
            } else if url.contains("/search/autocomplete/") && url.contains("Infection") {
                json(200, j!({"success": true, "data": [{"id": 88, "name": ".hack//Infection"}]}))
            } else if url.contains("/heroes/game/88") {
                json(200, j!({"success": true, "data": [{"id": 801, "url": "https://cdn/h.png", "thumb": "https://cdn/h_t.png"}]}))
            } else {
                json(200, j!({"success": true, "data": []}))
            }
        });
        let conn = crate::db::test_db();
        let id = add(&conn, "Dot Hack Part 1: Infection", "ok");
        conn.execute_batch(&format!(
            "UPDATE games SET lb_id = 7218 WHERE id = {id};
             INSERT INTO lb_games (db_id, name, fetched_at) VALUES (7218, '.hack//Infection: Part 1', 0);
             INSERT INTO lb_names (db_id, platform_id, name, is_alt) VALUES (7218, 'ps2', '.hack//Infection', 1),
                                                                         (7218, 'ps2', 'Dot Hack Part 1: Infection', 1);"
        ))
        .unwrap();
        assert_eq!(
            queries::sgdb_titles(&conn, id).unwrap(),
            ["Dot Hack Part 1: Infection", ".hack//Infection: Part 1", ".hack//Infection"],
            "own title first, LaunchBox's names after, no repeats"
        );
        let sgdb = Sgdb::new(&api, "key".into());
        let c = candidates(&conn, &sgdb, id, Kind::Hero, None).unwrap();
        assert_eq!(c.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["801"]);
    }

    #[test]
    fn picker_offers_heroes_and_a_pick_sticks() {
        let conn = crate::db::test_db();
        let id = add(&conn, "Okami", "ok");
        let api = FakePost::new(api);
        let sgdb = Sgdb::new(&api, "key".into());
        let c = candidates(&conn, &sgdb, id, Kind::Hero, None).unwrap();
        assert_eq!((c[0].source.as_str(), c[0].name.as_str(), c[0].preview.as_deref()), ("sgdb", "501", Some("https://cdn/hero_t.png")));
        let data = tempfile::tempdir().unwrap();
        let mut cdn = FakeHttp::default();
        cdn.pages.insert("https://cdn/hero.png".into(), images::test_png(1920, 620));
        apply(&conn, data.path(), &cdn, id, Extra::Hero, "sgdb", &c[0].name, &c[0].url).unwrap();
        let manual: bool = conn.query_row("SELECT extras_manual FROM games WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert!(manual);
        remove(&conn, data.path(), id, Extra::Hero).unwrap();
        assert!(!images::extra_path(data.path(), Extra::Hero, id).exists());
        assert_eq!(queries::library(&conn).unwrap()[0].hero, 0);
    }

    #[test]
    fn collection_background_search_asks_for_heroes() {
        let api = FakePost::new(|url: &str, _: &[(&str, &str)], _: &str| {
            if url.contains("/search/autocomplete/") {
                json(200, j!({"success": true, "data": [{"id": 9, "name": "Okami"}]}))
            } else if url.contains("/heroes/game/9") {
                json(200, j!({"success": true, "data": [{"id": 5, "url": "https://cdn/h.png", "thumb": "https://cdn/t.png"}]}))
            } else {
                Response { status: 404, body: vec![] }
            }
        });
        let c = search_images(&Sgdb::new(&api, "key".into()), Kind::Hero, "Okami").unwrap();
        assert_eq!(c.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["5"]);
    }

    #[test]
    fn collection_logo_search_takes_five_games_and_skips_failures() {
        let api = FakePost::new(|url: &str, _: &[(&str, &str)], _: &str| {
            if url.contains("/search/autocomplete/") {
                let g: Vec<_> = (1..=7).map(|i| j!({"id": i, "name": format!("G{i}")})).collect();
                json(200, j!({"success": true, "data": g}))
            } else if url.contains("/logos/game/2") {
                Response { status: 500, body: vec![] }
            } else if let Some(n) = url.split("/logos/game/").nth(1).and_then(|n| n.split('?').next()) {
                json(200, j!({"success": true, "data": [{"id": 100 + n.parse::<i64>().unwrap(), "url": "https://cdn/l.png", "thumb": "https://cdn/t.png", "width": 600, "height": 200}]}))
            } else {
                Response { status: 404, body: vec![] }
            }
        });
        let sgdb = Sgdb::new(&api, "key".into());
        let c = search_images(&sgdb, Kind::Logo, "Zelda").unwrap();
        assert_eq!(c.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["101", "103", "104", "105"]);
        assert_eq!(c[0].label, "G1 · 600×200");
        assert!(search_images(&sgdb, Kind::Logo, "  ").unwrap().is_empty());
    }
}
