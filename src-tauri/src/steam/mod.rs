//! Installed Steam library import. Reads local metadata only after explicit connection.
pub mod kv;
use crate::{
    db::queries,
    error::{Error, Result},
    scanner::{normalize, path_key},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone)]
pub struct Installed {
    pub app_id: u32,
    pub name: String,
    pub library: PathBuf,
    pub dir: PathBuf,
    pub size: i64,
    pub executables: Vec<PathBuf>,
}
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Unknown {
    pub app_id: u32,
    pub name: String,
}
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub added: usize,
    pub updated: usize,
    pub missing: usize,
    pub cancelled: bool,
    pub warnings: Vec<String>,
    pub unknown: Vec<Unknown>,
    pub changes: Vec<queries::GameChange>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub app_id: u32,
    pub library_path: PathBuf,
    pub install_dir: PathBuf,
    pub executables: Vec<PathBuf>,
    pub tracking_exe: Option<PathBuf>,
}

pub fn uri(id: u32) -> Result<String> {
    if id == 0 {
        return Err(Error::msg("Invalid Steam App ID."));
    }
    Ok(format!("steam://run/{id}"))
}
fn read(path: &Path, max: u64) -> Result<Vec<u8>> {
    let f = fs::File::open(path)?;
    if f.metadata()?.len() > max {
        return Err(Error::msg("Steam metadata file is too large."));
    }
    let mut bytes = Vec::new();
    f.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(Error::msg("Steam metadata file is too large."));
    }
    Ok(bytes)
}
fn read_text(path: &Path) -> Result<kv::Kv> {
    kv::text(
        std::str::from_utf8(&read(path, 8 * 1024 * 1024)?)
            .map_err(|_| Error::msg("Steam metadata is not UTF-8."))?,
    )
}
/// Reject rooted/drive/parent paths before joining, then reject junction escapes after canonicalization.
pub fn inside(base: &Path, relative: &str) -> Option<PathBuf> {
    let relative = relative.replace('\\', "/");
    if relative.is_empty()
        || relative.contains(':')
        || relative.starts_with('/')
        || relative.split('/').any(|s| s == ".." || s.is_empty())
    {
        return None;
    }
    let p = Path::new(&relative);
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let base = fs::canonicalize(base).ok()?;
    let p = fs::canonicalize(base.join(p)).ok()?;
    let base_key = path_key(&base);
    let path = path_key(&p);
    (path.starts_with(
        &(base_key.trim_end_matches(['/', '\\']).to_owned() + std::path::MAIN_SEPARATOR_STR),
    ))
    .then_some(p)
}
pub fn helper(path: &Path) -> bool {
    if path.components().any(|c| {
        matches!(
            c.as_os_str()
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_str(),
            "easyanticheat" | "battleye" | "_commonredist" | "redistributables" | "crashreporter"
        )
    }) {
        return true;
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    [
        "steam.exe",
        "steamservice.exe",
        "steamwebhelper.exe",
        "launcher",
        "anticheat",
        "anti-cheat",
        "easyanticheat",
        "battleye",
        "beservice",
        "crashreport",
        "crashhandler",
        "crashpad",
        "gamelaunchhelper",
        "reporter",
        "unins",
        "setup",
        "redist",
        "updater",
        "bootstrap",
        "ue4prereq",
    ]
    .iter()
    .any(|s| name.contains(s))
}
pub fn tracking_file(dir: &Path, file: &Path) -> Result<PathBuf> {
    let dir = fs::canonicalize(dir)?;
    let file = fs::canonicalize(file)?;
    let rel = file
        .strip_prefix(&dir)
        .map_err(|_| Error::msg("Choose the game executable inside this game's install folder."))?;
    let checked = inside(&dir, &rel.to_string_lossy())
        .ok_or_else(|| Error::msg("Game executable leaves its install folder."))?;
    if !checked.is_file()
        || !checked
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        || helper(&checked)
    {
        return Err(Error::msg(
            "Choose the game itself, not Steam, a launcher, installer or anti-cheat helper.",
        ));
    }
    Ok(checked)
}
fn libraries(root: &Path) -> Result<Vec<PathBuf>> {
    let root = fs::canonicalize(root)?;
    let mut out = vec![root.clone()];
    let file = [root.join("steamapps/libraryfolders.vdf"), root.join("config/libraryfolders.vdf")].into_iter().find(|p| p.is_file()).ok_or_else(|| Error::msg("Steam's library list was not found. Open Steam once or choose its installation folder."))?;
    let text = read_text(&file)?;
    let folders = text
        .get("libraryfolders")
        .ok_or_else(|| Error::msg("Invalid Steam library list."))?;
    let mut seen = HashSet::from([path_key(&root)]);
    for (key, value) in folders.entries() {
        if key.parse::<u32>().is_err() {
            continue;
        }
        let Some(p) = value.value("path").or_else(|| value.text()) else {
            continue;
        };
        let p = PathBuf::from(p);
        if !p.is_absolute() {
            return Err(Error::msg("Steam library path is not absolute."));
        }
        let canonical = fs::canonicalize(&p).unwrap_or(p);
        if seen.insert(path_key(&canonical)) {
            out.push(canonical);
        }
    }
    Ok(out)
}
fn manifest(file: &Path, library: &Path) -> Result<Option<Installed>> {
    let text = read_text(file)?;
    let state = text
        .get("appstate")
        .ok_or_else(|| Error::msg("Invalid Steam app manifest."))?;
    let app_id = state
        .value("appid")
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|id| *id > 0)
        .ok_or_else(|| Error::msg("Invalid Steam App ID."))?;
    if file.file_name().unwrap_or_default().to_string_lossy() != format!("appmanifest_{app_id}.acf")
    {
        return Err(Error::msg(
            "Steam manifest ID does not match its file name.",
        ));
    }
    let flags = state
        .value("stateflags")
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or_else(|| Error::msg("Steam manifest has no install state."))?;
    if flags & 4 == 0 {
        return Ok(None);
    }
    let dir = inside(
        &library.join("steamapps/common"),
        state.value("installdir").unwrap_or(""),
    )
    .filter(|p| p.is_dir())
    .ok_or_else(|| Error::msg("Steam game folder is missing or outside the library."))?;
    let name = state
        .value("name")
        .filter(|s| !s.trim().is_empty() && s.len() <= 1024)
        .ok_or_else(|| Error::msg("Steam manifest has no game name."))?
        .to_owned();
    let size = state
        .value("sizeondisk")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    Ok(Some(Installed {
        app_id,
        name,
        library: library.to_path_buf(),
        dir,
        size,
        executables: Vec::new(),
    }))
}

pub fn game(conn: &Connection, id: i64) -> Result<Option<Game>> {
    let row:Option<(u32,String,String,String,Option<String>)>=conn.query_row(
        "SELECT app_id,library_path,install_dir,executables,tracking_exe FROM steam_games WHERE game_id=?1",[id],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
    row.map(|(app_id, library, dir, exe, chosen)| {
        Ok(Game {
            app_id,
            library_path: library.into(),
            install_dir: dir.into(),
            executables: serde_json::from_str(&exe)?,
            tracking_exe: chosen.map(PathBuf::from),
        })
    })
    .transpose()
}

/// A stale KB row must not trigger Steam's install workflow. Recheck its own manifest before Play.
pub fn validate_installed(game: &Game) -> Result<()> {
    let file = game
        .library_path
        .join(format!("steamapps/appmanifest_{}.acf", game.app_id));
    let actual = manifest(&file, &game.library_path)?.ok_or_else(|| {
        Error::msg("This Steam game is no longer installed. Scan the Steam library again.")
    })?;
    if actual.app_id != game.app_id || path_key(&actual.dir) != path_key(&game.install_dir) {
        return Err(Error::msg(
            "This Steam installation moved. Scan the Steam library before playing.",
        ));
    }
    Ok(())
}

pub fn set_tracking(conn: &Connection, id: i64, path: Option<&Path>) -> Result<()> {
    let game = game(conn, id)?.ok_or_else(|| Error::msg("This isn't a Steam game."))?;
    let path = path
        .map(|p| tracking_file(&game.install_dir, p))
        .transpose()?;
    conn.execute(
        "UPDATE steam_games SET tracking_exe=?2 WHERE game_id=?1",
        params![id, path.map(|p| p.to_string_lossy().into_owned())],
    )?;
    Ok(())
}
fn upsert(conn: &Connection, app: &Installed) -> Result<bool> {
    let launch = uri(app.app_id)?;
    let old: Option<i64> = conn
        .query_row(
            "SELECT game_id FROM steam_games WHERE app_id=?1",
            [app.app_id],
            |r| r.get(0),
        )
        .optional()?;
    conn.execute("INSERT INTO games(platform_id,title,sort_title,search_key,launch_path,total_size,added_at) VALUES('steam',?1,?2,?3,?4,?5,?6) ON CONFLICT(launch_path) DO UPDATE SET title=excluded.title,sort_title=excluded.sort_title,search_key=excluded.search_key,total_size=excluded.total_size,missing=0", params![app.name, normalize::sort_title(&app.name), normalize::search_key(&app.name), launch, app.size, queries::now()])?;
    let id: i64 = conn.query_row("SELECT id FROM games WHERE launch_path=?1", [launch], |r| {
        r.get(0)
    })?;
    conn.execute("INSERT INTO steam_games(game_id,app_id,library_path,install_dir,executables) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(app_id) DO UPDATE SET library_path=excluded.library_path,install_dir=excluded.install_dir,executables=excluded.executables,tracking_exe=CASE WHEN steam_games.install_dir=excluded.install_dir THEN steam_games.tracking_exe ELSE NULL END", params![id,app.app_id,app.library.to_string_lossy(),app.dir.to_string_lossy(),serde_json::to_string(&app.executables)?])?;
    Ok(old.is_none())
}

#[cfg(test)]
pub fn scan(
    conn: &mut Connection,
    root: &Path,
    data: &Path,
    approved: &[u32],
    cancel: &AtomicBool,
) -> Result<Summary> {
    scan_with_progress(conn, root, data, approved, cancel, &|_, _, _| {})
}

pub fn scan_with_progress(
    conn: &mut Connection,
    root: &Path,
    data: &Path,
    approved: &[u32],
    cancel: &AtomicBool,
    progress: &dyn Fn(usize, usize, &str),
) -> Result<Summary> {
    progress(0, 0, "Discovering libraries");
    let mut summary = Summary::default();
    let mut apps = BTreeMap::new();
    let mut complete = Vec::new();
    for library in libraries(root)? {
        if cancel.load(Ordering::Acquire) {
            summary.cancelled = true;
            summary.changes.clear();
            summary.added = 0;
            summary.updated = 0;
            summary.missing = 0;
            return Ok(summary);
        }
        let entries = match fs::read_dir(library.join("steamapps")) {
            Ok(e) => e,
            Err(_) => {
                summary.warnings.push(format!(
                    "Steam library unavailable: {}. Its games were kept.",
                    library.display()
                ));
                continue;
            }
        };
        let mut valid = true;
        for entry in entries {
            if cancel.load(Ordering::Acquire) {
                summary.cancelled = true;
                summary.changes.clear();
                summary.added = 0;
                summary.updated = 0;
                summary.missing = 0;
                return Ok(summary);
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    valid = false;
                    continue;
                }
            };
            let file = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            match manifest(&file, &library) {
                Ok(Some(app)) => {
                    if apps.contains_key(&app.app_id) {
                        valid = false;
                        summary.warnings.push(format!(
                            "Duplicate Steam App ID {}; keeping the first library.",
                            app.app_id
                        ));
                    } else {
                        apps.insert(app.app_id, app);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    valid = false;
                    summary.warnings.push(format!(
                        "{name}: {e}. This library's missing games were kept."
                    ));
                }
            }
        }
        if valid {
            complete.push(library);
        }
    }
    let ids = apps.keys().copied().collect();
    let info = match read(&root.join("appcache/appinfo.vdf"), 512 * 1024 * 1024)
        .and_then(|b| kv::appinfo(&b, &ids))
    {
        Ok(v) => v,
        Err(e) => {
            summary.warnings.push(format!(
                "Steam game classification unavailable: {e}. Select unclassified games explicitly."
            ));
            BTreeMap::new()
        }
    };
    let tx = conn.transaction()?;
    let total = apps.len();
    for (done, app) in apps.values_mut().enumerate() {
        progress(done, total, &app.name);
        if cancel.load(Ordering::Acquire) {
            summary.cancelled = true;
            summary.changes.clear();
            summary.added = 0;
            summary.updated = 0;
            summary.missing = 0;
            return Ok(summary);
        }
        let metadata = info.get(&app.app_id).and_then(|v| v.get("appinfo"));
        let kind = metadata
            .and_then(|v| v.get("common"))
            .and_then(|v| v.value("type"))
            .map(str::to_ascii_lowercase);
        let known_game = kind
            .as_deref()
            .is_some_and(|s| matches!(s, "game" | "demo"));
        let existing: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM steam_games WHERE app_id=?1)",
            [app.app_id],
            |r| r.get(0),
        )?;
        if kind.is_some() && !known_game {
            continue;
        }
        if !known_game && !approved.contains(&app.app_id) && !existing {
            summary.unknown.push(Unknown {
                app_id: app.app_id,
                name: app.name.clone(),
            });
            continue;
        }
        if let Some(launch) = metadata
            .and_then(|v| v.get("config"))
            .and_then(|v| v.get("launch"))
        {
            for (_, option) in launch.entries() {
                let os = option.get("config").and_then(|v| v.value("oslist"));
                if os.is_some_and(|v| !v.split(',').any(|s| s.trim() == "windows")) {
                    continue;
                }
                if let Some(exe) = option
                    .value("executable")
                    .and_then(|s| inside(&app.dir, s))
                    .and_then(|p| tracking_file(&app.dir, &p).ok())
                {
                    if !app.executables.contains(&exe) {
                        app.executables.push(exe);
                    }
                }
            }
        }
        if upsert(&tx, app)? {
            summary.added += 1;
            summary.changes.push(queries::GameChange {
                kind: "added",
                platform_id: "steam".into(),
                from_platform: None,
                title: app.name.clone(),
                variant: None,
            });
        } else {
            summary.updated += 1;
        }
    }
    for library in complete {
        let mut stmt =
            tx.prepare("SELECT game_id,app_id FROM steam_games WHERE library_path=?1")?;
        let previous = stmt
            .query_map([library.to_string_lossy()], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, u32>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (game_id, app_id) in previous {
            if !apps.contains_key(&app_id) {
                let changed=tx.execute("UPDATE games SET missing=1 WHERE id=?1 AND missing=0 AND running_since IS NULL",[game_id])?;
                summary.missing += changed;
                if changed > 0 {
                    let title = tx.query_row(
                        "SELECT COALESCE(user_title,title) FROM games WHERE id=?1",
                        [game_id],
                        |r| r.get(0),
                    )?;
                    summary.changes.push(queries::GameChange {
                        kind: "hidden",
                        platform_id: "steam".into(),
                        from_platform: None,
                        title,
                        variant: None,
                    });
                }
            }
        }
    }
    if cancel.load(Ordering::Acquire) {
        summary.cancelled = true;
        summary.changes.clear();
        summary.added = 0;
        summary.updated = 0;
        summary.missing = 0;
        return Ok(summary);
    }
    tx.commit()?;
    progress(total, total, "Caching Steam artwork");
    // Cache artwork only for imported rows and never overwrite a selected picture.
    for (done, app) in apps.values().enumerate() {
        progress(done, total, "Caching Steam artwork");
        if cancel.load(Ordering::Acquire) {
            break;
        }
        if let Ok(Some(id)) = conn
            .query_row(
                "SELECT game_id FROM steam_games WHERE app_id=?1",
                [app.app_id],
                |r| r.get::<_, i64>(0),
            )
            .optional()
        {
            if let Err(e) = cache_art(conn, root, data, id, app.app_id) {
                summary.warnings.push(format!("{} artwork: {e}", app.name));
            }
        }
    }
    progress(total, total, "Steam scan finished");
    queries::set_setting(conn, "steamLastScan", &serde_json::to_value(&summary)?)?;
    Ok(summary)
}
fn cached_image(path: &Path) -> Result<Vec<u8>> {
    let bytes = read(path, 16 * 1024 * 1024)?;
    let (width, height) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()?
        .into_dimensions()
        .map_err(|_| Error::msg("Steam cached image format is unsupported."))?;
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || u64::from(width) * u64::from(height) > 16_000_000
    {
        return Err(Error::msg("Steam cached image dimensions are too large."));
    }
    Ok(bytes)
}

fn cache_art(conn: &Connection, root: &Path, data: &Path, id: i64, app: u32) -> Result<()> {
    use crate::art::images::{self, Extra};
    let cache = root.join("appcache/librarycache");
    let image_file = |kind: &str| -> Option<PathBuf> {
        let mut candidates = vec![
            cache.join(format!("{app}_{kind}.jpg")),
            cache.join(format!("{app}_{kind}.png")),
        ];
        if let Ok(entries) = fs::read_dir(cache.join(app.to_string())) {
            let mut hashed = entries
                .flatten()
                .take(256)
                .map(|e| e.path())
                .filter(|p| {
                    let n = p.file_name().unwrap_or_default().to_string_lossy();
                    n.contains(kind) && (n.ends_with(".jpg") || n.ends_with(".png"))
                })
                .collect::<Vec<_>>();
            hashed.sort();
            candidates.extend(hashed);
        }
        candidates.into_iter().find(|p| p.is_file())
    };
    let has_cover: bool = conn.query_row(
        "SELECT art_status='ok' OR art_manual=1 FROM games WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    if !has_cover {
        if let Some(file) = image_file("library_600x900") {
            let image = images::process(&cached_image(&file)?)?;
            images::store(data, id, &image)?;
            queries::set_boxart(
                conn,
                id,
                "steam",
                &app.to_string(),
                "",
                image.width,
                image.height,
                queries::now(),
            )?;
        }
    }
    for (kind, extra, suffix) in [
        ("hero", Extra::Hero, "library_hero"),
        ("logo", Extra::Logo, "logo"),
    ] {
        let has: bool = conn.query_row("SELECT extras_manual=1 OR EXISTS(SELECT 1 FROM artwork WHERE game_id=?1 AND kind=?2 AND selected=1) FROM games WHERE id=?1", params![id,kind], |r|r.get(0))?;
        if !has {
            if let Some(file) = image_file(suffix) {
                let (w, h) = images::store_extra(data, extra, id, &cached_image(&file)?)?;
                queries::set_extra(
                    conn,
                    id,
                    kind,
                    "steam",
                    &app.to_string(),
                    "",
                    w,
                    h,
                    queries::now(),
                )?;
            }
        }
    }
    Ok(())
}

/// Called only from the player-initiated Find Steam action. Does not enumerate games.
pub fn find_root() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        use windows::{
            core::w,
            Win32::System::Registry::{
                RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
                RRF_SUBKEY_WOW6432KEY,
            },
        };
        for (key, sub, value, flags) in [
            (
                HKEY_CURRENT_USER,
                w!("Software\\Valve\\Steam"),
                w!("SteamPath"),
                RRF_RT_REG_SZ,
            ),
            (
                HKEY_LOCAL_MACHINE,
                w!("Software\\Valve\\Steam"),
                w!("InstallPath"),
                RRF_RT_REG_SZ | RRF_SUBKEY_WOW6432KEY,
            ),
        ] {
            let mut buffer = [0u16; 32768];
            let mut bytes = std::mem::size_of_val(&buffer) as u32;
            let status = unsafe {
                RegGetValueW(
                    key,
                    sub,
                    value,
                    flags,
                    None,
                    Some(buffer.as_mut_ptr().cast()),
                    Some(&mut bytes),
                )
            };
            if status.0 == 0 && bytes >= 2 && bytes as usize <= std::mem::size_of_val(&buffer) {
                let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                let path = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
                if path.join("steam.exe").is_file() {
                    return Ok(path);
                }
            }
        }
    }
    Err(Error::msg(
        "Steam was not found. Use Browse to choose its installation folder.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "reads only explicitly authorized local Steam metadata; requires KB_STEAM_TEST_ROOT"]
    fn authorized_local_metadata_smoke() {
        let root = PathBuf::from(
            std::env::var_os("KB_STEAM_TEST_ROOT").expect("explicit Steam root required"),
        );
        let output = tempfile::tempdir().unwrap();
        let mut conn = crate::db::test_db();
        let report = scan(
            &mut conn,
            &root,
            output.path(),
            &[],
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(!report.cancelled);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let ids = conn
            .prepare("SELECT game_id FROM steam_games")
            .unwrap()
            .query_map([], |r| r.get::<_, i64>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        let mut automatic = 0;
        for id in &ids {
            let game = game(&conn, *id).unwrap().unwrap();
            validate_installed(&game).unwrap();
            for path in &game.executables {
                tracking_file(&game.install_dir, path).unwrap();
            }
            automatic += usize::from(!game.executables.is_empty());
        }
        let covers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM games WHERE art_status='ok'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        println!(
            "imported={} automatic_tracking={} manual_tracking={} cached_covers={} unclassified={}",
            ids.len(),
            automatic,
            ids.len() - automatic,
            covers,
            report.unknown.len()
        );
        let again = scan(
            &mut conn,
            &root,
            output.path(),
            &[],
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(again.added, 0);
        assert_eq!(again.updated, report.added);
        assert_eq!(again.missing, 0);
    }
    fn fixture() -> (tempfile::TempDir, Installed) {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().to_path_buf();
        let install = library.join("steamapps/common/Game");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("game.exe"), b"fixture, not executable").unwrap();
        let app = Installed {
            app_id: 123,
            name: "A Game".into(),
            library,
            dir: install,
            size: 12,
            executables: vec![],
        };
        (dir, app)
    }
    #[test]
    fn paths_do_not_escape_and_helpers_are_rejected() {
        let (_dir, app) = fixture();
        for p in [
            "../Game/game.exe",
            "/Game/game.exe",
            "C:/game.exe",
            "Game//game.exe",
        ] {
            assert!(inside(&app.library, p).is_none(), "{p}");
        }
        assert!(tracking_file(&app.dir, &app.dir.join("game.exe")).is_ok());
        fs::write(app.dir.join("launcher.exe"), b"x").unwrap();
        assert!(tracking_file(&app.dir, &app.dir.join("launcher.exe")).is_err());
        assert!(!helper(Path::new("CrashBandicoot4.exe")));
        assert!(helper(Path::new("CrashReportClient.exe")));
        assert!(uri(0).is_err());
        assert_eq!(uri(123).unwrap(), "steam://run/123");
    }
    #[test]
    fn stable_id_preserves_history_and_clears_stale_tracking_after_move() {
        let (_dir, mut app) = fixture();
        let conn = crate::db::test_db();
        let enabled = || {
            conn.query_row("SELECT enabled FROM platforms WHERE id='steam'", [], |r| {
                r.get::<_, bool>(0)
            })
            .unwrap()
        };
        assert!(!enabled(), "Steam starts disconnected");
        conn.execute("UPDATE platforms SET enabled=1 WHERE id='steam'", [])
            .unwrap();
        crate::platforms::seed(&conn).unwrap();
        assert!(enabled(), "startup preserves an established connection");
        assert!(upsert(&conn, &app).unwrap());
        let id: i64 = conn
            .query_row("SELECT game_id FROM steam_games", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "UPDATE games SET favorite=1,play_seconds=99,user_title='My title' WHERE id=?1",
            [id],
        )
        .unwrap();
        set_tracking(&conn, id, Some(&app.dir.join("game.exe"))).unwrap();
        app.dir = app.library.join("moved");
        app.name = "New name".into();
        assert!(!upsert(&conn, &app).unwrap());
        assert!(game(&conn, id).unwrap().unwrap().tracking_exe.is_none());
        let (fav, secs, title): (bool, i64, String) = conn
            .query_row(
                "SELECT favorite,play_seconds,COALESCE(user_title,title) FROM games WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((fav, secs, title), (true, 99, "My title".into()));
    }
    fn write_manifest(app: &Installed) {
        fs::write(
            app.library.join("steamapps/libraryfolders.vdf"),
            format!(
                r#""libraryfolders" {{ "0" {{ "path" "{}" }} }}"#,
                app.library.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        fs::write(app.library.join(format!("steamapps/appmanifest_{}.acf",app.app_id)), format!(r#""AppState" {{ "appid" "{}" "name" "{}" "installdir" "Game" "StateFlags" "4" "SizeOnDisk" "12" }}"#,app.app_id,app.name)).unwrap();
    }
    #[test]
    fn scan_requires_classification_and_distinguishes_uninstall_from_corruption() {
        let (_temp, app) = fixture();
        write_manifest(&app);
        let mut conn = crate::db::test_db();
        let cancel = AtomicBool::new(false);
        let data = app.library.join("kb-cache");
        let unknown = scan(&mut conn, &app.library, &data, &[], &cancel).unwrap();
        assert_eq!((unknown.added, unknown.unknown.len()), (0, 1));
        let progress = std::cell::RefCell::new(Vec::new());
        let added = scan_with_progress(
            &mut conn,
            &app.library,
            &data,
            &[123],
            &cancel,
            &|done, total, _| progress.borrow_mut().push((done, total)),
        )
        .unwrap();
        assert_eq!(added.added, 1);
        assert_eq!(progress.borrow().first(), Some(&(0, 0)));
        assert_eq!(progress.borrow().last(), Some(&(1, 1)));
        conn.execute(
            "UPDATE games SET favorite=1,play_seconds=100 WHERE platform_id='steam'",
            [],
        )
        .unwrap();
        let file = app.library.join("steamapps/appmanifest_123.acf");
        fs::write(&file, "broken").unwrap();
        let bad = scan(&mut conn, &app.library, &data, &[123], &cancel).unwrap();
        assert_eq!(bad.missing, 0);
        assert_eq!(queries::count_games(&conn).unwrap(), 1);
        fs::remove_file(file).unwrap();
        let gone = scan(&mut conn, &app.library, &data, &[123], &cancel).unwrap();
        assert_eq!(gone.missing, 1);
        assert_eq!(queries::prune_orphans(&conn).unwrap(), 0);
        let seconds: i64 = conn
            .query_row(
                "SELECT play_seconds FROM games WHERE platform_id='steam'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(seconds, 100);
        write_manifest(&app);
        assert_eq!(
            scan(&mut conn, &app.library, &data, &[123], &cancel)
                .unwrap()
                .added,
            0
        );
        cancel.store(true, Ordering::Release);
        assert!(
            scan(&mut conn, &app.library, &data, &[123], &cancel)
                .unwrap()
                .cancelled
        );
    }
    #[test]
    fn disconnected_library_does_not_hide_its_games() {
        let (_temp, app) = fixture();
        write_manifest(&app);
        let mut conn = crate::db::test_db();
        upsert(&conn, &app).unwrap();
        let library = app.library.join("unmounted");
        conn.execute(
            "UPDATE steam_games SET library_path=?1",
            [library.to_string_lossy()],
        )
        .unwrap();
        fs::write(
            app.library.join("steamapps/libraryfolders.vdf"),
            format!(
                r#""libraryfolders" {{ "1" {{ "path" "{}" }} }}"#,
                library.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        let report = scan(
            &mut conn,
            &app.library,
            &app.library.join("kb-cache"),
            &[],
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(report.missing, 0);
        assert!(!report.warnings.is_empty());
        assert_eq!(queries::count_games(&conn).unwrap(), 1);
    }
    #[test]
    fn launch_preflight_rechecks_the_specific_manifest() {
        let (_temp, app) = fixture();
        write_manifest(&app);
        let mut conn = crate::db::test_db();
        scan(
            &mut conn,
            &app.library,
            &app.library.join("kb-cache"),
            &[123],
            &AtomicBool::new(false),
        )
        .unwrap();
        let id: i64 = conn
            .query_row("SELECT game_id FROM steam_games", [], |r| r.get(0))
            .unwrap();
        let game = game(&conn, id).unwrap().unwrap();
        assert!(validate_installed(&game).is_ok());
        fs::remove_file(app.library.join("steamapps/appmanifest_123.acf")).unwrap();
        assert!(validate_installed(&game).is_err());
    }
}
