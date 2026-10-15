//! Thin `#[tauri::command]` layer: argument plumbing only, logic lives in
//! `db::queries`, `scanner` and `launcher`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::art;
use crate::db::{self, queries};
use crate::error::{Error, Result};
use crate::input::{ExitCombo, InputShared};
use crate::launcher::{self, template, Launcher};
use crate::scanner::{self, ps3};
use crate::{achievements, metadata, secrets};
use tauri::Manager;

pub struct AppState {
    db: Mutex<Connection>,
    db_path: PathBuf,
    scanning: Arc<AtomicBool>,
    rescan_pending: Arc<AtomicBool>,
    art_pending: Arc<AtomicBool>,
    meta_pending: Arc<AtomicBool>,
    ra_pending: Arc<AtomicBool>,
    sgdb_pending: Arc<AtomicBool>,
    lb_pending: Arc<AtomicBool>,
    /// The queued LaunchBox pass downloads even if nothing seems to need it.
    lb_force: Arc<AtomicBool>,
    wd_pending: Arc<AtomicBool>,
    /// The queued Wikidata pass asks about every platform ("Update now").
    wd_force: Arc<AtomicBool>,
    scan_cancel: Arc<AtomicBool>,
    pub input: Arc<InputShared>,
    pub launcher: Arc<Launcher>,
}

impl AppState {
    pub fn new(conn: Connection, db_path: PathBuf, input: Arc<InputShared>) -> Self {
        Self {
            db: Mutex::new(conn),
            db_path,
            scanning: Arc::default(),
            rescan_pending: Arc::default(),
            art_pending: Arc::default(),
            meta_pending: Arc::default(),
            ra_pending: Arc::default(),
            sgdb_pending: Arc::default(),
            lb_pending: Arc::default(),
            lb_force: Arc::default(),
            wd_pending: Arc::default(),
            wd_force: Arc::default(),
            scan_cancel: Arc::default(),
            input,
            launcher: Arc::default(),
        }
    }

    /// The app's data folder (library.db, art cache, playlists).
    pub fn data_dir(&self) -> &Path {
        self.db_path.parent().unwrap_or(Path::new("."))
    }

    pub fn db(&self) -> MutexGuard<'_, Connection> {
        // A panic mid-query can't leave SQLite inconsistent, so recover from poison.
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// `\\?\E:\x` -> `E:\x` so stored paths match what the user typed and what
/// `read_dir` returns.
fn clean_path(p: &Path) -> PathBuf {
    let canon = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let s = canon.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        canon
    }
}

// ---------------------------------------------------------------- library

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformSummary {
    id: String,
    name: String,
    short_name: String,
    count: i64,
    /// Typical cover height/width for this platform, if it has covers.
    art_aspect: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenreSummary {
    id: i64,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    platforms: Vec<PlatformSummary>,
    games: Vec<queries::GameRow>,
    /// Genres some game has (from IGDB); empty without IGDB.
    genres: Vec<GenreSummary>,
}

/// Startup payload: platforms with games plus every game row, in one IPC call.
#[tauri::command]
pub fn get_library(state: State<'_, AppState>) -> Result<Library> {
    let conn = state.db();
    let aspects = queries::platform_art_aspects(&conn)?;
    let platforms = queries::platforms(&conn)?
        .into_iter()
        .filter(|p| p.enabled && p.game_count > 0)
        .map(|p| PlatformSummary {
            art_aspect: aspects.get(&p.id).copied(),
            id: p.id,
            name: p.name,
            short_name: p.short_name,
            count: p.game_count,
        })
        .collect();
    let (genre_names, by_game) = queries::genres(&conn)?;
    let mut games = queries::library(&conn)?;
    for g in &mut games {
        if let Some(ids) = by_game.get(&g.id) {
            g.genres.clone_from(ids);
        }
    }
    let genres = genre_names.into_iter().map(|(id, name)| GenreSummary { id, name }).collect();
    Ok(Library { platforms, games, genres })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    #[serde(flatten)]
    detail: queries::GameDetail,
    meta: queries::GameMetadata,
}

/// The main color of a game's cover (`#rrggbb`), to tint its series; None
/// without a cover or for a colorless one.
#[tauri::command]
pub fn cover_accent(state: State<'_, AppState>, id: i64) -> Option<String> {
    art::color::accent(state.data_dir(), id)
}

#[tauri::command]
pub fn get_game(state: State<'_, AppState>, id: i64) -> Result<GameInfo> {
    let conn = state.db();
    Ok(GameInfo { detail: queries::game_detail(&conn, id)?, meta: queries::game_metadata(&conn, id)? })
}

#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, id: i64, favorite: bool) -> Result<()> {
    queries::set_favorite(&state.db(), id, favorite)
}

/// The exact argv a launch would run, for the details panel.
#[tauri::command]
pub fn preview_launch(state: State<'_, AppState>, id: i64) -> Result<Vec<String>> {
    let conn = state.db();
    let game = queries::game_detail(&conn, id)?;
    let cfg = queries::launch_config(&conn, &game.platform_id)?;
    let cores_dir: Option<String> = queries::get_setting(&conn, "retroarchCoresDir")?;
    let playlists = state.data_dir().join("playlists");
    Ok(launcher::prepare(&game, &cfg, cores_dir.as_deref().map(Path::new), &playlists)?.argv)
}

#[tauri::command]
pub fn launch_game(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<launcher::Started> {
    state.launcher.launch(&app, id)
}

/// Same as the controller exit combo: close politely, then force after a grace period.
#[tauri::command]
pub fn exit_game(state: State<'_, AppState>) {
    state.launcher.request_exit();
}

#[tauri::command]
pub fn get_running(state: State<'_, AppState>) -> Option<launcher::Started> {
    state.launcher.running()
}

// ---------------------------------------------------------------- settings

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    emulators: Vec<queries::EmulatorRow>,
    platforms: Vec<queries::PlatformRow>,
    sources: Vec<queries::SourceRow>,
    values: serde_json::Map<String, serde_json::Value>,
    scanning: bool,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    let conn = state.db();
    Ok(Settings {
        emulators: queries::emulators(&conn)?,
        platforms: queries::platforms(&conn)?,
        sources: queries::source_rows(&conn)?,
        values: queries::all_settings(&conn)?,
        scanning: state.scanning.load(Ordering::Relaxed),
    })
}

#[tauri::command]
pub fn update_emulator(
    state: State<'_, AppState>,
    id: String,
    exe_path: Option<String>,
    template_override: Option<String>,
) -> Result<()> {
    if let Some(t) = template_override.as_deref().filter(|t| !t.trim().is_empty()) {
        template::validate(t, id == "retroarch")?;
    }
    let exe_path = exe_path.filter(|p| !p.trim().is_empty()).map(|p| clean_path(Path::new(p.trim())));
    queries::update_emulator(&state.db(), &id, exe_path.map(|p| p.to_string_lossy().into_owned()), template_override)
}

/// Fills in executable paths for emulators found under `folder` and, if unset,
/// the RetroArch cores folder. Returns the ids that were detected.
#[tauri::command]
pub fn detect_emulators(state: State<'_, AppState>, folder: String) -> Result<Vec<String>> {
    let root = clean_path(Path::new(folder.trim()));
    let conn = state.db();
    let found = crate::platforms::detect_emulators(&root);
    for (id, exe) in &found {
        let current = queries::emulators(&conn)?.into_iter().find(|e| e.id == *id);
        let template = current.and_then(|e| e.template_override);
        queries::update_emulator(&conn, id, Some(exe.to_string_lossy().into_owned()), template)?;
        if *id == "retroarch" && queries::get_setting::<String>(&conn, "retroarchCoresDir")?.is_none() {
            if let Some(cores) = exe.parent().map(|d| d.join("cores")).filter(|d| d.is_dir()) {
                queries::set_setting(&conn, "retroarchCoresDir", &cores.to_string_lossy().into_owned().into())?;
            }
        }
    }
    Ok(found.into_iter().map(|(id, _)| id.to_string()).collect())
}

#[tauri::command]
pub fn update_platform(state: State<'_, AppState>, update: queries::PlatformUpdate) -> Result<()> {
    let conn = state.db();
    if let Some(t) = update.template_override.as_deref().filter(|t| !t.trim().is_empty()) {
        let cfg = queries::launch_config(&conn, &update.id)?;
        template::validate(t, cfg.emulator_id == "retroarch")?;
    }
    queries::update_platform(&conn, update)
}

#[tauri::command]
pub fn validate_template(template: String, needs_core: bool) -> Result<()> {
    template::validate(&template, needs_core)
}

#[tauri::command]
pub fn add_source(state: State<'_, AppState>, path: String, platform_id: Option<String>) -> Result<i64> {
    let p = clean_path(Path::new(path.trim()));
    if !p.is_dir() {
        return Err(Error::msg(format!("not a folder: {}", p.display())));
    }
    queries::add_source(&state.db(), &p.to_string_lossy(), platform_id.as_deref(), "files")
}

/// The platform a folder's name suggests ("PS2" -> ps2), to preselect when
/// adding it. Only platforms that are set up and enabled are suggested.
#[tauri::command]
pub fn suggest_platform(state: State<'_, AppState>, path: String) -> Result<Option<String>> {
    let Some(name) = Path::new(path.trim().trim_end_matches(['\\', '/'])).file_name() else { return Ok(None) };
    let Some(id) = scanner::hints::suggest(&name.to_string_lossy()) else { return Ok(None) };
    let enabled = queries::platform_rules(&state.db())?.iter().any(|r| r.id == id);
    Ok(enabled.then(|| id.to_string()))
}

/// Directory listing for the in-app browser. `path` None = drives; a path that
/// doesn't exist opens its nearest existing parent.
#[tauri::command]
pub fn list_dir(path: Option<String>, extensions: Vec<String>) -> Result<crate::fsbrowse::Listing> {
    let dir = path.as_deref().and_then(crate::fsbrowse::nearest_existing_dir);
    crate::fsbrowse::list(dir.as_deref(), &extensions)
}

/// Adds RPCS3's installed-games folder (dev_hdd0/game) as a PS3 source.
#[tauri::command]
pub fn add_rpcs3_installed_source(state: State<'_, AppState>) -> Result<i64> {
    let conn = state.db();
    let exe = queries::emulator_exe(&conn, "rpcs3")?
        .ok_or_else(|| Error::msg("set the RPCS3 executable path first"))?;
    let dir = ps3::hdd0_game_dir(Path::new(&exe))
        .ok_or_else(|| Error::msg("couldn't find RPCS3's dev_hdd0/game folder"))?;
    queries::add_source(&conn, &clean_path(&dir).to_string_lossy(), Some("ps3"), "ps3_hdd0")
}

#[tauri::command]
pub fn update_source(state: State<'_, AppState>, id: i64, platform_id: Option<String>, recursive: bool) -> Result<()> {
    queries::update_source(&state.db(), id, platform_id.as_deref(), recursive)
}

#[tauri::command]
pub fn remove_source(state: State<'_, AppState>, id: i64) -> Result<Vec<queries::GameChange>> {
    let conn = state.db();
    let gone = queries::remove_source(&conn, id)?;
    after_removals(&conn, state.data_dir());
    Ok(gone)
}

/// After games may have left the library: forgets the hidden ones when the
/// user asked for that ("Forget removed games automatically"), and deletes
/// the pictures of games that are gone.
fn after_removals(conn: &Connection, data_dir: &Path) {
    let auto = queries::get_setting::<bool>(conn, "forgetRemovedGames").ok().flatten() == Some(true);
    let r = if auto {
        crate::maintenance::forget_removed(conn, data_dir).map(|n| {
            if n > 0 {
                log::info!("forgot {n} removed games");
            }
        })
    } else {
        crate::maintenance::prune_orphan_art(conn, data_dir)
    };
    if let Err(e) = r {
        log::warn!("cleaning up after removed games: {e}");
    }
}

/// Settings → Storage: how many removed games are kept hidden.
#[tauri::command]
pub fn get_removed(state: State<'_, AppState>) -> Result<crate::maintenance::Removed> {
    crate::maintenance::removed(&state.db())
}

/// "Forget removed games": deletes them for good. Returns how many.
#[tauri::command]
pub fn forget_removed(app: AppHandle, state: State<'_, AppState>) -> Result<usize> {
    let n = crate::maintenance::forget_removed(&state.db(), state.data_dir())?;
    let _ = app.emit("library-changed", ());
    Ok(n)
}

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<serde_json::Value>> {
    queries::get_setting(&state.db(), &key)
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: serde_json::Value) -> Result<()> {
    let value = match (key.as_str(), value) {
        ("retroarchCoresDir", serde_json::Value::String(s)) if !s.trim().is_empty() => {
            serde_json::Value::String(clean_path(Path::new(s.trim())).to_string_lossy().into_owned())
        }
        ("exitCombo", v) if !v.is_null() => {
            let combo: ExitCombo = serde_json::from_value(v)?;
            if combo.buttons.iter().any(|b| crate::input::gamepad::button_from_name(b).is_none()) {
                return Err(Error::msg("unknown button in exit combo"));
            }
            if combo.buttons.len() < 2 {
                return Err(Error::msg("the exit combo needs at least two buttons"));
            }
            state.input.set_combo(combo.clone());
            serde_json::to_value(combo)?
        }
        (_, serde_json::Value::String(s)) if s.trim().is_empty() => serde_json::Value::Null,
        (_, v) => v,
    };
    queries::set_setting(&state.db(), &key, &value)
}

// ---------------------------------------------------------------- background jobs
//
// Scans and artwork fetches share one worker thread, fed by two "pending"
// flags: requests made while it's busy are queued, never dropped, and the two
// jobs never contend for the database. A scan that finds games without art is
// followed by an art pass automatically (unless "autoArt" is off).

/// Stops the running job after the item it's on, and drops queued ones.
/// Finished work is kept; an interrupted scan leaves its folder unchanged.
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) {
    state.rescan_pending.store(false, Ordering::Release);
    state.art_pending.store(false, Ordering::Release);
    state.meta_pending.store(false, Ordering::Release);
    state.ra_pending.store(false, Ordering::Release);
    state.sgdb_pending.store(false, Ordering::Release);
    state.lb_pending.store(false, Ordering::Release);
    state.wd_pending.store(false, Ordering::Release);
    state.scan_cancel.store(true, Ordering::Release);
}

/// Queues a library scan (followed by an art pass for new games).
/// Emits `scan-progress`, then `scan-finished` / `scan-failed`.
#[tauri::command]
pub fn start_scan(app: AppHandle, state: State<'_, AppState>) -> bool {
    state.rescan_pending.store(true, Ordering::Release);
    kick_worker(app, &state)
}

/// Queues a box-art pass for games without art.
/// Emits `art-progress`, then `art-finished` / `art-failed`.
#[tauri::command]
pub fn fetch_art(app: AppHandle, state: State<'_, AppState>) -> bool {
    state.art_pending.store(true, Ordering::Release);
    kick_worker(app, &state)
}

/// Which picture a picker works on: "cover" (default), or "hero" / "logo"
/// (TV Mode's background and logo, from SteamGridDB).
fn extra_kind(kind: Option<&str>) -> Result<Option<art::images::Extra>> {
    match kind.unwrap_or("cover") {
        "cover" => Ok(None),
        "hero" => Ok(Some(art::images::Extra::Hero)),
        "logo" => Ok(Some(art::images::Extra::Logo)),
        other => Err(Error::msg(format!("unknown picture kind {other}"))),
    }
}

/// Options for the "Change cover" picker; `query` searches the platform's
/// whole cover set by name (and SteamGridDB, with a key).
#[tauri::command]
pub async fn cover_candidates(app: AppHandle, id: i64, query: Option<String>, kind: Option<String>) -> Result<Vec<art::Candidate>> {
    // May download a cover index: keep it off the UI thread and the shared DB lock.
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let conn = db::open(&state.db_path)?;
        let http = art::http::Http::new();
        let key = sgdb_key()?;
        let sgdb = key.map(|k| art::sgdb::Sgdb::new(&http, k));
        match extra_kind(kind.as_deref())? {
            None => art::candidates(&conn, &http, sgdb.as_ref(), id, query.as_deref()),
            Some(extra) => {
                // LaunchBox's pictures for the matched game, then SteamGridDB's.
                let searching = query.as_deref().is_some_and(|q| !q.trim().is_empty());
                let mut out = if searching { Vec::new() } else { crate::launchbox::picker(&conn, id, Some(extra))? };
                if let Some(sgdb) = sgdb {
                    let kind = match extra {
                        art::images::Extra::Hero => art::sgdb::Kind::Hero,
                        art::images::Extra::Logo => art::sgdb::Kind::Logo,
                    };
                    match art::extras::candidates(&conn, &sgdb, id, kind, query.as_deref()) {
                        Ok(c) => out.extend(c),
                        Err(e) if out.is_empty() => return Err(e),
                        Err(e) => log::warn!("SteamGridDB pictures: {e}"),
                    }
                } else if out.is_empty() {
                    return Err(Error::msg(
                        "LaunchBox has no pictures of this kind for this game. Add a SteamGridDB key in Settings → Game info for more.",
                    ));
                }
                Ok(out)
            }
        }
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

/// Sets a cover (or, with `kind`, a hero/logo) from the picker, or a cover
/// from an image file the user picked (`source` "file", `url` = path).
#[tauri::command]
pub async fn set_cover(app: AppHandle, id: i64, source: String, name: String, url: String, kind: Option<String>) -> Result<()> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let conn = db::open(&state.db_path)?;
        let http = art::http::Http::new();
        match extra_kind(kind.as_deref())? {
            None => art::apply_choice(&conn, state.data_dir(), &http, id, &source, &name, &url),
            Some(extra) => art::extras::apply(&conn, state.data_dir(), &http, id, extra, &source, &name, &url),
        }
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// Removes a game's hero or logo for good (TV Mode then uses the cover).
#[tauri::command]
pub fn remove_extra(app: AppHandle, state: State<'_, AppState>, id: i64, kind: String) -> Result<()> {
    let extra = extra_kind(Some(&kind))?.ok_or_else(|| Error::msg("use remove_cover for covers"))?;
    art::extras::remove(&state.db(), state.data_dir(), id, extra)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// Removes a game's cover for good (shows the title card).
#[tauri::command]
pub fn remove_cover(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<()> {
    art::remove(&state.db(), state.data_dir(), id)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// Forgets one game's art and fetches it again.
#[tauri::command]
pub fn refresh_art(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<bool> {
    art::reset(&state.db(), state.data_dir(), id)?;
    state.art_pending.store(true, Ordering::Release);
    Ok(kick_worker(app, &state))
}

/// Covers left to fetch from last time (offline, interrupted, or a new cover
/// source): fetch them shortly after startup, out of the way of the first paint.
pub fn resume_art_on_startup(app: AppHandle) {
    let _ = std::thread::Builder::new().name("art-resume".into()).spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let state = app.state::<AppState>();
        let pending = (|| -> Result<(bool, bool, bool, bool, bool, bool)> {
            let conn = state.db();
            let auto = queries::get_setting::<bool>(&conn, "autoArt")?.unwrap_or(true);
            let art = auto && queries::count_games_needing_art(&conn, queries::now())? > 0;
            let lb = crate::launchbox::wanted(&conn)? || crate::launchbox::check_due(&conn)?;
            let wd = crate::wikidata::wanted(&conn)?;
            Ok((art, meta_wanted(&conn)?, ra_wanted(&conn)?, sgdb_wanted(&conn)?, lb, wd))
        })();
        let (art, meta, ra, sgdb, lb, wd) = pending.unwrap_or_default();
        if art || meta || ra || sgdb || lb || wd {
            state.lb_pending.fetch_or(lb, Ordering::AcqRel);
            state.wd_pending.fetch_or(wd, Ordering::AcqRel);
            state.art_pending.fetch_or(art, Ordering::AcqRel);
            state.meta_pending.fetch_or(meta, Ordering::AcqRel);
            state.ra_pending.fetch_or(ra, Ordering::AcqRel);
            state.sgdb_pending.fetch_or(sgdb, Ordering::AcqRel);
            kick_worker(app.clone(), &state);
        }
    });
}

/// IGDB is set up, automatic details are on, and some game needs them.
fn meta_wanted(conn: &Connection) -> Result<bool> {
    Ok(queries::get_setting::<bool>(conn, "autoMeta")?.unwrap_or(true)
        && igdb_login()?.is_some()
        && queries::count_games_needing_metadata(conn, queries::now())? > 0)
}

/// Starts the worker thread unless it's already running (it then picks up the
/// pending flags itself). Returns false only if the thread couldn't start.
fn kick_worker(app: AppHandle, state: &AppState) -> bool {
    if state.scanning.swap(true, Ordering::AcqRel) {
        return true;
    }
    let busy = state.scanning.clone();
    let scan_pending = state.rescan_pending.clone();
    let art_pending = state.art_pending.clone();
    let meta_pending = state.meta_pending.clone();
    let ra_pending = state.ra_pending.clone();
    let sgdb_pending = state.sgdb_pending.clone();
    let lb_pending = state.lb_pending.clone();
    let lb_force = state.lb_force.clone();
    let wd_pending = state.wd_pending.clone();
    let wd_force = state.wd_force.clone();
    let cancel = state.scan_cancel.clone();
    let db_path = state.db_path.clone();
    let data_dir = state.data_dir().to_path_buf();
    std::thread::Builder::new()
        .name("library-jobs".into())
        .spawn(move || loop {
            cancel.store(false, Ordering::Release);
            // Order: scan, LaunchBox (it decides covers, so before them),
            // Wikidata (LaunchBox's titles help it match), covers, details,
            // SteamGridDB extras (after covers, so it knows which games still
            // lack one), achievements.
            if scan_pending.swap(false, Ordering::AcqRel) {
                let next = run_scan(&app, &db_path, &cancel);
                lb_pending.fetch_or(next.lb, Ordering::AcqRel);
                wd_pending.fetch_or(next.wd, Ordering::AcqRel);
                art_pending.fetch_or(next.art, Ordering::AcqRel);
                meta_pending.fetch_or(next.meta, Ordering::AcqRel);
                ra_pending.fetch_or(next.ra, Ordering::AcqRel);
                sgdb_pending.fetch_or(next.sgdb, Ordering::AcqRel);
            } else if lb_pending.swap(false, Ordering::AcqRel) {
                let force = lb_force.swap(false, Ordering::AcqRel);
                art_pending.fetch_or(run_lb(&app, &db_path, &data_dir, force, &cancel), Ordering::AcqRel);
                // New LaunchBox matches give Wikidata better titles.
                let wd = db::open(&db_path).and_then(|c| crate::wikidata::wanted(&c)).unwrap_or(false);
                wd_pending.fetch_or(wd, Ordering::AcqRel);
            } else if wd_pending.swap(false, Ordering::AcqRel) {
                run_wd(&app, &db_path, wd_force.swap(false, Ordering::AcqRel), &cancel);
            } else if art_pending.swap(false, Ordering::AcqRel) {
                run_art(&app, &db_path, &data_dir, &cancel);
            } else if meta_pending.swap(false, Ordering::AcqRel) {
                // Matches can bring covers (IGDB) for games that had none.
                art_pending.fetch_or(run_meta(&app, &db_path, &cancel), Ordering::AcqRel);
            } else if sgdb_pending.swap(false, Ordering::AcqRel) {
                run_sgdb(&app, &db_path, &data_dir, &cancel);
            } else if ra_pending.swap(false, Ordering::AcqRel) {
                run_ra(&app, &db_path, &cancel);
            } else {
                busy.store(false, Ordering::Release);
                // A request may have slipped in after the checks above.
                let more = scan_pending.load(Ordering::Acquire)
                    || art_pending.load(Ordering::Acquire)
                    || meta_pending.load(Ordering::Acquire)
                    || ra_pending.load(Ordering::Acquire)
                    || sgdb_pending.load(Ordering::Acquire)
                    || lb_pending.load(Ordering::Acquire)
                    || wd_pending.load(Ordering::Acquire);
                if !more || busy.swap(true, Ordering::AcqRel) {
                    break;
                }
            }
        })
        .map(|_| true)
        .unwrap_or_else(|e| {
            log::error!("could not start the library worker: {e}");
            state.scanning.store(false, Ordering::Release);
            false
        })
}

/// Emits at most a few events per second (plus the final one).
fn throttled(min_ms: u128) -> impl Fn(bool) -> bool {
    let last = std::cell::Cell::new(None::<std::time::Instant>);
    move |force| {
        let due = force || last.get().is_none_or(|t| t.elapsed().as_millis() >= min_ms);
        if due {
            last.set(Some(std::time::Instant::now()));
        }
        due
    }
}

/// What should run after a scan.
#[derive(Default)]
struct FollowUp {
    lb: bool,
    wd: bool,
    art: bool,
    meta: bool,
    ra: bool,
    sgdb: bool,
}

/// Runs one scan.
fn run_scan(app: &AppHandle, db_path: &Path, cancel: &AtomicBool) -> FollowUp {
    let due = throttled(100);
    let result = db::open(db_path).and_then(|mut conn| {
        let summary = scanner::run(
            &mut conn,
            &|p| {
                if due(p.phase == "done") {
                    let _ = app.emit("scan-progress", &p);
                }
            },
            cancel,
        )?;
        if !summary.cancelled {
            after_removals(&conn, db_path.parent().unwrap_or(Path::new(".")));
        }
        let auto_art = queries::get_setting::<bool>(&conn, "autoArt")?.unwrap_or(true);
        let needs_art = queries::count_games_needing_art(&conn, queries::now())? > 0;
        let next = FollowUp {
            lb: crate::launchbox::wanted(&conn).unwrap_or(false),
            wd: crate::wikidata::wanted(&conn).unwrap_or(false),
            art: auto_art && needs_art,
            meta: meta_wanted(&conn).unwrap_or(false),
            ra: ra_wanted(&conn).unwrap_or(false),
            sgdb: sgdb_wanted(&conn).unwrap_or(false),
        };
        Ok((summary, next))
    });
    match result {
        Ok((summary, next)) => {
            log::info!("scan finished: {summary:?}");
            let _ = app.emit("scan-finished", &summary);
            if summary.cancelled {
                FollowUp::default()
            } else {
                next
            }
        }
        Err(e) => {
            log::error!("scan failed: {e}");
            let _ = app.emit("scan-failed", e.to_string());
            FollowUp::default()
        }
    }
}

// ---------------------------------------------------------------- IGDB

const IGDB_ID: &str = "igdb-client-id";
const IGDB_SECRET: &str = "igdb-client-secret";
const IGDB_TOKEN: &str = "igdb-token";

/// The user's IGDB keys and saved token, if they've added them.
fn igdb_login() -> Result<Option<(metadata::igdb::Credentials, Option<metadata::igdb::Token>)>> {
    let (Some(client_id), Some(client_secret)) = (secrets::get(IGDB_ID)?, secrets::get(IGDB_SECRET)?) else {
        return Ok(None);
    };
    let token = secrets::get(IGDB_TOKEN)?.and_then(|t| serde_json::from_str(&t).ok());
    Ok(Some((metadata::igdb::Credentials { client_id, client_secret }, token)))
}

/// Runs `f` with an IGDB client, then saves the token if it changed.
fn with_igdb<T>(f: impl FnOnce(&metadata::igdb::Igdb) -> Result<T>) -> Result<T> {
    let (creds, token) =
        igdb_login()?.ok_or_else(|| Error::msg("Add your IGDB keys in Settings → Services first"))?;
    let http = art::http::Http::new();
    let igdb = metadata::igdb::Igdb::new(&http, creds, token.clone());
    let out = f(&igdb);
    if let Some(t) = igdb.token().filter(|t| Some(t) != token.as_ref()) {
        if let Err(e) = secrets::set(IGDB_TOKEN, &serde_json::to_string(&t)?) {
            log::warn!("couldn't save the IGDB token: {e}");
        }
    }
    out
}

/// Runs one IGDB pass. Returns whether a cover pass should follow.
fn run_meta(app: &AppHandle, db_path: &Path, cancel: &AtomicBool) -> bool {
    let due = throttled(250);
    let reload_due = throttled(2000);
    let result = db::open(db_path).and_then(|mut conn| {
        with_igdb(|igdb| {
            metadata::run(
                &mut conn,
                igdb,
                &|p| {
                    if due(p.done == p.total) {
                        let _ = app.emit("meta-progress", &p);
                    }
                    if p.matched > 0 && reload_due(false) {
                        let _ = app.emit("library-changed", ());
                    }
                },
                cancel,
            )
        })
    });
    match result {
        Ok(summary) => {
            let _ = app.emit("meta-finished", &summary);
            summary.covers > 0 && auto_art(db_path)
        }
        Err(e) => {
            log::error!("IGDB details failed: {e}");
            let _ = app.emit("meta-failed", e.to_string());
            false
        }
    }
}

fn auto_art(db_path: &Path) -> bool {
    db::open(db_path)
        .and_then(|conn| queries::get_setting::<bool>(&conn, "autoArt"))
        .map(|v| v.unwrap_or(true))
        .unwrap_or(true)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Services {
    /// IGDB keys are saved.
    igdb: bool,
    /// The saved Client ID, shortened (it isn't secret, but no need to show it all).
    igdb_client_id: Option<String>,
    /// The RetroAchievements username, when RA is set up.
    ra_user: Option<String>,
    /// A SteamGridDB key is saved.
    sgdb: bool,
}

#[tauri::command]
pub fn get_services() -> Result<Services> {
    let id = secrets::get(IGDB_ID)?;
    let has_secret = secrets::get(IGDB_SECRET)?.is_some();
    Ok(Services {
        igdb: id.is_some() && has_secret,
        igdb_client_id: id.map(|s| format!("{}…", s.chars().take(6).collect::<String>())),
        ra_user: ra_login()?.map(|c| c.user),
        sgdb: sgdb_key()?.is_some(),
    })
}

/// Checks the keys with Twitch (by getting a token), then saves them and
/// fetches details for the library.
#[tauri::command]
pub async fn set_igdb_keys(app: AppHandle, client_id: String, client_secret: String) -> Result<()> {
    let (client_id, client_secret) = (client_id.trim().to_string(), client_secret.trim().to_string());
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(Error::msg("Enter both the Client ID and the Client Secret"));
    }
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        let http = art::http::Http::new();
        let creds = metadata::igdb::Credentials { client_id: client_id.clone(), client_secret: client_secret.clone() };
        let token = metadata::igdb::Igdb::new(&http, creds, None).fetch_token()?;
        secrets::set(IGDB_ID, &client_id)?;
        secrets::set(IGDB_SECRET, &client_secret)?;
        secrets::set(IGDB_TOKEN, &serde_json::to_string(&token)?)?;
        Ok(())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    let state = app.state::<AppState>();
    state.meta_pending.store(true, Ordering::Release);
    kick_worker(app.clone(), &state);
    Ok(())
}

/// Forgets the IGDB keys (details already fetched stay).
#[tauri::command]
pub fn remove_igdb_keys() -> Result<()> {
    for name in [IGDB_ID, IGDB_SECRET, IGDB_TOKEN] {
        secrets::delete(name)?;
    }
    Ok(())
}

/// Queues an IGDB pass for games without details.
/// Emits `meta-progress`, then `meta-finished` / `meta-failed`.
#[tauri::command]
pub fn fetch_metadata(app: AppHandle, state: State<'_, AppState>) -> bool {
    state.meta_pending.store(true, Ordering::Release);
    kick_worker(app, &state)
}

/// "Fix match": IGDB entries for `query` (default: the game's title).
#[tauri::command]
pub async fn igdb_search(app: AppHandle, id: i64, query: Option<String>) -> Result<Vec<metadata::Candidate>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (title, platform): (String, Option<i64>) = state.db().query_row(
            "SELECT g.title, p.igdb_platform FROM games g JOIN platforms p ON p.id = g.platform_id WHERE g.id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let q = query.map(|q| q.trim().to_string()).filter(|q| !q.is_empty()).unwrap_or(title);
        with_igdb(|igdb| metadata::search(igdb, &q, platform))
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

/// Links a game to the IGDB entry picked in "Fix match".
#[tauri::command]
pub async fn set_igdb_match(app: AppHandle, id: i64, igdb_id: i64) -> Result<()> {
    let app2 = app.clone();
    let new_cover = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let conn = db::open(&state.db_path)?;
        with_igdb(|igdb| metadata::apply_match(&conn, igdb, id, igdb_id))
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    if new_cover {
        let state = app.state::<AppState>();
        state.art_pending.store(true, Ordering::Release);
        kick_worker(app.clone(), &state);
    }
    let _ = app.emit("library-changed", ());
    Ok(())
}

// ---------------------------------------------------------------- SteamGridDB

const SGDB_KEY: &str = "sgdb-key";

fn sgdb_key() -> Result<Option<String>> {
    secrets::get(SGDB_KEY)
}

/// A SteamGridDB key is saved and some game hasn't been looked up.
fn sgdb_wanted(conn: &Connection) -> Result<bool> {
    Ok(sgdb_key()?.is_some() && queries::count_games_needing_sgdb(conn, queries::now())? > 0)
}

/// Finds games on SteamGridDB and fetches their heroes, logos and missing covers.
/// Emits `sgdb-progress`, then `sgdb-finished` / `sgdb-failed`.
fn run_sgdb(app: &AppHandle, db_path: &Path, data_dir: &Path, cancel: &AtomicBool) {
    let due = throttled(250);
    let reload_due = throttled(3000);
    let result = (|| {
        let key = sgdb_key()?.ok_or_else(|| Error::msg("SteamGridDB isn't set up"))?;
        let http = art::http::Http::new();
        let sgdb = art::sgdb::Sgdb::new(&http, key);
        let mut conn = db::open(db_path)?;
        art::extras::run(
            &mut conn,
            data_dir,
            &sgdb,
            &http,
            &|p| {
                if due(p.done == p.total) {
                    let _ = app.emit("sgdb-progress", p);
                }
                if p.matched > 0 && reload_due(false) {
                    let _ = app.emit("library-changed", ());
                }
            },
            cancel,
        )
    })();
    match result {
        Ok(summary) => {
            log::info!("SteamGridDB pass: {summary:?}");
            let _ = app.emit("sgdb-finished", &summary);
            let _ = app.emit("library-changed", ());
        }
        Err(e) => {
            log::error!("SteamGridDB pass failed: {e}");
            let _ = app.emit("sgdb-failed", e.to_string());
        }
    }
}

/// Checks the key with SteamGridDB (one search), saves it, and starts a pass.
#[tauri::command]
pub async fn set_sgdb_key(app: AppHandle, key: String) -> Result<()> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err(Error::msg("Enter your SteamGridDB API key"));
    }
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        let http = art::http::Http::new();
        art::sgdb::Sgdb::new(&http, key.clone()).search("Mario")?;
        secrets::set(SGDB_KEY, &key)
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    let state = app.state::<AppState>();
    state.sgdb_pending.store(true, Ordering::Release);
    kick_worker(app.clone(), &state);
    Ok(())
}

/// Forgets the SteamGridDB key (pictures already fetched stay).
#[tauri::command]
pub fn remove_sgdb_key() -> Result<()> {
    secrets::delete(SGDB_KEY)
}

/// Queues a SteamGridDB pass for games not looked up yet.
#[tauri::command]
pub fn fetch_extras(app: AppHandle, state: State<'_, AppState>) -> bool {
    state.sgdb_pending.store(true, Ordering::Release);
    kick_worker(app, &state)
}

// ---------------------------------------------------------------- LaunchBox

/// One LaunchBox pass. Emits `lb-progress`, then `lb-finished` / `lb-failed`.
/// Returns true when covers were queued for the art pass.
fn run_lb(app: &AppHandle, db_path: &Path, data_dir: &Path, force: bool, cancel: &AtomicBool) -> bool {
    let due = throttled(250);
    let result = (|| {
        let mut conn = db::open(db_path)?;
        if !crate::launchbox::enabled(&conn)? {
            return Ok(None);
        }
        let http = art::http::Http::new();
        let web = crate::launchbox::Web::new();
        crate::launchbox::run(
            &mut conn,
            data_dir,
            &web,
            &http,
            force,
            &|p| {
                if due(p.done == p.total) {
                    let _ = app.emit("lb-progress", &p);
                }
            },
            cancel,
        )
        .map(Some)
    })();
    match result {
        Ok(None) => false,
        Ok(Some(summary)) => {
            let _ = app.emit("lb-finished", &summary);
            let _ = app.emit("library-changed", ());
            summary.covers_queued > 0
        }
        Err(e) => {
            log::error!("LaunchBox pass failed: {e}");
            let _ = app.emit("lb-failed", e.to_string());
            false
        }
    }
}

/// Settings → Game info: LaunchBox's status.
#[tauri::command]
pub fn lb_status(state: State<'_, AppState>) -> Result<crate::launchbox::Status> {
    crate::launchbox::status(&state.db())
}

/// "Update now" (`force`), or a pass for games not looked up yet.
#[tauri::command]
pub fn fetch_launchbox(app: AppHandle, state: State<'_, AppState>, force: bool) -> bool {
    state.lb_force.fetch_or(force, Ordering::AcqRel);
    state.lb_pending.store(true, Ordering::Release);
    kick_worker(app, &state)
}

/// Turns LaunchBox on or off (on: a pass starts).
#[tauri::command]
pub fn set_lb_enabled(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> Result<()> {
    queries::set_setting(&state.db(), "lbEnabled", &serde_json::json!(enabled))?;
    if enabled {
        state.lb_pending.store(true, Ordering::Release);
        kick_worker(app, &state);
    }
    Ok(())
}

/// Fix match: LaunchBox entries for the game's platform.
#[tauri::command]
pub fn lb_search(state: State<'_, AppState>, id: i64, query: Option<String>) -> Result<Vec<crate::launchbox::matching::Hit>> {
    crate::launchbox::search(&state.db(), id, query.as_deref())
}

/// Fix match: the user's pick. Its details, cover, logo and background
/// follow (downloading LaunchBox's file if they aren't stored yet).
#[tauri::command]
pub fn set_lb_match(app: AppHandle, state: State<'_, AppState>, id: i64, db_id: i64) -> Result<()> {
    let needs_download = {
        let conn = state.db();
        let needs = crate::launchbox::set_manual(&conn, id, db_id)?;
        // Its series too: looked up again with the new title.
        crate::wikidata::relook(&conn, id)?;
        needs
    };
    state.lb_pending.store(true, Ordering::Release);
    state.lb_force.fetch_or(needs_download, Ordering::AcqRel);
    state.art_pending.store(true, Ordering::Release);
    kick_worker(app.clone(), &state);
    let _ = app.emit("library-changed", ());
    Ok(())
}

// ---------------------------------------------------------------- collections

#[tauri::command]
pub fn list_collections(state: State<'_, AppState>) -> Result<Vec<crate::collections::Collection>> {
    crate::collections::list(&state.db())
}

/// A new collection (with a first game, when made from a game's details). Returns its key.
#[tauri::command]
pub fn create_collection(state: State<'_, AppState>, name: String, game_id: Option<i64>) -> Result<String> {
    crate::collections::create(&state.db(), &name, game_id)
}

#[tauri::command]
pub fn rename_collection(state: State<'_, AppState>, key: String, name: String) -> Result<()> {
    crate::collections::rename(&state.db(), &key, &name)
}

#[tauri::command]
pub fn delete_collection(state: State<'_, AppState>, key: String) -> Result<()> {
    crate::collections::delete(&state.db(), state.data_dir(), &key)
}

#[tauri::command]
pub fn set_in_collection(state: State<'_, AppState>, key: String, game_id: i64, member: bool) -> Result<()> {
    crate::collections::set_member(&state.db(), &key, game_id, member)
}

#[tauri::command]
pub fn set_collection_order(state: State<'_, AppState>, key: String, order: String) -> Result<()> {
    crate::collections::set_game_order(&state.db(), &key, &order)
}

#[tauri::command]
pub fn move_collection(state: State<'_, AppState>, key: String, delta: i64) -> Result<()> {
    crate::collections::move_by(&mut state.db(), &key, delta)
}

#[tauri::command]
pub fn set_collection_hidden(state: State<'_, AppState>, key: String, hidden: bool) -> Result<()> {
    crate::collections::set_hidden(&state.db(), &key, hidden)
}

/// The logo: "auto", "game:<id>" or "cover:<id>".
#[tauri::command]
pub fn set_collection_logo(state: State<'_, AppState>, key: String, choice: String) -> Result<()> {
    crate::collections::set_logo(&state.db(), state.data_dir(), &key, &choice)
}

/// The logo from an image file the user picked (copied into KB's folder).
#[tauri::command]
pub fn set_collection_logo_file(state: State<'_, AppState>, key: String, path: String) -> Result<()> {
    crate::collections::set_logo_file(&state.db(), state.data_dir(), &key, Path::new(&path))
}

// ---------------------------------------------------------------- Wikidata

/// One Wikidata pass. Emits `wd-progress`, then `wd-finished` / `wd-failed`.
fn run_wd(app: &AppHandle, db_path: &Path, force: bool, cancel: &AtomicBool) {
    let due = throttled(250);
    let result = (|| {
        let mut conn = db::open(db_path)?;
        if !crate::wikidata::enabled(&conn)? {
            return Ok(None);
        }
        let web = crate::wikidata::Web::new();
        crate::wikidata::run(
            &mut conn,
            &web,
            force,
            &|p| {
                if due(p.done == p.total) {
                    let _ = app.emit("wd-progress", &p);
                }
            },
            cancel,
        )
        .map(Some)
    })();
    match result {
        Ok(None) => {}
        Ok(Some(summary)) => {
            let _ = app.emit("wd-finished", &summary);
        }
        Err(e) => {
            log::error!("Wikidata pass failed: {e}");
            let _ = app.emit("wd-failed", e.to_string());
        }
    }
}

/// Settings → Game info: Wikidata's status.
#[tauri::command]
pub fn wd_status(state: State<'_, AppState>) -> Result<crate::wikidata::Status> {
    crate::wikidata::status(&state.db())
}

/// "Update now" (`force`), or a pass for games not looked up yet.
#[tauri::command]
pub fn fetch_wikidata(app: AppHandle, state: State<'_, AppState>, force: bool) -> bool {
    state.wd_force.fetch_or(force, Ordering::AcqRel);
    state.wd_pending.store(true, Ordering::Release);
    kick_worker(app, &state)
}

/// Turns Wikidata on or off (on: a pass starts).
#[tauri::command]
pub fn set_wd_enabled(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> Result<()> {
    queries::set_setting(&state.db(), "wdEnabled", &serde_json::json!(enabled))?;
    if enabled {
        state.wd_pending.store(true, Ordering::Release);
        kick_worker(app, &state);
    }
    Ok(())
}

// ---------------------------------------------------------------- startup

/// A one-time message for the user from startup (e.g. the library was moved
/// to KaneegaBell's folder), shown once the window is up.
#[tauri::command]
pub fn get_startup_notice() -> Option<String> {
    crate::startup_notice()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    /// Running from a build folder (no automatic update checks).
    dev_build: bool,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    AppInfo { version: app.package_info().version.to_string(), dev_build: crate::updates::is_dev_build() }
}

// ---------------------------------------------------------------- storage

/// Holds the library worker's slot (no scan or lookup can start) until dropped.
struct WorkerSlot<'a>(&'a AtomicBool);

impl Drop for WorkerSlot<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl AppState {
    /// For clearing and resetting: refuses while a game, a scan or a lookup runs.
    fn exclusive(&self) -> Result<WorkerSlot<'_>> {
        if self.input.game_running.load(Ordering::Acquire) {
            return Err(Error::msg("Close the game first."));
        }
        if self.scanning.swap(true, Ordering::AcqRel) {
            return Err(Error::msg("KB is busy scanning or looking up games. Stop that first (or wait), then try again."));
        }
        Ok(WorkerSlot(&self.scanning))
    }
}

#[tauri::command]
pub fn get_storage(state: State<'_, AppState>) -> Result<crate::maintenance::Storage> {
    crate::maintenance::storage(&state.db(), state.data_dir())
}

/// Deletes the downloaded covers, artwork, game info and achievement data,
/// then fetches them again (where the automatic lookups are on). Returns the
/// bytes freed.
#[tauri::command]
pub fn clear_downloaded(app: AppHandle, state: State<'_, AppState>) -> Result<u64> {
    let freed = {
        let _slot = state.exclusive()?;
        crate::maintenance::clear_downloaded(&mut state.db(), state.data_dir())?
    };
    log::info!("cleared downloaded data ({freed} bytes)");
    let (art, meta, ra, sgdb, wd) = {
        let conn = state.db();
        let art = queries::get_setting::<bool>(&conn, "autoArt")?.unwrap_or(true);
        (art, meta_wanted(&conn)?, ra_wanted(&conn)?, sgdb_wanted(&conn)?, crate::wikidata::wanted(&conn)?)
    };
    state.wd_pending.fetch_or(wd, Ordering::AcqRel);
    state.art_pending.fetch_or(art, Ordering::AcqRel);
    state.meta_pending.fetch_or(meta, Ordering::AcqRel);
    state.ra_pending.fetch_or(ra, Ordering::AcqRel);
    state.sgdb_pending.fetch_or(sgdb, Ordering::AcqRel);
    if art || meta || ra || sgdb || wd {
        kick_worker(app, &state);
    }
    Ok(freed)
}

/// Resets KB to a first start (see `maintenance::reset`), optionally also
/// removing the saved keys. The page reloads afterwards.
#[tauri::command]
pub async fn reset_app(app: AppHandle, forget_keys: bool) -> Result<()> {
    let state = app.state::<AppState>();
    {
        let _slot = state.exclusive()?;
        for flag in [&state.rescan_pending, &state.art_pending, &state.meta_pending, &state.ra_pending, &state.sgdb_pending, &state.lb_pending, &state.wd_pending] {
            flag.store(false, Ordering::Release);
        }
        let backup = crate::maintenance::reset(&mut state.db(), state.data_dir())?;
        log::info!("reset KaneegaBell (the old library is kept as {})", backup.display());
    }
    state.input.set_combo(ExitCombo::default());
    if forget_keys {
        for name in [IGDB_ID, IGDB_SECRET, IGDB_TOKEN, SGDB_KEY, RA_USER, RA_KEY] {
            secrets::delete(name)?;
        }
    }
    // Cached images of the old library could otherwise show up for new
    // games that reuse its ids.
    if let Some(w) = app.get_webview_window("main") {
        if let Err(e) = w.clear_all_browsing_data() {
            log::warn!("couldn't clear the page cache: {e}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- displays

fn main_window(app: &AppHandle) -> Result<tauri::WebviewWindow> {
    app.get_webview_window("main").ok_or_else(|| Error::msg("the main window is gone"))
}

/// Connected displays, for the TV Mode display picker.
#[tauri::command]
pub fn list_displays(app: AppHandle) -> Result<Vec<crate::display::DisplayRow>> {
    Ok(crate::display::list(&main_window(&app)?))
}

/// TV Mode on: fullscreen, on the chosen display when it's connected.
#[tauri::command]
pub fn tv_enter(app: AppHandle) -> Result<crate::display::TvEntered> {
    crate::display::tv_enter(&app, &main_window(&app)?)
}

/// TV Mode off: the window goes back to where it was.
#[tauri::command]
pub fn tv_leave(app: AppHandle) -> Result<()> {
    crate::display::tv_leave(&app, &main_window(&app)?)
}

/// After the Interface size setting changes.
#[tauri::command]
pub fn refresh_zoom(app: AppHandle) {
    crate::display::apply_zoom(&app);
}

// ---------------------------------------------------------------- web links

/// Sites the UI may open in the browser (game pages on Backloggd and
/// RetroAchievements, key pages for the services).
const LINK_SITES: &[&str] = &["backloggd.com", "retroachievements.org", "www.steamgriddb.com", "www.igdb.com", "dev.twitch.tv"];

/// An https URL on one of `LINK_SITES`, with nothing that could confuse the shell.
fn allowed_link(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    LINK_SITES.contains(&host) && !url.chars().any(|c| c.is_control() || c == '"' || c == ' ')
}

/// Opens a web page in the default browser (only on known sites).
#[tauri::command]
pub fn open_url(url: String) -> Result<()> {
    if !allowed_link(&url) {
        return Err(Error::msg("that link can't be opened"));
    }
    #[cfg(windows)]
    {
        use windows::core::{w, HSTRING};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        // SAFETY: plain strings for the duration of the call.
        let r = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(url.as_str()), None, None, SW_SHOWNORMAL) };
        // Values above 32 mean success.
        if r.0 as usize <= 32 {
            return Err(Error::msg("couldn't open the browser"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- performance harness

/// Milliseconds since this process was created (by the OS clock, so it
/// includes loading the exe and WebView2 startup).
fn ms_since_process_start() -> Option<u128> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        let (mut created, mut a, mut b, mut c) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        // SAFETY: plain out-parameters for the current process's pseudo-handle.
        unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut a, &mut b, &mut c) }.ok()?;
        let created_100ns = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        // FILETIME counts 100 ns ticks since 1601-01-01.
        const UNIX_EPOCH_100NS: u64 = 116_444_736_000_000_000;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?;
        let now_100ns = now.as_nanos() / 100 + u128::from(UNIX_EPOCH_100NS);
        Some(now_100ns.saturating_sub(u128::from(created_100ns)) / 10_000)
    }
    #[cfg(not(windows))]
    None
}

/// The UI reports startup milestones ("interactive" = library grid painted).
/// Only recorded when `KB_PERF` (or `EMU_PERF`) is set (the performance harness): appended
/// to `perf.log` in the data folder as `name ms-since-process-start`.
#[tauri::command]
pub fn perf_mark(state: State<'_, AppState>, name: String) {
    if std::env::var_os("KB_PERF").is_none() && std::env::var_os("EMU_PERF").is_none() {
        return;
    }
    let Some(ms) = ms_since_process_start() else { return };
    use std::io::Write;
    let line = format!("{} {ms}\n", name.replace(char::is_whitespace, "_"));
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.data_dir().join("perf.log"))
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

// ---------------------------------------------------------------- RetroAchievements

const RA_USER: &str = "ra-user";
const RA_KEY: &str = "ra-web-api-key";

fn ra_login() -> Result<Option<achievements::ra::Credentials>> {
    let (Some(user), Some(key)) = (secrets::get(RA_USER)?, secrets::get(RA_KEY)?) else { return Ok(None) };
    Ok(Some(achievements::ra::Credentials { user, key }))
}

/// RA is set up and some game on an RA console still needs identifying.
fn ra_wanted(conn: &Connection) -> Result<bool> {
    if ra_login()?.is_none() {
        return Ok(false);
    }
    Ok(match achievements::active_consoles(conn, None)? {
        Some(active) => queries::count_games_needing_ra(conn, &active)? > 0,
        // No console list yet: a pass fetches it.
        None => true,
    })
}

/// Hashes and identifies games for RetroAchievements.
/// Emits `ra-progress`, then `ra-finished` / `ra-failed`.
fn run_ra(app: &AppHandle, db_path: &Path, cancel: &AtomicBool) {
    let due = throttled(250);
    let result = (|| {
        let creds = ra_login()?.ok_or_else(|| Error::msg("RetroAchievements isn't set up"))?;
        let http = art::http::Http::new();
        let client = achievements::ra::Ra::new(&http, creds);
        let mut conn = db::open(db_path)?;
        achievements::run(
            &mut conn,
            &client,
            &|p| {
                if due(p.done == p.total) {
                    let _ = app.emit("ra-progress", p);
                }
            },
            cancel,
        )
    })();
    match result {
        Ok(summary) => {
            log::info!("RetroAchievements pass: {summary:?}");
            let _ = app.emit("ra-finished", &summary);
        }
        Err(e) => {
            log::error!("RetroAchievements pass failed: {e}");
            let _ = app.emit("ra-failed", e.to_string());
        }
    }
}

/// Checks the username and Web API key with RA, then saves them and
/// identifies the library's games.
#[tauri::command]
pub async fn set_ra_keys(app: AppHandle, user: String, key: String) -> Result<String> {
    let (user, key) = (user.trim().to_string(), key.trim().to_string());
    if user.is_empty() || key.is_empty() {
        return Err(Error::msg("Enter both your RetroAchievements username and Web API key"));
    }
    let name = tauri::async_runtime::spawn_blocking(move || -> Result<String> {
        let http = art::http::Http::new();
        let client = achievements::ra::Ra::new(&http, achievements::ra::Credentials { user: user.clone(), key: key.clone() });
        let profile = client
            .profile()?
            .ok_or_else(|| Error::msg(format!("RetroAchievements has no user named \"{user}\"")))?;
        // Store the name as RA spells it.
        secrets::set(RA_USER, &profile.user)?;
        secrets::set(RA_KEY, &key)?;
        Ok(profile.user)
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
    let state = app.state::<AppState>();
    state.ra_pending.store(true, Ordering::Release);
    kick_worker(app.clone(), &state);
    Ok(name)
}

/// Forgets the RA login (identified games and cached lists stay).
#[tauri::command]
pub fn remove_ra_keys() -> Result<()> {
    secrets::delete(RA_USER)?;
    secrets::delete(RA_KEY)
}

/// A game's achievements and the user's unlocks (cached for a few minutes,
/// and shown from the cache when offline). `refresh` asks RA again now.
#[tauri::command]
pub async fn get_achievements(app: AppHandle, id: i64, refresh: bool) -> Result<achievements::GameAchievements> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let conn = db::open(&state.db_path)?;
        let http = art::http::Http::new();
        let client = ra_login()?.map(|c| achievements::ra::Ra::new(&http, c));
        achievements::for_game(&conn, client.as_ref(), id, refresh)
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

fn run_art(app: &AppHandle, db_path: &Path, data_dir: &Path, cancel: &AtomicBool) {
    let due = throttled(250);
    // Ask the UI to reload every couple of seconds so covers appear progressively.
    let reload_due = throttled(2000);
    let http = art::http::Http::new();
    let result = db::open(db_path).and_then(|mut conn| {
        art::run(
            &mut conn,
            data_dir,
            &http,
            &|p| {
                if due(p.done == p.total) {
                    let _ = app.emit("art-progress", &p);
                }
                if p.found > 0 && reload_due(false) {
                    let _ = app.emit("library-changed", ());
                }
            },
            cancel,
        )
    });
    match result {
        Ok(summary) => {
            let _ = app.emit("art-finished", &summary);
        }
        Err(e) => {
            log::error!("artwork fetch failed: {e}");
            let _ = app.emit("art-failed", e.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_https_links_to_known_sites_open() {
        use super::allowed_link as ok;
        assert!(ok("https://backloggd.com/games/golden-sun/"));
        assert!(ok("https://backloggd.com/search/games/golden%20sun/"));
        assert!(ok("https://retroachievements.org/game/2774"));
        assert!(!ok("http://backloggd.com/games/x/"), "https only");
        assert!(!ok("https://backloggd.com.evil.example/"));
        assert!(!ok("https://evil.example/?backloggd.com"));
        assert!(!ok("file:///C:/Windows/System32/calc.exe"));
        assert!(!ok("https://backloggd.com/\" & calc"));
    }
}