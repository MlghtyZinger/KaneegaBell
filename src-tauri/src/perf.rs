//! Performance harness: generates a synthetic 5,000-game library (tiny fake
//! files in real folder layouts, each game with a cover in the art cache)
//! in a folder of its own, for measuring the app against CLAUDE.md's targets.
//! Run the app on it with `KB_DATA_DIR=<root>\data`; the real
//! library is never touched. See README → Performance.
//!
//! `PERF_ROOT=C:\perf-lib cargo test --release --lib generate_perf_library -- --ignored --nocapture`

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::art::images;
use crate::db::{self, queries};
use crate::scanner::{self, sfo};

/// (platform, games, file extension, typical cover height/width)
const PLATFORMS: &[(&str, usize, &str, f64)] = &[
    ("snes", 600, "sfc", 0.72),
    ("n64", 300, "z64", 0.73),
    ("gba", 600, "gba", 1.0),
    ("gc", 350, "rvz", 1.4),
    ("nds", 600, "nds", 0.9),
    ("wii", 350, "rvz", 1.4),
    ("3ds", 300, "cci", 0.88),
    ("segacd", 150, "chd", 1.0),
    ("ps1", 600, "chd", 1.0),
    ("ps2", 700, "chd", 1.4),
    ("psp", 300, "cso", 1.72),
    ("ps3", 150, "", 1.16),
];

const WORDS: &[&str] = &[
    "Shadow", "Legend", "Dragon", "Quest", "Star", "Fire", "Emblem", "Kingdom", "Hearts", "Metal", "Gear", "Solid",
    "Final", "Fantasy", "Castle", "Night", "Moon", "Sun", "Dark", "Dawn", "Golden", "Silver", "Crystal", "Ocean",
    "Sky", "Iron", "Storm", "Thunder", "Blade", "Saga", "Chronicles", "Tales", "Mystic", "Ancient", "City", "Lost",
    "Age", "Revenge", "Return", "Rise", "Fall", "Empire", "Galaxy", "Racing", "Ninja", "Samurai", "Spirit", "Ghost",
    "Soul", "Heart", "Hero", "Knight", "Wars", "Island", "Rally", "Tactics", "Arena", "Frontier", "Machine", "Rider",
];
const REGIONS: &[&str] = &["(USA)", "(Europe)", "(Japan)", "(USA, Europe)", "(USA) (Rev 1)", "(Europe) (En,Fr,De,Es,It)"];

/// Small seeded PRNG (xorshift64*), so every run builds the same library.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn title(r: &mut Rng) -> String {
    let words = 1 + r.below(4);
    let mut t: Vec<&str> = (0..words).map(|_| WORDS[r.below(WORDS.len())]).collect();
    t.dedup();
    let mut s = t.join(" ");
    if r.below(10) < 3 {
        s.push_str(&format!(" - {} {}", WORDS[r.below(WORDS.len())], WORDS[r.below(WORDS.len())]));
    }
    if r.below(100) < 15 {
        s.push_str(&format!(" {}", 2 + r.below(4)));
    }
    s
}

/// A cover-like image (gradient plus noise, so it compresses like art does).
fn cover_png(w: u32, aspect: f64, seed: u64) -> Vec<u8> {
    let h = (f64::from(w) * aspect).round() as u32;
    let mut r = Rng(seed | 1);
    let base = [r.below(200) as u8, r.below(200) as u8, r.below(200) as u8];
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        let n = (r.next() & 0x3F) as u8;
        let g = ((x + y) * 255 / (w + h)) as u8 / 2;
        image::Rgb([base[0].wrapping_add(g).wrapping_add(n), base[1].wrapping_add(n), base[2].wrapping_add(g / 2)])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A logo-like image: bold light bars on a transparent background.
fn logo_png(seed: u64) -> Vec<u8> {
    let (w, h) = (900u32, 320u32);
    let mut r = Rng(seed + 31);
    let bars: Vec<(u32, u32)> = (0..5).map(|i| (60 + i * 160, 60 + r.below(120) as u32)).collect();
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let on = bars.iter().any(|&(bx, top)| x >= bx && x < bx + 120 && y >= top && y < h - 40);
        if on { image::Rgba([245, 240, 230, 255]) } else { image::Rgba([0, 0, 0, 0]) }
    });
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
#[ignore]
fn generate_perf_library() {
    let root = PathBuf::from(std::env::var("PERF_ROOT").expect("set PERF_ROOT to an empty folder"));
    let roms = root.join("roms");
    let data = root.join("data");
    assert!(!data.join("library.db").exists(), "{} already has a library; delete it first", data.display());
    let mut r = Rng(0x5EED_1234_ABCD);

    // 1. Files.
    let t = Instant::now();
    let mut files = 0;
    for &(platform, count, ext, _) in PLATFORMS {
        let dir = roms.join(platform);
        let mut used = std::collections::HashSet::new();
        for i in 0..count {
            let mut name = format!("{} {}", title(&mut r), REGIONS[r.below(REGIONS.len())]);
            if !used.insert(name.clone()) {
                name = format!("{name} [{i}]");
                used.insert(name.clone());
            }
            if platform == "ps3" {
                let game = dir.join(&name).join("PS3_GAME");
                let id = format!("BLUS{:05}", 30000 + i);
                let sfo = sfo::build(&[("TITLE", &name), ("TITLE_ID", &id), ("CATEGORY", "DG")]);
                write(&game.join("PARAM.SFO"), &sfo);
                write(&game.join("USRDIR").join("EBOOT.BIN"), b"SCE\0");
                files += 2;
            } else if platform == "ps1" && i % 20 == 0 {
                // Multi-disc games: grouped into one entry by the scanner.
                for disc in 1..=2 {
                    write(&dir.join(format!("{name} (Disc {disc}).{ext}")), &[0u8; 16]);
                    files += 1;
                }
            } else {
                write(&dir.join(format!("{name}.{ext}")), &[0u8; 16]);
                files += 1;
            }
        }
    }
    println!("generated {files} files in {:.1}s", t.elapsed().as_secs_f64());

    // 2. Library: one source per platform folder, then a real scan.
    std::fs::create_dir_all(&data).unwrap();
    let mut conn = db::init(&data.join("library.db")).unwrap();
    for &(platform, ..) in PLATFORMS {
        queries::add_source(&conn, &roms.join(platform).to_string_lossy(), Some(platform), "files").unwrap();
    }
    let cancel = AtomicBool::new(false);
    let s = scanner::run(&mut conn, &|_| {}, &cancel).unwrap();
    println!("first scan: {} games in {} ms", s.games_total, s.duration_ms);
    let s2 = scanner::run(&mut conn, &|_| {}, &cancel).unwrap();
    println!("rescan (nothing changed): {} ms", s2.duration_ms);
    assert!(s.games_total >= 4900, "expected ~5000 games, got {}", s.games_total);

    // 3. Covers: a pool of distinct images per platform, stored for every
    // game exactly as downloaded art is (grid thumbnail + details image).
    let t = Instant::now();
    let mut pools = std::collections::HashMap::new();
    for (k, &(platform, _, _, aspect)) in PLATFORMS.iter().enumerate() {
        let pool: Vec<images::Processed> =
            (0..6).map(|v| images::process(&cover_png(600, aspect, (k * 100 + v) as u64 + 7)).unwrap()).collect();
        pools.insert(platform, pool);
    }
    let games: Vec<(i64, String)> = conn
        .prepare("SELECT id, platform_id FROM games")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let tx = conn.transaction().unwrap();
    for (id, platform) in &games {
        let p = &pools[platform.as_str()][(*id as usize) % 6];
        images::store(&data, *id, p).unwrap();
        queries::set_boxart(&tx, *id, "libretro", &format!("perf-{id}"), "perf://cover", p.width, p.height, queries::now())
            .unwrap();
    }
    tx.commit().unwrap();

    // TV Mode extras for 80% of games, as SteamGridDB would provide: a wide
    // hero and a transparent logo (a few distinct images, reused).
    let heroes: Vec<Vec<u8>> = (0..6).map(|v| cover_png(1920, 620.0 / 1920.0, 900 + v)).collect();
    let logos: Vec<Vec<u8>> = (0..6).map(logo_png).collect();
    let tx = conn.transaction().unwrap();
    // Encode each image once, then copy the files.
    let mut first: [Option<i64>; 6] = [None; 6];
    for (id, _) in games.iter().filter(|(id, _)| id % 5 != 0) {
        let k = (*id as usize) % 6;
        for (extra, src) in [(images::Extra::Hero, &heroes[k]), (images::Extra::Logo, &logos[k])] {
            match first[k] {
                Some(from) => {
                    std::fs::copy(images::extra_path(&data, extra, from), images::extra_path(&data, extra, *id)).unwrap();
                }
                None => {
                    images::store_extra(&data, extra, *id, src).unwrap();
                }
            }
        }
        first[k].get_or_insert(*id);
        queries::set_extra(&tx, *id, "hero", "sgdb", &format!("perf-h{k}"), "perf://hero", 1920, 620, queries::now()).unwrap();
        queries::set_extra(&tx, *id, "logo", "sgdb", &format!("perf-l{k}"), "perf://logo", 800, 300, queries::now()).unwrap();
    }
    tx.commit().unwrap();
    // Nothing for the background jobs to do on startup.
    conn.execute("UPDATE games SET art_status = 'ok', match_status = 'notfound', meta_checked_at = ?1", [queries::now()])
        .unwrap();
    queries::set_setting(&conn, "autoArt", &serde_json::json!(false)).unwrap();
    // The generated titles aren't real games: no LaunchBox download mid-measurement.
    queries::set_setting(&conn, "lbEnabled", &serde_json::json!(false)).unwrap();
    queries::set_setting(&conn, "wdEnabled", &serde_json::json!(false)).unwrap();
    // A stand-in "emulator" for PS2 games: PowerShell waiting 25 s, so the
    // harness can measure the app while a game runs.
    #[cfg(windows)]
    queries::update_emulator(
        &conn,
        "pcsx2",
        Some(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".into()),
        Some(r#""{emulator}" -NoProfile -WindowStyle Hidden -Command "Start-Sleep 25 #" "{rom}""#.into()),
    )
    .unwrap();
    println!("covers for {} games in {:.1}s", games.len(), t.elapsed().as_secs_f64());
    println!("library ready: set KB_DATA_DIR={}", data.display());
}
