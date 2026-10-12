//! Turns downloaded artwork into what the UI displays: a small WebP for the
//! grid (never decode full-size art for thumbnails) and a larger WebP for the
//! details view. Files are written atomically.

use std::fs;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::DynamicImage;

use crate::error::{Error, Result};

/// Grid cards are ~230 px wide; 2x for high-DPI screens.
pub const THUMB_MAX: (u32, u32) = (460, 640);
/// Details view.
pub const FULL_MAX: (u32, u32) = (1200, 1200);
const THUMB_QUALITY: f32 = 80.0;
const FULL_QUALITY: f32 = 85.0;

pub struct Processed {
    pub thumb: Vec<u8>,
    pub full: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

fn webp(img: &DynamicImage, quality: f32) -> Result<Vec<u8>> {
    let rgba = img.to_rgba8();
    let enc = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
    Ok(enc.encode(quality).to_vec())
}

fn fit(img: &DynamicImage, (w, h): (u32, u32)) -> DynamicImage {
    if img.width() <= w && img.height() <= h {
        img.clone()
    } else {
        img.resize(w, h, FilterType::Triangle)
    }
}

pub fn process(bytes: &[u8]) -> Result<Processed> {
    let img = image::load_from_memory(bytes).map_err(|e| Error::msg(format!("bad image: {e}")))?;
    Ok(Processed {
        thumb: webp(&fit(&img, THUMB_MAX), THUMB_QUALITY)?,
        full: webp(&fit(&img, FULL_MAX), FULL_QUALITY)?,
        width: img.width(),
        height: img.height(),
    })
}

pub fn thumb_path(data_dir: &Path, game_id: i64) -> PathBuf {
    data_dir.join("art").join("t").join(format!("{game_id}.webp"))
}

pub fn full_path(data_dir: &Path, game_id: i64) -> PathBuf {
    data_dir.join("art").join("f").join(format!("{game_id}.webp"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn store(data_dir: &Path, game_id: i64, p: &Processed) -> Result<()> {
    write_atomic(&full_path(data_dir, game_id), &p.full)?;
    write_atomic(&thumb_path(data_dir, game_id), &p.thumb)?;
    Ok(())
}

#[cfg(test)]
pub fn test_png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dims(webp: &[u8]) -> (u32, u32) {
        let d = webp::Decoder::new(webp).decode().unwrap();
        (d.width(), d.height())
    }

    #[test]
    fn large_cover_is_downscaled_for_the_grid_keeping_aspect() {
        let p = process(&test_png(1400, 2000)).unwrap();
        assert_eq!(&p.thumb[..4], b"RIFF");
        assert_eq!(&p.thumb[8..12], b"WEBP");
        let (w, h) = dims(&p.thumb);
        assert!(w <= THUMB_MAX.0 && h <= THUMB_MAX.1, "{w}x{h}");
        assert_eq!(h, THUMB_MAX.1, "portrait art fills the height");
        assert!((w as f32 / h as f32 - 0.7).abs() < 0.01);
        let (fw, fh) = dims(&p.full);
        assert_eq!((fw, fh), (840, 1200));
        assert_eq!((p.width, p.height), (1400, 2000));
        assert!(p.thumb.len() < p.full.len());
    }

    #[test]
    fn small_art_is_not_upscaled() {
        let p = process(&test_png(200, 140)).unwrap();
        assert_eq!(dims(&p.thumb), (200, 140));
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(process(b"<html>404</html>").is_err());
    }

    #[test]
    fn store_writes_both_files() {
        let t = tempfile::tempdir().unwrap();
        let p = process(&test_png(300, 400)).unwrap();
        store(t.path(), 42, &p).unwrap();
        assert!(thumb_path(t.path(), 42).is_file());
        assert!(full_path(t.path(), 42).is_file());
    }
}
