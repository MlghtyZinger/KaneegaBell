mod achievements;
mod art;
mod commands;
mod db;
mod error;
mod fsbrowse;
mod input;
mod launcher;
mod metadata;
mod migrate;
#[cfg(test)]
mod perf;
mod platforms;
mod scanner;
mod secrets;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};

use tauri::{Manager, WindowEvent};

/// `KB_DATA_DIR` (or the older `EMU_FRONTEND_DATA_DIR`) points the app at
/// another data folder, e.g. a generated test library for the performance
/// harness, leaving the real library untouched.
pub(crate) fn data_dir_override() -> Option<PathBuf> {
    ["KB_DATA_DIR", "EMU_FRONTEND_DATA_DIR"]
        .iter()
        .find_map(|k| std::env::var_os(k).filter(|d| !d.is_empty()))
        .map(PathBuf::from)
}

/// The data folder chosen at startup (library.db, art cache).
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Something to tell the user once the window is up (the library move).
static STARTUP_NOTICE: OnceLock<String> = OnceLock::new();

pub(crate) fn startup_notice() -> Option<String> {
    STARTUP_NOTICE.get().cloned()
}

/// Picks the data folder: the override, else the app's own folder, moving
/// the library over from the folder used before the KaneegaBell rename the
/// first time (see `migrate`). If that move fails, the old folder stays in use.
fn choose_data_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    if let Some(dir) = data_dir_override() {
        return dir;
    }
    let dir = app.path().app_data_dir().unwrap_or_default();
    let Some(old) = dir.parent().map(|p| p.join(migrate::LEGACY_IDENTIFIER)) else { return dir };
    match migrate::migrate(&old, &dir) {
        Ok(migrate::Outcome::NotNeeded) => dir,
        Ok(migrate::Outcome::Migrated { games }) => {
            log::info!("moved the library ({games} games) from {} to {}", old.display(), dir.display());
            let _ = STARTUP_NOTICE.set(format!(
                "KaneegaBell moved your library ({games} games, play time and favorites) to its own folder. \
                 The old folder is still there ({}); you can delete it once everything looks right.",
                old.display()
            ));
            dir
        }
        Err(e) => {
            log::error!("couldn't move the library to {}: {e}", dir.display());
            let _ = STARTUP_NOTICE.set(format!(
                "Couldn't move your library to KaneegaBell's folder ({e}). Your library is safe: KaneegaBell is \
                 using the old folder ({}) for now and will try again next start.",
                old.display()
            ));
            old
        }
    }
}

pub fn run() {
    // Logs to stderr (visible when run from a terminal). RUST_LOG overrides.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("kaneegabell_lib=info")).init();

    tauri::Builder::default()
        // Cached artwork, read from disk off the UI thread.
        .register_asynchronous_uri_scheme_protocol("thumb", |_ctx, request, responder| {
            // Set in setup, which runs before the page loads.
            let dir = DATA_DIR.get().cloned().unwrap_or_default();
            let path = request.uri().path().to_string();
            tauri::async_runtime::spawn_blocking(move || responder.respond(art::protocol::respond(&dir, &path)));
        })
        .setup(|app| {
            let dir = DATA_DIR.get_or_init(|| choose_data_dir(app.handle())).clone();
            std::fs::create_dir_all(&dir)?;
            secrets::init(&dir);
            let db_path = dir.join("library.db");
            let conn = db::init(&db_path)?;
            // Fullscreen (or TV Mode) at start: go fullscreen before the page
            // loads, so there's no resize flash.
            let fullscreen = db::queries::get_setting::<bool>(&conn, "tvMode")? == Some(true)
                || db::queries::get_setting::<bool>(&conn, "startInTv")? == Some(true);
            if fullscreen {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_fullscreen(true);
                }
            }

            let input = Arc::new(input::InputShared::default());
            input.set_combo(db::queries::get_setting(&conn, "exitCombo")?.unwrap_or_default());
            input.app_focused.store(true, Ordering::Release);
            app.manage(commands::AppState::new(conn, db_path, input.clone()));
            commands::resume_art_on_startup(app.handle().clone());

            let handle = app.handle().clone();
            input::gamepad::spawn(
                app.handle().clone(),
                input,
                Arc::new(move || handle.state::<commands::AppState>().launcher.request_exit()),
            );
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(focused) = event {
                if let Some(state) = window.try_state::<commands::AppState>() {
                    state.input.app_focused.store(*focused, Ordering::Release);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_library,
            commands::get_game,
            commands::set_favorite,
            commands::preview_launch,
            commands::launch_game,
            commands::exit_game,
            commands::get_running,
            commands::get_settings,
            commands::update_emulator,
            commands::detect_emulators,
            commands::update_platform,
            commands::validate_template,
            commands::list_dir,
            commands::suggest_platform,
            commands::add_source,
            commands::add_rpcs3_installed_source,
            commands::update_source,
            commands::remove_source,
            commands::get_setting,
            commands::set_setting,
            commands::start_scan,
            commands::cancel_scan,
            commands::fetch_art,
            commands::refresh_art,
            commands::cover_candidates,
            commands::set_cover,
            commands::remove_cover,
            commands::get_services,
            commands::set_igdb_keys,
            commands::remove_igdb_keys,
            commands::fetch_metadata,
            commands::igdb_search,
            commands::set_igdb_match,
            commands::set_ra_keys,
            commands::remove_ra_keys,
            commands::get_achievements,
            commands::perf_mark,
            commands::remove_extra,
            commands::set_sgdb_key,
            commands::remove_sgdb_key,
            commands::fetch_extras,
            commands::open_url,
            commands::get_startup_notice,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
