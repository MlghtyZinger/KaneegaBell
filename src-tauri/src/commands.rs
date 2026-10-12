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
use tauri::Manager;

pub struct AppState {
    db: Mutex<Connection>,
    db_path: PathBuf,
    scanning: Arc<AtomicBool>,
    rescan_pending: Arc<AtomicBool>,
    art_pending: Arc<AtomicBool>,
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
pub struct Library {
    platforms: Vec<PlatformSummary>,
    games: Vec<queries::GameRow>,
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
    Ok(Library { platforms, games: queries::library(&conn)? })
}

#[tauri::command]
pub fn get_game(state: State<'_, AppState>, id: i64) -> Result<queries::GameDetail> {
    queries::game_detail(&state.db(), id)
}

#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, id: i64, favorite: bool) -> Result<()> {
    queries::set_favorite(&state.db(), id, favorite)
}

/// The exact argv a launch would run, for the details panel.
#[tauri::command]
pub fn preview_launch(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<Vec<String>> {
    let conn = state.db();
    let game = queries::game_detail(&conn, id)?;
    let cfg = queries::launch_config(&conn, &game.platform_id)?;
    let cores_dir: Option<String> = queries::get_setting(&conn, "retroarchCoresDir")?;
    let playlists = app.path().app_data_dir().map_err(|e| Error::msg(e.to_string()))?.join("playlists");
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
pub fn remove_source(state: State<'_, AppState>, id: i64) -> Result<()> {
    queries::remove_source(&state.db(), id)
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

/// Options for the "Change cover" picker; `query` searches the platform's
/// whole cover set by name.
#[tauri::command]
pub async fn cover_candidates(app: AppHandle, id: i64, query: Option<String>) -> Result<Vec<art::Candidate>> {
    // May download a cover index: keep it off the UI thread and the shared DB lock.
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let conn = db::open(&state.db_path)?;
        art::candidates(&conn, &art::http::Http::new(), id, query.as_deref())
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

/// Sets a cover from the picker (`source` "libretro" / "local") or from an
/// image file the user picked (`source` "file", `url` = path).
#[tauri::command]
pub async fn set_cover(app: AppHandle, id: i64, source: String, name: String, url: String) -> Result<()> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let conn = db::open(&state.db_path)?;
        art::apply_choice(&conn, state.data_dir(), &art::http::Http::new(), id, &source, &name, &url)
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))??;
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

/// Starts the worker thread unless it's already running (it then picks up the
/// pending flags itself). Returns false only if the thread couldn't start.
fn kick_worker(app: AppHandle, state: &AppState) -> bool {
    if state.scanning.swap(true, Ordering::AcqRel) {
        return true;
    }
    let busy = state.scanning.clone();
    let scan_pending = state.rescan_pending.clone();
    let art_pending = state.art_pending.clone();
    let cancel = state.scan_cancel.clone();
    let db_path = state.db_path.clone();
    let data_dir = state.data_dir().to_path_buf();
    std::thread::Builder::new()
        .name("library-jobs".into())
        .spawn(move || loop {
            cancel.store(false, Ordering::Release);
            if scan_pending.swap(false, Ordering::AcqRel) {
                let auto_art = run_scan(&app, &db_path, &cancel);
                if auto_art {
                    art_pending.store(true, Ordering::Release);
                }
            } else if art_pending.swap(false, Ordering::AcqRel) {
                run_art(&app, &db_path, &data_dir, &cancel);
            } else {
                busy.store(false, Ordering::Release);
                // A request may have slipped in after the checks above.
                let more = scan_pending.load(Ordering::Acquire) || art_pending.load(Ordering::Acquire);
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

/// Runs one scan. Returns whether an art pass should follow.
fn run_scan(app: &AppHandle, db_path: &Path, cancel: &AtomicBool) -> bool {
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
        let auto_art = queries::get_setting::<bool>(&conn, "autoArt")?.unwrap_or(true);
        let needs_art = queries::count_games_needing_art(&conn, queries::now())? > 0;
        Ok((summary, auto_art && needs_art))
    });
    match result {
        Ok((summary, art_next)) => {
            log::info!("scan finished: {summary:?}");
            let _ = app.emit("scan-finished", &summary);
            art_next && !summary.cancelled
        }
        Err(e) => {
            log::error!("scan failed: {e}");
            let _ = app.emit("scan-failed", e.to_string());
            false
        }
    }
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
