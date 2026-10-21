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

// ------------------------------------------------------ full covers (back, spine, front in one picture)

/// Where the spine sits in a platform's full cover, as fractions of its width
/// (measured on GameTDB's pictures: back | spine | front).
fn spine_span(platform: &str) -> Option<(f32, f32)> {
    Some(match platform {
        "ps3" => (0.4725, 0.5225),
        "wii" | "gc" => (0.4727, 0.5273),
        "nds" | "3ds" => (0.479, 0.521),
        _ => return None,
    })
}

/// URLs of the platform's full cover for a game, best first (English-language regions before the game's own).
pub fn wrap_urls(platform: &str, serial: Option<&str>) -> Vec<String> {
    let (Some((sys, _)), Some(id)) = (system(platform), serial.and_then(|s| game_id(platform, s))) else { return Vec::new() };
    let kinds: &[(&str, &str)] = match platform {
        "ps3" | "nds" | "3ds" => &[("coverfullHQ", "jpg"), ("coverfull", "jpg")],
        _ => &[("coverfull", "png")],
    };
    let mut regions = vec!["US", "EN"];
    for r in self::regions(platform, &id) {
        if !regions.contains(&r) {
            regions.push(r);
        }
    }
    let mut out = Vec::new();
    for region in regions {
        for (kind, ext) in kinds {
            out.push(format!("https://art.gametdb.com/{sys}/{kind}/{region}/{id}.{ext}"));
        }
    }
    out
}

/// The back cover and the spine cut out of a full cover.
pub fn split_wrap(platform: &str, bytes: &[u8]) -> crate::error::Result<(image::DynamicImage, image::DynamicImage)> {
    let (s0, s1) = spine_span(platform).ok_or_else(|| crate::error::Error::msg("no full covers for this platform"))?;
    let img = image::load_from_memory(bytes).map_err(|e| crate::error::Error::msg(format!("bad image: {e}")))?;
    let (w, h) = (img.width() as f32, img.height());
    // A little inside each fold, so neighbouring panels don't bleed in.
    let back = img.crop_imm(0, 0, ((s0 - 0.004) * w) as u32, h);
    let spine = img.crop_imm(((s0 + 0.006) * w) as u32, 0, (((s1 - s0) - 0.012) * w) as u32, h);
    Ok((back, spine))
}

/// The first full cover GameTDB has for the game. `Ok(None)` means GameTDB answered and has none;
/// a network error stops the search and is returned, so it isn't mistaken for "none".
pub fn find_wrap(fetch: &dyn super::http::Fetch, platform: &str, serial: Option<&str>) -> crate::error::Result<Option<(String, Vec<u8>)>> {
    for url in wrap_urls(platform, serial) {
        if let Some(bytes) = fetch.get(&url)? {
            return Ok(Some((url, bytes)));
        }
    }
    Ok(None)
}

/// Fills a game's missing back cover and spine (LaunchBox's come first) from GameTDB's full cover; once per game.
pub fn fill_case_art(data_dir: &std::path::Path, fetch: &dyn super::http::Fetch, game_id: i64, platform: &str, serial: Option<&str>) {
    use super::images::{self, Case};
    let marker = images::tried_marker(data_dir, game_id);
    let missing = |c: Case| !images::case_path(data_dir, c, game_id).exists();
    if marker.exists() || !(missing(Case::Back) || missing(Case::Spine)) || spine_span(platform).is_none() {
        return;
    }
    // Offline or rate-limited: no marker, so the next pass asks again.
    let Ok(found) = find_wrap(fetch, platform, serial) else { return };
    if let Some((_, bytes)) = found {
        if let Ok((back, spine)) = split_wrap(platform, &bytes) {
            for (case, img) in [(Case::Back, back), (Case::Spine, spine)] {
                if missing(case) {
                    let _ = images::store_case_image(data_dir, case, game_id, &img);
                }
            }
        }
    }
    // Remembered whether or not GameTDB had one.
    let _ = std::fs::create_dir_all(marker.parent().unwrap());
    let _ = std::fs::write(marker, b"");
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
    fn a_full_cover_gives_a_back_and_a_spine_and_fills_only_what_is_missing() {
        use crate::art::http::fake::FakeHttp;
        use crate::art::images::{self, Case};
        // 1000 x 100: red back, green spine in the PS3 span, blue front.
        let img = image::RgbImage::from_fn(1000, 100, |x, _| match x {
            0..=470 => image::Rgb([255, 0, 0]),
            472..=522 => image::Rgb([0, 255, 0]),
            525.. => image::Rgb([0, 0, 255]),
            _ => image::Rgb([0, 0, 0]),
        });
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut png, image::ImageFormat::Jpeg).unwrap();
        let urls = wrap_urls("ps3", Some("BLUS30516"));
        assert_eq!(urls[0], "https://art.gametdb.com/ps3/coverfullHQ/US/BLUS30516.jpg");
        assert!(wrap_urls("snes", Some("x")).is_empty());
        let mut http = FakeHttp::default();
        http.pages.insert(urls[1].clone(), png.into_inner()); // only the smaller one exists

        let dir = tempfile::tempdir().unwrap();
        // The back was already there (LaunchBox's): it stays; the spine is cut out.
        std::fs::create_dir_all(dir.path().join("art").join("b")).unwrap();
        std::fs::write(images::case_path(dir.path(), Case::Back, 5), b"LB").unwrap();
        fill_case_art(dir.path(), &http, 5, "ps3", Some("BLUS30516"));
        assert_eq!(std::fs::read(images::case_path(dir.path(), Case::Back, 5)).unwrap(), b"LB");
        let bytes = std::fs::read(images::case_path(dir.path(), Case::Spine, 5)).unwrap();
        let spine = webp::Decoder::new(&bytes).decode().unwrap();
        assert!(spine.width() >= 35 && spine.width() <= 50, "{}", spine.width());
        let ch = if spine.is_alpha() { 4 } else { 3 };
        let at = ((50 * spine.width() + spine.width() / 2) as usize) * ch;
        let mid = &spine[at..at + 3];
        assert!(mid[1] > 200 && mid[0] < 60 && mid[2] < 60, "green spine, got {mid:?}");
        // Asked once: a second game with nothing on GameTDB leaves a marker and nothing else.
        fill_case_art(dir.path(), &http, 6, "ps3", Some("BLUS99999"));
        assert!(images::tried_marker(dir.path(), 6).exists() && !images::case_path(dir.path(), Case::Spine, 6).exists());
        let (back, _) = split_wrap("ps3", &http.pages[&urls[1]]).unwrap();
        let px = back.to_rgb8();
        assert!(px.get_pixel(10, 50)[0] > 200 && px.get_pixel(10, 50)[1] < 60, "red back");
    }

    #[test]
    fn a_network_error_leaves_no_marker_so_the_game_is_asked_again() {
        use crate::art::http::fake::FakeHttp;
        use crate::art::images;
        let dir = tempfile::tempdir().unwrap();
        let offline = FakeHttp { offline: true, ..Default::default() };
        fill_case_art(dir.path(), &offline, 7, "ps3", Some("BLUS30516"));
        assert!(!images::tried_marker(dir.path(), 7).exists());
        // Only the first URL was tried: an error stops the search.
        assert_eq!(offline.requests.lock().unwrap().len(), 1);
        fill_case_art(dir.path(), &FakeHttp::default(), 7, "ps3", Some("BLUS30516"));
        assert!(images::tried_marker(dir.path(), 7).exists());
    }

    #[test]
    fn unusable_ids_give_nothing() {
        assert!(cover_urls("ps3", None).is_empty());
        assert!(cover_urls("ps3", Some("BLUS")).is_empty());
        assert!(cover_urls("ps2", Some("SLUS-20946")).is_empty());
        assert!(cover_urls("wii", Some("../../x")).is_empty());
    }
}
