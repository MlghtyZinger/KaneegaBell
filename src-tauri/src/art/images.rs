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

/// TV Mode extras: a hero (wide background) or a logo (transparent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extra {
    Hero,
    Logo,
}

impl Extra {
    pub fn kind(self) -> &'static str {
        match self {
            Extra::Hero => "hero",
            Extra::Logo => "logo",
        }
    }
    fn dir(self) -> &'static str {
        match self {
            Extra::Hero => "h",
            Extra::Logo => "l",
        }
    }
}

/// Heroes fill a 1080p screen's width; logos sit over them.
const HERO_MAX: (u32, u32) = (1920, 1080);
const LOGO_MAX: (u32, u32) = (800, 400);

pub fn extra_path(data_dir: &Path, extra: Extra, game_id: i64) -> PathBuf {
    data_dir.join("art").join(extra.dir()).join(format!("{game_id}.webp"))
}

/// Resizes and stores a hero or logo (logos keep their transparency).
/// Returns the source image's size.
pub fn store_extra(data_dir: &Path, extra: Extra, game_id: i64, bytes: &[u8]) -> Result<(u32, u32)> {
    let img = image::load_from_memory(bytes).map_err(|e| Error::msg(format!("bad image: {e}")))?;
    let (max, quality) = match extra {
        Extra::Hero => (HERO_MAX, FULL_QUALITY),
        Extra::Logo => (LOGO_MAX, 90.0),
    };
    write_atomic(&extra_path(data_dir, extra, game_id), &webp(&fit(&img, max), quality)?)?;
    Ok((img.width(), img.height()))
}

pub fn remove_extra(data_dir: &Path, extra: Extra, game_id: i64) {
    let _ = fs::remove_file(extra_path(data_dir, extra, game_id));
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
    fn heroes_are_capped_and_logos_keep_transparency() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(store_extra(t.path(), Extra::Hero, 3, &test_png(3840, 1240)).unwrap(), (3840, 1240));
        assert_eq!(dims(&fs::read(extra_path(t.path(), Extra::Hero, 3)).unwrap()), (1920, 620));
        let logo = image::RgbaImage::from_fn(1000, 300, |x, _| image::Rgba([255, 255, 255, if x < 500 { 0 } else { 255 }]));
        let mut png = std::io::Cursor::new(Vec::new());
        logo.write_to(&mut png, image::ImageFormat::Png).unwrap();
        store_extra(t.path(), Extra::Logo, 3, &png.into_inner()).unwrap();
        let d = webp::Decoder::new(&fs::read(extra_path(t.path(), Extra::Logo, 3)).unwrap()).decode().unwrap();
        assert_eq!((d.width(), d.height()), (800, 240));
        assert!(d.is_alpha(), "logos keep their alpha channel");
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
