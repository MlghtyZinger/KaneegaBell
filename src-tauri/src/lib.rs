mod art;
mod commands;
mod db;
mod error;
mod fsbrowse;
mod input;
mod launcher;
mod platforms;
mod scanner;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{Manager, WindowEvent};

pub fn run() {
    // Logs to stderr (visible when run from a terminal). RUST_LOG overrides.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("emu_frontend_lib=info")).init();

    tauri::Builder::default()
        // Cached artwork, read from disk off the UI thread.
        .register_asynchronous_uri_scheme_protocol("thumb", |ctx, request, responder| {
            let dir = ctx.app_handle().path().app_data_dir().unwrap_or_default();
            let path = request.uri().path().to_string();
            tauri::async_runtime::spawn_blocking(move || responder.respond(art::protocol::respond(&dir, &path)));
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("library.db");
            let conn = db::init(&db_path)?;
            // TV mode: go fullscreen before the page loads, so there's no resize flash.
            if db::queries::get_setting::<bool>(&conn, "tvMode")? == Some(true) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_fullscreen(true);
                }
            }

            let input = Arc::new(input::InputShared::default());
            input.set_combo(db::queries::get_setting(&conn, "exitCombo")?.unwrap_or_default());
            input.app_focused.store(true, Ordering::Release);
            app.manage(commands::AppState::new(conn, db_path, input.clone()));

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
            commands::set_setting,
            commands::start_scan,
            commands::cancel_scan,
            commands::fetch_art,
            commands::refresh_art,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
