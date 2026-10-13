//! In-app updates (Tauri's updater plugin).
//!
//! Releases live on GitHub Releases with a `latest.json` manifest. Every
//! installer is signed with the owner's private key; the matching public
//! key is built into the app (tauri.conf.json → plugins.updater.pubkey), so a
//! download that wasn't signed by that key is refused.
//!
//! - One quiet check ~10 s after startup (no polling), unless turned off
//!   (setting `autoUpdateCheck`), a game is running, or this is a
//!   development build (an exe under `target\`): then only "Check now".
//! - A found update is offered (version + changelog); "Skip this version"
//!   is remembered (`skipUpdateVersion`).
//! - Installing never happens while a game runs. On Windows the app exits,
//!   the installer runs with a small progress window, and it starts KB again.

use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::commands::AppState;
use crate::db::queries;
use crate::error::{Error, Result};

/// The update found by the last check, ready to install.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// The release's changelog.
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Running from a cargo build folder (the owner's own builds): no automatic
/// checks, so a newer published release doesn't install over the copy under
/// development.
pub fn is_dev_build() -> bool {
    std::env::current_exe()
        .map(|p| p.components().any(|c| c.as_os_str().eq_ignore_ascii_case("target")))
        .unwrap_or(false)
}

fn info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone().filter(|b| !b.trim().is_empty()),
        date: update.date.map(|d| d.to_string()),
    }
}

async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>> {
    let updater = app.updater().map_err(|e| Error::msg(format!("updater: {e}")))?;
    let found = match updater.check().await {
        Ok(found) => found,
        // No manifest at the endpoint (no release published yet): up to date.
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => None,
        Err(e) => return Err(Error::msg(format!("couldn't check for updates: {e}"))),
    };
    let out = found.as_ref().map(info);
    *app.state::<Pending>().0.lock().unwrap_or_else(|e| e.into_inner()) = found;
    Ok(out)
}

/// "Check now" (Settings). Returns the update, or None when up to date.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>> {
    check(&app).await
}

/// Downloads and installs the update found by the last check. Emits
/// `update-progress`; on Windows the app then exits and the installer
/// restarts it.
#[tauri::command]
pub async fn install_update(app: AppHandle, pending: State<'_, Pending>) -> Result<()> {
    if app.state::<AppState>().input.game_running.load(Ordering::Acquire) {
        return Err(Error::msg("Close the game first: KB updates when no game is running."));
    }
    let update = pending
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or_else(|| Error::msg("No update to install; check again."))?;
    let mut downloaded = 0u64;
    let mut last_emit = std::time::Instant::now();
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if last_emit.elapsed() >= Duration::from_millis(150) {
                    last_emit = std::time::Instant::now();
                    let _ = app.emit("update-progress", UpdateProgress { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| Error::msg(format!("download failed: {e}")))?;
    log::info!("installing KB {} (exits the app)", update.version);
    update.install(bytes).map_err(|e| Error::msg(format!("couldn't start the installer: {e}")))
}

/// The one automatic check, shortly after startup. Offers the update with
/// an `update-available` event (not for a version the user skipped).
pub fn check_on_startup(app: AppHandle) {
    let _ = std::thread::Builder::new().name("update-check".into()).spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        let state = app.state::<AppState>();
        let (enabled, skipped) = {
            let conn = state.db();
            (
                queries::get_setting::<bool>(&conn, "autoUpdateCheck").ok().flatten().unwrap_or(true),
                queries::get_setting::<String>(&conn, "skipUpdateVersion").ok().flatten(),
            )
        };
        if !enabled || is_dev_build() || state.input.game_running.load(Ordering::Acquire) {
            return;
        }
        match tauri::async_runtime::block_on(check(&app)) {
            Ok(Some(u)) if skipped.as_deref() != Some(u.version.as_str()) => {
                log::info!("update available: {} -> {}", u.current_version, u.version);
                let _ = app.emit("update-available", u);
            }
            Ok(_) => {}
            // Offline or GitHub unreachable: try again next start.
            Err(e) => log::info!("{e}"),
        }
    });
}
#[cfg(test)]
mod tests {
    #[test]
    fn dev_builds_are_recognized_by_their_target_folder() {
        // Tests themselves run from target\debug\deps.
        assert!(super::is_dev_build());
    }
}
