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
    /// Looks like a disc scan, not a box (see `looks_like_disc`).
    pub disc: bool,
}

/// A disc scan rather than box art: a square picture whose four corners are
/// see-through or one plain color, with the middle (the disc's hole) the same.
/// Box art has artwork in the middle. LaunchBox sometimes files a disc under
/// "Box - Front" (Budokai Tenkaichi 3's Spanish "box").
pub fn looks_like_disc(img: &DynamicImage) -> bool {
    let (w, h) = (img.width(), img.height());
    if w < 16 || h < 16 || !(0.9..=1.1).contains(&(w as f32 / h as f32)) {
        return false;
    }
    let rgba = img.to_rgba8();
    let px = |fx: f32, fy: f32| *rgba.get_pixel(((w - 1) as f32 * fx) as u32, ((h - 1) as f32 * fy) as u32);
    let clear = |p: image::Rgba<u8>| p[3] < 40;
    let near = |a: image::Rgba<u8>, b: image::Rgba<u8>| (0..3).all(|i| a[i].abs_diff(b[i]) <= 24) && a[3].abs_diff(b[3]) <= 40;
    let corners = [px(0.02, 0.02), px(0.98, 0.02), px(0.02, 0.98), px(0.98, 0.98)];
    let center = px(0.5, 0.5);
    if corners.iter().all(|&c| clear(c)) {
        return clear(center) || corners.iter().all(|&c| near(c, center));
    }
    corners.iter().all(|&c| near(c, corners[0])) && near(center, corners[0])
}

fn webp(img: &DynamicImage, quality: f32) -> Result<Vec<u8>> {
    let rgba = img.to_rgba8();
    let enc = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
    Ok(enc.encode(quality).to_vec())
}

/// Decodes PNG, JPEG, ICO (SteamGridDB icons come as .ico) or WebP bytes (the `image` crate here reads PNG and JPEG;
/// WebP goes through libwebp).
pub fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    match image::load_from_memory(bytes) {
        Ok(img) => Ok(img),
        Err(_) => {
            let bad = || Error::msg("That file isn't a PNG, JPEG, ICO or WebP image.");
            let d = webp::Decoder::new(bytes).decode().ok_or_else(bad)?;
            let (w, h, px) = (d.width(), d.height(), d.to_vec());
            Ok(if d.is_alpha() {
                DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, px).ok_or_else(bad)?)
            } else {
                DynamicImage::ImageRgb8(image::RgbImage::from_raw(w, h, px).ok_or_else(bad)?)
            })
        }
    }
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
        disc: looks_like_disc(&img),
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

/// Stores a user's logo image at `path` (PNG, JPEG or WebP; resized like the
/// game logos, transparency kept).
pub fn store_logo_at(path: &Path, bytes: &[u8]) -> Result<()> {
    let img = decode(bytes)?;
    write_atomic(path, &webp(&fit(&img, LOGO_MAX), 90.0)?)
}

pub fn remove_extra(data_dir: &Path, extra: Extra, game_id: i64) {
    let _ = fs::remove_file(extra_path(data_dir, extra, game_id));
}

// ---------------------------------------------------------------- game icons

/// The sidebar's tile is 56 px, Recent's 24: 192 leaves room for 200%
/// scaling and the later list view, without a big file per game.
pub const ICON: u32 = 192;
const ICON_QUALITY: f32 = 82.0;

pub fn icon_path(data_dir: &Path, game_id: i64) -> PathBuf {
    data_dir.join("art").join("i").join(format!("{game_id}.webp"))
}

/// A square from the middle of the picture, biased up. Box art keeps its
/// title at the top and the character in the middle, so a dead-centre square
/// cuts heads off; 45% of the spare height looks right on covers.
pub fn square_crop(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let left = (w - side) / 2;
    let top = ((h - side) as f32 * 0.45).round() as u32;
    img.crop_imm(left, top, side, side)
}

/// A picture kept whole inside a transparent square: a logo isn't cropped and
/// a wide image isn't stretched.
fn icon_square(img: &DynamicImage) -> DynamicImage {
    let scaled = if img.width() > ICON || img.height() > ICON {
        img.resize(ICON, ICON, FilterType::Lanczos3)
    } else {
        img.clone()
    };
    let mut canvas = image::RgbaImage::from_pixel(ICON, ICON, image::Rgba([0, 0, 0, 0]));
    let x = ((ICON - scaled.width()) / 2) as i64;
    let y = ((ICON - scaled.height()) / 2) as i64;
    image::imageops::overlay(&mut canvas, &scaled.to_rgba8(), x, y);
    DynamicImage::ImageRgba8(canvas)
}

/// Crops the game's cached cover thumbnail into its icon. False when the game
/// has no cover (nothing to crop).
pub fn store_icon_from_cover(data_dir: &Path, game_id: i64) -> Result<bool> {
    let Ok(bytes) = fs::read(thumb_path(data_dir, game_id)) else { return Ok(false) };
    let img = decode(&bytes)?;
    let icon = fit(&square_crop(&img), (ICON, ICON));
    write_atomic(&icon_path(data_dir, game_id), &webp(&icon, ICON_QUALITY)?)?;
    Ok(true)
}

/// Stores a user's icon image (PNG, JPEG or WebP). Returns the source size.
pub fn store_icon(data_dir: &Path, game_id: i64, bytes: &[u8]) -> Result<(u32, u32)> {
    let img = decode(bytes)?;
    write_atomic(&icon_path(data_dir, game_id), &webp(&icon_square(&img), ICON_QUALITY)?)?;
    Ok((img.width(), img.height()))
}

pub fn remove_icon(data_dir: &Path, game_id: i64) {
    let _ = fs::remove_file(icon_path(data_dir, game_id));
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

    /// A disc: a colored ring around a hole, on `bg` (see-through or a color).
    fn disc(bg: image::Rgba<u8>) -> DynamicImage {
        DynamicImage::ImageRgba8(image::RgbaImage::from_fn(300, 300, |x, y| {
            let d = ((x as f32 - 150.0).powi(2) + (y as f32 - 150.0).powi(2)).sqrt();
            if (25.0..145.0).contains(&d) {
                image::Rgba([(x % 200) as u8 + 30, (y % 200) as u8 + 20, 90, 255])
            } else {
                bg
            }
        }))
    }

    #[test]
    fn disc_scans_are_told_from_boxes() {
        assert!(looks_like_disc(&disc(image::Rgba([0, 0, 0, 0]))), "disc with see-through corners and hole");
        assert!(looks_like_disc(&disc(image::Rgba([255, 255, 255, 255]))), "disc scanned on white");
        let art = |w, h| DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])));
        assert!(!looks_like_disc(&art(300, 420)), "a tall box");
        assert!(!looks_like_disc(&art(300, 300)), "a square box (GBA) with artwork in the middle");
        // A square box with a white border: the middle is still artwork.
        let framed = DynamicImage::ImageRgb8(image::RgbImage::from_fn(300, 300, |x, y| {
            if x < 12 || y < 12 || x > 287 || y > 287 { image::Rgb([255, 255, 255]) } else { image::Rgb([200, (x % 256) as u8, 40]) }
        }));
        assert!(!looks_like_disc(&framed), "a framed square box");
    }

    /// Checks real pictures: `DISC_FILES="a.png;b.jpg" cargo test disc_check_files -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn disc_check_files() {
        for p in std::env::var("DISC_FILES").unwrap().split(';') {
            let img = if p.ends_with(".webp") {
                let bytes = std::fs::read(p).unwrap();
                let d = webp::Decoder::new(&bytes).decode().unwrap();
                let (w, h) = (d.width(), d.height());
                if d.is_alpha() {
                    DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, d.to_vec()).unwrap())
                } else {
                    DynamicImage::ImageRgb8(image::RgbImage::from_raw(w, h, d.to_vec()).unwrap())
                }
            } else {
                image::open(p).unwrap()
            };
            println!("{} {}x{} disc={}", p, img.width(), img.height(), looks_like_disc(&img));
        }
    }

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

    #[test]
    fn icon_crops_are_square_biased_up() {
        // 400 wide, 800 tall: a 400 px square starting at 45% of the 400 px
        // of spare height, not dead centre at 200.
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(400, 800, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 0])));
        let c = square_crop(&img);
        assert_eq!((c.width(), c.height()), (400, 400));
        assert_eq!(c.to_rgb8().get_pixel(0, 0), img.to_rgb8().get_pixel(0, 180));
        // A wide picture is cropped from its middle, not its left edge.
        let wide = DynamicImage::ImageRgb8(image::RgbImage::from_fn(800, 400, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 0])));
        assert_eq!((square_crop(&wide).width(), square_crop(&wide).height()), (400, 400));
        assert_eq!(square_crop(&wide).to_rgb8().get_pixel(0, 0), wide.to_rgb8().get_pixel(200, 0));
    }

    #[test]
    fn a_cover_becomes_a_192_icon_and_needs_a_cover() {
        let t = tempfile::tempdir().unwrap();
        assert!(!store_icon_from_cover(t.path(), 7).unwrap(), "no cover, no icon");
        let p = process(&test_png(600, 900)).unwrap();
        store(t.path(), 7, &p).unwrap();
        assert!(store_icon_from_cover(t.path(), 7).unwrap());
        assert_eq!(dims(&fs::read(icon_path(t.path(), 7)).unwrap()), (192, 192));
        remove_icon(t.path(), 7);
        assert!(!icon_path(t.path(), 7).exists());
    }

    #[test]
    fn an_ico_file_decodes() {
        let rgba = image::RgbaImage::from_pixel(64, 64, image::Rgba([200, 30, 30, 255]));
        let mut ico = Vec::new();
        image::ImageEncoder::write_image(image::codecs::ico::IcoEncoder::new(&mut ico), rgba.as_raw(), 64, 64, image::ExtendedColorType::Rgba8).unwrap();
        assert_eq!(decode(&ico).unwrap().width(), 64);
        let t = tempfile::tempdir().unwrap();
        assert_eq!(store_icon(t.path(), 9, &ico).unwrap(), (64, 64));
    }

    #[test]
    fn a_picked_icon_is_kept_whole_and_not_stretched() {
        let t = tempfile::tempdir().unwrap();
        // A wide logo (300x100): letterboxed on a transparent square.
        assert_eq!(store_icon(t.path(), 8, &test_png(300, 100)).unwrap(), (300, 100));
        let d = webp::Decoder::new(&fs::read(icon_path(t.path(), 8)).unwrap()).decode().unwrap();
        assert_eq!((d.width(), d.height()), (192, 192));
        assert!(d.is_alpha());
        // The top band is transparent; the middle row has the picture.
        let px = d.to_vec();
        assert_eq!(px[3], 0, "top-left is transparent padding");
        let mid = ((192 / 2) * 192 + 96) * 4;
        assert!(px[mid + 3] > 200, "the picture itself is opaque");
    }
}
