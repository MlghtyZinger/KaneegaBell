//! Collections: the user's own (named lists of games, any game in any number
//! of them) and the series KB knows from Wikidata, shown as ready-made ones
//! once the library has two or more of their games. Each can have a logo the
//! user picks (a game's clear logo, a cover, or their own image, copied into
//! KB's folder); series can be hidden.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::queries;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    /// "c:<id>" (the user's) or "s:<Wikidata item>" (a series or franchise).
    pub key: String,
    pub name: String,
    /// "mine" | "series" | "franchise"
    pub kind: String,
    /// The games in it, in its order (present games only).
    pub games: Vec<i64>,
    pub logo: Option<Logo>,
    pub hidden: bool,
    /// The user's collections: "added" | "title" | "release".
    pub game_order: Option<String>,
    /// Smart collections: the saved filter rules (none for manual/series).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Rules>,
}

/// A logo the user picked.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Logo {
    /// That game's clear logo.
    Game { id: i64 },
    /// That game's cover.
    Cover { id: i64 },
    /// The user's own image (version: cache-buster).
    File { version: i64 },
}

/// A smart collection's saved filter rules. Every field is optional; an empty
/// set matches every game. Stored as JSON in `collections.rules`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Rules {
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub genres: Vec<i64>,
    #[serde(default)]
    pub developers: Vec<String>,
    #[serde(default)]
    pub publishers: Vec<String>,
    #[serde(default)]
    pub year_from: Option<i64>,
    #[serde(default)]
    pub year_to: Option<i64>,
    /// "played" | "unplayed" | null (any).
    #[serde(default)]
    pub played: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    /// Play-time bounds in seconds (inclusive).
    #[serde(default)]
    pub play_from: Option<i64>,
    #[serde(default)]
    pub play_to: Option<i64>,
}

impl Rules {
    /// Trims, dedups and sorts the list fields so the same filter is written the
    /// same way, then rejects nonsense (negative ids, a from above its to).
    fn normalize(&mut self) {
        let tidy = |v: &mut Vec<String>| {
            for s in v.iter_mut() {
                *s = s.trim().to_string();
            }
            v.retain(|s| !s.is_empty());
            v.sort();
            v.dedup();
        };
        tidy(&mut self.platforms);
        tidy(&mut self.developers);
        tidy(&mut self.publishers);
        self.genres.sort_unstable();
        self.genres.dedup();
    }

    fn validate(&self) -> Result<()> {
        if self.genres.iter().any(|&g| g < 1) {
            return Err(Error::msg("A genre id must be positive."));
        }
        if self.year_from.is_some_and(|y| y < 1) || self.year_to.is_some_and(|y| y < 1) {
            return Err(Error::msg("A year must be positive."));
        }
        if let (Some(a), Some(b)) = (self.year_from, self.year_to) {
            if a > b {
                return Err(Error::msg("The year range is backwards."));
            }
        }
        if let Some(p) = &self.played {
            if !matches!(p.as_str(), "played" | "unplayed") {
                return Err(Error::msg("Played must be 'played' or 'unplayed'."));
            }
        }
        if self.play_from.is_some_and(|p| p < 0) || self.play_to.is_some_and(|p| p < 0) {
            return Err(Error::msg("Play-time values can't be negative."));
        }
        if let (Some(a), Some(b)) = (self.play_from, self.play_to) {
            if a > b {
                return Err(Error::msg("The play-time range is wrong."));
            }
        }
        Ok(())
    }

    fn to_json(mut self) -> Result<String> {
        self.normalize();
        self.validate()?;
        serde_json::to_string(&self).map_err(Error::from)
    }
}

/// Where a collection's own logo image lives (`data\art\c\`).
pub fn logo_path(data_dir: &Path, key: &str) -> Option<PathBuf> {
    let (kind, id) = parse_key(key)?;
    Some(data_dir.join("art").join("c").join(format!("{kind}-{id}.webp")))
}

/// "c:12" -> ('c', 12); "s:526700" -> ('s', 526700).
fn parse_key(key: &str) -> Option<(char, i64)> {
    let (kind, id) = key.split_once(':')?;
    let kind = match kind {
        "c" => 'c',
        "s" => 's',
        _ => return None,
    };
    Some((kind, id.parse().ok().filter(|n: &i64| *n > 0)?))
}

fn mine_id(key: &str) -> Result<i64> {
    match parse_key(key) {
        Some(('c', id)) => Ok(id),
        _ => Err(Error::msg("Only your own collections can be changed that way.")),
    }
}

struct Look {
    logo: Option<String>,
    version: i64,
    hidden: bool,
}

fn looks(conn: &Connection) -> Result<HashMap<String, Look>> {
    let mut stmt = conn.prepare("SELECT key, logo, logo_version, hidden FROM collection_looks")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, Look { logo: r.get(1)?, version: r.get(2)?, hidden: r.get(3)? })))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn logo_of(look: Option<&Look>) -> Option<Logo> {
    let look = look?;
    let logo = look.logo.as_deref()?;
    let id = |p: &str| logo.strip_prefix(p).and_then(|s| s.parse().ok());
    if logo == "file" {
        Some(Logo::File { version: look.version })
    } else if let Some(id) = id("game:") {
        Some(Logo::Game { id })
    } else {
        id("cover:").map(|id| Logo::Cover { id })
    }
}

/// Every collection: the user's (in their order), then series and franchises
/// with two or more of the library's games (by name). Hidden series are
/// included, marked, so they can be shown again.
pub fn list(conn: &Connection) -> Result<Vec<Collection>> {
    let looks = looks(conn)?;
    let mut out = Vec::new();

    let mut stmt = conn.prepare("SELECT id, name, game_order, rules FROM collections ORDER BY sort_order, id")?;
    let mine: Vec<(i64, String, String, Option<String>)> =
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
    for (id, name, order, rules_json) in mine {
        let key = format!("c:{id}");
        let look = looks.get(&key);
        let rules = rules_json.as_deref().and_then(|s| serde_json::from_str::<Rules>(s).ok());
        let smart = rules.is_some();
        out.push(Collection {
            games: if smart { Vec::new() } else { games_of(conn, id, &order)? },
            logo: logo_of(look),
            hidden: false,
            game_order: (!smart).then_some(order),
            rules,
            name,
            key,
            kind: if smart { "smart".into() } else { "mine".into() },
        });
    }

    // Series: one game per Wikidata entry (the first copy), in release order.
    let mut stmt = conn.prepare(
        "SELECT gs.series_qid, s.name, gs.kind, g.id, g.wd_id, w.year, gs.ord, g.sort_title
         FROM wd_game_series gs
         JOIN wd_series s ON s.qid = gs.series_qid
         JOIN games g ON g.wd_id = gs.game_qid AND g.wd_status = 'ok' AND g.missing = 0
         LEFT JOIN wd_games w ON w.qid = gs.game_qid
         ORDER BY g.id",
    )?;
    type Row = (i64, String, String, i64, i64, Option<i64>, Option<String>, String);
    let rows: Vec<Row> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?
        .collect::<rusqlite::Result<_>>()?;
    /// (game, Wikidata entry, year, number in the series, sort title)
    type Member = (i64, i64, Option<i64>, Option<String>, String);
    struct Series {
        name: String,
        kind: String,
        members: Vec<Member>,
    }
    let mut series: HashMap<i64, Series> = HashMap::new();
    for (qid, name, kind, game, entry, year, ord, title) in rows {
        let s = series.entry(qid).or_insert_with(|| Series { name, kind: kind.clone(), members: Vec::new() });
        if kind == "series" {
            s.kind = kind;
        }
        if !s.members.iter().any(|m| m.1 == entry) {
            s.members.push((game, entry, year, ord, title));
        }
    }
    let mut stmt = conn.prepare(
        "SELECT series_qid, MAX(series_name), MAX(series_kind)
         FROM series_membership_overrides GROUP BY series_qid",
    )?;
    let saved_series: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (qid, name, kind) in saved_series {
        series.entry(qid).or_insert_with(|| Series { name, kind, members: Vec::new() });
    }
    let mut series: Vec<(i64, Series)> = series.into_iter().collect();
    series.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()).then(a.1.kind.cmp(&b.1.kind).reverse()));
    let mut names = std::collections::HashSet::new();
    for (qid, s) in series {
        // A franchise named like a series shows once (as the series).
        let games = series_game_ids(conn, qid)?;
        let customized: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM series_membership_overrides WHERE series_qid = ?1)",
            [qid],
            |r| r.get(0),
        )?;
        if games.len() < 2 && !customized {
            continue;
        }
        if !names.insert(s.name.to_lowercase()) {
            continue;
        }
        let key = format!("s:{qid}");
        let look = looks.get(&key);
        out.push(Collection {
            games,
            logo: logo_of(look),
            hidden: look.is_some_and(|l| l.hidden),
            game_order: None,
            rules: None,
            name: s.name,
            kind: s.kind,
            key,
        });
    }
    Ok(out)
}

/// A Wikidata series' games, with persistent user additions and removals
/// applied on top of its current source membership.
pub fn series_game_ids(conn: &Connection, qid: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare_cached(
        "SELECT g.id, g.wd_id, w.year, gs.ord, g.sort_title
         FROM wd_game_series gs
         JOIN games g ON g.wd_id = gs.game_qid AND g.wd_status = 'ok' AND g.missing = 0
         LEFT JOIN wd_games w ON w.qid = gs.game_qid
         WHERE gs.series_qid = ?1
         ORDER BY g.id",
    )?;
    type Member = (i64, i64, Option<i64>, Option<String>, String);
    let rows: Vec<Member> = stmt
        .query_map([qid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut seen_entries = std::collections::HashSet::new();
    let mut members: Vec<Member> = rows.into_iter().filter(|m| seen_entries.insert(m.1)).collect();

    let mut stmt = conn.prepare_cached(
        "SELECT g.id, g.wd_id, w.year, g.sort_title, o.member
         FROM series_membership_overrides o
         JOIN games g ON g.id = o.game_id AND g.missing = 0
         LEFT JOIN wd_games w ON w.qid = g.wd_id
         WHERE o.series_qid = ?1",
    )?;
    let overrides: Vec<(i64, Option<i64>, Option<i64>, String, bool)> = stmt
        .query_map([qid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (game, entry, year, title, member) in overrides {
        if member {
            if !members.iter().any(|m| m.0 == game) {
                members.push((game, entry.unwrap_or(game), year, None, title));
            }
        } else {
            members.retain(|m| m.0 != game);
        }
    }

    let number = |o: &Option<String>| o.as_deref().and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(f64::MAX);
    members.sort_by(|a, b| {
        a.2.unwrap_or(i64::MAX)
            .cmp(&b.2.unwrap_or(i64::MAX))
            .then(number(&a.3).total_cmp(&number(&b.3)))
            .then_with(|| a.4.cmp(&b.4))
    });
    Ok(members.into_iter().map(|m| m.0).collect())
}

/// A collection's present games in its chosen order.
fn games_of(conn: &Connection, id: i64, order: &str) -> Result<Vec<i64>> {
    let by = match order {
        "title" => "g.sort_title COLLATE NOCASE, g.id",
        // Release date from LaunchBox, IGDB or Wikidata's year; unknown last.
        "release" => {
            "COALESCE(l.release_date, m.release_date, (w.year - 1970) * 31556952) IS NULL,
             COALESCE(l.release_date, m.release_date, (w.year - 1970) * 31556952), g.sort_title COLLATE NOCASE"
        }
        _ => "cg.added_at, g.id",
    };
    let sql = format!(
        "SELECT g.id FROM collection_games cg
         JOIN games g ON g.id = cg.game_id AND g.missing = 0
         LEFT JOIN lb_games l ON l.db_id = g.lb_id AND g.lb_status IN ('ok', 'manual')
         LEFT JOIN metadata m ON m.game_id = g.id
         LEFT JOIN wd_games w ON w.qid = g.wd_id AND g.wd_status = 'ok'
         WHERE cg.collection_id = ?1
         ORDER BY {by}"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let ids = stmt.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

fn clean_name(name: &str) -> Result<String> {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).collect();
    if name.is_empty() {
        return Err(Error::msg("Give the collection a name."));
    }
    Ok(name.chars().take(80).collect())
}

/// A new collection (last in the list), optionally with a first game. Returns its key.
pub fn create(conn: &Connection, name: &str, first_game: Option<i64>) -> Result<String> {
    let name = clean_name(name)?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM collections", [], |r| r.get(0))?;
    conn.execute("INSERT INTO collections (name, sort_order, created_at) VALUES (?1, ?2, ?3)", params![name, next, queries::now()])?;
    let key = format!("c:{}", conn.last_insert_rowid());
    if let Some(game) = first_game {
        set_member(conn, &key, game, true)?;
    }
    Ok(key)
}

/// A new smart collection (last in the list) whose games follow `rules`. Returns its key.
pub fn create_smart(conn: &Connection, name: &str, rules: Rules) -> Result<String> {
    let name = clean_name(name)?;
    let json = rules.to_json()?;
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM collections", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO collections (name, sort_order, created_at, rules) VALUES (?1, ?2, ?3, ?4)",
        params![name, next, queries::now(), json],
    )?;
    Ok(format!("c:{}", conn.last_insert_rowid()))
}

/// Replaces a smart collection's rules (turns a manual one smart if given).
pub fn set_rules(conn: &Connection, key: &str, rules: Rules) -> Result<()> {
    let json = rules.to_json()?;
    conn.execute("UPDATE collections SET rules = ?2 WHERE id = ?1", params![mine_id(key)?, json])?;
    Ok(())
}

pub fn rename(conn: &Connection, key: &str, name: &str) -> Result<()> {
    conn.execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![mine_id(key)?, clean_name(name)?])?;
    Ok(())
}

/// Deletes one of the user's collections (the games stay in the library).
pub fn delete(conn: &Connection, data_dir: &Path, key: &str) -> Result<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", [mine_id(key)?])?;
    conn.execute("DELETE FROM collection_looks WHERE key = ?1", [key])?;
    if let Some(p) = logo_path(data_dir, key) {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

/// Milliseconds, so games added in the same second keep their order.
fn now_millis() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Adds a game to (or removes it from) one of the user's collections.
pub fn set_member(conn: &Connection, key: &str, game_id: i64, member: bool) -> Result<()> {
    match parse_key(key) {
        Some(('c', id)) => {
            let smart: Option<bool> = conn
                .query_row("SELECT rules IS NOT NULL FROM collections WHERE id = ?1", [id], |r| r.get(0))
                .optional()?;
            match smart {
                None => return Err(Error::msg("That collection no longer exists.")),
                Some(true) => return Err(Error::msg("Games in a smart collection follow its rules.")),
                Some(false) => {}
            }
            if member {
                conn.execute(
                    "INSERT OR IGNORE INTO collection_games (collection_id, game_id, added_at) VALUES (?1, ?2, ?3)",
                    params![id, game_id, now_millis()],
                )?;
            } else {
                conn.execute("DELETE FROM collection_games WHERE collection_id = ?1 AND game_id = ?2", params![id, game_id])?;
            }
            Ok(())
        }
        Some(('s', qid)) => set_series_member(conn, qid, game_id, member),
        _ => Err(Error::msg("That collection can't be changed.")),
    }
}

fn set_series_member(conn: &Connection, qid: i64, game_id: i64, member: bool) -> Result<()> {
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1 AND missing = 0)", [game_id], |r| r.get(0))?;
    if !exists {
        return Err(Error::msg("That game is no longer in the library."));
    }
    let known: Option<(String, String)> = conn
        .query_row(
            "SELECT s.name,
                    CASE WHEN EXISTS(SELECT 1 FROM wd_game_series gs WHERE gs.series_qid = s.qid AND gs.kind = 'series')
                         THEN 'series' ELSE 'franchise' END
             FROM wd_series s WHERE s.qid = ?1",
            [qid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let saved: Option<(String, String)> = conn
        .query_row(
            "SELECT series_name, series_kind FROM series_membership_overrides WHERE series_qid = ?1 LIMIT 1",
            [qid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (name, kind) = known.or(saved).ok_or_else(|| Error::msg("That series is no longer available."))?;
    conn.execute(
        "INSERT INTO series_membership_overrides (series_qid, game_id, member, series_name, series_kind)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(series_qid, game_id) DO UPDATE SET
           member = excluded.member, series_name = excluded.series_name, series_kind = excluded.series_kind",
        params![qid, game_id, member, name, kind],
    )?;
    Ok(())
}

pub fn set_game_order(conn: &Connection, key: &str, order: &str) -> Result<()> {
    if !matches!(order, "added" | "title" | "release") {
        return Err(Error::msg("unknown order"));
    }
    conn.execute("UPDATE collections SET game_order = ?2 WHERE id = ?1", params![mine_id(key)?, order])?;
    Ok(())
}

/// Moves one of the user's collections earlier (-1) or later (+1) in the list.
pub fn move_by(conn: &mut Connection, key: &str, delta: i64) -> Result<()> {
    let id = mine_id(key)?;
    let tx = conn.transaction()?;
    let mut ids: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT id FROM collections ORDER BY sort_order, id")?;
        let ids = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        ids
    };
    let Some(i) = ids.iter().position(|&x| x == id) else { return Ok(()) };
    let j = (i as i64 + delta).clamp(0, ids.len() as i64 - 1) as usize;
    let moved = ids.remove(i);
    ids.insert(j, moved);
    for (n, id) in ids.iter().enumerate() {
        tx.execute("UPDATE collections SET sort_order = ?2 WHERE id = ?1", params![id, n as i64 + 1])?;
    }
    tx.commit()?;
    Ok(())
}

fn check_key(key: &str) -> Result<()> {
    parse_key(key).map(|_| ()).ok_or_else(|| Error::msg("unknown collection"))
}

/// Hides a series from the Collections tab (or shows it again).
pub fn set_hidden(conn: &Connection, key: &str, hidden: bool) -> Result<()> {
    check_key(key)?;
    conn.execute(
        "INSERT INTO collection_looks (key, hidden) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET hidden = excluded.hidden",
        params![key, hidden],
    )?;
    Ok(())
}

/// Picks the logo: "auto", "game:<id>" (its clear logo) or "cover:<id>".
pub fn set_logo(conn: &Connection, data_dir: &Path, key: &str, choice: &str) -> Result<()> {
    check_key(key)?;
    let logo = match choice {
        "auto" => None,
        c if c.strip_prefix("game:").or(c.strip_prefix("cover:")).and_then(|n| n.parse::<i64>().ok()).is_some() => Some(c),
        _ => return Err(Error::msg("unknown logo")),
    };
    conn.execute(
        "INSERT INTO collection_looks (key, logo) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET logo = excluded.logo",
        params![key, logo],
    )?;
    if let Some(p) = logo_path(data_dir, key) {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

/// Uses an image file as the logo: copied (resized, transparency kept) into
/// KB's folder, so the original can move or go.
pub fn set_logo_file(conn: &Connection, data_dir: &Path, key: &str, file: &Path) -> Result<()> {
    let path = logo_path(data_dir, key).ok_or_else(|| Error::msg("unknown collection"))?;
    let bytes = std::fs::read(file).map_err(|e| Error::msg(format!("Couldn't read that file: {e}")))?;
    crate::art::images::store_logo_at(&path, &bytes)?;
    conn.execute(
        "INSERT INTO collection_looks (key, logo, logo_version) VALUES (?1, 'file', 1)
         ON CONFLICT(key) DO UPDATE SET logo = 'file', logo_version = logo_version + 1",
        [key],
    )?;
    Ok(())
}

/// The logo look of a key, if any (for checks).
#[cfg(test)]
fn look_logo(conn: &Connection, key: &str) -> Option<String> {
    use rusqlite::OptionalExtension;
    conn.query_row("SELECT logo FROM collection_looks WHERE key = ?1", [key], |r| r.get(0)).optional().ok().flatten().flatten()
}

#[cfg(test)]
mod tests;
