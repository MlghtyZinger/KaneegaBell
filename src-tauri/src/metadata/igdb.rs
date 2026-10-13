//! IGDB API client (https://api-docs.igdb.com).
//!
//! - Auth: Twitch client-credentials. POST id.twitch.tv/oauth2/token with the
//!   user's Client ID + Secret returns an app token (~60 days); every API
//!   request sends `Client-ID` and `Authorization: Bearer <token>`.
//! - Limits: 4 requests/second and at most 8 open requests; going over gives
//!   429. This client is sequential and spaces requests >= 260 ms apart.
//!   (Searches can't be batched: the multiquery endpoint silently returns
//!   empty results for `search` queries.)
//! - Terms: free for non-commercial use under the Twitch Developer Services
//!   Agreement. Each user brings their own key; nothing is shared.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::art::http::Post;
use crate::error::{Error, Result};

pub const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
pub const API: &str = "https://api.igdb.com/v4";
/// 4 requests/second, with a little slack.
const MIN_INTERVAL: Duration = Duration::from_millis(260);
/// Renew tokens this long before IGDB says they expire.
const TOKEN_MARGIN_SECS: i64 = 24 * 3600;

/// Everything stored per game. `version_parent` is null for the main game
/// (editions like "Collector's Edition" point at their parent).
pub const FIELDS: &str = "name,summary,first_release_date,genres.name,platforms,cover.image_id,\
alternative_names.name,involved_companies.company.name,involved_companies.developer,\
involved_companies.publisher,version_parent,game_type.type";

#[derive(Debug, Clone)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    /// Unix seconds.
    pub expires_at: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Named {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Involved {
    pub company: Option<Named>,
    #[serde(default)]
    pub developer: bool,
    #[serde(default)]
    pub publisher: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Cover {
    pub image_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Game {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    pub first_release_date: Option<i64>,
    #[serde(default)]
    pub genres: Vec<Named>,
    #[serde(default)]
    pub platforms: Vec<i64>,
    pub cover: Option<Cover>,
    #[serde(default)]
    pub alternative_names: Vec<Named>,
    #[serde(default)]
    pub involved_companies: Vec<Involved>,
    pub version_parent: Option<i64>,
    /// "Main Game", "Mod", "Remake", "Port"... (IGDB's game_types table).
    pub game_type: Option<GameType>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct GameType {
    #[serde(rename = "type")]
    pub name: String,
}

impl Game {
    /// Fan-made modifications (ROM hacks, fan remakes) listed on IGDB.
    pub fn is_mod(&self) -> bool {
        self.game_type.as_ref().is_some_and(|t| t.name.eq_ignore_ascii_case("mod"))
    }

    fn companies(&self, pick: impl Fn(&Involved) -> bool) -> Option<String> {
        let names: Vec<&str> =
            self.involved_companies.iter().filter(|c| pick(c)).filter_map(|c| c.company.as_ref()).map(|c| c.name.as_str()).collect();
        (!names.is_empty()).then(|| names.join(", "))
    }
    pub fn developer(&self) -> Option<String> {
        self.companies(|c| c.developer)
    }
    pub fn publisher(&self) -> Option<String> {
        self.companies(|c| c.publisher)
    }
    /// Cover image URL for previews (`t_cover_big` is 264x374; `_2x` doubles it).
    pub fn cover_url(&self) -> Option<String> {
        self.cover.as_ref().map(|c| cover_image(&c.image_id, "cover_big_2x"))
    }
}

/// An IGDB image by id and size name (`t_1080p` fits within 1920x1080).
pub fn cover_image(image_id: &str, size: &str) -> String {
    format!("https://images.igdb.com/igdb/image/upload/t_{size}/{image_id}.jpg")
}

/// The preview-size version of a stored full-size IGDB cover URL.
pub fn preview_of(url: &str) -> String {
    url.replacen("/t_1080p/", "/t_cover_big/", 1)
}

/// Apicalypse strings are double-quoted; drop characters that would end them.
pub fn quote(s: &str) -> String {
    let clean: String = s.chars().map(|c| if c == '"' || c == '\\' || c.is_control() { ' ' } else { c }).collect();
    format!("\"{}\"", clean.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// A /games search, optionally limited to one platform.
pub fn search_query(title: &str, platform: Option<i64>, limit: u32) -> String {
    let filter = platform.map(|p| format!(" where platforms = ({p});")).unwrap_or_default();
    format!("search {}; fields {FIELDS};{filter} limit {limit};", quote(title))
}

/// A /games lookup by ids.
pub fn ids_query(ids: &[i64]) -> String {
    let list = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    format!("fields {FIELDS}; where id = ({list}); limit {};", ids.len().max(1))
}

pub struct Igdb<'a> {
    http: &'a dyn Post,
    creds: Credentials,
    token: Mutex<Option<Token>>,
    last_request: Mutex<Option<Instant>>,
}

impl<'a> Igdb<'a> {
    pub fn new(http: &'a dyn Post, creds: Credentials, token: Option<Token>) -> Self {
        Self { http, creds, token: Mutex::new(token), last_request: Mutex::new(None) }
    }

    /// The current token, to save for next time (it lasts ~60 days).
    pub fn token(&self) -> Option<Token> {
        self.token.lock().unwrap().clone()
    }

    fn throttle(&self) {
        let mut last = self.last_request.lock().unwrap();
        if let Some(t) = *last {
            let since = t.elapsed();
            if since < MIN_INTERVAL {
                std::thread::sleep(MIN_INTERVAL - since);
            }
        }
        *last = Some(Instant::now());
    }

    /// Gets a fresh app token. Also how "Save" checks the user's keys.
    pub fn fetch_token(&self) -> Result<Token> {
        let enc = |s: &str| percent_encoding::utf8_percent_encode(s.trim(), percent_encoding::NON_ALPHANUMERIC).to_string();
        let url = format!(
            "{TOKEN_URL}?client_id={}&client_secret={}&grant_type=client_credentials",
            enc(&self.creds.client_id),
            enc(&self.creds.client_secret)
        );
        let resp = self.http.post(&url, &[], "")?;
        match resp.status {
            200 => {
                #[derive(Deserialize)]
                struct Raw {
                    access_token: String,
                    expires_in: i64,
                }
                let raw: Raw = serde_json::from_slice(&resp.body)?;
                let token = Token {
                    access_token: raw.access_token,
                    expires_at: crate::db::queries::now() + raw.expires_in - TOKEN_MARGIN_SECS.min(raw.expires_in / 2),
                };
                *self.token.lock().unwrap() = Some(token.clone());
                Ok(token)
            }
            400 | 401 | 403 => Err(Error::msg(
                "Twitch didn't accept that Client ID / Client Secret. Check both were copied in full \
                 (and that the app's Client Type is Confidential).",
            )),
            s => Err(Error::msg(format!("Twitch login failed (HTTP {s}); try again later"))),
        }
    }

    fn bearer(&self) -> Result<String> {
        let current = self.token.lock().unwrap().clone();
        match current {
            Some(t) if t.expires_at > crate::db::queries::now() => Ok(t.access_token),
            _ => Ok(self.fetch_token()?.access_token),
        }
    }

    /// POSTs an Apicalypse query to an endpoint ("games", "multiquery").
    pub fn query(&self, endpoint: &str, body: &str) -> Result<serde_json::Value> {
        let url = format!("{API}/{endpoint}");
        let mut renewed = false;
        let mut throttled = 0;
        loop {
            let bearer = format!("Bearer {}", self.bearer()?);
            self.throttle();
            let headers = [("Client-ID", self.creds.client_id.trim()), ("Authorization", bearer.as_str()), ("Accept", "application/json")];
            let resp = self.http.post(&url, &headers, body)?;
            match resp.status {
                200 => return Ok(serde_json::from_slice(&resp.body)?),
                // Token revoked or expired early: get a new one, once.
                401 if !renewed => {
                    renewed = true;
                    *self.token.lock().unwrap() = None;
                }
                // Over the rate limit (another app using the same key?): back off.
                429 if throttled < 3 => {
                    throttled += 1;
                    std::thread::sleep(Duration::from_secs(1 << throttled));
                }
                s => {
                    let text = String::from_utf8_lossy(&resp.body);
                    let snippet: String = text.chars().take(200).collect();
                    return Err(Error::msg(format!("IGDB answered HTTP {s}: {snippet}")));
                }
            }
        }
    }

    pub fn games(&self, query: &str) -> Result<Vec<Game>> {
        Ok(serde_json::from_value(self.query("games", query)?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::http::fake::{json, FakePost};
    use crate::art::http::Response;
    use serde_json::json as j;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn creds() -> Credentials {
        Credentials { client_id: "cid".into(), client_secret: "s&cret".into() }
    }

    #[test]
    fn queries_are_well_formed_and_quote_safely() {
        assert_eq!(
            search_query("Jak \"II\"", Some(8), 5),
            format!("search \"Jak II\"; fields {FIELDS}; where platforms = (8); limit 5;")
        );
        assert_eq!(ids_query(&[1, 2]), format!("fields {FIELDS}; where id = (1,2); limit 2;"));
        assert_eq!(quote("a\\b\nc"), "\"a b c\"");
    }

    #[test]
    fn token_then_query_with_headers() {
        let http = FakePost::new(|url: &str, _h: &[(&str, &str)], _b: &str| {
            if url.starts_with(TOKEN_URL) {
                json(200, j!({"access_token": "tok", "expires_in": 5_000_000, "token_type": "bearer"}))
            } else {
                json(200, j!([{"id": 1, "name": "Okami"}]))
            }
        });
        let igdb = Igdb::new(&http, creds(), None);
        let games = igdb.games("search \"okami\"; fields name;").unwrap();
        assert_eq!(games[0].name, "Okami");
        let reqs = http.requests.lock().unwrap();
        assert!(reqs[0].0.contains("client_secret=s%26cret"), "secret is URL-encoded: {}", reqs[0].0);
        assert!(reqs[1].1.contains(&("Client-ID".into(), "cid".into())));
        assert!(reqs[1].1.contains(&("Authorization".into(), "Bearer tok".into())));
        assert!(igdb.token().unwrap().expires_at > crate::db::queries::now());
    }

    #[test]
    fn expired_token_is_renewed_once_on_401() {
        let calls = AtomicUsize::new(0);
        let http = FakePost::new(|url: &str, h: &[(&str, &str)], _b: &str| {
            if url.starts_with(TOKEN_URL) {
                return json(200, j!({"access_token": "new", "expires_in": 5_000_000}));
            }
            calls.fetch_add(1, Ordering::Relaxed);
            if h.contains(&("Authorization", "Bearer old")) {
                Response { status: 401, body: b"{}".to_vec() }
            } else {
                json(200, j!([]))
            }
        });
        let stale = Token { access_token: "old".into(), expires_at: crate::db::queries::now() + 1000 };
        let igdb = Igdb::new(&http, creds(), Some(stale));
        assert!(igdb.games("fields name;").unwrap().is_empty());
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(igdb.token().unwrap().access_token, "new");
    }

    #[test]
    fn bad_keys_give_a_helpful_error() {
        let http = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| Response { status: 400, body: b"{}".to_vec() });
        let err = Igdb::new(&http, creds(), None).fetch_token().unwrap_err().to_string();
        assert!(err.contains("Client ID / Client Secret"), "{err}");
        assert!(!err.contains("s&cret") && !err.contains("s%26cret"));
    }

    /// Live probe with the keys saved in the app (network; prints raw responses, never the keys):
    /// `cargo test real_igdb_probe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_igdb_probe() {
        let (Some(client_id), Some(client_secret)) =
            (crate::secrets::get("igdb-client-id").unwrap(), crate::secrets::get("igdb-client-secret").unwrap())
        else {
            panic!("no IGDB keys saved in the app");
        };
        let http = crate::art::http::Http::new();
        let igdb = Igdb::new(&http, Credentials { client_id, client_secret }, None);
        if let Ok(list) = std::env::var("IGDB_PROBE") {
            // IGDB_PROBE="Killer 7|9;Black|8": title|platform pairs.
            for item in list.split(';') {
                let (title, platform) = item.split_once('|').unwrap_or((item, ""));
                let body = format!(
                    "search {}; fields name,alternative_names.name,platforms,game_type.type,version_parent,first_release_date;{} limit 10;",
                    quote(title),
                    platform.parse::<i64>().map(|p| format!(" where platforms = ({p});")).unwrap_or_default()
                );
                println!("== {title} [{platform}]");
                match igdb.query("games", &body) {
                    Ok(serde_json::Value::Array(games)) => {
                        for g in games {
                            let alts: Vec<_> = g["alternative_names"].as_array().map(|a| a.iter().filter_map(|n| n["name"].as_str()).collect()).unwrap_or_default();
                            println!("   {} | type {} | parent {} | alts {:?}", g["name"], g["game_type"]["type"], g["version_parent"], alts);
                        }
                    }
                    Ok(v) => println!("   {v}"),
                    Err(e) => println!("   ERROR {e}"),
                }
            }
            return;
        }
        for (endpoint, body) in [
            ("games", "search \"Dante's Inferno\"; fields name,platforms;".to_string()),
            ("games", search_query("Dante's Inferno", Some(9), 5)),
            ("games", search_query("Dante's Inferno", None, 5)),
            // Kept to notice if IGDB ever fixes search inside multiquery (it returns []).
            ("multiquery", format!("query games \"a\" {{ {} }};", search_query("Dante's Inferno", None, 3))),
        ] {
            println!("== {endpoint}: {body}");
            match igdb.query(endpoint, &body) {
                Ok(v) => println!("{}", serde_json::to_string(&v).unwrap().chars().take(1500).collect::<String>()),
                Err(e) => println!("ERROR {e}"),
            }
        }
    }

    #[test]
    fn game_fields_parse() {
        let okami: Game = serde_json::from_value(j!({"id": 7, "name": "Okami",
            "involved_companies": [{"company": {"name": "Clover Studio"}, "developer": true, "publisher": false},
                                   {"company": {"name": "Capcom"}, "developer": false, "publisher": true}],
            "genres": [{"id": 31, "name": "Adventure"}], "cover": {"id": 1, "image_id": "abc"}}))
        .unwrap();
        assert_eq!(okami.developer().as_deref(), Some("Clover Studio"));
        assert_eq!(okami.publisher().as_deref(), Some("Capcom"));
        assert_eq!(okami.cover_url().unwrap(), "https://images.igdb.com/igdb/image/upload/t_cover_big_2x/abc.jpg");
    }
}
