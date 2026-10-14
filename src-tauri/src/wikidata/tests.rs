use std::sync::Mutex;

use super::*;
use crate::db;
use crate::scanner::normalize;

/// Wikidata from memory: answers the names query for PS2 and the series
/// query, and records every query.
#[derive(Default)]
struct FakeSource {
    queries: Mutex<Vec<String>>,
}

fn row(pairs: &[(&str, &str)]) -> Row {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn e(q: i64) -> String {
    format!("http://www.wikidata.org/entity/Q{q}")
}

impl Source for FakeSource {
    fn select(&self, query: &str) -> Result<Vec<Row>> {
        self.queries.lock().unwrap().push(query.to_string());
        if query.contains("VALUES ?g") {
            let mut out = Vec::new();
            for (g, s, label, kind, ord) in [
                (526700, 1, "Jak and Daxter", "series", "2"),
                (960458, 1, "Jak and Daxter", "series", "3"),
                (633011, 2, "Final Fantasy X subseries", "series", "2"),
                (633011, 3, "Final Fantasy", "franchise", ""),
                (999, 3, "Final Fantasy", "franchise", ""),
            ] {
                if query.contains(&format!("wd:Q{g} ")) || query.contains(&format!("wd:Q{g} }}")) {
                    let mut r = row(&[("g", &e(g)), ("s", &e(s)), ("sl", label), ("kind", kind)]);
                    if !ord.is_empty() {
                        r.insert("ord".into(), ord.into());
                    }
                    out.push(r);
                }
            }
            return Ok(out);
        }
        if query.contains("wd:Q10680") {
            let r = |q: i64, name: &str, alt: &str, y: &str, igdb: &str| {
                let mut v = vec![("g", e(q)), ("name", name.to_string()), ("alt", alt.to_string()), ("y", y.to_string())];
                if !igdb.is_empty() {
                    v.push(("igdb", igdb.to_string()));
                }
                v.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<Row>()
            };
            return Ok(vec![
                r(526700, "Jak II", "0", "2003", "jak-ii"),
                r(526700, "Jak II: Renegade", "1", "2003", "jak-ii"),
                r(960458, "Jak 3", "0", "2004", "jak-3"),
                r(633011, "Final Fantasy X-2", "0", "2003", "final-fantasy-x-2"),
                // Two different games with the same English name.
                r(111, "Tetris Worlds", "0", "2001", ""),
                r(112, "Tetris Worlds", "0", "2002", ""),
                r(999, "Unowned Game", "0", "2005", ""),
            ]);
        }
        Ok(Vec::new())
    }
}

impl FakeSource {
    fn count(&self) -> usize {
        self.queries.lock().unwrap().len()
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
    conn.query_row("SELECT wd_status, wd_id FROM games WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn pass(conn: &mut Connection, source: &FakeSource, force: bool) -> Summary {
    run(conn, source, force, &|_| {}, &AtomicBool::new(false)).unwrap()
}

#[test]
fn a_pass_matches_and_keeps_only_the_library_s_series() {
    let mut conn = db::test_db();
    let jak2 = add_game(&conn, "Jak II (USA)");
    let jak3 = add_game(&conn, "Jak 3 (USA)");
    let ffx2 = add_game(&conn, "Final Fantasy X-2 (USA)");
    let tetris = add_game(&conn, "Tetris Worlds (USA)");
    let stranger = add_game(&conn, "Totally Unknown Game (USA)");

    let source = FakeSource::default();
    let s = pass(&mut conn, &source, false);
    // Names and years for PS2, then one series query for its matches.
    assert_eq!(source.count(), 3);
    assert_eq!((s.platforms, s.matched, s.not_found, s.ambiguous, s.with_series), (1, 3, 1, 1, 3));
    assert_eq!(status_of(&conn, jak2), ("ok".into(), Some(526700)));
    assert_eq!(status_of(&conn, jak3), ("ok".into(), Some(960458)));
    assert_eq!(status_of(&conn, tetris), ("ambiguous".into(), None));
    assert_eq!(status_of(&conn, stranger), ("notfound".into(), None));

    // Each game knows its number, and the series lists the library's games in order.
    let jak = |number: &str| Series {
        name: "Jak and Daxter".into(),
        kind: "series".into(),
        number: Some(number.into()),
        games: vec![jak2, jak3],
    };
    assert_eq!(series_for(&conn, jak2).unwrap(), vec![jak("2")]);
    assert_eq!(series_for(&conn, jak3).unwrap(), vec![jak("3")]);
    let ff: Vec<(String, String, Option<String>)> =
        series_for(&conn, ffx2).unwrap().into_iter().map(|s| (s.name, s.kind, s.number)).collect();
    assert_eq!(
        ff,
        vec![
            ("Final Fantasy X subseries".into(), "series".into(), Some("2".into())),
            ("Final Fantasy".into(), "franchise".into(), None),
        ],
        "series before franchise"
    );
    assert!(series_for(&conn, stranger).unwrap().is_empty());
    // Only the matched entries are stored, not the platform's whole list.
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM wd_games"), 3);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM wd_games WHERE qid = 999"), 0);
    let (name, year): (String, i64) =
        conn.query_row("SELECT name, year FROM wd_games WHERE qid = 526700", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((name.as_str(), year), ("Jak II", 2003), "the main name, not the alternate title");

    // Nothing new: nothing asked. A month later the unmatched games are worth
    // another look; "Update now" asks right away.
    assert!(!wanted(&conn).unwrap());
    pass(&mut conn, &source, false);
    assert_eq!(source.count(), 3);
    queries::set_setting(&conn, STAMP_KEY, &serde_json::json!({ "ps2": queries::now() - RECHECK_SECS })).unwrap();
    assert!(wanted(&conn).unwrap());
    pass(&mut conn, &source, false);
    assert_eq!(source.count(), 6);
    pass(&mut conn, &source, true);
    assert_eq!(source.count(), 9);

    // A new game brings its platform back.
    add_game(&conn, "Jak II - Renegade (Europe)");
    assert!(wanted(&conn).unwrap());
    pass(&mut conn, &source, false);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM games WHERE wd_id = 526700"), 2, "matched by the alternate title");
    // Two copies of Jak II: the series shows each game once (the one looked at).
    let europe: i64 = conn.query_row("SELECT MAX(id) FROM games", [], |r| r.get(0)).unwrap();
    assert_eq!(series_for(&conn, jak2).unwrap()[0].games, vec![jak2, jak3]);
    assert_eq!(series_for(&conn, europe).unwrap()[0].games, vec![europe, jak3]);
    // Release order decides, whatever order the games were added in or numbered.
    conn.execute("UPDATE wd_game_series SET ord = '9' WHERE game_qid = 960458", []).unwrap();
    conn.execute("UPDATE wd_games SET year = 2001 WHERE qid = 960458", []).unwrap();
    assert_eq!(series_for(&conn, jak2).unwrap()[0].games, vec![jak3, jak2]);
    // Same year: the series number settles it.
    conn.execute("UPDATE wd_games SET year = 2003 WHERE qid = 960458", []).unwrap();
    assert_eq!(series_for(&conn, jak2).unwrap()[0].games, vec![jak2, jak3]);

    // Removed games' entries go at the next pass.
    conn.execute("DELETE FROM games WHERE id = ?1", [ffx2]).unwrap();
    pass(&mut conn, &source, true);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM wd_games WHERE qid = 633011"), 0);
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM wd_series WHERE name LIKE 'Final Fantasy%'"), 0);
}

#[test]
fn launchbox_titles_igdb_ids_and_release_years_make_matches_exact() {
    let mut conn = db::test_db();
    // A file name Wikidata doesn't know, but its LaunchBox match does.
    let odd = add_game(&conn, "SLUS-20000");
    conn.execute(
        "INSERT INTO lb_games (db_id, name, release_date, fetched_at) VALUES (7, 'Jak 3', NULL, 0)",
        [],
    )
    .unwrap();
    conn.execute("UPDATE games SET lb_id = 7, lb_status = 'ok' WHERE id = ?1", [odd]).unwrap();
    // Two "Tetris Worlds": the release year decides.
    let tetris = add_game(&conn, "Tetris Worlds (USA)");
    conn.execute("INSERT INTO lb_games (db_id, name, release_date, fetched_at) VALUES (8, 'Tetris Worlds', 1012521600, 0)", [])
        .unwrap(); // 2002-02-01
    conn.execute("UPDATE games SET lb_id = 8, lb_status = 'ok' WHERE id = ?1", [tetris]).unwrap();
    // An IGDB match whose URL name Wikidata lists.
    let igdb = add_game(&conn, "Some Odd Name (USA)");
    conn.execute(
        "INSERT INTO metadata (game_id, title, source, fetched_at, slug) VALUES (?1, 'x', 'igdb', 0, 'final-fantasy-x-2')",
        [igdb],
    )
    .unwrap();

    pass(&mut conn, &FakeSource::default(), false);
    assert_eq!(status_of(&conn, odd), ("ok".into(), Some(960458)));
    assert_eq!(status_of(&conn, tetris), ("ok".into(), Some(112)));
    assert_eq!(status_of(&conn, igdb), ("ok".into(), Some(633011)));

    // Fixing the LaunchBox match looks the game up again.
    relook(&conn, odd).unwrap();
    assert!(wanted(&conn).unwrap());
}

#[test]
fn turned_off_means_no_pass() {
    let conn = db::test_db();
    add_game(&conn, "Jak II (USA)");
    assert!(wanted(&conn).unwrap());
    queries::set_setting(&conn, "wdEnabled", &serde_json::json!(false)).unwrap();
    assert!(!wanted(&conn).unwrap());
}

#[test]
fn results_years_and_ids_parse() {
    let body = br#"{"head":{"vars":["g","name"]},"results":{"bindings":[
        {"g":{"type":"uri","value":"http://www.wikidata.org/entity/Q526700"},"name":{"xml:lang":"en","type":"literal","value":"Jak II"}},
        {"g":{"type":"uri","value":"http://www.wikidata.org/entity/Q1"}}]}}"#;
    let rows = parse_bindings(body).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "Jak II");
    assert!(!rows[1].contains_key("name"));
    assert_eq!(qid(&rows[0]["g"]), Some(526700));
    assert_eq!(qid("http://www.wikidata.org/entity/P400"), None);
    assert!(parse_bindings(b"{}").is_err());

    assert_eq!(year_of(0), 1970);
    assert_eq!(year_of(1_012_521_600), 2002);
    assert_eq!(year_of(1_009_843_199), 2001); // 2001-12-31 23:59:59
    assert_eq!(year_of(-1), 1969);
}

#[test]
fn every_platform_has_a_wikidata_item() {
    for p in PLATFORMS {
        assert!(p.wikidata.is_some_and(|q| q.starts_with('Q')), "{}", p.id);
    }
}

/// Against the real service, on a throwaway library:
/// `cargo test --lib wikidata_live -- --ignored --nocapture`
#[test]
#[ignore]
fn wikidata_live() {
    let mut conn = db::test_db();
    let jak = add_game(&conn, "Jak II (USA)");
    let ff = add_game(&conn, "Final Fantasy X-2 (USA)");
    struct Timed(Web);
    impl Source for Timed {
        fn select(&self, query: &str) -> Result<Vec<Row>> {
            let t = std::time::Instant::now();
            let r = self.0.select(query);
            let first = query.lines().next().unwrap_or("").trim();
            println!("{:?} rows in {:?}: {first}", r.as_ref().map(Vec::len), t.elapsed());
            r
        }
    }
    let t = std::time::Instant::now();
    let s = run(&mut conn, &Timed(Web::new()), false, &|p| println!("{p:?}"), &AtomicBool::new(false)).unwrap();
    println!("{s:?} in {:?}", t.elapsed());
    println!("Jak II: {:?}", series_for(&conn, jak).unwrap());
    println!("FF X-2: {:?}", series_for(&conn, ff).unwrap());
    assert!(series_for(&conn, jak).unwrap().iter().any(|s| s.name.contains("Jak") && s.number.as_deref() == Some("2")));
}
