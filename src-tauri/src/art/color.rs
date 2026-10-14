//! A cover's main color, for tinting a series in the game details: the most
//! common vivid hue on the cover, made bright enough to read on KB's dark
//! background. Computed from the small grid thumbnail (a few ms), on demand.

use std::path::Path;

use super::images;

/// Roughly how many pixels are looked at (the thumbnail is sampled evenly).
const SAMPLES: usize = 12_000;
/// Hue buckets of 10 degrees.
const BUCKETS: usize = 36;

/// `#rrggbb`, or None when the cover is missing or has no real color
/// (black-and-white, grey).
pub fn accent(data_dir: &Path, game_id: i64) -> Option<String> {
    let bytes = std::fs::read(images::thumb_path(data_dir, game_id)).ok()?;
    let img = webp::Decoder::new(&bytes).decode()?;
    let channels = if img.is_alpha() { 4 } else { 3 };
    accent_of(&img, channels)
}

/// The main vivid color of packed RGB/RGBA pixels.
pub fn accent_of(pixels: &[u8], channels: usize) -> Option<String> {
    let count = pixels.len() / channels;
    if count == 0 {
        return None;
    }
    let step = (count / SAMPLES).max(1);
    let mut weight = [0f64; BUCKETS];
    let mut sums = [[0f64; 3]; BUCKETS];
    let mut total = 0f64;
    for i in (0..count).step_by(step) {
        let p = &pixels[i * channels..i * channels + channels];
        if channels == 4 && p[3] < 128 {
            continue;
        }
        let (r, g, b) = (p[0] as f64 / 255.0, p[1] as f64 / 255.0, p[2] as f64 / 255.0);
        let (h, s, v) = hsv(r, g, b);
        total += 1.0;
        // Dark or greyish pixels say little about the cover's color.
        if v < 0.2 || s < 0.25 {
            continue;
        }
        let w = s * s * v;
        let k = ((h / 360.0 * BUCKETS as f64) as usize).min(BUCKETS - 1);
        weight[k] += w;
        sums[k][0] += r * w;
        sums[k][1] += g * w;
        sums[k][2] += b * w;
    }
    // The strongest run of three neighbouring buckets (hues wrap around).
    let window = |k: usize| [(k + BUCKETS - 1) % BUCKETS, k, (k + 1) % BUCKETS];
    let best = (0..BUCKETS).max_by(|&a, &b| {
        let wa: f64 = window(a).iter().map(|&i| weight[i]).sum();
        let wb: f64 = window(b).iter().map(|&i| weight[i]).sum();
        wa.total_cmp(&wb)
    })?;
    let w: f64 = window(best).iter().map(|&i| weight[i]).sum();
    // Too little color overall: a grey or black-and-white cover.
    if total == 0.0 || w / total < 0.02 {
        return None;
    }
    let mut rgb = [0f64; 3];
    for i in window(best) {
        for c in 0..3 {
            rgb[c] += sums[i][c];
        }
    }
    let (h, s, _) = hsl(rgb[0] / w, rgb[1] / w, rgb[2] / w);
    // Bright and saturated enough to read as text and glow on a dark panel.
    let (r, g, b) = from_hsl(h, s.clamp(0.5, 0.85), 0.66);
    Some(format!("#{:02x}{:02x}{:02x}", (r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8))
}

fn hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    (hue(r, g, b, max, d), if max == 0.0 { 0.0 } else { d / max }, max)
}

fn hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let l = (max + min) / 2.0;
    let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    (hue(r, g, b, max, d), s, l)
}

fn hue(r: f64, g: f64, b: f64, max: f64, d: f64) -> f64 {
    if d == 0.0 {
        return 0.0;
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

fn from_hsl(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(parts: &[([u8; 3], usize)]) -> Vec<u8> {
        parts.iter().flat_map(|(c, n)| std::iter::repeat_n(*c, *n).flatten()).collect()
    }

    fn hue_of(hex: &str) -> f64 {
        let v = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
        hsl(v(1), v(3), v(5)).0
    }

    #[test]
    fn the_main_vivid_hue_wins_over_dark_and_grey_areas() {
        // Mostly a dark background and grey text, with an orange logo and a bit of blue.
        let px = image(&[([10, 10, 12], 6000), ([128, 128, 128], 2000), ([230, 120, 20], 1500), ([40, 80, 220], 400)]);
        let c = accent_of(&px, 3).unwrap();
        assert!((20.0..40.0).contains(&hue_of(&c)), "orange, got {c}");
    }

    #[test]
    fn grey_covers_have_no_color_and_dark_colors_are_brightened() {
        assert_eq!(accent_of(&image(&[([20, 20, 20], 5000), ([200, 200, 200], 5000)]), 3), None);
        // A deep navy cover still gives a blue that reads on a dark panel.
        let c = accent_of(&image(&[([10, 20, 90], 5000)]), 3).unwrap();
        let v = |i: usize| u8::from_str_radix(&c[i..i + 2], 16).unwrap();
        assert!(v(5) > 200 && (200.0..260.0).contains(&hue_of(&c)), "{c}");
    }

    #[test]
    fn transparent_pixels_are_ignored_and_files_decode() {
        let mut px = Vec::new();
        for i in 0..4000 {
            px.extend_from_slice(if i % 2 == 0 { &[0, 200, 60, 255] } else { &[255, 0, 0, 0] });
        }
        let c = accent_of(&px, 4).unwrap();
        assert!((110.0..160.0).contains(&hue_of(&c)), "green, got {c}");

        let dir = tempfile::tempdir().unwrap();
        let path = images::thumb_path(dir.path(), 7);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let rgb = image(&[([200, 30, 40], 64 * 64)]);
        std::fs::write(&path, &*webp::Encoder::from_rgb(&rgb, 64, 64).encode(90.0)).unwrap();
        let c = accent(dir.path(), 7).unwrap();
        assert!(hue_of(&c) < 10.0 || hue_of(&c) > 350.0, "red, got {c}");
        assert_eq!(accent(dir.path(), 8), None, "no thumbnail");
    }
}
