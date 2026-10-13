//! All SQL lives here. Row structs serialize straight to the frontend (camelCase).

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::{named_params, params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::scanner::{path_key, Candidate, Known, PlatformRule, SourceSpec};

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

// ---------------------------------------------------------------- library

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRow {
    pub id: i64,
    pub platform_id: String,
    pub title: String,
    pub variant: Option<String>,
    pub region: Option<String>,
    pub favorite: bool,
    pub play_seconds: i64,
    pub last_played: Option<i64>,
    pub is_multidisc: bool,
    /// Art version when the game has art (thumbnail URL cache-buster), else 0.
    pub art: i64,
    /// The cover's height / width, so its card can take the cover's shape
    /// before the image loads.
    pub art_aspect: Option<f64>,
    /// Genre ids (from IGDB), for the genre filter.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<i64>,
}

/// Every present game, ordered by platform display order then title. This is
/// the one payload the UI needs at startup; ~100 bytes/game.
pub fn library(conn: &Connection) -> Result<Vec<GameRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT g.id, g.platform_id, g.title, g.variant, g.region, g.favorite, g.play_seconds,
                g.last_played, g.is_multidisc, CASE WHEN g.art_status = 'ok' THEN g.art_version ELSE 0 END,
                CASE WHEN g.art_status = 'ok' AND a.width > 0 AND a.height > 0
                     THEN ROUND(CAST(a.height AS REAL) / a.width, 3) END
         FROM games g JOIN platforms p ON p.id = g.platform_id
         LEFT JOIN artwork a ON a.game_id = g.id AND a.kind = 'boxart' AND a.selected = 1
         WHERE g.missing = 0 AND p.enabled = 1
         ORDER BY p.sort_order, g.sort_title, g.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(GameRow {
            id: r.get(0)?,
            platform_id: r.get(1)?,
            title: r.get(2)?,
            variant: r.get(3)?,
            region: r.get(4)?,
            favorite: r.get(5)?,
            play_seconds: r.get(6)?,
            last_played: r.get(7)?,
            is_multidisc: r.get(8)?,
            art: r.get(9)?,
            art_aspect: r.get(10)?,
            genres: Vec::new(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn count_games(conn: &Connection) -> Result<usize> {
    Ok(conn.query_row("SELECT COUNT(*) FROM games WHERE missing = 0", [], |r| r.get::<_, i64>(0))? as usize)
}

pub fn set_favorite(conn: &Connection, game_id: i64, favorite: bool) -> Result<()> {
    conn.execute("UPDATE games SET favorite = ?2 WHERE id = ?1", params![game_id, favorite])?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    pub id: i64,
    pub platform_id: String,
    pub title: String,
    pub variant: Option<String>,
    pub region: Option<String>,
    pub serial: Option<String>,
    pub launch_path: String,
    pub total_size: i64,
    pub is_multidisc: bool,
    pub play_seconds: i64,
    pub last_played: Option<i64>,
    /// Art version when the game has art, else 0.
    pub art: i64,
    /// Where the art came from ("libretro", "igdb", "sgdb").
    pub art_source: Option<String>,
    pub files: Vec<(String, Option<u32>)>,
}

pub fn game_detail(conn: &Connection, id: i64) -> Result<GameDetail> {
    let mut d = conn.query_row(
        "SELECT id, platform_id, title, variant, region, serial, launch_path, total_size, is_multidisc,
                play_seconds, last_played, CASE WHEN art_status = 'ok' THEN art_version ELSE 0 END,
                (SELECT source FROM artwork a WHERE a.game_id = games.id AND a.kind = 'boxart' AND a.selected = 1)
         FROM games WHERE id = ?1",
        [id],
        |r| {
            Ok(GameDetail {
                id: r.get(0)?,
                platform_id: r.get(1)?,
                title: r.get(2)?,
                variant: r.get(3)?,
                region: r.get(4)?,
                serial: r.get(5)?,
                launch_path: r.get(6)?,
                total_size: r.get(7)?,
                is_multidisc: r.get(8)?,
                play_seconds: r.get(9)?,
                last_played: r.get(10)?,
                art: r.get(11)?,
                art_source: r.get(12)?,
                files: Vec::new(),
            })
        },
    )?;
    let mut stmt = conn.prepare_cached("SELECT path, disc_no FROM game_files WHERE game_id = ?1 ORDER BY disc_no, path")?;
    d.files = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    Ok(d)
}

pub fn start_session(conn: &Connection, game_id: i64, now: i64) -> Result<()> {
    conn.execute("UPDATE games SET running_since = ?2 WHERE id = ?1", params![game_id, now])?;
    Ok(())
}

/// Adds a finished session's duration and stamps last-played with the end time.
pub fn end_session(conn: &Connection, game_id: i64, seconds: i64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE games SET play_seconds = play_seconds + ?2, last_played = ?3, running_since = NULL WHERE id = ?1",
        params![game_id, seconds.max(0), now],
    )?;
    Ok(())
}

/// A session that never recorded its end (app crash/power loss): keep the
/// last-played time but don't guess the duration.
pub fn reconcile_interrupted_sessions(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE games SET last_played = running_since, running_since = NULL WHERE running_since IS NOT NULL",
        [],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- artwork

/// Games whose platform has a libretro thumbnail set and that have no art:
/// never tried, failed last time, or not found more than a week ago.
pub fn games_needing_art(conn: &Connection, now: i64) -> Result<Vec<crate::art::Target>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, p.libretro_system, g.launch_path, g.title, g.search_key, g.region, g.platform_id, g.serial,
                (SELECT a.url FROM artwork a WHERE a.game_id = g.id AND a.kind = 'boxart' AND a.source = 'igdb' LIMIT 1)
         FROM games g JOIN platforms p ON p.id = g.platform_id
         WHERE g.missing = 0 AND p.enabled = 1 AND p.libretro_system IS NOT NULL
           AND (g.art_status IN ('none', 'error')
                OR (g.art_status = 'notfound' AND COALESCE(g.art_checked_at, 0) < ?1))
         ORDER BY p.sort_order, g.sort_title",
    )?;
    let rows = stmt.query_map([now - crate::art::NOTFOUND_RETRY_SECS], |r| {
        Ok(crate::art::Target {
            id: r.get(0)?,
            libretro_system: r.get(1)?,
            launch_path: r.get(2)?,
            title: r.get(3)?,
            search_key: r.get(4)?,
            region: r.get(5)?,
            platform_id: r.get(6)?,
            serial: r.get(7)?,
            igdb_cover: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn art_target(conn: &Connection, game_id: i64) -> Result<crate::art::ArtInfo> {
    Ok(conn.query_row(
        "SELECT g.launch_path, g.search_key, g.region, p.libretro_system, g.platform_id, g.serial,
                (SELECT a.url FROM artwork a WHERE a.game_id = g.id AND a.kind = 'boxart' AND a.source = 'igdb' LIMIT 1)
         FROM games g JOIN platforms p ON p.id = g.platform_id WHERE g.id = ?1",
        [game_id],
        |r| {
            Ok(crate::art::ArtInfo {
                platform_id: r.get(4)?,
                serial: r.get(5)?,
                igdb_cover: r.get(6)?,
                launch_path: r.get(0)?,
                search_key: r.get(1)?,
                region: r.get(2)?,
                libretro_system: r.get(3)?,
            })
        },
    )?)
}

/// Typical cover shape (height / width) per platform: the median of the
/// covers the library actually has, so GBA boxes come out square, SNES/N64
/// landscape and PSP tall, with no hard-coded table. Platforms without covers
/// are absent.
pub fn platform_art_aspects(conn: &Connection) -> Result<HashMap<String, f64>> {
    let mut stmt = conn.prepare(
        "SELECT g.platform_id, CAST(a.height AS REAL) / a.width
         FROM artwork a JOIN games g ON g.id = a.game_id
         WHERE a.kind = 'boxart' AND a.selected = 1 AND g.missing = 0 AND a.width > 0 AND a.height > 0",
    )?;
    let mut by_platform: HashMap<String, Vec<f64>> = HashMap::new();
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))? {
        let (p, aspect) = row?;
        by_platform.entry(p).or_default().push(aspect);
    }
    Ok(by_platform
        .into_iter()
        .map(|(p, mut v)| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            (p, v[v.len() / 2].clamp(0.5, 1.8))
        })
        .collect())
}

pub fn count_games_needing_art(conn: &Connection, now: i64) -> Result<usize> {
    Ok(games_needing_art(conn, now)?.len())
}

/// Records new box art as the game's selected art and bumps its version.
#[allow(clippy::too_many_arguments)]
pub fn set_boxart(
    conn: &Connection,
    game_id: i64,
    source: &str,
    remote_id: &str,
    url: &str,
    width: u32,
    height: u32,
    now: i64,
) -> Result<()> {
    conn.execute("UPDATE artwork SET selected = 0 WHERE game_id = ?1 AND kind = 'boxart'", [game_id])?;
    conn.execute(
        "INSERT INTO artwork (game_id, kind, source, remote_id, url, width, height, selected, fetched_at)
         VALUES (?1, 'boxart', ?2, ?3, ?4, ?5, ?6, 1, ?7)
         ON CONFLICT(game_id, kind, source, remote_id) DO UPDATE SET
             url = excluded.url, width = excluded.width, height = excluded.height,
             selected = 1, fetched_at = excluded.fetched_at",
        params![game_id, source, remote_id, url, width, height, now],
    )?;
    conn.execute(
        "UPDATE games SET art_status = 'ok', art_version = art_version + 1, art_checked_at = ?2 WHERE id = ?1",
        params![game_id, now],
    )?;
    Ok(())
}

pub fn set_art_status(conn: &Connection, game_id: i64, status: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE games SET art_status = ?2, art_checked_at = ?3 WHERE id = ?1 AND art_status != 'ok'",
        params![game_id, status, now],
    )?;
    Ok(())
}

/// Forgets a game's cover so the next pass picks one automatically again
/// (the IGDB match's cover stays known as a source).
pub fn reset_art(conn: &Connection, game_id: i64) -> Result<()> {
    conn.execute("DELETE FROM artwork WHERE game_id = ?1 AND kind = 'boxart' AND source != 'igdb'", [game_id])?;
    conn.execute("UPDATE artwork SET selected = 0 WHERE game_id = ?1 AND kind = 'boxart'", [game_id])?;
    conn.execute(
        "UPDATE games SET art_status = 'none', art_version = art_version + 1, art_checked_at = NULL, art_manual = 0
         WHERE id = ?1",
        [game_id],
    )?;
    Ok(())
}

/// Marks the cover as the user's choice (or removal): never replaced automatically.
pub fn set_art_manual(conn: &Connection, game_id: i64) -> Result<()> {
    conn.execute("UPDATE games SET art_manual = 1 WHERE id = ?1", [game_id])?;
    Ok(())
}

// ---------------------------------------------------------------- metadata

/// Games to look up on IGDB: never tried, failed last time, not found more
/// than a week ago, or matched but missing their stored details.
pub fn games_needing_metadata(conn: &Connection, now: i64) -> Result<Vec<crate::metadata::Target>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.title, g.search_key, p.igdb_platform, g.igdb_id, g.match_status
         FROM games g JOIN platforms p ON p.id = g.platform_id
         WHERE g.missing = 0 AND p.enabled = 1
           AND (g.match_status IN ('none', 'error')
                OR (g.match_status = 'notfound' AND COALESCE(g.meta_checked_at, 0) < ?1)
                OR (g.match_status IN ('ok', 'manual') AND NOT EXISTS (SELECT 1 FROM metadata m WHERE m.game_id = g.id)))
         ORDER BY p.sort_order, g.sort_title",
    )?;
    let rows = stmt.query_map([now - crate::metadata::NOTFOUND_RETRY_SECS], |r| {
        Ok(crate::metadata::Target {
            id: r.get(0)?,
            title: r.get(1)?,
            search_key: r.get(2)?,
            igdb_platform: r.get(3)?,
            igdb_id: r.get(4)?,
            manual: r.get::<_, String>(5)? == "manual",
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn count_games_needing_metadata(conn: &Connection, now: i64) -> Result<usize> {
    Ok(games_needing_metadata(conn, now)?.len())
}

/// Stores an IGDB match: the game's details, genres and IGDB id.
/// `status` is "ok" (automatic) or "manual" (the user picked it).
/// Returns true when the match brought a cover the game should now get (it
/// had none, or only GameTDB's generic PSN frame), so a cover pass should run.
pub fn set_metadata(conn: &Connection, game_id: i64, g: &crate::metadata::igdb::Game, status: &str, now: i64) -> Result<bool> {
    conn.execute(
        "INSERT INTO metadata (game_id, title, description, release_date, developer, publisher, source, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'igdb', ?7)
         ON CONFLICT(game_id) DO UPDATE SET title = excluded.title, description = excluded.description,
             release_date = excluded.release_date, developer = excluded.developer, publisher = excluded.publisher,
             source = excluded.source, fetched_at = excluded.fetched_at",
        params![game_id, g.name, g.summary, g.first_release_date, g.developer(), g.publisher(), now],
    )?;
    conn.execute("DELETE FROM game_genres WHERE game_id = ?1", [game_id])?;
    for genre in &g.genres {
        conn.execute("INSERT OR IGNORE INTO genres (name) VALUES (?1)", [&genre.name])?;
        conn.execute(
            "INSERT OR IGNORE INTO game_genres (game_id, genre_id) SELECT ?1, id FROM genres WHERE name = ?2",
            params![game_id, genre.name],
        )?;
    }
    conn.execute(
        "UPDATE games SET igdb_id = ?2, match_status = ?3, meta_checked_at = ?4 WHERE id = ?1",
        params![game_id, g.id, status, now],
    )?;
    // The match's cover: one more automatic cover source (not selected here).
    conn.execute("DELETE FROM artwork WHERE game_id = ?1 AND source = 'igdb' AND selected = 0", [game_id])?;
    let Some(cover) = &g.cover else { return Ok(false) };
    let url = crate::metadata::igdb::cover_image(&cover.image_id, "1080p");
    conn.execute(
        "INSERT INTO artwork (game_id, kind, source, remote_id, url, selected, fetched_at)
         VALUES (?1, 'boxart', 'igdb', ?2, ?3, 0, ?4)
         ON CONFLICT(game_id, kind, source, remote_id) DO UPDATE SET url = excluded.url",
        params![game_id, cover.image_id, url, now],
    )?;
    // Queue it for games without a cover, or showing GameTDB's generic PSN
    // frame (PSN title IDs start with NP); never over a cover the user chose.
    let queued = conn.execute(
        "UPDATE games SET art_status = 'none', art_checked_at = NULL
         WHERE id = ?1 AND art_manual = 0
           AND (art_status IN ('notfound', 'error')
                OR (art_status = 'ok' AND serial LIKE 'NP%'
                    AND EXISTS (SELECT 1 FROM artwork a WHERE a.game_id = games.id AND a.selected = 1 AND a.source = 'gametdb')))",
        [game_id],
    )?;
    Ok(queued > 0)
}

/// Records a failed lookup ('notfound' / 'error') without touching a match.
pub fn set_match_status(conn: &Connection, game_id: i64, status: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE games SET match_status = ?2, meta_checked_at = ?3 WHERE id = ?1 AND match_status NOT IN ('ok', 'manual')",
        params![game_id, status, now],
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub release_date: Option<i64>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genres: Vec<String>,
    pub igdb_id: Option<i64>,
    pub match_status: String,
}

pub fn game_metadata(conn: &Connection, game_id: i64) -> Result<GameMetadata> {
    let (igdb_id, match_status): (Option<i64>, String) =
        conn.query_row("SELECT igdb_id, match_status FROM games WHERE id = ?1", [game_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let row = conn
        .query_row(
            "SELECT title, description, release_date, developer, publisher FROM metadata WHERE game_id = ?1",
            [game_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let mut stmt = conn.prepare_cached(
        "SELECT g.name FROM game_genres gg JOIN genres g ON g.id = gg.genre_id WHERE gg.game_id = ?1 ORDER BY g.name",
    )?;
    let genres = stmt.query_map([game_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let (title, description, release_date, developer, publisher) = row.unwrap_or_default();
    Ok(GameMetadata { title, description, release_date, developer, publisher, genres, igdb_id, match_status })
}

/// Genre names by id, and each present game's genre ids, for the genre filter.
pub fn genres(conn: &Connection) -> Result<(Vec<(i64, String)>, HashMap<i64, Vec<i64>>)> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT ge.id, ge.name FROM genres ge JOIN game_genres gg ON gg.genre_id = ge.id ORDER BY ge.name",
    )?;
    let names = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut by_game: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT game_id, genre_id FROM game_genres")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (g, ge) = row?;
        by_game.entry(g).or_default().push(ge);
    }
    Ok((names, by_game))
}

// ---------------------------------------------------------------- RetroAchievements

/// Games on the given RA consoles that aren't hashed yet, or are hashed but
/// not looked up.
pub fn games_needing_ra(
    conn: &Connection,
    consoles: &std::collections::HashSet<u32>,
) -> Result<Vec<crate::achievements::Target>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, p.ra_console_id, g.launch_path, g.ra_hash
         FROM games g JOIN platforms p ON p.id = g.platform_id
         WHERE g.missing = 0 AND p.enabled = 1 AND p.ra_console_id IS NOT NULL
           AND (g.ra_hash IS NULL OR (g.ra_hash != '' AND g.ra_game_id IS NULL))
         ORDER BY p.sort_order, g.sort_title",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(crate::achievements::Target {
            id: r.get(0)?,
            console: r.get(1)?,
            launch_path: PathBuf::from(r.get::<_, String>(2)?),
            hash: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for t in rows {
        let t = t?;
        if consoles.contains(&t.console) {
            out.push(t);
        }
    }
    Ok(out)
}

/// "RA doesn't know this hash" goes back to "not looked up" for consoles
/// whose hash list was last fetched before `stale_before`, so the next pass
/// fetches a fresh list and tries again.
pub fn reopen_ra_unknown(conn: &Connection, stale_before: i64) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE games SET ra_game_id = NULL
         WHERE ra_game_id = 0 AND platform_id IN (
             SELECT p.id FROM platforms p
             WHERE COALESCE((SELECT fetched_at FROM api_cache WHERE key = 'ra:hashes:' || p.ra_console_id), 0) < ?1)",
        [stale_before],
    )?)
}

pub fn set_ra_hash(conn: &Connection, game_id: i64, hash: &str) -> Result<()> {
    conn.execute("UPDATE games SET ra_hash = ?2, ra_game_id = NULL WHERE id = ?1", params![game_id, hash])?;
    Ok(())
}

pub fn set_ra_game(conn: &Connection, game_id: i64, ra_game_id: i64) -> Result<()> {
    conn.execute("UPDATE games SET ra_game_id = ?2 WHERE id = ?1", params![game_id, ra_game_id])?;
    Ok(())
}

/// (RA console, launch path, ra_hash, ra_game_id) for one game.
pub fn ra_state(conn: &Connection, game_id: i64) -> Result<(Option<u32>, String, Option<String>, Option<i64>)> {
    Ok(conn.query_row(
        "SELECT p.ra_console_id, g.launch_path, g.ra_hash, g.ra_game_id
         FROM games g JOIN platforms p ON p.id = g.platform_id WHERE g.id = ?1",
        [game_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?)
}

/// A game's names to match against RA's titles: its own and IGDB's.
pub fn game_titles(conn: &Connection, game_id: i64) -> Result<Vec<String>> {
    let (title, igdb): (String, Option<String>) = conn.query_row(
        "SELECT g.title, m.title FROM games g LEFT JOIN metadata m ON m.game_id = g.id WHERE g.id = ?1",
        [game_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(std::iter::once(title).chain(igdb).collect())
}

/// Present games on RA consoles still to hash or look up (for deciding
/// whether a pass is worth starting).
pub fn count_games_needing_ra(conn: &Connection, consoles: &std::collections::HashSet<u32>) -> Result<usize> {
    Ok(games_needing_ra(conn, consoles)?.len())
}

pub fn cache_get(conn: &Connection, key: &str) -> Result<Option<(Vec<u8>, i64)>> {
    Ok(conn
        .query_row("SELECT body, fetched_at FROM api_cache WHERE key = ?1", [key], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?)
}

pub fn cache_put(conn: &Connection, key: &str, body: &[u8], now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO api_cache (key, body, fetched_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET body = excluded.body, fetched_at = excluded.fetched_at",
        params![key, body, now],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- scanning

pub fn platform_rules(conn: &Connection) -> Result<Vec<PlatformRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(extensions_override, extensions) FROM platforms WHERE enabled = 1 ORDER BY sort_order",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, exts) = row?;
        out.push(PlatformRule { id, extensions: serde_json::from_str(&exts)? });
    }
    Ok(out)
}

pub fn sources(conn: &Connection) -> Result<Vec<SourceSpec>> {
    let mut stmt = conn.prepare("SELECT id, path, platform_id, recursive, kind FROM rom_sources ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok(SourceSpec {
            id: r.get(0)?,
            path: PathBuf::from(r.get::<_, String>(1)?),
            platform_id: r.get(2)?,
            recursive: r.get(3)?,
            kind: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn known_games(conn: &Connection) -> Result<HashMap<String, Known>> {
    let mut stmt = conn.prepare("SELECT launch_path, platform_id, serial, total_size, mtime FROM games")?;
    let rows = stmt.query_map([], |r| {
        let path: String = r.get(0)?;
        Ok((
            path_key(std::path::Path::new(&path)),
            Known { platform: r.get(1)?, serial: r.get(2)?, size: r.get::<_, i64>(3)? as u64, mtime: r.get(4)? },
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ApplyResult {
    pub added: usize,
    pub updated: usize,
    /// No longer on disk, kept (hidden) because they carry user data.
    pub missing: usize,
    /// No longer on disk and nothing worth keeping: deleted.
    pub removed: usize,
    /// Which games were added, removed or moved, for the scan report.
    pub changes: Vec<GameChange>,
}

/// One line of the scan report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameChange {
    /// "added" | "removed" (deleted) | "hidden" (gone, kept for its play time
    /// or favorite) | "moved" (now detected as another platform)
    pub kind: &'static str,
    pub platform_id: String,
    /// For "moved": the platform it was listed under before.
    pub from_platform: Option<String>,
    pub title: String,
    pub variant: Option<String>,
}

/// Writes one source's scan results.
pub fn apply_scan(
    tx: &Transaction,
    source_id: i64,
    candidates: &[Candidate],
    known: &HashMap<String, Known>,
) -> Result<ApplyResult> {
    tx.execute_batch("CREATE TEMP TABLE IF NOT EXISTS scan_seen (id INTEGER PRIMARY KEY); DELETE FROM scan_seen;")?;
    let mut upsert = tx.prepare_cached(
        "INSERT INTO games (platform_id, title, variant, sort_title, search_key, region, serial, launch_path,
                            source_id, total_size, mtime, is_multidisc, missing, added_at)
         VALUES (:platform, :title, :variant, :sort, :search, :region, :serial, :path,
                 :source, :size, :mtime, :multi, 0, :now)
         ON CONFLICT(launch_path) DO UPDATE SET
             platform_id = excluded.platform_id, title = excluded.title, variant = excluded.variant,
             sort_title = excluded.sort_title, search_key = excluded.search_key, region = excluded.region,
             serial = excluded.serial, source_id = excluded.source_id, total_size = excluded.total_size,
             mtime = excluded.mtime, is_multidisc = excluded.is_multidisc, missing = 0,
             -- Changed files get hashed again for RetroAchievements.
             ra_hash = CASE WHEN games.total_size = excluded.total_size AND games.mtime = excluded.mtime
                            THEN games.ra_hash END,
             ra_game_id = CASE WHEN games.total_size = excluded.total_size AND games.mtime = excluded.mtime
                               THEN games.ra_game_id END
         RETURNING id",
    )?;
    let mut seen = tx.prepare_cached("INSERT OR IGNORE INTO scan_seen (id) VALUES (?1)")?;
    let mut clear_files = tx.prepare_cached("DELETE FROM game_files WHERE game_id = ?1")?;
    let mut add_file = tx.prepare_cached(
        "INSERT OR REPLACE INTO game_files (game_id, path, disc_no, size, mtime) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;

    let now = now();
    let (mut added, mut updated) = (0, 0);
    let mut changes = Vec::new();
    let change = |kind, c: &Candidate, from_platform| GameChange {
        kind,
        platform_id: c.platform.clone(),
        from_platform,
        title: c.name.title.clone(),
        variant: c.name.variant.clone(),
    };
    for c in candidates {
        match known.get(&path_key(&c.launch_path)) {
            None => {
                added += 1;
                changes.push(change("added", c, None));
            }
            Some(k) if k.platform != c.platform => {
                updated += 1;
                changes.push(change("moved", c, Some(k.platform.clone())));
            }
            Some(k) if k.size != c.total_size || k.mtime != c.mtime => updated += 1,
            Some(_) => {}
        }
        let id: i64 = upsert.query_row(
            named_params! {
                ":platform": c.platform,
                ":title": c.name.title,
                ":variant": c.name.variant,
                ":sort": c.name.sort_title,
                ":search": c.name.search_key,
                ":region": c.name.region,
                ":serial": c.serial,
                ":path": c.launch_path.to_string_lossy(),
                ":source": source_id,
                ":size": c.total_size as i64,
                ":mtime": c.mtime,
                ":multi": c.is_multidisc,
                ":now": now,
            },
            |r| r.get(0),
        )?;
        seen.execute([id])?;
        clear_files.execute([id])?;
        for f in &c.files {
            add_file.execute(params![id, f.path.to_string_lossy(), f.disc, f.size as i64, f.mtime])?;
        }
    }
    // Gone from disk: delete rows with nothing worth keeping (junk that a
    // looser scan picked up, files the user deleted); keep the rest hidden,
    // so play time and favorites survive a file being moved back.
    {
        let mut gone = tx.prepare_cached(&format!(
            "SELECT platform_id, title, variant, {HAS_USER_DATA} FROM games
             WHERE source_id = ?1 AND missing = 0 AND id NOT IN (SELECT id FROM scan_seen)
             ORDER BY sort_title"
        ))?;
        let rows = gone.query_map([source_id], |r| {
            Ok(GameChange {
                kind: if r.get::<_, bool>(3)? { "hidden" } else { "removed" },
                platform_id: r.get(0)?,
                from_platform: None,
                title: r.get(1)?,
                variant: r.get(2)?,
            })
        })?;
        for row in rows {
            changes.push(row?);
        }
    }
    let removed = tx.execute(
        &format!(
            "DELETE FROM games WHERE source_id = ?1 AND id NOT IN (SELECT id FROM scan_seen) AND NOT {HAS_USER_DATA}"
        ),
        [source_id],
    )?;
    let missing = tx.execute(
        "UPDATE games SET missing = 1
         WHERE source_id = ?1 AND missing = 0 AND id NOT IN (SELECT id FROM scan_seen)",
        [source_id],
    )?;
    Ok(ApplyResult { added, updated, missing, removed, changes })
}

// ---------------------------------------------------------------- settings

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorRow {
    pub id: String,
    pub name: String,
    pub exe_path: Option<String>,
    pub exe_exists: bool,
    pub default_template: String,
    pub template_override: Option<String>,
    pub supports_m3u: bool,
}

pub fn emulators(conn: &Connection) -> Result<Vec<EmulatorRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, exe_path, default_template, template_override, supports_m3u FROM emulators ORDER BY name",
    )?;
    let rows = stmt.query_map([], |r| {
        let exe_path: Option<String> = r.get(2)?;
        Ok(EmulatorRow {
            id: r.get(0)?,
            name: r.get(1)?,
            exe_exists: exe_path.as_deref().is_some_and(|p| std::path::Path::new(p).is_file()),
            exe_path,
            default_template: r.get(3)?,
            template_override: r.get(4)?,
            supports_m3u: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn emulator_exe(conn: &Connection, id: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT exe_path FROM emulators WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .flatten())
}

fn empty_to_none(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn update_emulator(conn: &Connection, id: &str, exe_path: Option<String>, template_override: Option<String>) -> Result<()> {
    let n = conn.execute(
        "UPDATE emulators SET exe_path = ?2, template_override = ?3 WHERE id = ?1",
        params![id, empty_to_none(exe_path), empty_to_none(template_override)],
    )?;
    if n == 0 {
        return Err(Error::msg(format!("unknown emulator {id}")));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformRow {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub emulator_id: String,
    pub default_core: Option<String>,
    pub core_override: Option<String>,
    pub template_override: Option<String>,
    pub default_extensions: Vec<String>,
    pub extensions_override: Option<Vec<String>>,
    pub enabled: bool,
    pub game_count: i64,
}

pub fn platforms(conn: &Connection) -> Result<Vec<PlatformRow>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.short_name, p.emulator_id, p.default_core, p.core_override, p.template_override,
                p.extensions, p.extensions_override, p.enabled,
                (SELECT COUNT(*) FROM games g WHERE g.platform_id = p.id AND g.missing = 0)
         FROM platforms p ORDER BY p.sort_order",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            PlatformRow {
                id: r.get(0)?,
                name: r.get(1)?,
                short_name: r.get(2)?,
                emulator_id: r.get(3)?,
                default_core: r.get(4)?,
                core_override: r.get(5)?,
                template_override: r.get(6)?,
                default_extensions: Vec::new(),
                extensions_override: None,
                enabled: r.get(9)?,
                game_count: r.get(10)?,
            },
            r.get::<_, String>(7)?,
            r.get::<_, Option<String>>(8)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (mut p, exts, over) = row?;
        p.default_extensions = serde_json::from_str(&exts)?;
        p.extensions_override = over.map(|o| serde_json::from_str(&o)).transpose()?;
        out.push(p);
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformUpdate {
    pub id: String,
    pub core_override: Option<String>,
    pub template_override: Option<String>,
    pub extensions_override: Option<Vec<String>>,
    pub enabled: bool,
}

pub fn update_platform(conn: &Connection, u: PlatformUpdate) -> Result<()> {
    let exts = u
        .extensions_override
        .map(|v| {
            let v: Vec<String> = v
                .into_iter()
                .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|e| !e.is_empty())
                .collect();
            serde_json::to_string(&v)
        })
        .transpose()?;
    let n = conn.execute(
        "UPDATE platforms SET core_override = ?2, template_override = ?3, extensions_override = ?4, enabled = ?5
         WHERE id = ?1",
        params![u.id, empty_to_none(u.core_override), empty_to_none(u.template_override), exts, u.enabled],
    )?;
    if n == 0 {
        return Err(Error::msg(format!("unknown platform {}", u.id)));
    }
    Ok(())
}

/// Everything needed to render a launch for a platform.
pub struct LaunchConfig {
    pub emulator_id: String,
    pub emulator_name: String,
    pub exe_path: Option<String>,
    pub template: String,
    pub core: Option<String>,
    pub supports_m3u: bool,
}

pub fn launch_config(conn: &Connection, platform_id: &str) -> Result<LaunchConfig> {
    Ok(conn.query_row(
        "SELECT e.id, e.name, e.exe_path,
                COALESCE(p.template_override, e.template_override, e.default_template),
                COALESCE(p.core_override, p.default_core), e.supports_m3u
         FROM platforms p JOIN emulators e ON e.id = p.emulator_id WHERE p.id = ?1",
        [platform_id],
        |r| {
            Ok(LaunchConfig {
                emulator_id: r.get(0)?,
                emulator_name: r.get(1)?,
                exe_path: r.get(2)?,
                template: r.get(3)?,
                core: r.get(4)?,
                supports_m3u: r.get(5)?,
            })
        },
    )?)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRow {
    pub id: i64,
    pub path: String,
    pub platform_id: Option<String>,
    pub recursive: bool,
    pub kind: String,
    pub exists: bool,
}

pub fn source_rows(conn: &Connection) -> Result<Vec<SourceRow>> {
    Ok(sources(conn)?
        .into_iter()
        .map(|s| SourceRow {
            exists: s.path.is_dir(),
            id: s.id,
            path: s.path.to_string_lossy().into_owned(),
            platform_id: s.platform_id,
            recursive: s.recursive,
            kind: s.kind,
        })
        .collect())
}

pub fn add_source(conn: &Connection, path: &str, platform_id: Option<&str>, kind: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO rom_sources (path, platform_id, kind) VALUES (?1, ?2, ?3)",
        params![path, platform_id, kind],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
            Error::msg("that folder is already a source")
        }
        e => e.into(),
    })?;
    Ok(conn.last_insert_rowid())
}

pub fn update_source(conn: &Connection, id: i64, platform_id: Option<&str>, recursive: bool) -> Result<()> {
    conn.execute(
        "UPDATE rom_sources SET platform_id = ?2, recursive = ?3 WHERE id = ?1",
        params![id, platform_id, recursive],
    )?;
    Ok(())
}

/// Rows worth keeping when their files are gone: the user played or
/// favorited them.
const HAS_USER_DATA: &str =
    "(play_seconds > 0 OR favorite = 1 OR last_played IS NOT NULL OR running_since IS NOT NULL)";

/// Removing a source deletes its games, except ones with play time or a
/// favorite, which stay hidden in case the folder is added back. Returns the
/// games that left the library.
pub fn remove_source(conn: &Connection, id: i64) -> Result<Vec<GameChange>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT platform_id, title, variant, {HAS_USER_DATA} FROM games
         WHERE source_id = ?1 AND missing = 0 ORDER BY platform_id, sort_title"
    ))?;
    let gone = stmt
        .query_map([id], |r| {
            Ok(GameChange {
                kind: if r.get::<_, bool>(3)? { "hidden" } else { "removed" },
                platform_id: r.get(0)?,
                from_platform: None,
                title: r.get(1)?,
                variant: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    conn.execute(&format!("DELETE FROM games WHERE source_id = ?1 AND NOT {HAS_USER_DATA}"), [id])?;
    conn.execute("UPDATE games SET missing = 1 WHERE source_id = ?1", [id])?;
    conn.execute("DELETE FROM rom_sources WHERE id = ?1", [id])?;
    Ok(gone)
}

/// Startup housekeeping: hidden rows whose source is gone and that carry no
/// user data are leftovers (e.g. junk from an over-broad Auto folder that was
/// later removed). Returns how many were deleted.
pub fn prune_orphans(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        &format!("DELETE FROM games WHERE missing = 1 AND source_id IS NULL AND NOT {HAS_USER_DATA}"),
        [],
    )?)
}

pub fn get_setting<T: serde::de::DeserializeOwned>(conn: &Connection, key: &str) -> Result<Option<T>> {
    let raw: Option<String> =
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?;
    raw.map(|v| serde_json::from_str(&v).map_err(Error::from)).transpose()
}

pub fn set_setting(conn: &Connection, key: &str, value: &serde_json::Value) -> Result<()> {
    if value.is_null() {
        conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
    } else {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.to_string()],
        )?;
    }
    Ok(())
}

pub fn all_settings(conn: &Connection) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut map = serde_json::Map::new();
    for row in rows {
        let (k, v) = row?;
        map.insert(k, serde_json::from_str(&v)?);
    }
    Ok(map)
}
