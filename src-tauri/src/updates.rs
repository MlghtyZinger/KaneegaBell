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

/// The update access code (a read-only GitHub token for the private
/// releases), saved encrypted with the other keys.
pub const ACCESS_CODE: &str = "update-token";

/// Why a check found nothing to read (the updater can't tell a missing file
/// from a refused code: both are "no release found").
pub const NEEDS_CODE: &str = "Updates need an access code: ask the person who gave you KaneegaBell for one, then enter it in Settings → General → Updates.";
pub const CODE_REJECTED: &str = "The update access code doesn't work anymore (it may have expired): ask the person who gave you KaneegaBell for a new one.";

/// The manifest (`latest.json` on the repo's `updates` branch) and the
/// installers are read through GitHub's API, with the access code when there
/// is one, so updates keep working once the repo is private.
fn updater_with(app: &AppHandle, code: Option<&str>) -> Result<tauri_plugin_updater::Updater> {
    let err = |e: tauri_plugin_updater::Error| Error::msg(format!("updater: {e}"));
    let mut b = app
        .updater_builder()
        .header("Accept", "application/vnd.github.raw+json")
        .map_err(err)?
        .header("X-GitHub-Api-Version", "2022-11-28")
        .map_err(err)?;
    if let Some(code) = code {
        b = b.header("Authorization", format!("Bearer {}", code.trim())).map_err(err)?;
    }
    b.build().map_err(err)
}

async fn check_with(app: &AppHandle, code: Option<&str>) -> Result<Option<Update>> {
    match updater_with(app, code)?.check().await {
        Ok(found) => Ok(found),
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => {
            Err(Error::msg(if code.is_some() { CODE_REJECTED } else { NEEDS_CODE }))
        }
        Err(e) => Err(Error::msg(format!("couldn't check for updates: {e}"))),
    }
}

async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>> {
    let code = crate::secrets::get(ACCESS_CODE)?;
    let found = check_with(app, code.as_deref()).await?;
    let out = found.as_ref().map(info);
    *app.state::<Pending>().0.lock().unwrap_or_else(|e| e.into_inner()) = found;
    Ok(out)
}

/// Saves an update access code after checking that it can read the updates.
#[tauri::command]
pub async fn set_update_code(app: AppHandle, code: String) -> Result<()> {
    let code = code.trim().to_string();
    if code.is_empty() || code.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(Error::msg("That doesn't look like an access code."));
    }
    match check_with(&app, Some(&code)).await {
        Ok(_) => crate::secrets::set(ACCESS_CODE, &code),
        Err(e) if e.to_string() == CODE_REJECTED => Err(Error::msg("That code doesn't work: check that it was copied completely.")),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub fn remove_update_code() -> Result<()> {
    crate::secrets::delete(ACCESS_CODE)
}

/// Whether an update access code is saved.
#[tauri::command]
pub fn has_update_code() -> Result<bool> {
    Ok(crate::secrets::get(ACCESS_CODE)?.is_some())
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
            // A code that stopped working is worth saying every start; a
            // missing one only once (Settings keeps showing it).
            Err(e) if e.to_string() == CODE_REJECTED => {
                let _ = app.emit("update-access", e.to_string());
            }
            Err(e) if e.to_string() == NEEDS_CODE => {
                let conn = state.db();
                if queries::get_setting::<bool>(&conn, "updateCodeNoticeShown").ok().flatten() != Some(true) {
                    let _ = queries::set_setting(&conn, "updateCodeNoticeShown", &serde_json::json!(true));
                    let _ = app.emit("update-access", e.to_string());
                }
            }
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
