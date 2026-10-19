//! SteamGridDB API client (https://www.steamgriddb.com/api/v2).
//!
//! - Auth: each user's own API key (steamgriddb.com → Preferences → API),
//!   sent as `Authorization: Bearer <key>`. Kept in the OS credential store.
//! - Limits: none published. This client is sequential and spaces requests
//!   >= 250 ms apart, backs off once on 429, and everything it learns is
//!   stored, so each game is looked up once.
//! - Terms: personal, non-commercial use; no scraping or excessive load. The
//!   API is the sanctioned way to automate; images come from SteamGridDB's
//!   CDN and are cached locally.
//! - Games are found by title (`/search/autocomplete`); there's no lookup by
//!   IGDB id, and console games have no store ids.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;

use super::http::Get;
use crate::error::{Error, Result};

pub const API: &str = "https://www.steamgriddb.com/api/v2";
const MIN_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Game {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub verified: bool,
}

/// A grid (cover), hero (wide background) or logo.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Image {
    pub id: i64,
    pub url: String,
    pub thumb: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Portrait covers (Steam's library "capsule" sizes).
    Grid,
    Hero,
    Logo,
    Icon,
}

impl Kind {
    fn path(self, game_id: i64) -> String {
        // Static images only; no NSFW, joke or flashing-image entries.
        let base = "types=static&nsfw=false&humor=false&epilepsy=false&limit=24";
        match self {
            Kind::Grid => format!("/grids/game/{game_id}?dimensions=600x900,342x482,660x930&{base}"),
            Kind::Hero => format!("/heroes/game/{game_id}?{base}"),
            Kind::Logo => format!("/logos/game/{game_id}?{base}"),
            Kind::Icon => format!("/icons/game/{game_id}?{base}"),
        }
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    #[serde(default)]
    success: bool,
    data: Option<T>,
}

pub struct Sgdb<'a> {
    http: &'a dyn Get,
    key: String,
    last: Mutex<Option<Instant>>,
}

impl<'a> Sgdb<'a> {
    pub fn new(http: &'a dyn Get, key: String) -> Self {
        Self { http, key, last: Mutex::new(None) }
    }

    fn wait_turn(&self) {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(rest) = last.and_then(|t| MIN_INTERVAL.checked_sub(t.elapsed())) {
            std::thread::sleep(rest);
        }
        *last = Some(Instant::now());
    }

    /// One API call; `Ok(None)` when SteamGridDB has nothing (404).
    fn call<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<Option<T>> {
        let url = format!("{API}{path}");
        let auth = format!("Bearer {}", self.key);
        for attempt in 0..2 {
            self.wait_turn();
            let resp = self.http.get_raw(&url, &[("Authorization", &auth)])?;
            match resp.status {
                200..=299 => {
                    let env: Envelope<T> = serde_json::from_slice(&resp.body)?;
                    return Ok(if env.success { env.data } else { None });
                }
                404 => return Ok(None),
                401 | 403 => return Err(Error::msg("SteamGridDB didn't accept the API key (check it in Settings)")),
                429 if attempt == 0 => std::thread::sleep(Duration::from_secs(5)),
                429 => return Err(Error::msg("SteamGridDB is limiting requests; try again in a minute")),
                s => return Err(Error::msg(format!("SteamGridDB answered HTTP {s}"))),
            }
        }
        unreachable!()
    }

    pub fn search(&self, term: &str) -> Result<Vec<Game>> {
        let term = term.trim();
        if term.is_empty() {
            return Ok(Vec::new());
        }
        let path = format!("/search/autocomplete/{}", utf8_percent_encode(term, NON_ALPHANUMERIC));
        Ok(self.call(&path)?.unwrap_or_default())
    }

    /// Best-voted first (SteamGridDB's own order).
    pub fn images(&self, kind: Kind, game_id: i64) -> Result<Vec<Image>> {
        Ok(self.call(&kind.path(game_id))?.unwrap_or_default())
    }
}

/// The search result whose name matches one of `titles` (same rules as
/// covers and IGDB: sequel numbers must agree); verified entries win ties.
pub fn best_match<'a>(results: &'a [Game], titles: &[String]) -> Option<&'a Game> {
    use super::matcher::{title_similarity, FUZZY_MIN};
    use crate::scanner::normalize::search_key;
    let keys: Vec<String> = titles.iter().map(|t| search_key(t)).filter(|k| !k.is_empty()).collect();
    let mut best: Option<(f32, bool, &Game)> = None;
    for g in results {
        let k = search_key(&g.name);
        for q in &keys {
            let s = title_similarity(q, &k);
            let better = best.is_none_or(|(bs, bv, _)| s > bs || (s == bs && g.verified && !bv));
            if s >= FUZZY_MIN && better {
                best = Some((s, g.verified, g));
            }
        }
    }
    best.map(|(_, _, g)| g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::http::fake::{json, FakePost};
    use crate::art::http::Response;
    use serde_json::json as j;

    #[test]
    fn requests_send_the_key_as_a_bearer_token() {
        let http = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| {
            json(200, j!({"success": true, "data": [{"id": 5, "name": "Okami", "verified": true}]}))
        });
        let s = Sgdb::new(&http, "abc".into());
        assert_eq!(s.search("Ōkami HD").unwrap()[0].id, 5);
        let reqs = http.requests.lock().unwrap();
        assert_eq!(reqs[0].0, "https://www.steamgriddb.com/api/v2/search/autocomplete/%C5%8Ckami%20HD");
        assert_eq!(reqs[0].1, [("Authorization".to_string(), "Bearer abc".to_string())]);
    }

    #[test]
    fn errors_and_empty_results() {
        let http = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| Response { status: 401, body: vec![] });
        let err = Sgdb::new(&http, "abc".into()).search("x").unwrap_err().to_string();
        assert!(err.contains("API key") && !err.contains("abc"), "{err}");
        let http = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| Response { status: 404, body: vec![] });
        assert!(Sgdb::new(&http, "k".into()).images(Kind::Hero, 1).unwrap().is_empty());
        let http = FakePost::new(|url: &str, _: &[(&str, &str)], _: &str| {
            assert!(url.contains("/logos/game/7?") && url.contains("nsfw=false"), "{url}");
            json(200, j!({"success": true, "data": [{"id": 1, "url": "https://cdn/x.png", "thumb": "https://cdn/t.png", "style": "official"}]}))
        });
        let logos = Sgdb::new(&http, "k".into()).images(Kind::Logo, 7).unwrap();
        assert_eq!((logos[0].id, logos[0].style.as_deref()), (1, Some("official")));
    }

    #[test]
    fn matches_titles_and_prefers_verified_entries() {
        let results = vec![
            Game { id: 1, name: "Jak II".into(), verified: false },
            Game { id: 2, name: "Jak 3".into(), verified: true },
            Game { id: 3, name: "Jak II".into(), verified: true },
        ];
        let pick = |t: &str| best_match(&results, &[t.to_string()]).map(|g| g.id);
        assert_eq!(pick("Jak II"), Some(3));
        assert_eq!(pick("Jak 3"), Some(2));
        assert_eq!(pick("Jak X"), None);
    }
}
