mod achievements;
mod art;
mod collections;
mod commands;
mod db;
mod disc;
mod display;
mod error;
mod fsbrowse;
mod input;
mod launchbox;
mod launcher;
mod maintenance;
mod metadata;
mod migrate;
#[cfg(test)]
mod perf;
mod platforms;
mod scanner;
mod secrets;
mod updates;
mod wikidata;

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

/// `data` next to KB's exe, if KB may write there (a program folder under
/// Program Files usually isn't writable). Checked with a probe file next to
/// the exe, so no empty `data` folder is left behind.
fn kb_folder_data_dir() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let probe = exe_dir.join(".kb-write-test");
    std::fs::write(&probe, b"").ok()?;
    let _ = std::fs::remove_file(&probe);
    Some(exe_dir.join("data"))
}

/// WebView2's browser arguments. Besides wry's defaults (no mini menu, no
/// SmartScreen), these keep memory down without slowing the grid:
/// - the GPU process may keep 16 MB of decoded covers instead of growing to
///   ~850 MB after scrolling a big library (measured on 5,000 games: still
///   240 fps, every frame ~4.2 ms). GPU memory itself isn't capped: a 64 MB
///   cap left parts of the screen undrawn with two panels open on a big
///   high-DPI window (holes in the grid, half-drawn pickers);
/// - the network service runs inside the browser process (one process less);
/// - WebView2's web cache stays under 32 MB. KB keeps its own pictures in
///   `data\art`; the cache only held the pickers' previews (full-size web
///   images), and without a cap it had grown past 300 MB on the owner's PC;
/// - no component downloads: WebView2 otherwise fetches ~20 MB of add-ons KB
///   never uses (ad-filter lists, a speech model, hyphenation data).
///
/// Arguments in `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` (the perf tools use it
/// for DevTools) would replace these, so they're merged in and the variable is
/// cleared.
fn webview_args() -> String {
    let mut disable = vec!["msWebOOUI", "msPdfOOUI", "msSmartScreenProtection"].into_iter().map(String::from).collect::<Vec<_>>();
    let mut enable = vec!["NetworkServiceInProcess2".to_string()];
    let mut other = vec![
        "--force-gpu-mem-discardable-limit-mb=16".to_string(),
        format!("--disk-cache-size={WEB_CACHE_BYTES}"),
        "--disable-component-update".to_string(),
    ];
    if let Ok(extra) = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS") {
        std::env::remove_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS");
        merge_browser_args(&extra, &mut disable, &mut enable, &mut other);
    }
    format!("--disable-features={} --enable-features={} {}", disable.join(","), enable.join(","), other.join(" "))
}

const WEB_CACHE_BYTES: u64 = 32 * 1024 * 1024;

/// Once: deletes a web cache that grew before the cap existed (it only holds
/// downloaded previews; WebView2 rebuilds it), and again after "Clear
/// downloaded data" or a reset. Runs before WebView2 starts.
fn trim_web_cache(conn: &rusqlite::Connection, webview_dir: &std::path::Path) {
    if db::queries::get_setting::<bool>(conn, "webCacheTrimmed").ok().flatten() == Some(true) {
        return;
    }
    let cache = webview_dir.join("EBWebView").join("Default").join("Cache");
    if cache.is_dir() {
        match std::fs::remove_dir_all(&cache) {
            Ok(()) => log::info!("removed the old web cache ({})", cache.display()),
            Err(e) => {
                log::warn!("couldn't remove the old web cache: {e}");
                return;
            }
        }
    }
    let _ = db::queries::set_setting(conn, "webCacheTrimmed", &serde_json::Value::Bool(true));
}

/// Add-ons earlier versions let WebView2 download (see `webview_args`).
/// The certificate lists are kept.
const UNUSED_WEB_COMPONENTS: &[&str] = &[
    "Subresource Filter",
    "Speech Recognition",
    "hyphen-data",
    "component_crx_cache",
    "Trust Protection Lists",
    "WidevineCdm",
    "MEIPreload",
    "AutoLaunchProtocolsComponent",
    "OriginTrials",
    "TrustTokenKeyCommitments",
];

/// Once, before WebView2 starts: deletes those add-ons.
fn remove_unused_web_components(conn: &rusqlite::Connection, webview_dir: &std::path::Path) {
    if db::queries::get_setting::<bool>(conn, "webComponentsRemoved").ok().flatten() == Some(true) {
        return;
    }
    let root = webview_dir.join("EBWebView");
    for name in UNUSED_WEB_COMPONENTS {
        match std::fs::remove_dir_all(root.join(name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                log::warn!("couldn't remove WebView2's {name}: {e}");
                return;
            }
            _ => {}
        }
    }
    let _ = db::queries::set_setting(conn, "webComponentsRemoved", &serde_json::Value::Bool(true));
}

/// Chromium keeps only the last `--enable-features` / `--disable-features`,
/// so their lists are joined; other arguments are added as they are.
fn merge_browser_args(extra: &str, disable: &mut Vec<String>, enable: &mut Vec<String>, other: &mut Vec<String>) {
    for arg in extra.split_whitespace() {
        if let Some(list) = arg.strip_prefix("--disable-features=") {
            disable.extend(list.split(',').filter(|f| !f.is_empty()).map(String::from));
        } else if let Some(list) = arg.strip_prefix("--enable-features=") {
            enable.extend(list.split(',').filter(|f| !f.is_empty()).map(String::from));
        } else {
            other.push(arg.to_string());
        }
    }
}

/// Picks the data folder: the override (a test library), else `data` in
/// KB's own folder, so everything stays together. The first time, the library
/// moves there from where earlier versions kept it (AppData, or the folder
/// from before the KaneegaBell rename): copied, verified, then switched (see
/// `migrate`); the old folder is left as it was. A development copy (run
/// from a build folder) keeps its own separate library and moves nothing.
/// If KB's folder can't be written to, AppData is used instead.
fn choose_data_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    if let Some(dir) = data_dir_override() {
        return dir;
    }
    let appdata = app.path().app_data_dir().unwrap_or_default();
    let legacy = appdata.parent().map(|p| p.join(migrate::LEGACY_IDENTIFIER));
    let earlier = [Some(appdata.clone()), legacy].into_iter().flatten().find(|d| d.join("library.db").exists());

    let Some(dir) = kb_folder_data_dir() else {
        let _ = STARTUP_NOTICE.set(format!(
            "KaneegaBell can't write to its own folder, so your library is kept in {} instead. \
             Install KaneegaBell in a folder you own (for example C:\\Games\\KaneegaBell) to keep everything together.",
            appdata.display()
        ));
        return earlier.unwrap_or(appdata);
    };
    if updates::is_dev_build() {
        return dir;
    }
    let Some(old) = earlier else { return dir };
    match migrate::migrate(&old, &dir) {
        Ok(migrate::Outcome::NotNeeded) => dir,
        Ok(migrate::Outcome::Migrated { games }) => {
            log::info!("moved the library ({games} games) from {} to {}", old.display(), dir.display());
            let _ = STARTUP_NOTICE.set(format!(
                "KaneegaBell moved your library ({games} games, play time and favorites) into its own folder ({}). \
                 The old folder is still there ({}); you can delete it once everything looks right.",
                dir.display(),
                old.display()
            ));
            dir
        }
        Err(e) => {
            log::error!("couldn't move the library to {}: {e}", dir.display());
            let _ = STARTUP_NOTICE.set(format!(
                "Couldn't move your library into KaneegaBell's folder ({e}). Your library is safe: KaneegaBell is \
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Pending::default())
        .manage(display::TvState::default())
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
            // The installed app's own library: keys saved by earlier versions
            // move from Windows Credential Manager into the data folder.
            if data_dir_override().is_none() && !updates::is_dev_build() {
                match secrets::import_from_credential_manager() {
                    Ok(0) => {}
                    Ok(n) => log::info!("moved {n} saved key(s) from Credential Manager into the data folder"),
                    Err(e) => log::warn!("couldn't move the saved keys: {e}"),
                }
            }
            let db_path = dir.join("library.db");
            let conn = db::init(&db_path)?;
            // The window is made here (not from the config) so WebView2 keeps
            // its own files in the data folder too, instead of AppData.
            trim_web_cache(&conn, &dir.join("webview"));
            remove_unused_web_components(&conn, &dir.join("webview"));
            let window = match app.config().app.windows.iter().find(|w| w.label == "main") {
                Some(cfg) => Some(
                    tauri::WebviewWindowBuilder::from_config(app.handle(), cfg)?
                        .data_directory(dir.join("webview"))
                        .additional_browser_args(&webview_args())
                        .build()?,
                ),
                None => app.get_webview_window("main"),
            };
            // Back where the window was last time, before the first paint.
            if let Some(w) = &window {
                display::restore(w, &conn);
            }
            let start_fullscreen = db::queries::get_setting::<bool>(&conn, "tvMode")? == Some(true);
            let start_in_tv = db::queries::get_setting::<bool>(&conn, "startInTv")? == Some(true);

            let input = Arc::new(input::InputShared::default());
            input.set_combo(db::queries::get_setting(&conn, "exitCombo")?.unwrap_or_default());
            input.app_focused.store(true, Ordering::Release);
            app.manage(commands::AppState::new(conn, db_path, input.clone()));
            // Fullscreen (or TV Mode, on its display) before the page loads,
            // so there's no resize flash.
            if let Some(w) = &window {
                if start_in_tv {
                    if let Err(e) = display::tv_enter(app.handle(), w) {
                        log::warn!("couldn't open TV Mode's window: {e}");
                    }
                } else if start_fullscreen {
                    let _ = w.set_fullscreen(true);
                }
            }
            display::apply_zoom(app.handle());
            commands::resume_art_on_startup(app.handle().clone());
            updates::check_on_startup(app.handle().clone());

            let handle = app.handle().clone();
            input::gamepad::spawn(
                app.handle().clone(),
                input,
                Arc::new(move || handle.state::<commands::AppState>().launcher.request_exit()),
            );
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(focused) => {
                if let Some(state) = window.try_state::<commands::AppState>() {
                    state.input.app_focused.store(*focused, Ordering::Release);
                }
            }
            WindowEvent::Moved(_) => display::schedule_save(window.app_handle()),
            WindowEvent::Resized(_) => {
                display::schedule_save(window.app_handle());
                display::apply_zoom(window.app_handle());
            }
            WindowEvent::ScaleFactorChanged { .. } => display::apply_zoom(window.app_handle()),
            WindowEvent::CloseRequested { .. } => {
                if let Err(e) = display::save(window.app_handle()) {
                    log::warn!("couldn't save the window placement: {e}");
                }
            }
            _ => {}
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
            commands::app_info,
            commands::get_storage,
            commands::clear_downloaded,
            commands::delete_reset_backup,
            commands::reset_app,
            commands::list_displays,
            commands::tv_enter,
            commands::tv_leave,
            commands::refresh_zoom,
            commands::lb_status,
            commands::fetch_launchbox,
            commands::set_lb_enabled,
            commands::lb_search,
            commands::set_lb_match,
            commands::wd_status,
            commands::cover_accent,
            commands::achievement_glance,
            commands::get_removed,
            commands::forget_removed,
            commands::list_collections,
            commands::create_collection,
            commands::rename_collection,
            commands::delete_collection,
            commands::set_in_collection,
            commands::set_collection_order,
            commands::move_collection,
            commands::set_collection_hidden,
            commands::set_collection_logo,
            commands::set_collection_logo_file,
            commands::fetch_wikidata,
            commands::set_wd_enabled,
            updates::check_for_update,
            updates::install_update,
            updates::set_update_code,
            updates::remove_update_code,
            updates::has_update_code,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::merge_browser_args;

    #[test]
    fn extra_browser_args_join_the_feature_lists() {
        let (mut d, mut e, mut o) = (vec!["A".to_string()], vec!["B".to_string()], vec!["--x".to_string()]);
        merge_browser_args("--remote-debugging-port=9333 --disable-features=C,D --enable-features=E", &mut d, &mut e, &mut o);
        assert_eq!(d, ["A", "C", "D"]);
        assert_eq!(e, ["B", "E"]);
        assert_eq!(o, ["--x", "--remote-debugging-port=9333"]);
    }
}
