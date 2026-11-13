//! `thumb://` protocol: serves cached artwork to the webview straight from
//! disk (`/t/{game_id}` grid thumbnail, `/f/{game_id}` details image,
//! `/h/{game_id}` hero background, `/l/{game_id}` logo). URLs
//! carry `?v={art_version}`, so responses can be cached forever.

use std::path::{Path, PathBuf};

use super::images;

#[derive(Debug, PartialEq, Eq)]
pub enum Asset {
    Thumb(i64),
    Full(i64),
    Hero(i64),
    Logo(i64),
}

/// Parses the request path ("/t/12", "/f/12"; query string ignored).
pub fn parse(path: &str) -> Option<Asset> {
    let path = path.split('?').next()?.trim_start_matches('/');
    let (kind, id) = path.split_once('/')?;
    let id: i64 = id.trim_end_matches(".webp").parse().ok()?;
    match kind {
        "t" => Some(Asset::Thumb(id)),
        "f" => Some(Asset::Full(id)),
        "h" => Some(Asset::Hero(id)),
        "l" => Some(Asset::Logo(id)),
        _ => None,
    }
}

pub fn file_for(data_dir: &Path, asset: &Asset) -> PathBuf {
    match asset {
        Asset::Thumb(id) => images::thumb_path(data_dir, *id),
        Asset::Full(id) => images::full_path(data_dir, *id),
        Asset::Hero(id) => images::extra_path(data_dir, images::Extra::Hero, *id),
        Asset::Logo(id) => images::extra_path(data_dir, images::Extra::Logo, *id),
    }
}

/// RetroAchievements badge/icon requests (`/ra/{name}`, `/rai/{name}`).
fn ra_media(path: &str) -> Option<(&str, &str)> {
    let path = path.split('?').next()?.trim_start_matches('/');
    let (kind, name) = path.split_once('/')?;
    matches!(kind, "ra" | "rai").then_some((kind, name))
}

pub fn respond(data_dir: &Path, path: &str) -> tauri::http::Response<Vec<u8>> {
    let builder = tauri::http::Response::builder();
    let (found, mime) = match ra_media(path) {
        Some((kind, name)) => {
            let bytes = crate::achievements::media(data_dir, &super::http::Http::new(), kind, name);
            (bytes, "image/png")
        }
        None => (parse(path).and_then(|a| std::fs::read(file_for(data_dir, &a)).ok()), "image/webp"),
    };
    match found {
        Some(bytes) => builder
            .status(200)
            .header("Content-Type", mime)
            .header("Cache-Control", "public, max-age=31536000, immutable")
            .header("Access-Control-Allow-Origin", "*")
            .body(bytes),
        None => builder.status(404).body(Vec::new()),
    }
    .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_asset_paths() {
        assert_eq!(parse("/t/12"), Some(Asset::Thumb(12)));
        assert_eq!(parse("/f/7?v=3"), Some(Asset::Full(7)));
        assert_eq!(parse("t/9.webp"), Some(Asset::Thumb(9)));
        assert_eq!(parse("/h/4?v=2"), Some(Asset::Hero(4)));
        assert_eq!(parse("/l/4"), Some(Asset::Logo(4)));
        assert_eq!(parse("/x/1"), None);
        assert_eq!(parse("/t/../../secret"), None);
        assert_eq!(parse("/t/"), None);
    }

    #[test]
    fn serves_files_and_404s() {
        let t = tempfile::tempdir().unwrap();
        let p = images::process(&images::test_png(100, 140)).unwrap();
        images::store(t.path(), 5, &p).unwrap();
        let ok = respond(t.path(), "/t/5");
        assert_eq!(ok.status(), 200);
        assert_eq!(ok.headers()["Content-Type"], "image/webp");
        assert_eq!(respond(t.path(), "/t/6").status(), 404);
    }

    #[test]
    fn serves_cached_achievement_badges() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("ra/badges")).unwrap();
        std::fs::write(t.path().join("ra/badges/12345.png"), b"png").unwrap();
        let ok = respond(t.path(), "/ra/12345");
        assert_eq!((ok.status().as_u16(), ok.headers()["Content-Type"].to_str().unwrap()), (200, "image/png"));
        assert_eq!(ok.body(), b"png");
        assert_eq!(ra_media("/rai/9?x"), Some(("rai", "9")));
        assert_eq!(ra_media("/t/9"), None);
        // Bad names never reach the network or the disk.
        assert_eq!(respond(t.path(), "/ra/..%2Fsecret").status(), 404);
    }
}
