use super::*;
fn fixture() -> (tempfile::TempDir, Installed) {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("Game");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("game.exe"), b"fixture, never executed").unwrap();
    let app = Installed {
        id: "123".into(),
        title: "A Game".into(),
        dir: dir.clone(),
        executables: vec![fs::canonicalize(dir.join("game.exe")).unwrap()],
        action: Action::Uri("uplay://launch/123/0".into()),
    };
    (t, app)
}
#[test]
fn importer_preserves_identity_history_and_disconnects_without_uninstall() {
    let (_t, mut app) = fixture();
    let mut c = crate::db::test_db();
    connect(&mut c, "ubisoft", true).unwrap();
    let cancel = AtomicBool::new(false);
    let catalog = |a: Installed| Catalog {
        games: vec![a],
        complete: true,
        ..Default::default()
    };
    assert_eq!(
        import(
            &mut c,
            "ubisoft",
            catalog(app.clone()),
            &cancel,
            &|_, _, _| {}
        )
        .unwrap()
        .added,
        1
    );
    let id: i64 = c
        .query_row("SELECT game_id FROM launcher_games", [], |r| r.get(0))
        .unwrap();
    c.execute(
        "UPDATE games SET favorite=1,play_seconds=91,user_title='Mine' WHERE id=?1",
        [id],
    )
    .unwrap();
    set_tracking(&c, id, Some(&app.executables[0])).unwrap();
    app.title = "New title".into();
    assert_eq!(
        import(
            &mut c,
            "ubisoft",
            catalog(app.clone()),
            &cancel,
            &|_, _, _| {}
        )
        .unwrap()
        .added,
        0
    );
    assert!(game(&c, id).unwrap().unwrap().tracking_exe.is_some());
    connect(&mut c, "ubisoft", false).unwrap();
    assert_eq!(
        c.query_row("SELECT missing FROM games WHERE id=?1", [id], |r| r
            .get::<_, bool>(0))
            .unwrap(),
        false
    );
    assert_eq!(queries::prune_orphans(&c).unwrap(), 0);
    let data: (bool, i64, String) = c
        .query_row(
            "SELECT favorite,play_seconds,user_title FROM games WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(data, (true, 91, "Mine".into()));
    let outside = app.dir.parent().unwrap().join("outside.exe");
    fs::write(&outside, b"fixture").unwrap();
    assert!(set_tracking(&c, id, Some(&outside)).is_err());
    app.dir = app.dir.parent().unwrap().join("Moved");
    import(&mut c, "ubisoft", catalog(app), &cancel, &|_, _, _| {}).unwrap();
    assert!(game(&c, id).unwrap().unwrap().tracking_exe.is_none());
}
#[test]
fn unavailable_records_and_cancel_do_not_mark_games_removed() {
    let (_t, app) = fixture();
    let mut c = crate::db::test_db();
    upsert(&c, "gog", &app).unwrap();
    let cancel = AtomicBool::new(false);
    let incomplete = Catalog {
        warnings: vec!["offline".into()],
        ..Default::default()
    };
    assert_eq!(
        import(&mut c, "gog", incomplete, &cancel, &|_, _, _| {})
            .unwrap()
            .missing,
        0
    );
    cancel.store(true, Ordering::Release);
    assert!(
        import(
            &mut c,
            "gog",
            Catalog {
                complete: true,
                ..Default::default()
            },
            &cancel,
            &|_, _, _| {}
        )
        .unwrap()
        .cancelled
    );
    cancel.store(false, Ordering::Release);
    assert_eq!(
        import(
            &mut c,
            "gog",
            Catalog {
                complete: true,
                ..Default::default()
            },
            &cancel,
            &|_, _, _| {}
        )
        .unwrap()
        .missing,
        1
    );
    assert_eq!(queries::prune_orphans(&c).unwrap(), 0);
    c.execute("DELETE FROM games", []).unwrap();
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM launcher_games", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn epic_routes_all_three_ids_and_rejects_incomplete_or_escaping_targets() {
    let (_t, app) = fixture();
    let mut m = serde_json::json!({"AppName":"artifact","CatalogNamespace":"sandbox","CatalogItemId":"catalog","bIsApplication":true,"bIsExecutable":true,"InstallLocation":app.dir,"LaunchExecutable":"game.exe","DisplayName":"A Game","AppCategories":["games"]});
    let g = readers::epic(&m, None).unwrap().unwrap();
    assert_eq!(g.executables.len(), 1);
    assert_eq!(
        g.action.preview(),
        ["com.epicgames.launcher://apps/sandbox%3Acatalog%3Aartifact?action=launch&silent=true"]
    );
    m["LaunchExecutable"] = "../escape.exe".into();
    assert!(readers::epic(&m, None)
        .unwrap()
        .unwrap()
        .executables
        .is_empty());
    m["bIsIncompleteInstall"] = true.into();
    assert!(readers::epic(&m, None).unwrap().is_none());
    m["bIsIncompleteInstall"] = false.into();
    m["CatalogItemId"] = "evil?install=true".into();
    assert!(readers::epic(&m, None).is_err());
}
#[test]
fn gog_excludes_dlc_and_keeps_client_arguments_out_of_a_shell() {
    let (t, app) = fixture();
    let client = t.path().join("GalaxyClient.exe");
    fs::write(&client, b"fixture").unwrap();
    let mut m = serde_json::json!({"gameId":"123","rootGameId":"123","name":"A Game","playTasks":[{"type":"FileTask","path":"game.exe","isPrimary":true}]});
    let g = readers::gog("123", &app.dir, &client, &m).unwrap().unwrap();
    assert_eq!(g.executables.len(), 1);
    let argv = g.action.preview();
    assert!(argv.contains(&"/gameId=123".into()));
    assert!(argv.contains(&"/command=runGame".into()));
    m["rootGameId"] = "456".into();
    assert!(readers::gog("123", &app.dir, &client, &m)
        .unwrap()
        .is_none());
}
#[test]
fn ea_manifest_requires_installed_state_and_encodes_offer_id() {
    let (_t, app) = fixture();
    let base = format!(
        "https://local/?id=OFB-EAST%3A123&dipinstallpath={}",
        component(&app.dir.to_string_lossy())
    );
    assert!(readers::ea_manifest(&base).unwrap().is_none());
    let g = readers::ea_manifest(&(base.clone() + "&ddinstallalreadycompleted=1"))
        .unwrap()
        .unwrap();
    assert_eq!(
        g.action.preview(),
        ["origin2://game/launch/?offerIds=OFB%2DEAST%3A123&autoDownload=false"]
    );
    assert!(readers::ea_manifest(&(base + "&id=another&ddinstallalreadycompleted=1")).is_err());
}
#[test]
fn product_database_reader_is_bounded_and_skips_unknown_wire_fields() {
    assert!(readers::fields(&[10, 8, 1]).is_err());
    assert!(readers::fields(&[0]).is_err());
    assert!(readers::fields(&[11]).is_err());
    let fields = readers::fields(&[16, 1, 10, 3, b'w', b'o', b'w']).unwrap();
    assert_eq!(fields, vec![(1, b"wow".as_slice())]);
    assert!(identifier("foo&install=1").is_err());
    assert!(identifier("../game").is_err());
}
#[test]
#[ignore = "reads explicitly authorized launcher records only; opt-in by provider"]
fn authorized_local_inventory() {
    let provider = std::env::var("KB_LAUNCHER_TEST_PROVIDER").expect("explicit provider required");
    let catalog = readers::discover(&provider).unwrap();
    let count = catalog.games.len();
    for g in &catalog.games {
        assert!(g.dir.is_dir());
        for exe in &g.executables {
            crate::steam::tracking_file(&g.dir, exe).unwrap();
        }
    }
    let mut conn = crate::db::test_db();
    connect(&mut conn, &provider, true).unwrap();
    let report = import(
        &mut conn,
        &provider,
        catalog,
        &AtomicBool::new(false),
        &|_, _, _| {},
    )
    .unwrap();
    println!(
        "provider={provider} records={count} added={} warnings={}",
        report.added,
        report.warnings.len()
    );
}

#[test]
fn ea_installer_xml_requires_a_content_id_and_keeps_executables_inside_install() {
    let (_t, app) = fixture();
    let bytes=br#"<game><gameTitle>Game &amp; Name</gameTitle><contentID>OFB-EAST:123</contentID><filePath>[INSTALLDIR]\game.exe</filePath></game>"#;
    let game = readers::ea_xml(bytes, &app.dir).unwrap().unwrap();
    assert_eq!(game.title, "Game & Name");
    assert_eq!(game.executables.len(), 1);
    assert!(readers::ea_xml(b"<game><gameTitle>Game</gameTitle></game>", &app.dir).is_err());
    assert!(readers::ea_xml(b"<!DOCTYPE x><game/>", &app.dir).is_err());
}
#[test]
fn battle_product_record_selects_its_exact_wow_variant() {
    fn field(n: u8, data: &[u8]) -> Vec<u8> {
        let mut b = vec![n << 3 | 2];
        let mut len = data.len();
        while len >= 128 {
            b.push((len as u8 & 0x7f) | 0x80);
            len >>= 7;
        }
        b.push(len as u8);
        b.extend(data);
        b
    }
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("WoW");
    fs::create_dir_all(dir.join("_classic_era_")).unwrap();
    fs::write(dir.join("_classic_era_/WowClassic.exe"), b"fixture").unwrap();
    let client = temp.path().join("Battle.net.exe");
    fs::write(&client, b"fixture").unwrap();
    let data = field(1, dir.to_string_lossy().as_bytes());
    let mut record = field(1, b"wow_classic_era");
    record.extend(field(3, &data));
    let catalog = readers::battle(&field(1, &record), &client).unwrap();
    assert!(catalog.warnings.is_empty());
    assert_eq!(catalog.games.len(), 1);
    assert_eq!(catalog.games[0].executables.len(), 1);
    assert!(catalog.games[0]
        .action
        .preview()
        .contains(&"--exec=launch WoW_wow_classic_era".into()));
}
