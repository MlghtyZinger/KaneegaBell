use super::*;
use crate::db;

fn add_game(conn: &Connection, title: &str) -> i64 {
    conn.execute(
        "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at) VALUES ('ps2', ?1, ?1, ?1, ?1, 0)",
        [title],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// Puts games in a Wikidata series: (game, entry qid, year).
fn series(conn: &Connection, qid: i64, name: &str, kind: &str, members: &[(i64, i64, i64)]) {
    conn.execute("INSERT OR REPLACE INTO wd_series (qid, name) VALUES (?1, ?2)", params![qid, name]).unwrap();
    for (game, entry, year) in members {
        conn.execute("INSERT OR REPLACE INTO wd_games (qid, name, year, fetched_at) VALUES (?1, 'x', ?2, 0)", params![entry, year]).unwrap();
        conn.execute("INSERT OR IGNORE INTO wd_game_series (game_qid, series_qid, kind) VALUES (?1, ?2, ?3)", params![entry, qid, kind]).unwrap();
        conn.execute("UPDATE games SET wd_id = ?2, wd_status = 'ok' WHERE id = ?1", params![game, entry]).unwrap();
    }
}

fn find<'a>(list: &'a [Collection], key: &str) -> &'a Collection {
    list.iter().find(|c| c.key == key).unwrap_or_else(|| panic!("{key} missing"))
}

#[test]
fn your_collections_hold_games_in_the_order_you_choose() {
    let mut conn = db::test_db();
    let zelda = add_game(&conn, "Zelda");
    let okami = add_game(&conn, "Okami");
    let ico = add_game(&conn, "Ico");
    conn.execute("UPDATE games SET sort_title = title", []).unwrap();

    let faves = create(&conn, "  Couch co-op  ", Some(zelda)).unwrap();
    set_member(&conn, &faves, okami, true).unwrap();
    set_member(&conn, &faves, ico, true).unwrap();
    set_member(&conn, &faves, ico, true).unwrap(); // twice: still once
    let list = list(&conn).unwrap();
    let c = find(&list, &faves);
    assert_eq!((c.name.as_str(), c.kind.as_str()), ("Couch co-op", "mine"));
    assert_eq!(c.games, vec![zelda, okami, ico], "the order they were added");

    set_game_order(&conn, &faves, "title").unwrap();
    assert_eq!(find(&super::list(&conn).unwrap(), &faves).games, vec![ico, okami, zelda]);
    // Release order: known dates first (LaunchBox, IGDB or Wikidata's year), unknown last.
    conn.execute("INSERT INTO metadata (game_id, title, source, fetched_at, release_date) VALUES (?1, 'x', 'igdb', 0, 1000)", [okami]).unwrap();
    conn.execute("INSERT INTO metadata (game_id, title, source, fetched_at, release_date) VALUES (?1, 'x', 'igdb', 0, 500)", [zelda]).unwrap();
    set_game_order(&conn, &faves, "release").unwrap();
    assert_eq!(find(&super::list(&conn).unwrap(), &faves).games, vec![zelda, okami, ico]);
    assert!(set_game_order(&conn, &faves, "random").is_err());

    // Removing, missing games, renaming, a game in several collections.
    set_member(&conn, &faves, okami, false).unwrap();
    conn.execute("UPDATE games SET missing = 1 WHERE id = ?1", [ico]).unwrap();
    assert_eq!(find(&super::list(&conn).unwrap(), &faves).games, vec![zelda]);
    rename(&conn, &faves, "Friday nights").unwrap();
    assert!(rename(&conn, &faves, "   ").is_err());
    let rpg = create(&conn, "RPGs", Some(zelda)).unwrap();
    let list = super::list(&conn).unwrap();
    assert_eq!(find(&list, &faves).name, "Friday nights");
    assert_eq!(find(&list, &rpg).games, vec![zelda]);

    // Reordering the collections themselves.
    assert_eq!(list.iter().map(|c| c.key.clone()).collect::<Vec<_>>(), vec![faves.clone(), rpg.clone()]);
    move_by(&mut conn, &rpg, -1).unwrap();
    move_by(&mut conn, &rpg, -1).unwrap(); // already first
    let keys: Vec<String> = super::list(&conn).unwrap().into_iter().map(|c| c.key).collect();
    assert_eq!(keys, vec![rpg.clone(), faves.clone()]);

    // Deleting a collection keeps the games; deleting a game empties it from collections.
    let dir = tempfile::tempdir().unwrap();
    delete(&conn, dir.path(), &faves).unwrap();
    assert!(super::list(&conn).unwrap().iter().all(|c| c.key != faves));
    conn.execute("DELETE FROM games WHERE id = ?1", [zelda]).unwrap();
    assert!(find(&super::list(&conn).unwrap(), &rpg).games.is_empty());
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
}

#[test]
fn series_with_two_or_more_games_become_collections() {
    let conn = db::test_db();
    let jak2 = add_game(&conn, "Jak II");
    let jak1 = add_game(&conn, "Jak and Daxter");
    let jak1_eu = add_game(&conn, "Jak and Daxter (Europe)");
    let okami = add_game(&conn, "Okami");
    series(&conn, 1, "Jak and Daxter", "series", &[(jak2, 20, 2003), (jak1, 10, 2001), (jak1_eu, 10, 2001)]);
    series(&conn, 2, "Jak and Daxter", "franchise", &[(jak2, 20, 2003), (jak1, 10, 2001)]);
    series(&conn, 3, "Okami", "series", &[(okami, 30, 2006)]);

    let list = list(&conn).unwrap();
    // One "Jak and Daxter" (the series, not the franchise of the same name),
    // in release order, each game once; Okami alone isn't a collection.
    assert_eq!(list.len(), 1);
    let jak = &list[0];
    assert_eq!((jak.key.as_str(), jak.kind.as_str()), ("s:1", "series"));
    assert_eq!(jak.games, vec![jak1, jak2]);
    assert!(rename(&conn, "s:1", "Mine now").is_err(), "series keep Wikidata's name");

    set_hidden(&conn, "s:1", true).unwrap();
    assert!(super::list(&conn).unwrap()[0].hidden);
    set_hidden(&conn, "s:1", false).unwrap();
    assert!(!super::list(&conn).unwrap()[0].hidden);
}

#[test]
fn logos_can_be_a_game_s_logo_a_cover_or_your_own_image() {
    let conn = db::test_db();
    let dir = tempfile::tempdir().unwrap();
    let game = add_game(&conn, "Jak II");
    let key = create(&conn, "Platformers", Some(game)).unwrap();
    assert_eq!(find(&list(&conn).unwrap(), &key).logo, None, "automatic at first");

    set_logo(&conn, dir.path(), &key, &format!("game:{game}")).unwrap();
    assert_eq!(find(&list(&conn).unwrap(), &key).logo, Some(Logo::Game { id: game }));
    set_logo(&conn, dir.path(), &key, &format!("cover:{game}")).unwrap();
    assert_eq!(find(&list(&conn).unwrap(), &key).logo, Some(Logo::Cover { id: game }));
    assert!(set_logo(&conn, dir.path(), &key, "https://example.com/x.png").is_err());

    // Your own image: copied into KB's folder (the original can go).
    let png = dir.path().join("mine.png");
    std::fs::write(&png, crate::art::images::test_png(1200, 400)).unwrap();
    set_logo_file(&conn, dir.path(), &key, &png).unwrap();
    std::fs::remove_file(&png).unwrap();
    let stored = logo_path(dir.path(), &key).unwrap();
    assert!(stored.is_file());
    assert_eq!(find(&list(&conn).unwrap(), &key).logo, Some(Logo::File { version: 1 }));
    // A WebP works too, and each new image gets a new version.
    let webp_file = dir.path().join("mine.webp");
    let rgba = vec![200u8; 64 * 32 * 4];
    std::fs::write(&webp_file, &*webp::Encoder::from_rgba(&rgba, 64, 32).encode(90.0)).unwrap();
    set_logo_file(&conn, dir.path(), &key, &webp_file).unwrap();
    assert_eq!(find(&list(&conn).unwrap(), &key).logo, Some(Logo::File { version: 2 }));
    set_logo_bytes(&conn, dir.path(), &key, &crate::art::images::test_png(600, 200)).unwrap();
    assert_eq!(find(&list(&conn).unwrap(), &key).logo, Some(Logo::File { version: 3 }));
    let text = dir.path().join("notes.txt");
    std::fs::write(&text, b"not an image").unwrap();
    assert!(set_logo_file(&conn, dir.path(), &key, &text).is_err());

    // Back to automatic removes the copied image; series logos work the same way.
    set_logo(&conn, dir.path(), &key, "auto").unwrap();
    assert!(!stored.exists());
    assert_eq!(look_logo(&conn, &key), None);
    set_logo(&conn, dir.path(), "s:526700", &format!("game:{game}")).unwrap();
    assert_eq!(look_logo(&conn, "s:526700").as_deref(), Some(format!("game:{game}").as_str()));
    assert!(set_logo(&conn, dir.path(), "x:1", "auto").is_err());

    // A background of its own: versioned, removable, and deleted with the collection.
    assert_eq!(find(&list(&conn).unwrap(), &key).hero, None);
    set_hero_bytes(&conn, dir.path(), &key, &crate::art::images::test_png(1600, 600)).unwrap();
    set_hero_bytes(&conn, dir.path(), &key, &crate::art::images::test_png(1600, 600)).unwrap();
    let hero = hero_path(dir.path(), &key).unwrap();
    assert!(hero.is_file());
    assert_eq!(find(&list(&conn).unwrap(), &key).hero, Some(2));
    remove_hero(&conn, dir.path(), &key).unwrap();
    assert!(!hero.exists());
    assert_eq!(find(&list(&conn).unwrap(), &key).hero, None);
    set_hero_bytes(&conn, dir.path(), &key, &crate::art::images::test_png(1600, 600)).unwrap();

    // Deleting the collection removes its images too.
    set_logo_file(&conn, dir.path(), &key, &webp_file).unwrap();
    delete(&conn, dir.path(), &key).unwrap();
    assert!(!stored.exists());
    assert!(!hero.exists());
}

#[test]
fn smart_collections_store_and_validate_their_rules() {
    let conn = db::test_db();
    let a = add_game(&conn, "A");
    let _b = add_game(&conn, "B");

    let key = create_smart(&conn, "  Platformers  ", Rules { genres: vec![3], played: Some("played".into()), ..Default::default() }).unwrap();
    let cols = list(&conn).unwrap();
    let c = find(&cols, &key);
    assert_eq!((c.name.as_str(), c.kind.as_str()), ("Platformers", "smart"));
    assert!(c.games.is_empty(), "smart membership is computed in the UI");
    assert!(c.game_order.is_none());
    assert_eq!(c.rules.as_ref().unwrap().genres, vec![3]);
    assert_eq!(c.rules.as_ref().unwrap().played.as_deref(), Some("played"));

    // A manual collection has no rules and keeps its kind.
    let manual = create(&conn, "Manual", Some(a)).unwrap();
    let cols = list(&conn).unwrap();
    let mc = find(&cols, &manual);
    assert_eq!(mc.kind.as_str(), "mine");
    assert_eq!(mc.rules, None);

    // Updating rules sorts and dedups; nonsense is rejected.
    set_rules(&conn, &key, Rules { genres: vec![5, 3, 5], ..Default::default() }).unwrap();
    assert_eq!(find(&list(&conn).unwrap(), &key).rules.as_ref().unwrap().genres, vec![3, 5]);
    assert!(set_rules(&conn, &key, Rules { genres: vec![0], ..Default::default() }).is_err());
    assert!(set_rules(&conn, &key, Rules { year_from: Some(2000), year_to: Some(1990), ..Default::default() }).is_err());
    assert!(set_rules(&conn, &key, Rules { played: Some("sometimes".into()), ..Default::default() }).is_err());
    assert!(set_rules(&conn, &key, Rules { play_from: Some(500), play_to: Some(100), ..Default::default() }).is_err());
    assert!(create_smart(&conn, "Bad", Rules { genres: vec![-1], ..Default::default() }).is_err());
}
