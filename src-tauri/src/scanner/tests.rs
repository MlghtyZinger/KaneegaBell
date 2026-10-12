//! Scanner integration tests over a fake library on disk that mirrors the real
//! one's layout and edge cases (see the survey in the architecture plan).

use super::*;
use std::fs;
use tempfile::TempDir;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn gc_iso(id: &[u8; 6]) -> Vec<u8> {
    let mut b = vec![0u8; 0x400];
    b[..6].copy_from_slice(id);
    b[0x1C..0x20].copy_from_slice(&0xC233_9F3Du32.to_be_bytes());
    b
}

fn wbfs(id: &[u8; 6]) -> Vec<u8> {
    let mut b = vec![0u8; 0x400];
    b[..4].copy_from_slice(b"WBFS");
    b[8] = 9;
    b[0x200..0x206].copy_from_slice(id);
    b
}

fn sega_track() -> Vec<u8> {
    let mut b = vec![0u8; 0x100];
    b[0x10..0x1E].copy_from_slice(b"SEGADISCSYSTEM");
    b
}

fn psx_track() -> Vec<u8> {
    let mut b = vec![0u8; 16 * 2352 + 24 + 0x40];
    let pvd = 16 * 2352 + 24;
    b[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
    b[pvd + 8..pvd + 19].copy_from_slice(b"PLAYSTATION");
    b
}

fn cue(bins: &[&str]) -> Vec<u8> {
    bins.iter()
        .enumerate()
        .map(|(i, b)| format!("FILE \"{b}\" BINARY\r\n  TRACK {:02} MODE2/2352\r\n    INDEX 01 00:00:00\r\n", i + 1))
        .collect::<String>()
        .into_bytes()
}

fn rules() -> Vec<PlatformRule> {
    crate::platforms::PLATFORMS
        .iter()
        .map(|p| PlatformRule { id: p.id.into(), extensions: p.extensions.iter().map(|e| e.to_string()).collect() })
        .collect()
}

fn src(path: &Path, platform: Option<&str>) -> SourceSpec {
    SourceSpec { id: 1, path: path.into(), platform_id: platform.map(Into::into), recursive: true, kind: "files".into() }
}

fn scan(path: &Path, platform: Option<&str>) -> (Vec<Candidate>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut c = scan_files(&src(path, platform), &rules(), &HashMap::new(), &mut warnings);
    c.sort_by(|a, b| a.name.title.cmp(&b.name.title));
    (c, warnings)
}

fn titles(c: &[Candidate]) -> Vec<&str> {
    c.iter().map(|c| c.name.title.as_str()).collect()
}

fn gc_wii_folder() -> TempDir {
    let t = tempfile::tempdir().unwrap();
    let r = t.path();
    write(&r.join("Luigi's Mansion (USA)").join("Luigi's Mansion (USA).iso"), &gc_iso(b"GLME01"));
    let mgs = r.join("Metal gear solid - The Twin Snakes");
    write(&mgs.join("Metal Gear Solid - The Twin Snakes (Disc 1).iso"), &gc_iso(b"GGSEA4"));
    write(&mgs.join("Metal Gear Solid - The Twin Snakes (Disc 2).iso"), &gc_iso(b"GGSEA4"));
    write(&r.join("Super Mario Galaxy").join("Super Mario Galaxy [RMGE01].wbfs"), &wbfs(b"RMGE01"));
    write(&r.join("Legend of Zelda, The - The Wind Waker (USA).iso"), &gc_iso(b"GZLE01"));
    t
}

#[test]
fn gamecube_wii_mixed_folder() {
    let t = gc_wii_folder();
    let (c, warnings) = scan(t.path(), Some("gc"));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        titles(&c),
        ["Luigi's Mansion", "Metal Gear Solid: The Twin Snakes", "Super Mario Galaxy", "The Legend of Zelda: The Wind Waker"]
    );
    let galaxy = &c[2];
    assert_eq!((galaxy.platform.as_str(), galaxy.serial.as_deref()), ("wii", Some("RMGE01")));
    let mgs = &c[1];
    assert_eq!(mgs.platform, "gc");
    assert!(mgs.is_multidisc);
    assert_eq!(mgs.files.len(), 2);
    assert_eq!(mgs.files.iter().map(|f| f.disc).collect::<Vec<_>>(), [Some(1), Some(2)]);
    assert_eq!(c[0].serial.as_deref(), Some("GLME01"));
}

#[test]
fn gamecube_wii_folder_in_auto_mode_uses_headers() {
    let t = gc_wii_folder();
    let (c, warnings) = scan(t.path(), None);
    assert!(warnings.is_empty(), "{warnings:?}");
    let plats: Vec<_> = c.iter().map(|c| c.platform.as_str()).collect();
    assert_eq!(plats, ["gc", "gc", "wii", "gc"]);
}

#[test]
fn ps1_cue_bin_multidisc_and_m3u() {
    let t = tempfile::tempdir().unwrap();
    let r = t.path();
    let ff7 = r.join("Final Fantasy VII (USA) (Disc 1 & 2 & 3)");
    for d in 1..=3 {
        let bin = format!("Final Fantasy VII (USA) (Disc {d}).bin");
        write(&ff7.join(&bin), &psx_track());
        write(&ff7.join(format!("Final Fantasy VII (USA) (Disc {d}).cue")), &cue(&[&bin]));
    }
    let sotn = r.join("Castlevania Rondo Of The Night (v1.7)");
    write(&sotn.join("Castlevania - Symphony of the Night (USA) (Track 1).bin"), &psx_track());
    write(&sotn.join("Castlevania - Symphony of the Night (USA) (Track 2).bin"), &[0; 16]);
    write(
        &sotn.join("Castlevania - Symphony of the Night (USA).cue"),
        &cue(&[
            "Castlevania - Symphony of the Night (USA) (Track 1).bin",
            "Castlevania - Symphony of the Night (USA) (Track 2).bin",
        ]),
    );
    // A user-made playlist wins over disc grouping.
    let gx = r.join("Game X");
    for d in 1..=2 {
        let bin = format!("Game X (Disc {d}).bin");
        write(&gx.join(&bin), &psx_track());
        write(&gx.join(format!("Game X (Disc {d}).cue")), &cue(&[&bin]));
    }
    write(&gx.join("Game X.m3u"), b"Game X (Disc 1).cue\r\nGame X (Disc 2).cue\r\n");
    // A .bin without a cue is still a game on PS1.
    write(&r.join("Loose (USA).bin"), &psx_track());

    let (c, warnings) = scan(r, Some("ps1"));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        titles(&c),
        ["Castlevania: Symphony of the Night", "Final Fantasy VII", "Game X", "Loose"]
    );
    let ff7c = &c[1];
    assert!(ff7c.is_multidisc && ff7c.files.len() == 3);
    assert!(ff7c.launch_path.ends_with("Final Fantasy VII (USA) (Disc 1).cue"));
    assert!(c[2].launch_path.ends_with("Game X.m3u"));
    assert!(!c[2].is_multidisc, "m3u is a single launch target");
    // Track bins count toward the size: cue + track 1 + track 2.
    let cue_len = fs::metadata(sotn.join("Castlevania - Symphony of the Night (USA).cue")).unwrap().len();
    assert_eq!(c[0].total_size, cue_len + psx_track().len() as u64 + 16);
}

#[test]
fn sega_cd_tracks_are_hidden_and_auto_cue_is_sniffed() {
    let t = tempfile::tempdir().unwrap();
    let s = t.path().join("Snatcher (USA)");
    let bins: Vec<String> = (1..=3).map(|i| format!("Snatcher (USA) (Track {i:02}).bin")).collect();
    write(&s.join(&bins[0]), &sega_track());
    write(&s.join(&bins[1]), &[0; 8]);
    write(&s.join(&bins[2]), &[0; 8]);
    write(&s.join("Snatcher (USA).cue"), &cue(&bins.iter().map(String::as_str).collect::<Vec<_>>()));

    let (c, w) = scan(t.path(), Some("segacd"));
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(titles(&c), ["Snatcher"]);

    // Auto mode: .cue could be Sega CD or PS1; the first track decides.
    let (c, w) = scan(t.path(), None);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].platform, "segacd");
}

#[test]
fn romhack_folder_auto_mode_ignores_saves_patches_and_tools() {
    let t = tempfile::tempdir().unwrap();
    let r = t.path();
    write(&r.join("Pokemon - Radical Red.gba"), &[0; 0xC0]);
    write(&r.join("Pokemon - Radical Red.sav"), &[0; 8]);
    write(&r.join("Pokemon - Radical Red.ss1"), &[0; 8]);
    let sg = r.join("v1.0.5_SacredGoldStormSilver");
    write(&sg.join("Pokemon Sacred Gold.nds"), &[0; 0x10]);
    write(&sg.join("Pokemon Sacred Gold.ml1"), &[0; 8]);
    write(&sg.join("Pokemon Sacred Gold2.sav"), &[0; 8]);
    write(&r.join("Aria of Sorrow Alter (U)(Hack).ips"), &[0; 8]);
    write(&r.join("readme.txt"), b"hi");
    write(&r.join("patcher.exe"), b"MZ");
    write(&r.join("guide.pdf"), b"%PDF");

    let (c, w) = scan(r, None);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(titles(&c), ["Pokemon Sacred Gold", "Pokemon: Radical Red"]);
    assert_eq!(c[0].platform, "nds");
    assert_eq!(c[1].platform, "gba");
}

#[test]
fn ambiguous_files_in_auto_mode_are_skipped_with_a_warning() {
    let t = tempfile::tempdir().unwrap();
    write(&t.path().join("Mystery.chd"), &[0; 64]);
    let mut psp = vec![0u8; 0x8100];
    psp[0x8001..0x8006].copy_from_slice(b"CD001");
    psp[0x8008..0x8010].copy_from_slice(b"PSP GAME");
    write(&t.path().join("Persona 2 - Eternal Punishment (English v1.00).iso"), &psp);

    let (c, w) = scan(t.path(), None);
    assert_eq!(titles(&c), ["Persona 2: Eternal Punishment"]);
    assert_eq!(c[0].platform, "psp");
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("Mystery.chd"), "{w:?}");
}

#[test]
fn auto_folder_of_ps2_isos_is_ps2_not_ps1() {
    // The user's report: an Auto source pointed at the PS2 folder showed PS1.
    let t = tempfile::tempdir().unwrap();
    let ps2 = detect::tests::build_disc(
        detect::SectorLayout { size: 2048, data: 0 },
        "PLAYSTATION",
        Some("BOOT2 = cdrom0:\\SCUS_971.24;1\r\n"),
    );
    write(&t.path().join("Jak II (USA)").join("Jak II (USA).iso"), &ps2);
    let (c, w) = scan(t.path(), None);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c[0].platform, "ps2");
    assert_eq!(c[0].serial.as_deref(), Some("SCUS-97124"));
}

#[test]
fn folder_name_breaks_ties_when_headers_cant() {
    let t = tempfile::tempdir().unwrap();
    // A PlayStation disc with no SYSTEM.CNF, and an unreadable CHD: only the
    // folder names can tell.
    let bare = detect::tests::build_disc(detect::SectorLayout { size: 2048, data: 0 }, "PLAYSTATION", None);
    write(&t.path().join("PS2").join("Game A.iso"), &bare);
    write(&t.path().join("PSX").join("Game B.chd"), &[0; 64]);
    write(&t.path().join("Unsorted").join("Game C.chd"), &[0; 64]);
    let (c, w) = scan(t.path(), None);
    let plats: Vec<_> = c.iter().map(|c| (c.name.title.as_str(), c.platform.as_str())).collect();
    assert_eq!(plats, [("Game A", "ps2"), ("Game B", "ps1")]);
    assert_eq!(w.len(), 1, "Unsorted/Game C stays ambiguous: {w:?}");
}

#[test]
fn non_recursive_source_stays_at_top_level() {
    let t = tempfile::tempdir().unwrap();
    write(&t.path().join("Top (USA).sfc"), &[0; 8]);
    write(&t.path().join("sub").join("Deep (USA).sfc"), &[0; 8]);
    let mut s = src(t.path(), Some("snes"));
    s.recursive = false;
    let c = scan_files(&s, &rules(), &HashMap::new(), &mut Vec::new());
    assert_eq!(titles(&c), ["Top"]);
}

#[test]
fn ps3_disc_titles_prefer_redump_folder_names_over_sfo() {
    let t = tempfile::tempdir().unwrap();
    let g = t.path().join("Darkness, The (USA) (En,Fr,De,Es,It)").join("PS3_GAME");
    write(&g.join("PARAM.SFO"), &sfo::build(&[("TITLE", "THE DARKNESS"), ("TITLE_ID", "BLUS30035"), ("CATEGORY", "DG")]));
    write(&g.join("USRDIR").join("EBOOT.BIN"), b"SCE\0");
    // A folder without dump-style tags falls back to the SFO title.
    let g2 = t.path().join("my ps3 rip").join("PS3_GAME");
    write(&g2.join("PARAM.SFO"), &sfo::build(&[("TITLE", "Kane & Lynch 2: Dog Days"), ("TITLE_ID", "BLUS30383")]));
    write(&g2.join("USRDIR").join("EBOOT.BIN"), b"SCE\0");

    let mut c = scan_ps3(&src(t.path(), Some("ps3")));
    c.sort_by(|a, b| a.name.title.cmp(&b.name.title));
    assert_eq!(titles(&c), ["Kane & Lynch 2: Dog Days", "The Darkness"]);
    assert_eq!(c[1].name.region.as_deref(), Some("USA"));
    assert_eq!(c[1].serial.as_deref(), Some("BLUS30035"));
    assert!(c[1].launch_path.ends_with("EBOOT.BIN"));
}

#[test]
fn ps3_hdd0_titles_come_from_sfo() {
    let t = tempfile::tempdir().unwrap();
    let g = t.path().join("NPUA80480");
    write(&g.join("PARAM.SFO"), &sfo::build(&[("TITLE", "inFamous"), ("TITLE_ID", "NPUA80480"), ("CATEGORY", "HG")]));
    write(&g.join("USRDIR").join("EBOOT.BIN"), b"SCE\0");
    let mut s = src(t.path(), Some("ps3"));
    s.kind = "ps3_hdd0".into();
    assert_eq!(titles(&scan_ps3(&s)), ["inFamous"]);
}

/// Read-only report over a real library. Not run by default:
/// `EMU_REAL_SOURCES="E:\Games\GBA=gba;E:\Hacks=auto;E:\RPCS3\dev_hdd0\game=ps3_hdd0" cargo test real_library -- --ignored --nocapture`
#[test]
#[ignore]
fn real_library_report() {
    let spec = std::env::var("EMU_REAL_SOURCES").expect("set EMU_REAL_SOURCES");
    let mut total = 0;
    for (i, entry) in spec.split(';').filter(|s| !s.is_empty()).enumerate() {
        let (path, plat) = entry.rsplit_once('=').expect("path=platform");
        let mut s = src(Path::new(path), (plat != "auto" && plat != "ps3_hdd0").then_some(plat));
        s.id = i as i64;
        let mut warnings = Vec::new();
        let started = Instant::now();
        let mut c = if plat == "ps3" || plat == "ps3_hdd0" {
            if plat == "ps3_hdd0" {
                s.kind = "ps3_hdd0".into();
            }
            scan_ps3(&s)
        } else {
            scan_files(&s, &rules(), &HashMap::new(), &mut warnings)
        };
        c.sort_by(|a, b| a.name.sort_title.cmp(&b.name.sort_title));
        println!("\n== {path} [{plat}] -> {} games in {:?}", c.len(), started.elapsed());
        for g in &c {
            println!(
                "  {:<6} {:<45} {:<28} {:<14} {:<12} {}{}",
                g.platform,
                g.name.title,
                g.name.variant.as_deref().unwrap_or(""),
                g.name.region.as_deref().unwrap_or(""),
                g.serial.as_deref().unwrap_or(""),
                if g.is_multidisc { format!("[{} discs] ", g.files.len()) } else { String::new() },
                g.launch_path.file_name().unwrap().to_string_lossy(),
            );
        }
        for w in &warnings {
            println!("  ! {w}");
        }
        total += c.len();
    }
    println!("\nTOTAL {total}");
}

/// Runs a full scan against a *copy* of a real library.db and prints the
/// summary per platform. Not run by default:
/// `EMU_DB_COPY=C:\path\to\copy.db cargo test real_db_scan -- --ignored --nocapture`
#[test]
#[ignore]
fn real_db_scan() {
    let path = std::env::var("EMU_DB_COPY").expect("set EMU_DB_COPY to a copy of library.db");
    let mut conn = crate::db::open(Path::new(&path)).unwrap();
    crate::db::migrate(&mut conn).unwrap();
    let s = run(&mut conn, &|p| println!("progress: {} {}/{} {}", p.phase, p.done, p.total, p.source)).unwrap();
    println!("summary: {s:#?}");
    let mut stmt = conn
        .prepare("SELECT source_id, platform_id, COUNT(*) FROM games WHERE missing = 0 GROUP BY 1, 2")
        .unwrap();
    for row in stmt.query_map([], |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))).unwrap() {
        println!("{:?}", row.unwrap());
    }
}

// ------------------------------------------------------------ database round-trip

fn db_with_source(path: &Path, platform: Option<&str>) -> Connection {
    let conn = crate::db::test_db();
    queries::add_source(&conn, &path.to_string_lossy(), platform, "files").unwrap();
    conn
}

#[test]
fn rescan_is_incremental_and_flags_missing_games() {
    let t = gc_wii_folder();
    let mut conn = db_with_source(t.path(), Some("gc"));

    let s1 = run(&mut conn, &|_| {}).unwrap();
    assert_eq!((s1.added, s1.updated, s1.missing, s1.games_total), (4, 0, 0, 4));
    assert_eq!(queries::library(&conn).unwrap().len(), 4);

    let s2 = run(&mut conn, &|_| {}).unwrap();
    assert_eq!((s2.added, s2.updated, s2.missing, s2.games_total), (0, 0, 0, 4));

    // Mark a favorite, then remove the file: the row survives as missing.
    let luigi = t.path().join("Luigi's Mansion (USA)").join("Luigi's Mansion (USA).iso");
    let id: i64 = conn
        .query_row("SELECT id FROM games WHERE launch_path = ?1", [luigi.to_string_lossy()], |r| r.get(0))
        .unwrap();
    queries::set_favorite(&conn, id, true).unwrap();
    let bytes = fs::read(&luigi).unwrap();
    fs::remove_file(&luigi).unwrap();
    let s3 = run(&mut conn, &|_| {}).unwrap();
    assert_eq!((s3.missing, s3.games_total), (1, 3));

    // Restored: same row, favorite kept.
    fs::write(&luigi, bytes).unwrap();
    let s4 = run(&mut conn, &|_| {}).unwrap();
    assert_eq!(s4.games_total, 4);
    let fav: bool = conn.query_row("SELECT favorite FROM games WHERE id = ?1", [id], |r| r.get(0)).unwrap();
    assert!(fav);

    // Multi-disc files are recorded.
    let discs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM game_files f JOIN games g ON g.id = f.game_id WHERE g.is_multidisc = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(discs, 2);
}

#[test]
fn unplugged_source_does_not_flag_games_missing() {
    let t = gc_wii_folder();
    let mut conn = db_with_source(t.path(), Some("gc"));
    run(&mut conn, &|_| {}).unwrap();
    conn.execute("UPDATE rom_sources SET path = 'Z:/definitely/not/here'", []).unwrap();
    let s = run(&mut conn, &|_| {}).unwrap();
    assert_eq!(s.games_total, 4);
    assert_eq!(s.warnings.len(), 1);
}

#[test]
fn removing_a_source_hides_but_keeps_games() {
    let t = gc_wii_folder();
    let mut conn = db_with_source(t.path(), Some("gc"));
    run(&mut conn, &|_| {}).unwrap();
    let id: i64 = conn.query_row("SELECT id FROM rom_sources", [], |r| r.get(0)).unwrap();
    queries::remove_source(&conn, id).unwrap();
    assert!(queries::library(&conn).unwrap().is_empty());
    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 4);
}

#[test]
fn disabled_platforms_are_not_scanned_or_listed() {
    let t = gc_wii_folder();
    let mut conn = db_with_source(t.path(), None);
    conn.execute("UPDATE platforms SET enabled = 0 WHERE id = 'wii'", []).unwrap();
    run(&mut conn, &|_| {}).unwrap();
    let lib = queries::library(&conn).unwrap();
    assert!(lib.iter().all(|g| g.platform_id == "gc"));
    assert_eq!(lib.len(), 3);
}
