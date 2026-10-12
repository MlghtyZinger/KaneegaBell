//! Launching games: build the command, spawn the emulator, track the session,
//! hand the screen over and back, and record play time.

pub mod process;
pub mod template;
#[cfg(windows)]
mod window;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::queries::{self, GameDetail, LaunchConfig};
use crate::error::{Error, Result};
use crate::scanner::cue_m3u;
use process::Tracker;

/// What a launch will run, plus a playlist file to write first (multi-disc
/// sets without an .m3u of their own, for emulators that support m3u).
#[derive(Debug, PartialEq)]
pub struct Prepared {
    pub argv: Vec<String>,
    pub playlist: Option<(PathBuf, String)>,
}

/// Pure: decides the ROM argument and renders the template.
pub fn prepare(game: &GameDetail, cfg: &LaunchConfig, cores_dir: Option<&Path>, playlist_dir: &Path) -> Result<Prepared> {
    let exe = cfg
        .exe_path
        .as_deref()
        .ok_or_else(|| Error::msg(format!("Set the {} executable path in Settings → Emulators.", cfg.emulator_name)))?;
    let core = match (&cfg.core, cfg.emulator_id.as_str()) {
        (Some(c), "retroarch") => Some(template::resolve_core(cores_dir, c)?),
        _ => None,
    };

    let launch = PathBuf::from(&game.launch_path);
    let own_playlist = launch.extension().is_some_and(|e| e.eq_ignore_ascii_case("m3u"));
    let mut playlist = None;
    let rom = if game.is_multidisc && cfg.supports_m3u && !own_playlist && game.files.len() > 1 {
        let discs: Vec<PathBuf> = game.files.iter().map(|(p, _)| PathBuf::from(p)).collect();
        let path = playlist_dir.join(format!("game-{}.m3u", game.id));
        playlist = Some((path.clone(), cue_m3u::build_m3u(&discs)));
        path
    } else {
        launch
    };

    let ctx = template::Context { emulator: exe.into(), core, rom };
    Ok(Prepared { argv: template::render(&cfg.template, &ctx)?, playlist })
}

/// Checks the files a launch needs, with messages a user can act on.
fn check_files(argv: &[String], cfg: &LaunchConfig, game: &GameDetail, core: Option<&Path>) -> Result<()> {
    if !Path::new(&argv[0]).is_file() {
        return Err(Error::msg(format!("{} not found at {}", cfg.emulator_name, argv[0])));
    }
    if let Some(core) = core {
        if !core.is_file() {
            return Err(Error::msg(format!(
                "RetroArch core not found: {}. Check the cores folder or the platform's core in Settings.",
                core.display()
            )));
        }
    }
    if !Path::new(&game.launch_path).exists() {
        return Err(Error::msg(format!("Game file not found: {} (rescan the library?)", game.launch_path)));
    }
    Ok(())
}

pub struct Session {
    pub game_id: i64,
    pub title: String,
    tracker: Tracker,
    exiting: AtomicBool,
    /// Restored when the game exits (minimize/restore drops fullscreen).
    was_fullscreen: bool,
}

#[derive(Default)]
pub struct Launcher {
    current: Mutex<Option<Arc<Session>>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    pub game_id: i64,
    pub title: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exited {
    pub game_id: i64,
    pub session_seconds: i64,
}

/// Grace period between asking the emulator to close and force-killing it.
const EXIT_GRACE: Duration = Duration::from_secs(4);

impl Launcher {
    pub fn running(&self) -> Option<Started> {
        let cur = self.current.lock().unwrap_or_else(|e| e.into_inner());
        cur.as_ref().map(|s| Started { game_id: s.game_id, title: s.title.clone() })
    }

    pub fn launch(self: &Arc<Self>, app: &AppHandle, game_id: i64) -> Result<Started> {
        let state = app.state::<crate::commands::AppState>();
        let mut slot = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = slot.as_ref() {
            return Err(Error::msg(format!("{} is already running.", s.title)));
        }

        let (game, cfg, cores_dir) = {
            let conn = state.db();
            let game = queries::game_detail(&conn, game_id)?;
            let cfg = queries::launch_config(&conn, &game.platform_id)?;
            let cores: Option<String> = queries::get_setting(&conn, "retroarchCoresDir")?;
            (game, cfg, cores.map(PathBuf::from))
        };
        let playlist_dir = app.path().app_data_dir().map_err(|e| Error::msg(e.to_string()))?.join("playlists");
        let prepared = prepare(&game, &cfg, cores_dir.as_deref(), &playlist_dir)?;
        let core_path = match (&cfg.core, cfg.emulator_id.as_str()) {
            (Some(c), "retroarch") => Some(template::resolve_core(cores_dir.as_deref(), c)?),
            _ => None,
        };
        check_files(&prepared.argv, &cfg, &game, core_path.as_deref())?;
        if let Some((path, body)) = &prepared.playlist {
            std::fs::create_dir_all(&playlist_dir)?;
            std::fs::write(path, body)?;
        }

        log::info!("launching {}: {:?}", game.title, prepared.argv);
        let exe = Path::new(&prepared.argv[0]);
        let child = Command::new(exe)
            .args(&prepared.argv[1..])
            .current_dir(exe.parent().unwrap_or(Path::new(".")))
            .spawn()
            .map_err(|e| Error::msg(format!("couldn't start {}: {e}", cfg.emulator_name)))?;
        let tracker = Tracker::attach(child)?;

        let window = app.get_webview_window("main");
        let was_fullscreen = window.as_ref().and_then(|w| w.is_fullscreen().ok()).unwrap_or(false);
        let session = Arc::new(Session {
            game_id,
            title: game.title.clone(),
            tracker,
            exiting: AtomicBool::new(false),
            was_fullscreen,
        });
        *slot = Some(session.clone());
        drop(slot);

        queries::start_session(&state.db(), game_id, queries::now())?;
        state.input.game_running.store(true, Ordering::Release);
        let started = Started { game_id, title: game.title.clone() };
        let _ = app.emit("game-started", &started);
        if let Some(w) = window {
            // Minimized, WebView2 stops rendering; it also gets out of the
            // emulator's way. (Hiding the window instead risks WebView2 not
            // repainting when it's shown again.)
            let _ = w.minimize();
        }

        let me = self.clone();
        let app = app.clone();
        std::thread::Builder::new()
            .name("game-wait".into())
            .spawn(move || {
                let t0 = Instant::now();
                session.tracker.wait();
                me.finish(&app, &session, t0.elapsed());
            })
            .map_err(|e| Error::msg(format!("couldn't start the session thread: {e}")))?;
        Ok(started)
    }

    fn finish(&self, app: &AppHandle, session: &Session, played: Duration) {
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let state = app.state::<crate::commands::AppState>();
        state.input.game_running.store(false, Ordering::Release);
        let secs = played.as_secs() as i64;
        if let Err(e) = queries::end_session(&state.db(), session.game_id, secs, queries::now()) {
            log::error!("couldn't record play time: {e}");
        }
        log::info!("{} exited after {secs} s", session.title);
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            if session.was_fullscreen {
                // Restoring from minimized leaves the window flagged fullscreen
                // but drawn windowed, so toggle to re-apply it.
                let _ = w.set_fullscreen(false);
                let _ = w.set_fullscreen(true);
            }
            #[cfg(windows)]
            if let Ok(hwnd) = w.hwnd() {
                window::bring_to_front(hwnd);
            }
            #[cfg(not(windows))]
            let _ = w.set_focus();
        }
        let _ = app.emit("game-exited", &Exited { game_id: session.game_id, session_seconds: secs });
    }

    /// Closes the running game: politely first, forcefully after a grace period.
    pub fn request_exit(&self) {
        let Some(session) = self.current.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return };
        if session.exiting.swap(true, Ordering::AcqRel) {
            return; // already exiting
        }
        let asked = session.tracker.close_windows();
        log::info!("exit requested for {}: sent WM_CLOSE to {asked} window(s)", session.title);
        let _ = std::thread::Builder::new().name("game-exit".into()).spawn(move || {
            let deadline = Instant::now() + EXIT_GRACE;
            while Instant::now() < deadline {
                if !session.tracker.is_running() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            log::warn!("{} didn't close within {EXIT_GRACE:?}; terminating", session.title);
            session.tracker.terminate();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(launch: &str, files: &[&str], multi: bool) -> GameDetail {
        GameDetail {
            id: 7,
            platform_id: "ps1".into(),
            title: "Final Fantasy VII".into(),
            variant: None,
            region: None,
            serial: None,
            launch_path: launch.into(),
            total_size: 0,
            is_multidisc: multi,
            play_seconds: 0,
            last_played: None,
            art: 0,
            art_source: None,
            files: files.iter().enumerate().map(|(i, f)| (f.to_string(), Some(i as u32 + 1))).collect(),
        }
    }

    fn cfg(emu: &str, template: &str, core: Option<&str>, m3u: bool) -> LaunchConfig {
        LaunchConfig {
            emulator_id: emu.into(),
            emulator_name: emu.into(),
            exe_path: Some(format!(r"E:\Emulators\{emu}.exe")),
            template: template.into(),
            core: core.map(Into::into),
            supports_m3u: m3u,
        }
    }

    #[test]
    fn multidisc_without_playlist_gets_generated_m3u() {
        let g = game(r"E:\PS1\FF7 (Disc 1).cue", &[r"E:\PS1\FF7 (Disc 1).cue", r"E:\PS1\FF7 (Disc 2).cue"], true);
        let c = cfg("duckstation", r#""{emulator}" -batch -fullscreen -- "{rom}""#, None, true);
        let p = prepare(&g, &c, None, Path::new(r"C:\data\playlists")).unwrap();
        let (path, body) = p.playlist.unwrap();
        assert_eq!(path, Path::new(r"C:\data\playlists\game-7.m3u"));
        assert_eq!(body, "E:\\PS1\\FF7 (Disc 1).cue\r\nE:\\PS1\\FF7 (Disc 2).cue\r\n");
        assert_eq!(p.argv.last().unwrap(), r"C:\data\playlists\game-7.m3u");
    }

    #[test]
    fn emulator_without_m3u_support_boots_disc_one() {
        let g = game(r"E:\PS2\Game (Disc 1).iso", &[r"E:\PS2\Game (Disc 1).iso", r"E:\PS2\Game (Disc 2).iso"], true);
        let c = cfg("pcsx2", r#""{emulator}" -batch -- "{rom}""#, None, false);
        let p = prepare(&g, &c, None, Path::new("x")).unwrap();
        assert_eq!(p.playlist, None);
        assert_eq!(p.argv.last().unwrap(), r"E:\PS2\Game (Disc 1).iso");
    }

    #[test]
    fn existing_m3u_is_used_directly() {
        let g = game(r"E:\PS1\Game X.m3u", &[r"E:\PS1\Game X.m3u"], false);
        let c = cfg("duckstation", r#""{emulator}" "{rom}""#, None, true);
        let p = prepare(&g, &c, None, Path::new("x")).unwrap();
        assert_eq!(p.playlist, None);
        assert_eq!(p.argv[1], r"E:\PS1\Game X.m3u");
    }

    #[test]
    fn retroarch_core_resolved_and_missing_exe_reported() {
        let g = game(r"E:\GBA\Golden Sun.gba", &[r"E:\GBA\Golden Sun.gba"], false);
        let c = cfg("retroarch", r#""{emulator}" -L "{core}" -f "{rom}""#, Some("mgba"), true);
        let p = prepare(&g, &c, Some(Path::new(r"E:\RA\cores")), Path::new("x")).unwrap();
        assert_eq!(p.argv[2], r"E:\RA\cores\mgba_libretro.dll");

        let mut no_exe = c;
        no_exe.exe_path = None;
        let err = prepare(&g, &no_exe, None, Path::new("x")).unwrap_err().to_string();
        assert!(err.contains("Settings"), "{err}");
    }

    #[test]
    fn core_is_ignored_for_standalone_emulators() {
        let g = game(r"E:\3DS\x.cci", &[r"E:\3DS\x.cci"], false);
        // Even if a core value is set, a non-RetroArch template never gets one.
        let c = cfg("azahar", r#""{emulator}" -f "{rom}""#, Some("junk"), false);
        assert!(prepare(&g, &c, None, Path::new("x")).is_ok());
    }
}
