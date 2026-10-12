//! Thin `#[tauri::command]` layer: argument plumbing only, logic lives in
//! `db::queries`, `scanner` and `launcher`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

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
    pub input: Arc<InputShared>,
    pub launcher: Arc<Launcher>,
}

impl AppState {
    pub fn new(conn: Connection, db_path: PathBuf, input: Arc<InputShared>) -> Self {
        Self { db: Mutex::new(conn), db_path, scanning: Arc::default(), input, launcher: Arc::default() }
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
    let platforms = queries::platforms(&conn)?
        .into_iter()
        .filter(|p| p.enabled && p.game_count > 0)
        .map(|p| PlatformSummary { id: p.id, name: p.name, short_name: p.short_name, count: p.game_count })
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

// ---------------------------------------------------------------- scanning

/// Starts a background scan. Returns false if one is already running.
/// Emits `scan-progress` while running and `scan-finished` / `scan-failed` at the end.
#[tauri::command]
pub fn start_scan(app: AppHandle, state: State<'_, AppState>) -> bool {
    if state.scanning.swap(true, Ordering::AcqRel) {
        return false;
    }
    let flag = state.scanning.clone();
    let db_path = state.db_path.clone();
    std::thread::Builder::new()
        .name("scanner".into())
        .spawn(move || {
            let result = db::open(&db_path).and_then(|mut conn| {
                let last_emit = std::cell::Cell::new(None::<std::time::Instant>);
                scanner::run(&mut conn, &|p| {
                    // Throttle progress events; the UI only needs a few per second.
                    let due = last_emit.get().is_none_or(|t| t.elapsed().as_millis() >= 100);
                    if p.phase == "done" || due {
                        last_emit.set(Some(std::time::Instant::now()));
                        let _ = app.emit("scan-progress", &p);
                    }
                })
            });
            flag.store(false, Ordering::Release);
            match result {
                Ok(summary) => {
                    log::info!("scan finished: {summary:?}");
                    let _ = app.emit("scan-finished", &summary);
                }
                Err(e) => {
                    log::error!("scan failed: {e}");
                    let _ = app.emit("scan-failed", e.to_string());
                }
            }
        })
        .map(|_| true)
        .unwrap_or_else(|e| {
            log::error!("could not start scanner thread: {e}");
            state.scanning.store(false, Ordering::Release);
            false
        })
}
