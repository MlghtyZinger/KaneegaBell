//! `thumb://` protocol: serves cached artwork to the webview straight from
//! disk (`/t/{game_id}` grid thumbnail, `/f/{game_id}` details image). URLs
//! carry `?v={art_version}`, so responses can be cached forever.

use std::path::{Path, PathBuf};

use super::images;

#[derive(Debug, PartialEq, Eq)]
pub enum Asset {
    Thumb(i64),
    Full(i64),
}

/// Parses the request path ("/t/12", "/f/12"; query string ignored).
pub fn parse(path: &str) -> Option<Asset> {
    let path = path.split('?').next()?.trim_start_matches('/');
    let (kind, id) = path.split_once('/')?;
    let id: i64 = id.trim_end_matches(".webp").parse().ok()?;
    match kind {
        "t" => Some(Asset::Thumb(id)),
        "f" => Some(Asset::Full(id)),
        _ => None,
    }
}

pub fn file_for(data_dir: &Path, asset: &Asset) -> PathBuf {
    match asset {
        Asset::Thumb(id) => images::thumb_path(data_dir, *id),
        Asset::Full(id) => images::full_path(data_dir, *id),
    }
}

pub fn respond(data_dir: &Path, path: &str) -> tauri::http::Response<Vec<u8>> {
    let builder = tauri::http::Response::builder();
    let found = parse(path).and_then(|a| std::fs::read(file_for(data_dir, &a)).ok());
    match found {
        Some(bytes) => builder
            .status(200)
            .header("Content-Type", "image/webp")
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
}
