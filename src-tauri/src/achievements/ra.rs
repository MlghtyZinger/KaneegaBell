//! RetroAchievements Web API client (https://api-docs.retroachievements.org).
//!
//! - Auth: the user's username plus their personal Web API key (from their
//!   RA control panel), sent as the `y` query parameter on every request.
//!   The key is as sensitive as a password: it's kept in the OS credential
//!   store and request URLs never reach errors or logs.
//! - Limits: RA rate-limits the API but publishes no numbers. This client is
//!   sequential, spaces requests >= 500 ms apart, backs off once on 429, and
//!   the expensive calls (per-console hash lists) are cached for a week.
//! - Terms: personal, non-commercial use of the user's own key; no scraping.
//! - Games are identified by RA's own hash of the game files (see `hash`),
//!   looked up in the console's hash list (`API_GetGameList` with `h=1`).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};

use crate::art::http::Get;
use crate::error::{Error, Result};

pub const API: &str = "https://retroachievements.org/API/";
pub const MEDIA: &str = "https://media.retroachievements.org";
const MIN_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub struct Credentials {
    pub user: String,
    pub key: String,
}

pub struct Ra<'a> {
    http: &'a dyn Get,
    creds: Credentials,
    last: Mutex<Option<Instant>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Console {
    #[serde(rename = "ID")]
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub active: bool,
}

/// One entry of a console's game list (`h=1`: with its hashes).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListedGame {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(default)]
    pub hashes: Vec<String>,
}

/// hash (lowercase hex) -> RA game id.
pub fn hash_index(list_json: &[u8]) -> Result<HashMap<String, i64>> {
    let games: Vec<ListedGame> = serde_json::from_slice(list_json)?;
    let mut out = HashMap::new();
    for g in games {
        for h in g.hashes {
            out.insert(h.to_ascii_lowercase(), g.id);
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Profile {
    pub user: String,
}

impl<'a> Ra<'a> {
    pub fn new(http: &'a dyn Get, creds: Credentials) -> Self {
        Self { http, creds, last: Mutex::new(None) }
    }

    pub fn user(&self) -> &str {
        &self.creds.user
    }

    fn url(&self, endpoint: &str, params: &[(&str, &str)]) -> String {
        let mut url = format!("{API}{endpoint}?y={}", utf8_percent_encode(&self.creds.key, NON_ALPHANUMERIC));
        for (k, v) in params {
            url.push_str(&format!("&{k}={}", utf8_percent_encode(v, NON_ALPHANUMERIC)));
        }
        url
    }

    fn wait_turn(&self) {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = *last {
            if let Some(rest) = MIN_INTERVAL.checked_sub(t.elapsed()) {
                std::thread::sleep(rest);
            }
        }
        *last = Some(Instant::now());
    }

    /// One API call. `Ok(None)` for 404 (unknown user or game).
    fn call(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Option<Vec<u8>>> {
        let url = self.url(endpoint, params);
        for attempt in 0..2 {
            self.wait_turn();
            let resp = self.http.get_raw(&url)?;
            match resp.status {
                200..=299 => return Ok(Some(resp.body)),
                404 => return Ok(None),
                401 | 403 => {
                    return Err(Error::msg(
                        "RetroAchievements didn't accept the username and Web API key (check them in Settings)",
                    ))
                }
                429 if attempt == 0 => std::thread::sleep(Duration::from_secs(5)),
                429 => return Err(Error::msg("RetroAchievements is limiting requests; try again in a minute")),
                s => return Err(Error::msg(format!("RetroAchievements answered HTTP {s}"))),
            }
        }
        unreachable!()
    }

    fn call_required(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Vec<u8>> {
        self.call(endpoint, params)?.ok_or_else(|| Error::msg("RetroAchievements: not found"))
    }

    /// Consoles RA currently runs achievements for.
    pub fn active_consoles(&self) -> Result<Vec<u8>> {
        self.call_required("API_GetConsoleIDs.php", &[("a", "1"), ("g", "1")])
    }

    /// The console's games that have achievements, with their hashes (raw
    /// JSON, for caching; see `hash_index`).
    pub fn hash_list(&self, console: u32) -> Result<Vec<u8>> {
        self.call_required("API_GetGameList.php", &[("i", &console.to_string()), ("h", "1"), ("f", "1")])
    }

    /// Checks the credentials: `None` if the user doesn't exist.
    pub fn profile(&self) -> Result<Option<Profile>> {
        let Some(body) = self.call("API_GetUserProfile.php", &[("u", &self.creds.user)])? else { return Ok(None) };
        // Unknown users come back as an empty object or array on some versions.
        Ok(serde_json::from_slice::<Profile>(&body).ok())
    }

    /// A game's achievements with the user's unlocks (raw JSON, for caching;
    /// see `parse_progress`).
    pub fn game_progress(&self, game_id: i64) -> Result<Vec<u8>> {
        self.call_required(
            "API_GetGameInfoAndUserProgress.php",
            &[("g", &game_id.to_string()), ("u", &self.creds.user)],
        )
    }
}

pub fn parse_consoles(body: &[u8]) -> Result<Vec<Console>> {
    Ok(serde_json::from_slice(body)?)
}

// ---------------------------------------------------------------- progress

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawProgress {
    title: String,
    #[serde(default)]
    image_icon: Option<String>,
    #[serde(default)]
    num_distinct_players: Option<serde_json::Value>,
    /// An object keyed by achievement id, or `[]` when there are none.
    #[serde(default)]
    achievements: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawAchievement {
    #[serde(rename = "ID")]
    id: i64,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    points: i64,
    #[serde(default)]
    badge_name: String,
    #[serde(default)]
    display_order: i64,
    #[serde(default)]
    num_awarded: Option<serde_json::Value>,
    date_earned: Option<String>,
    date_earned_hardcore: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub points: i64,
    /// Badge image name (served by the thumb protocol as `/ra/{badge}`).
    pub badge: String,
    /// When the user unlocked it ("YYYY-MM-DD HH:MM:SS", RA's time), if they did.
    pub earned: Option<String>,
    pub hardcore: bool,
    /// "progression" | "win_condition" | "missable" | None.
    pub kind: Option<String>,
    /// Share of the game's players who have it (0..1).
    pub rarity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub ra_game_id: i64,
    pub title: String,
    /// Game icon image number (served as `/rai/{n}`).
    pub icon: Option<String>,
    pub total: usize,
    pub earned: usize,
    pub earned_hardcore: usize,
    pub points_total: i64,
    pub points_earned: i64,
    pub achievements: Vec<Achievement>,
}

/// RA sends some numbers as strings on older endpoints.
fn number(v: &Option<serde_json::Value>) -> Option<f64> {
    match v.as_ref()? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

pub fn parse_progress(ra_game_id: i64, body: &[u8]) -> Result<Progress> {
    let raw: RawProgress = serde_json::from_slice(body)?;
    let players = number(&raw.num_distinct_players).filter(|p| *p > 0.0);
    let mut list: Vec<(i64, Achievement)> = Vec::new();
    if let serde_json::Value::Object(map) = raw.achievements {
        for v in map.into_values() {
            let a: RawAchievement = serde_json::from_value(v)?;
            let earned_hc = a.date_earned_hardcore.filter(|d| !d.is_empty());
            let earned = a.date_earned.filter(|d| !d.is_empty()).or_else(|| earned_hc.clone());
            list.push((
                a.display_order,
                Achievement {
                    id: a.id,
                    title: a.title,
                    description: a.description,
                    points: a.points,
                    badge: a.badge_name,
                    hardcore: earned_hc.is_some(),
                    earned,
                    kind: a.kind.filter(|k| !k.is_empty()),
                    rarity: players.and_then(|p| number(&a.num_awarded).map(|n| (n / p).clamp(0.0, 1.0))),
                },
            ));
        }
    }
    list.sort_by_key(|(order, a)| (*order, a.id));
    let achievements: Vec<Achievement> = list.into_iter().map(|(_, a)| a).collect();
    let icon = raw
        .image_icon
        .as_deref()
        .and_then(|p| p.rsplit('/').next())
        .map(|f| f.trim_end_matches(".png").to_string())
        .filter(|n| is_media_name(n));
    Ok(Progress {
        ra_game_id,
        title: raw.title,
        icon,
        total: achievements.len(),
        earned: achievements.iter().filter(|a| a.earned.is_some()).count(),
        earned_hardcore: achievements.iter().filter(|a| a.hardcore).count(),
        points_total: achievements.iter().map(|a| a.points).sum(),
        points_earned: achievements.iter().filter(|a| a.earned.is_some()).map(|a| a.points).sum(),
        achievements,
    })
}

/// Badge and icon names are digits (badges may end in `_lock`); anything
/// else is refused before it gets near a URL or a file path.
pub fn is_media_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 40 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::http::fake::{json, FakePost};
    use crate::art::http::Response;
    use serde_json::json as j;

    fn creds() -> Credentials {
        Credentials { user: "Some User".into(), key: "k&y".into() }
    }

    #[test]
    fn requests_carry_the_key_and_encoded_params() {
        let http = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| json(200, j!({"User": "Some User"})));
        let ra = Ra::new(&http, creds());
        assert_eq!(ra.profile().unwrap().unwrap().user, "Some User");
        let reqs = http.requests.lock().unwrap();
        assert_eq!(reqs[0].0, "https://retroachievements.org/API/API_GetUserProfile.php?y=k%26y&u=Some%20User");
    }

    #[test]
    fn bad_keys_and_unknown_users_are_told_apart() {
        let http = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| Response { status: 401, body: vec![] });
        let err = Ra::new(&http, creds()).profile().unwrap_err().to_string();
        assert!(err.contains("Web API key"), "{err}");
        assert!(!err.contains("k&y") && !err.contains("k%26y"), "the key never appears in errors");
        let http = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| Response { status: 404, body: vec![] });
        assert!(Ra::new(&http, creds()).profile().unwrap().is_none());
        let http = FakePost::new(|_: &str, _: &[(&str, &str)], _: &str| json(200, j!([])));
        assert!(Ra::new(&http, creds()).profile().unwrap().is_none());
    }

    #[test]
    fn hash_lists_index_every_hash_of_every_game() {
        let body = serde_json::to_vec(&j!([
            {"Title": "A", "ID": 10, "ConsoleID": 5, "NumAchievements": 20, "Hashes": ["ABC123", "def456"]},
            {"Title": "B", "ID": 11, "ConsoleID": 5, "NumAchievements": 3, "Hashes": []}
        ]))
        .unwrap();
        let idx = hash_index(&body).unwrap();
        assert_eq!(idx.get("abc123"), Some(&10));
        assert_eq!(idx.get("def456"), Some(&10));
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn progress_is_ordered_counted_and_tolerates_empty_lists() {
        let body = serde_json::to_vec(&j!({
            "Title": "Game", "ImageIcon": "/Images/012345.png", "NumDistinctPlayers": "200",
            "Achievements": {
                "2": {"ID": 2, "Title": "Second", "Description": "d2", "Points": 10, "BadgeName": "222",
                      "DisplayOrder": 2, "NumAwarded": 50, "DateEarned": "2024-05-01 10:00:00",
                      "DateEarnedHardcore": "2024-05-01 10:00:00", "type": "progression"},
                "1": {"ID": 1, "Title": "First", "Description": "d1", "Points": 5, "BadgeName": "111",
                      "DisplayOrder": 1, "NumAwarded": 150, "type": ""},
                "3": {"ID": 3, "Title": "Third", "Description": "d3", "Points": 25, "BadgeName": "333",
                      "DisplayOrder": 2, "NumAwarded": 10, "DateEarned": "2024-06-01 09:00:00"}
            }
        }))
        .unwrap();
        let p = parse_progress(99, &body).unwrap();
        assert_eq!(p.achievements.iter().map(|a| a.id).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!((p.total, p.earned, p.earned_hardcore), (3, 2, 1));
        assert_eq!((p.points_total, p.points_earned), (40, 35));
        assert_eq!(p.icon.as_deref(), Some("012345"));
        assert_eq!(p.achievements[0].rarity, Some(0.75));
        assert_eq!(p.achievements[0].kind, None);
        assert_eq!(p.achievements[1].kind.as_deref(), Some("progression"));

        let empty = serde_json::to_vec(&j!({"Title": "No cheevos", "Achievements": []})).unwrap();
        let p = parse_progress(5, &empty).unwrap();
        assert_eq!((p.total, p.earned), (0, 0));
    }

    #[test]
    fn media_names_are_plain() {
        assert!(is_media_name("123456"));
        assert!(is_media_name("123456_lock"));
        assert!(!is_media_name("../x"));
        assert!(!is_media_name("a/b"));
        assert!(!is_media_name(""));
    }
}
