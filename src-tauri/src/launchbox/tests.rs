use std::sync::atomic::AtomicUsize;
use std::sync::Mutex;

use super::stream::tests::{zip_of, SAMPLE};
use super::*;
use crate::art::http::fake::FakeHttp;
use crate::db;
use crate::scanner::normalize;

/// LaunchBox's server, from memory: counts downloads.
struct FakeSource {
    zip: Vec<u8>,
    last_modified: Mutex<String>,
    opens: AtomicUsize,
}

impl FakeSource {
    fn new(xml: &str) -> Self {
        Self { zip: zip_of(xml), last_modified: Mutex::new("Mon, 12 Oct 2026 08:00:00 GMT".into()), opens: AtomicUsize::new(0) }
    }
    fn opens(&self) -> usize {
        self.opens.load(Ordering::Relaxed)
    }
}

impl Source for FakeSource {
    fn head(&self) -> Result<(Option<String>, Option<String>, Option<u64>)> {
        Ok((Some(self.last_modified.lock().unwrap().clone()), None, Some(self.zip.len() as u64)))
    }
    fn open(&self) -> Result<Box<dyn Read + Send>> {
        self.opens.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(std::io::Cursor::new(self.zip.clone())))
    }
}

fn add_game(conn: &Connection, stem: &str) -> i64 {
    let p = normalize::parse(stem);
    conn.execute(
        "INSERT INTO games (platform_id, title, sort_title, search_key, region, launch_path, added_at)
         VALUES ('ps2', ?1, ?2, ?3, ?4, ?5, 0)",
        params![p.title, p.sort_title, p.search_key, p.region, format!("E:\\PS2\\{stem}.iso")],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn status_of(conn: &Connection, id: i64) -> (String, Option<i64>) {
    conn.query_row("SELECT lb_status, lb_id FROM games WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn run_pass(conn: &mut Connection, dir: &Path, source: &FakeSource, http: &FakeHttp, force: bool) -> Summary {
    run(conn, dir, source, http, force, &|_| {}, &AtomicBool::new(false)).unwrap()
}

#[test]
fn a_pass_matches_keeps_only_the_library_and_applies_art_and_info() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::test_db();
    let jak = add_game(&conn, "Jak & Daxter - The Precursor Legacy (USA)");
    let ookami = add_game(&conn, "Ookami (Japan)");
    let stranger = add_game(&conn, "Totally Unknown Game (USA)");
    // Jak already has an automatic libretro cover: LaunchBox's replaces it.
    queries::set_boxart(&conn, jak, "libretro", "Jak", "https://libretro/jak.png", 10, 14, 0).unwrap();

    let source = FakeSource::new(SAMPLE);
    let mut http = FakeHttp::default();
    http.pages.insert(image_url("bbbb-01.png"), images::test_png(400, 150));
    let s = run_pass(&mut conn, dir.path(), &source, &http, false);

    assert!(s.downloaded);
    assert_eq!(source.opens(), 1);
    assert_eq!((s.matched, s.not_found, s.ambiguous), (2, 1, 0));
    assert_eq!(status_of(&conn, jak), ("ok".into(), Some(101)));
    assert_eq!(status_of(&conn, ookami), ("ok".into(), Some(102)), "matched by its Japanese alternate title");
    assert_eq!(status_of(&conn, stranger), ("notfound".into(), None));
    // Settings lists the game that wasn't found by name.
    let st = status(&conn).unwrap();
    assert_eq!(st.unmatched.iter().map(|u| (u.id, u.ambiguous)).collect::<Vec<_>>(), [(stranger, false)]);

    // Names for the enabled platforms only (not the Windows game); details and
    // images only for the library's matches.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM lb_names"), 3);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM lb_names WHERE db_id = 900"), 0);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM lb_games"), 2);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM lb_images"), 2);

    // Cover queued for the art pass; logo stored; genres and info from LaunchBox.
    assert_eq!(s.covers_queued, 1);
    let art: String = conn.query_row("SELECT art_status FROM games WHERE id = ?1", [jak], |r| r.get(0)).unwrap();
    assert_eq!(art, "none");
    assert_eq!(cover_urls(&conn, 101, Some("USA")).unwrap()[0].1, "https://images.launchbox-app.com/aaaa%2D01%2Ejpg");
    assert_eq!((s.logos, s.heroes), (1, 0));
    assert!(images::extra_path(dir.path(), Extra::Logo, jak).is_file());
    let meta = queries::game_metadata(&conn, jak).unwrap();
    assert_eq!(meta.developer.as_deref(), Some("Naughty Dog"));
    assert_eq!(meta.genres, vec!["Adventure".to_string(), "Platform".to_string()]);
    assert_eq!(meta.players, Some(1));
    assert_eq!(meta.release_date, Some(1_007_337_600), "2001-12-03");

    // Nothing new: no second download.
    let s = run_pass(&mut conn, dir.path(), &source, &http, false);
    assert!(!s.downloaded);
    assert_eq!(source.opens(), 1);

    // A new game matches offline, from the stored name list.
    let okami_us = add_game(&conn, "Okami (USA)");
    let s = run_pass(&mut conn, dir.path(), &source, &http, false);
    assert!(!s.downloaded, "its details are already stored");
    assert_eq!(status_of(&conn, okami_us), ("ok".into(), Some(102)));
}

#[test]
fn disc_and_back_pictures_come_with_one_more_download_for_older_lists() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::test_db();
    let jak = add_game(&conn, "Jak & Daxter - The Precursor Legacy (USA)");
    let xml = SAMPLE.replace(
        "</LaunchBox>",
        "  <GameImage><DatabaseID>101</DatabaseID><FileName>cccc-01.png</FileName><Type>Disc</Type></GameImage>
  <GameImage><DatabaseID>101</DatabaseID><FileName>dddd-01.jpg</FileName><Type>Box - Back</Type></GameImage>
</LaunchBox>",
    );
    let source = FakeSource::new(&xml);
    let mut http = FakeHttp::default();
    http.pages.insert(image_url("cccc-01.png"), images::test_png(300, 300));
    http.pages.insert(image_url("dddd-01.jpg"), images::test_png(300, 420));
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert!(images::case_path(dir.path(), images::Case::Disc, jak).is_file());
    assert!(images::case_path(dir.path(), images::Case::Back, jak).is_file());
    assert_eq!(source.opens(), 1);

    // A list saved before discs and backs were kept: downloaded once more, then not again.
    let mut st = stamp(&conn).unwrap();
    st.kinds = 0;
    save_stamp(&conn, &st).unwrap();
    assert!(wanted(&conn).unwrap());
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert_eq!(source.opens(), 2);
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert_eq!(source.opens(), 2);
}

#[test]
fn back_covers_prefer_english_regions_and_spines_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::test_db();
    let jak = add_game(&conn, "Jak & Daxter - The Precursor Legacy (Japan)");
    let xml = SAMPLE.replace(
        "</LaunchBox>",
        "  <GameImage><DatabaseID>101</DatabaseID><FileName>jp-back.jpg</FileName><Type>Box - Back</Type><Region>Japan</Region></GameImage>
           <GameImage><DatabaseID>101</DatabaseID><FileName>na-back.jpg</FileName><Type>Box - Back</Type><Region>North America</Region></GameImage>
           <GameImage><DatabaseID>101</DatabaseID><FileName>spine.png</FileName><Type>Box - Spine</Type></GameImage>
</LaunchBox>",
    );
    let source = FakeSource::new(&xml);
    let mut http = FakeHttp::default();
    http.pages.insert(image_url("jp-back.jpg"), images::test_png(300, 420));
    http.pages.insert(image_url("na-back.jpg"), images::test_png(310, 430));
    http.pages.insert(image_url("spine.png"), images::test_png(40, 400));
    run_pass(&mut conn, dir.path(), &source, &http, false);
    // The Japanese game still gets the English back first.
    let backs = images_ranked(&conn, 101, &["Box - Back"], Some("Japan"), true).unwrap();
    assert_eq!(backs.iter().map(|b| b.0.as_str()).collect::<Vec<_>>(), ["na-back.jpg", "jp-back.jpg"]);
    let own = images_ranked(&conn, 101, &["Box - Back"], Some("Japan"), false).unwrap();
    assert_eq!(own[0].0, "jp-back.jpg");
    let stored = std::fs::read(images::case_path(dir.path(), images::Case::Back, jak)).unwrap();
    assert_eq!(webp::Decoder::new(&stored).decode().unwrap().width(), 310, "the English back, not the Japanese one");
    assert!(images::case_path(dir.path(), images::Case::Spine, jak).is_file());
    // The picker lists both, English first, labelled with their region.
    let pick = case_picker(&conn, jak, images::Case::Back).unwrap();
    assert_eq!(pick[0].label, "LaunchBox back cover · North America");
}

#[test]
fn searching_for_a_back_cover_loads_another_entry_s_pictures_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::test_db();
    let game = add_game(&conn, "Totally Unknown Game (USA)");
    let xml = SAMPLE.replace(
        "</LaunchBox>",
        "  <GameImage><DatabaseID>102</DatabaseID><FileName>okami-back.jpg</FileName><Type>Box - Back</Type><Region>North America</Region></GameImage>
</LaunchBox>",
    );
    let source = FakeSource::new(&xml);
    let http = FakeHttp::default();
    run_pass(&mut conn, dir.path(), &source, &http, false);
    let opens = source.opens();

    // Okami isn't in the library: found by name, but its pictures aren't stored yet.
    let found = case_search(&conn, game, images::Case::Back, "okami").unwrap();
    assert_eq!((found[0].source.as_str(), found[0].url.as_str()), ("lbgame", "102"));
    assert!(!wanted(&conn).unwrap());
    // Choosing it asks for them: the next pass downloads once and keeps them.
    add_browse(&conn, &[102]).unwrap();
    assert!(wanted(&conn).unwrap());
    assert!(run_pass(&mut conn, dir.path(), &source, &http, false).downloaded);
    assert_eq!(source.opens(), opens + 1);
    let found = case_search(&conn, game, images::Case::Back, "okami").unwrap();
    assert_eq!((found[0].source.as_str(), found[0].name.as_str()), ("launchbox", "okami-back.jpg"));
    assert!(found[0].label.ends_with("North America"));
    // Stored, so no more downloads, and later passes keep it.
    assert!(!wanted(&conn).unwrap());
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert_eq!(source.opens(), opens + 1);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM lb_images WHERE db_id = 102"), 1);
}

#[test]
fn the_weekly_check_downloads_only_when_launchbox_changed() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::test_db();
    add_game(&conn, "Totally Unknown Game (USA)");
    let source = FakeSource::new(SAMPLE);
    let http = FakeHttp::default();
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert_eq!(source.opens(), 1);
    assert!(!check_due(&conn).unwrap(), "just checked");

    // A week later, same file: a HEAD request only.
    let mut st = stamp(&conn).unwrap();
    st.checked_at = Some(0);
    save_stamp(&conn, &st).unwrap();
    assert!(check_due(&conn).unwrap());
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert_eq!(source.opens(), 1);
    assert!(!check_due(&conn).unwrap());

    // A week later and LaunchBox changed: downloaded again.
    let mut st = stamp(&conn).unwrap();
    st.checked_at = Some(0);
    save_stamp(&conn, &st).unwrap();
    *source.last_modified.lock().unwrap() = "Tue, 13 Oct 2026 08:00:00 GMT".into();
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert_eq!(source.opens(), 2);
}

#[test]
fn a_platform_added_after_the_last_download_gets_one_more() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::test_db();
    add_game(&conn, "Okami (USA)");
    let source = FakeSource::new(SAMPLE);
    let http = FakeHttp::default();
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert_eq!(source.opens(), 1);

    // A download from before Switch existed, and a Switch game it couldn't find.
    let mut st = stamp(&conn).unwrap();
    st.platforms.retain(|p| p != "switch");
    save_stamp(&conn, &st).unwrap();
    conn.execute(
        "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at, lb_status)
         VALUES ('switch', 'Bayonetta 2', 'bayonetta 00002', 'bayonetta 2', 'E:\\Switch\\Bayonetta 2.xci', 0, 'notfound')",
        [],
    )
    .unwrap();
    assert!(wanted(&conn).unwrap());
    assert!(run_pass(&mut conn, dir.path(), &source, &http, false).downloaded);
    assert_eq!(source.opens(), 2);

    // This LaunchBox file lists no Switch games: that's remembered, not downloaded every pass.
    assert!(!wanted(&conn).unwrap());
    run_pass(&mut conn, dir.path(), &source, &http, false);
    assert_eq!(source.opens(), 2);
}

#[test]
fn fix_match_searches_offline_and_a_pick_sticks() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::test_db();
    let game = add_game(&conn, "Totally Unknown Game (USA)");
    let source = FakeSource::new(SAMPLE);
    let http = FakeHttp::default();
    run_pass(&mut conn, dir.path(), &source, &http, false);

    let hits = search(&conn, game, Some("okami")).unwrap();
    assert_eq!(hits[0].db_id, 102);
    // Okami's details are stored only once some library game matches it: a
    // pick of an unstored entry asks for a download, which then keeps it.
    assert!(set_manual(&conn, game, 102).unwrap());
    let s = run_pass(&mut conn, dir.path(), &source, &http, false);
    assert!(s.downloaded);
    assert_eq!(status_of(&conn, game), ("manual".into(), Some(102)));
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM lb_games WHERE db_id = 102"), 1);
    // Later passes never undo the pick.
    run_pass(&mut conn, dir.path(), &source, &http, true);
    assert_eq!(status_of(&conn, game), ("manual".into(), Some(102)));
}

#[test]
fn two_entries_with_the_same_name_are_left_for_fix_match() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::test_db();
    let game = add_game(&conn, "Okami (USA)");
    let xml = SAMPLE.replace("<Name>Some PC Game</Name>", "<Name>Okami</Name>").replace("<Platform>Windows</Platform>", "<Platform>Sony Playstation 2</Platform>");
    let source = FakeSource::new(&xml);
    let s = run_pass(&mut conn, dir.path(), &source, &FakeHttp::default(), false);
    assert_eq!(s.ambiguous, 1);
    assert_eq!(status_of(&conn, game), ("ambiguous".into(), None));
}

/// Match quality on real data: LaunchBox's real file against real dump names
/// (No-Intro/Redump, from libretro's public cover lists), and how long reading
/// the file takes.
/// `LB_ZIP=<path to Metadata.zip> cargo test --release --lib launchbox_real -- --ignored --nocapture`
#[test]
#[ignore]
fn launchbox_real() {
    use crate::art::{http::Http, libretro};
    let zip = std::env::var("LB_ZIP").expect("set LB_ZIP to a downloaded Metadata.zip");
    let platforms: HashMap<&str, &str> =
        crate::platforms::PLATFORMS.iter().filter_map(|p| Some((p.launchbox?, p.id))).collect();
    let t0 = std::time::Instant::now();
    let mut names: HashMap<&str, Vec<Name>> = HashMap::new();
    let mut platform_of: HashMap<i64, &str> = HashMap::new();
    let mut records = 0usize;
    let file = std::io::BufReader::new(std::fs::File::open(zip).unwrap());
    stream::records(stream::open_entry(file).unwrap(), |r| {
        records += 1;
        match r {
            Record::Game(g) => {
                if let Some(kb) = platforms.get(g.platform.as_str()) {
                    platform_of.insert(g.db_id, kb);
                    names.entry(kb).or_default().push(Name { db_id: g.db_id, name: g.name, region: None, is_alt: false });
                }
            }
            Record::AltName { db_id, name, region } => {
                if let Some(kb) = platform_of.get(&db_id) {
                    names.entry(kb).or_default().push(Name { db_id, name, region, is_alt: true });
                }
            }
            Record::Image { .. } => {}
        }
        Ok(())
    })
    .unwrap();
    println!("read {records} records in {:.1} s", t0.elapsed().as_secs_f32());

    let http = Http::new();
    let skip = ["(beta", "(proto", "(demo", "(sample", "[", "(hack", "(unl)", "(pirate", "(aftermarket", "(kiosk", "(debug", "(program"];
    let (mut all, mut hit) = (0, 0);
    for p in crate::platforms::PLATFORMS {
        let (Some(system), Some(cands)) = (p.libretro, names.remove(p.id)) else { continue };
        let Ok(Some(html)) = crate::art::http::Fetch::get(&http, &libretro::index_url(system)) else { continue };
        let dumps: Vec<String> = libretro::parse_index(&String::from_utf8_lossy(&html))
            .into_iter()
            .filter(|n| !skip.iter().any(|s| n.to_ascii_lowercase().contains(s)))
            .collect();
        let step = (dumps.len() / 150).max(1);
        let cands = Candidates::new(cands);
        let (mut ok, mut amb, mut miss) = (0, 0, Vec::new());
        let sample: Vec<&String> = dumps.iter().step_by(step).collect();
        for (i, d) in sample.iter().enumerate() {
            let parsed = normalize::parse(d);
            match cands.best(&parsed.search_key, parsed.region.as_deref()) {
                Outcome::Match(id) => {
                    ok += 1;
                    if std::env::var_os("LB_SHOW").is_some() && i % 9 == 0 {
                        println!("    {d}  ->  {}", cands.main_name(id).unwrap_or("?"));
                    }
                }
                Outcome::Ambiguous => amb += 1,
                Outcome::NotFound => miss.push(d.as_str()),
            }
        }
        all += sample.len();
        hit += ok;
        println!(
            "{:7} {:3}/{:3} matched ({:.0}%), {} left for Fix match, {} not found  e.g. {:?}",
            p.id,
            ok,
            sample.len(),
            100.0 * ok as f32 / sample.len().max(1) as f32,
            amb,
            miss.len(),
            &miss[..miss.len().min(4)]
        );
    }
    println!("overall {hit}/{all} ({:.0}%)", 100.0 * hit as f32 / all.max(1) as f32);
}

#[test]
fn release_dates_and_years() {
    let g = |d: Option<&str>, y: Option<i32>| LbGame { release_date: d.map(String::from), release_year: y, ..Default::default() };
    assert_eq!(release_secs(&g(Some("2001-12-03T00:00:00-08:00"), None)), Some(1_007_337_600));
    assert_eq!(release_secs(&g(Some("1970-01-01"), None)), Some(0));
    assert_eq!(release_secs(&g(None, Some(2005))), Some(1_104_537_600));
    assert_eq!(release_secs(&g(Some("garbage"), None)), None);
}
