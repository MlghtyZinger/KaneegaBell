//! GameTDB (art.gametdb.com): keyless covers looked up by the game's own ID
//! (PS3 title ID, GameCube/Wii disc ID, DS/3DS game code), the same source
//! Dolphin and the Wii USB loaders use. It fills the gaps libretro leaves,
//! above all PS3, where libretro has very few covers.
//!
//! Images live at `https://art.gametdb.com/{system}/{kind}/{region}/{id}.{ext}`
//! and a missing one is a plain 404, so each game costs a handful of small
//! requests at most, once (results are cached like every other cover).

/// One URL to try, best first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverUrl {
    pub url: String,
    pub region: &'static str,
}

/// Image kinds per system, largest first. Wii/GC only have small front
/// covers (the bigger ones are full wraps or 3D renders).
fn system(platform: &str) -> Option<(&'static str, &'static [(&'static str, &'static str)])> {
    Some(match platform {
        "ps3" => ("ps3", &[("coverHQ", "jpg"), ("coverM", "jpg"), ("cover", "jpg")]),
        "wii" | "gc" => ("wii", &[("cover", "png")]),
        "nds" => ("ds", &[("coverHQ", "jpg"), ("coverM", "jpg")]),
        "3ds" => ("3ds", &[("coverHQ", "jpg"), ("coverM", "jpg")]),
        _ => return None,
    })
}

/// The GameTDB ID for a stored serial ("CTR-P-AREE" -> "AREE").
fn game_id(platform: &str, serial: &str) -> Option<String> {
    let s = serial.trim().to_ascii_uppercase();
    let id = match platform {
        "3ds" => s.rsplit('-').next()?.to_string(),
        _ => s,
    };
    let ok_len = match platform {
        "ps3" => id.len() == 9,
        "wii" | "gc" => id.len() == 6,
        "nds" | "3ds" => id.len() == 4,
        _ => false,
    };
    (ok_len && id.chars().all(|c| c.is_ascii_alphanumeric())).then_some(id)
}

/// GameTDB region folder from the ID's region letter, then the common fallbacks.
fn regions(platform: &str, id: &str) -> Vec<&'static str> {
    let b = id.as_bytes();
    let letter = match platform {
        // BLUS30443 / NPEB00001: third letter. U = Americas, E = Europe, J = Japan, A/H = Asia, K = Korea.
        "ps3" => b[2],
        // RMGE01 / YQUE / AREE: fourth letter.
        _ => b[3],
    };
    let primary = match (platform, letter) {
        ("ps3", b'U') => "US",
        ("ps3", b'E') => "EN",
        ("ps3", b'J') => "JA",
        ("ps3", b'K') => "KO",
        ("ps3", _) => "US",
        (_, b'E' | b'N') => "US",
        (_, b'J') => "JA",
        (_, b'K') => "KO",
        (_, b'D') => "DE",
        (_, b'F') => "FR",
        (_, b'S') => "ES",
        (_, b'I') => "IT",
        _ => "EN", // P and other PAL letters
    };
    let mut out = vec![primary];
    for r in ["US", "EN", "JA"] {
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

/// URLs to try for a game, best first; empty when the platform or ID doesn't fit.
pub fn cover_urls(platform: &str, serial: Option<&str>) -> Vec<CoverUrl> {
    let (Some((sys, kinds)), Some(id)) = (system(platform), serial.and_then(|s| game_id(platform, s))) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for region in regions(platform, &id) {
        for (kind, ext) in kinds {
            out.push(CoverUrl { url: format!("https://art.gametdb.com/{sys}/{kind}/{region}/{id}.{ext}"), region });
        }
    }
    out
}

/// The medium-size version of a cover URL, for picker previews.
pub fn preview_url(url: &str) -> String {
    url.replacen("/coverHQ/", "/coverM/", 1)
}

/// The largest version of a (preview) cover URL.
pub fn best_url(url: &str) -> String {
    url.replacen("/coverM/", "/coverHQ/", 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_follow_the_id_region() {
        let u = cover_urls("ps3", Some("BLUS30516"));
        assert_eq!(u[0].url, "https://art.gametdb.com/ps3/coverHQ/US/BLUS30516.jpg");
        assert_eq!(u[0].region, "US");
        assert_eq!(cover_urls("ps3", Some("BLES01315"))[0].region, "EN");
        assert_eq!(cover_urls("wii", Some("RMGE01"))[0].url, "https://art.gametdb.com/wii/cover/US/RMGE01.png");
        assert_eq!(cover_urls("gc", Some("GFEP01"))[0].region, "EN");
        assert_eq!(cover_urls("3ds", Some("CTR-P-AREE"))[0].url, "https://art.gametdb.com/3ds/coverHQ/US/AREE.jpg");
        assert_eq!(cover_urls("nds", Some("VI2J"))[0].region, "JA");
        // Every region is tried once.
        let regions: Vec<_> = cover_urls("nds", Some("YKWP")).iter().map(|u| u.region).collect();
        assert_eq!(regions, ["EN", "EN", "US", "US", "JA", "JA"]);
    }

    #[test]
    fn unusable_ids_give_nothing() {
        assert!(cover_urls("ps3", None).is_empty());
        assert!(cover_urls("ps3", Some("BLUS")).is_empty());
        assert!(cover_urls("ps2", Some("SLUS-20946")).is_empty());
        assert!(cover_urls("wii", Some("../../x")).is_empty());
    }
}
