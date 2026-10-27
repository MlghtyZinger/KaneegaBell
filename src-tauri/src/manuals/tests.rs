use super::sources::*;
use super::*;
use crate::art::http::fake::FakeHttp;

const RK_N64: &str = "https://archive.org/download/retrokit-manuals/n64/n64-compressed.zip/";

fn listing() -> Vec<u8> {
    r#"<table><tr><td><a href="//archive.org/download/retrokit-manuals/n64/n64-compressed.zip/Banjo-Kazooie%20%28en%29.pdf">Banjo-Kazooie (en).pdf</a><td><td>2022-02-01 23:44<td id="size">2666293</tr>
<tr><td><a href="//archive.org/download/retrokit-manuals/n64/n64-compressed.zip/Banjo-Kazooie%20%28en-gb%2Cfr%29.pdf">Banjo-Kazooie (en-gb,fr).pdf</a><td><td>2022-02-01 23:44<td id="size">3005527</tr>
<tr><td><a href="//archive.org/download/retrokit-manuals/n64/n64-compressed.zip/Mario%20Kart%2064%20%28ja%29.pdf">Mario Kart 64 (ja).pdf</a><td><td>2022-02-01 23:44<td id="size">1362780</tr></table>"#
        .as_bytes()
        .to_vec()
}

fn add_game(conn: &Connection, platform: &str, title: &str, region: Option<&str>) -> i64 {
    let key = crate::scanner::normalize::parse(title).search_key;
    conn.execute(
        "INSERT INTO games (platform_id, title, sort_title, search_key, region, launch_path, added_at) VALUES (?1, ?2, ?2, ?3, ?4, ?2, 0)",
        rusqlite::params![platform, title, key, region],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn downloads(conn: &Connection, on: bool) {
    queries::set_setting(conn, "manualDownloads", &serde_json::json!(on)).unwrap();
}

#[test]
fn retrokit_listings_become_titles_with_regions() {
    let e = parse_retrokit(&String::from_utf8(listing()).unwrap());
    assert_eq!(e.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Banjo-Kazooie (USA)", "Banjo-Kazooie (Europe)", "Mario Kart 64 (Japan)"]);
    assert_eq!(e[0].url, "https://archive.org/download/retrokit-manuals/n64/n64-compressed.zip/Banjo-Kazooie%20%28en%29.pdf");
    assert_eq!((e[0].label.as_str(), e[0].size), ("Retrokit · Banjo-Kazooie (en)", Some(2666293)));
}

#[test]
fn internet_archive_titles_are_cleaned_to_game_titles() {
    let cases = [
        ("XBOX Manual: 007 - Agent Under Fire (2002)(Electronic Arts)(US)", "007 - Agent Under Fire", Some("USA")),
        ("Sky Target (1997)(U)", "Sky Target", Some("USA")),
        ("Red Dead Revolver (USA)", "Red Dead Revolver", Some("USA")),
        ("Final Fantasy V Advance Manual GBA", "Final Fantasy V Advance", None),
        ("Nintendo 64 - Donkey Kong 64 - Instruction Booklet", "Donkey Kong 64", None),
        ("Metal Slug 3 (PAL) PlayStation 2 manual", "Metal Slug 3", Some("Europe")),
        ("Star Soldier: Vanishing Earth N64 Manual [NUS-NS6E-USA]", "Star Soldier: Vanishing Earth", Some("USA")),
        ("Tiny Bullets (PS1) Packaging and Manual", "Tiny Bullets", None),
        ("Front Mission SNES JP Manual 600dpi", "Front Mission SNES JP Manual 600dpi", None),
    ];
    for (raw, title, region) in cases {
        assert_eq!(clean_ia_title(raw), (title.to_string(), region), "{raw}");
    }
}

#[test]
fn the_best_source_and_the_games_region_win_and_only_same_titles_match() {
    let rk = parse_retrokit(&String::from_utf8(listing()).unwrap());
    let ia = vec![Entry { name: "Banjo-Kazooie (USA)".into(), url: "ia:banjo".into(), label: "IA".into(), size: None }];
    let lists = vec![rk, ia];
    let key = |t: &str, r: Option<&str>| GameKey { search_key: crate::scanner::normalize::parse(t).search_key, region: r.map(String::from) };
    assert_eq!(best(&lists, &key("Banjo-Kazooie", Some("Europe"))).unwrap().label, "Retrokit · Banjo-Kazooie (en-gb,fr)");
    assert_eq!(best(&lists, &key("Banjo-Kazooie", Some("USA"))).unwrap().label, "Retrokit · Banjo-Kazooie (en)");
    assert!(best(&lists, &key("Banjo-Tooie", Some("USA"))).is_none(), "a sequel is not the same game");
    // Only in the second list: found there.
    let only_ia = vec![Vec::new(), vec![Entry { name: "Okami (USA)".into(), url: "ia:okami".into(), label: "IA".into(), size: None }]];
    assert_eq!(best(&only_ia, &key("Okami", None)).unwrap().url, "ia:okami");
}

#[test]
fn matching_a_whole_library_finds_each_games_manual_once() {
    let conn = crate::db::test_db();
    let found = add_game(&conn, "n64", "Banjo-Kazooie", Some("Europe"));
    let missing = add_game(&conn, "n64", "Banjo-Tooie", Some("USA"));
    let other = add_game(&conn, "wii", "Wii Sports", None);
    let mut http = FakeHttp::default();
    http.pages.insert(RK_N64.into(), listing());
    assert_eq!(match_all(&conn, &http, &|| false).unwrap(), 1);
    let source = |id: i64| conn.query_row("SELECT source FROM game_manuals WHERE game_id = ?1", [id], |r| r.get::<_, String>(0)).unwrap();
    assert_eq!((source(found).as_str(), source(missing).as_str(), source(other).as_str()), ("retrokit", "none", "none"));
    // Done: a second pass asks for nothing.
    let asked = http.requests.lock().unwrap().len();
    assert_eq!(match_all(&conn, &http, &|| false).unwrap(), 0);
    assert_eq!(http.requests.lock().unwrap().len(), asked);
}

#[test]
fn only_internet_archive_addresses_are_allowed() {
    assert!(allowed_url("https://archive.org/download/x/y.pdf"));
    assert!(allowed_url("https://ia801901.us.archive.org/view_archive.php"));
    assert!(allowed_url("ia:ps2_Red_Dead_Revolver_USA"));
    assert!(!allowed_url("ia:../etc"));
    assert!(!allowed_url("http://archive.org/x"));
    assert!(!allowed_url("https://archive.org.evil.com/x"));
    assert!(!allowed_url("https://evil.com/archive.org"));
    assert!(!allowed_url("file:///C:/x.pdf"));
}

#[test]
fn an_items_full_scan_pdf_is_picked() {
    let mut http = FakeHttp::default();
    http.pages.insert(
        "https://archive.org/metadata/rdr/files".into(),
        serde_json::to_vec(&serde_json::json!({"result": [
            {"name": "Red_Dead_Revolver_USA_text.pdf", "format": "Additional Text PDF", "size": "2808476"},
            {"name": "Red_Dead_Revolver_USA.pdf", "format": "Image Container PDF", "size": "15346043"},
            {"name": "Red_Dead_Revolver_USA_jp2.zip", "format": "Single Page Processed JP2 ZIP", "size": "8430716"}
        ]}))
        .unwrap(),
    );
    assert_eq!(resolve_item(&http, "rdr").unwrap(), "https://archive.org/download/rdr/Red_Dead_Revolver_USA.pdf");
    assert!(resolve_item(&http, "../x").is_err());
}

#[test]
fn downloads_off_means_offline_and_greyed_buttons() {
    let conn = crate::db::test_db();
    let data = tempfile::tempdir().unwrap();
    let id = add_game(&conn, "n64", "Banjo-Kazooie", Some("USA"));
    let http = FakeHttp::default();
    assert_eq!(status(&conn, data.path(), id).unwrap(), Status { state: "off", source: None });
    let err = open(&conn, data.path(), &http, id, false).unwrap_err().to_string();
    assert!(err.contains("Manual downloads are off"), "{err}");
    assert!(http.requests.lock().unwrap().is_empty(), "nothing went online");
}

fn pdf() -> Vec<u8> {
    b"%PDF-1.4 fake manual".to_vec()
}

fn webp() -> Vec<u8> {
    let mut b = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
    b.extend([0u8; 20]);
    b
}

#[test]
fn a_manual_downloads_when_opened_and_becomes_pages() {
    let conn = crate::db::test_db();
    let data = tempfile::tempdir().unwrap();
    let id = add_game(&conn, "n64", "Banjo-Kazooie", Some("USA"));
    downloads(&conn, true);
    let mut http = FakeHttp::default();
    http.pages.insert(RK_N64.into(), listing());
    http.pages.insert("https://archive.org/download/retrokit-manuals/n64/n64-compressed.zip/Banjo-Kazooie%20%28en%29.pdf".into(), pdf());
    assert_eq!(status(&conn, data.path(), id).unwrap().state, "unknown");

    let o = open(&conn, data.path(), &http, id, false).unwrap();
    assert_eq!((o.kind, o.convert, o.source.as_str()), ("pdf", true, "Retrokit (Internet Archive)"));
    assert_eq!(pdf_bytes(&conn, data.path(), id).unwrap(), pdf());
    assert_eq!(status(&conn, data.path(), id).unwrap().state, "ready");
    // While a game runs, nothing is converted.
    assert!(!open(&conn, data.path(), &http, id, true).unwrap().convert);

    // The viewer hands the pages back; the PDF goes once they're all there.
    assert!(save_page(&conn, data.path(), id, 0, b"not a webp").is_err());
    save_page(&conn, data.path(), id, 0, &webp()).unwrap();
    assert!(finish(&conn, data.path(), id, 2, None).is_err(), "page 1 is missing");
    save_page(&conn, data.path(), id, 1, &webp()).unwrap();
    let outline = serde_json::json!([{"title": "Controls", "page": 2}]);
    finish(&conn, data.path(), id, 2, Some(&outline)).unwrap();
    assert!(!source_pdf(data.path(), id).exists());
    let o = open(&conn, data.path(), &http, id, false).unwrap();
    assert_eq!((o.kind, o.pages, o.outline), ("pages", Some(2), Some(outline)));
    assert!(bytes(data.path()) > 0);

    // Clearing keeps the match, drops the files; it downloads again next time.
    clear(&conn, data.path()).unwrap();
    assert_eq!(status(&conn, data.path(), id).unwrap().state, "available");
}

#[test]
fn nothing_found_says_so_and_a_typed_search_offers_choices() {
    let conn = crate::db::test_db();
    let data = tempfile::tempdir().unwrap();
    let id = add_game(&conn, "n64", "Unknown Game", Some("USA"));
    downloads(&conn, true);
    let mut http = FakeHttp::default();
    http.pages.insert(RK_N64.into(), listing());
    let err = open(&conn, data.path(), &http, id, false).unwrap_err().to_string();
    assert!(err.contains("No manual found"), "{err}");
    assert_eq!(status(&conn, data.path(), id).unwrap().state, "none");

    // The picker: related titles from the lists (the search is offline here, so it's skipped).
    let c = candidates(&conn, &http, id, Some("Banjo")).unwrap();
    assert!(c.iter().any(|e| e.label == "Retrokit · Banjo-Kazooie (en)"), "{c:?}");
    choose(&conn, data.path(), id, &c[0].url, &c[0].label).unwrap();
    assert_eq!(status(&conn, data.path(), id).unwrap().state, "available");
    assert!(choose(&conn, data.path(), id, "https://evil.com/x.pdf", "x").is_err());
}

#[test]
fn a_cbz_download_is_turned_into_pages_straight_away() {
    let conn = crate::db::test_db();
    let data = tempfile::tempdir().unwrap();
    let id = add_game(&conn, "ps2", "Okami", Some("USA"));
    downloads(&conn, true);
    let page = crate::art::images::test_png(60, 80);
    let cbz = zip::build(&[("01.png", &page), ("02.png", &page)]);
    let mut http = FakeHttp::default();
    http.pages.insert("https://archive.org/metadata/okami/files".into(), serde_json::to_vec(&serde_json::json!({"result": [{"name": "okami.cbz", "format": "Comic Book ZIP", "size": "9"}]})).unwrap());
    http.pages.insert("https://archive.org/download/okami/okami.cbz".into(), cbz);
    choose(&conn, data.path(), id, "ia:okami", "Internet Archive · Okami").unwrap();
    let o = open(&conn, data.path(), &http, id, false).unwrap();
    assert_eq!((o.kind, o.pages), ("pages", Some(2)));
    assert!(page_path(data.path(), id, 1).is_file());
}

#[test]
fn junk_downloads_are_refused() {
    let conn = crate::db::test_db();
    let data = tempfile::tempdir().unwrap();
    let id = add_game(&conn, "n64", "Banjo-Kazooie", Some("USA"));
    downloads(&conn, true);
    let mut http = FakeHttp::default();
    http.pages.insert(RK_N64.into(), listing());
    http.pages.insert("https://archive.org/download/retrokit-manuals/n64/n64-compressed.zip/Banjo-Kazooie%20%28en%29.pdf".into(), b"MZ\x90\0 an exe".to_vec());
    assert!(open(&conn, data.path(), &http, id, false).unwrap_err().to_string().contains("isn't a PDF or CBZ"));
    assert!(!dir(data.path(), id).exists());
}

#[test]
fn the_users_own_file_wins_and_is_checked() {
    let conn = crate::db::test_db();
    let data = tempfile::tempdir().unwrap();
    let id = add_game(&conn, "gc", "Metroid Prime", Some("USA"));
    let own = data.path().join("Metroid Prime.pdf");
    fs::write(&own, pdf()).unwrap();
    let fake = data.path().join("fake.pdf");
    fs::write(&fake, b"MZ not a pdf").unwrap();
    assert!(choose_file(&conn, data.path(), id, fake.to_str().unwrap()).is_err());
    assert!(choose_file(&conn, data.path(), id, data.path().join("x.exe").to_str().unwrap()).is_err());
    choose_file(&conn, data.path(), id, own.to_str().unwrap()).unwrap();
    // Downloads are off, yet the user's file opens.
    assert_eq!(status(&conn, data.path(), id).unwrap(), Status { state: "ready", source: Some("Your file: Metroid Prime.pdf".into()) });
    let o = open(&conn, data.path(), &FakeHttp::default(), id, false).unwrap();
    assert_eq!((o.kind, o.convert), ("pdf", false));
    assert!(save_page(&conn, data.path(), id, 0, &webp()).is_err(), "the user's own file is never converted");
    // Automatic lookups never replace it.
    downloads(&conn, true);
    assert!(lookup(&conn, &FakeHttp::default(), id).unwrap().is_none());
    assert_eq!(status(&conn, data.path(), id).unwrap().state, "ready");
}

#[test]
fn the_size_cap_drops_the_least_recently_opened_first() {
    let conn = crate::db::test_db();
    let data = tempfile::tempdir().unwrap();
    let ids: Vec<i64> = ["A", "B", "C"].iter().map(|t| add_game(&conn, "n64", t, None)).collect();
    for (i, id) in ids.iter().enumerate() {
        set_row(&conn, *id, "retrokit", "x", Some("https://archive.org/x"), false).unwrap();
        fs::create_dir_all(dir(data.path(), *id)).unwrap();
        fs::write(page_path(data.path(), *id, 0), vec![0u8; 600 * 1024]).unwrap();
        conn.execute("UPDATE game_manuals SET pages = 1, bytes = ?2, opened_at = ?3 WHERE game_id = ?1", rusqlite::params![id, 600 * 1024, i as i64]).unwrap();
    }
    queries::set_setting(&conn, "manualCap", &serde_json::json!(1)).unwrap();
    enforce_cap(&conn, data.path(), Some(ids[0])).unwrap();
    // A was just opened and stays; B (the oldest of the others) goes, then C, until under 1 MB.
    assert!(page_path(data.path(), ids[0], 0).exists());
    assert!(!page_path(data.path(), ids[1], 0).exists());
    assert!(!page_path(data.path(), ids[2], 0).exists());
    let total: i64 = conn.query_row("SELECT SUM(bytes) FROM game_manuals", [], |r| r.get(0)).unwrap();
    assert_eq!(total, 600 * 1024);
    // They download again when opened: still known, just not on disk.
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM game_manuals WHERE pages IS NULL", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
}

#[test]
fn folders_of_removed_games_are_pruned() {
    let conn = crate::db::test_db();
    let data = tempfile::tempdir().unwrap();
    let id = add_game(&conn, "n64", "Kept", None);
    for d in [id.to_string(), "999".into(), "junk".into()] {
        fs::create_dir_all(data.path().join("manuals").join(d)).unwrap();
    }
    prune(&conn, data.path()).unwrap();
    assert!(dir(data.path(), id).exists());
    assert!(!dir(data.path(), 999).exists());
    assert!(!data.path().join("manuals").join("junk").exists());
}
