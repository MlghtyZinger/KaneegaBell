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

/// The emulator a launch uses, and why it isn't the one the game asked for
/// (when it isn't).
#[derive(Debug)]
pub struct Choice {
    pub cfg: LaunchConfig,
    pub notice: Option<String>,
}

/// Why an emulator can't run a game right now (exe or RetroArch core
/// missing), or None when it can.
pub fn not_ready(cfg: &LaunchConfig, cores_dir: Option<&Path>) -> Option<String> {
    match cfg.exe_path.as_deref() {
        None => return Some(format!("{} isn't set up yet (Settings → Emulators).", cfg.emulator_name)),
        Some(p) if !Path::new(p).is_file() => return Some(format!("{} wasn't found at {p}.", cfg.emulator_name)),
        _ => {}
    }
    if cfg.emulator_id == "retroarch" {
        let Some(core) = cfg.core.as_deref() else {
            return Some("No RetroArch core is set for this platform (Settings → Platforms).".into());
        };
        match template::resolve_core(cores_dir, core) {
            Ok(path) if path.is_file() => {}
            _ => return Some(format!("RetroArch's {core} core isn't installed.")),
        }
    }
    None
}

/// What KB can see may stop a RetroArch launch but can't be sure of: a core
/// that needs a BIOS RetroArch's system folder doesn't seem to have.
pub fn bios_warning(cfg: &LaunchConfig, platform_id: &str) -> Option<String> {
    if cfg.emulator_id != "retroarch" {
        return None;
    }
    let exe = cfg.exe_path.as_deref()?;
    let core = cfg.core.as_deref()?;
    crate::platforms::retroarch_bios_problem(core, platform_id, &crate::platforms::retroarch_system_dir(Path::new(exe)))
}

/// For Settings: what's wrong with a RetroArch core on a platform (not set,
/// not installed, BIOS missing), or None.
pub fn core_problem(core: Option<&str>, platform_id: &str, retroarch_exe: Option<&str>, cores_dir: Option<&Path>) -> Option<String> {
    let Some(core) = core else { return Some("No core set.".into()) };
    match template::resolve_core(cores_dir, core) {
        Ok(path) if path.is_file() => {}
        Ok(path) => return Some(format!("Not installed ({} not found).", path.display())),
        Err(e) => return Some(e.to_string()),
    }
    let exe = retroarch_exe?;
    crate::platforms::retroarch_bios_problem(core, platform_id, &crate::platforms::retroarch_system_dir(Path::new(exe)))
}

/// Whether the window is fullscreen and fills its whole screen (a mode change
/// by the emulator can leave a "fullscreen" window at the old size).
fn covers_its_screen(w: &tauri::WebviewWindow) -> bool {
    let (Ok(true), Ok(size), Ok(Some(monitor))) = (w.is_fullscreen(), w.outer_size(), w.current_monitor()) else {
        return false;
    };
    let screen = monitor.size();
    size.width >= screen.width && size.height >= screen.height
}

/// An emulator that exits this soon (without KB asking it to) didn't start
/// the game: KB says so and doesn't count it as played.
const QUICK_EXIT: Duration = Duration::from_secs(5);

fn quick_exit_message(title: &str, emulator: &str, secs: u64, bios: Option<&str>) -> String {
    let hint = match bios {
        Some(b) => format!(" {b}"),
        None if emulator == "RetroArch" => " Check the core in RetroArch (some need BIOS files in RetroArch's system folder).".into(),
        None => format!(" Try starting the game in {emulator} itself to see why."),
    };
    let after = if secs == 0 { "right away".to_string() } else { format!("after {secs} s") };
    format!("{emulator} closed {after} without running {title}.{hint}")
}

/// Picks the emulator for a launch: the game's own pick, else the platform's
/// default. A game's pick that can't run (not set up, removed) falls back to
/// the default with a notice saying so.
pub fn choose(conn: &rusqlite::Connection, game: &GameDetail, cores_dir: Option<&Path>) -> Result<Choice> {
    let platform = &game.platform_id;
    let default = queries::platform_default(conn, platform)?;
    let mut notice = None;
    if let Some(pick) = queries::game_emulator_choice(conn, game.id)?.filter(|p| *p != default) {
        match queries::launch_config(conn, platform, &pick) {
            Ok(cfg) => match not_ready(&cfg, cores_dir) {
                None => return Ok(Choice { cfg, notice: None }),
                Some(why) => notice = Some(why),
            },
            Err(_) => notice = Some("The emulator picked for this game isn't available any more.".into()),
        }
    }
    let cfg = queries::launch_config(conn, platform, &default)?;
    let notice = notice.map(|why| format!("{why} Playing with {} instead.", cfg.emulator_name));
    Ok(Choice { cfg, notice })
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
    emulator: String,
    notice: Option<String>,
    /// See `bios_warning`; said if the emulator closes right away.
    bios: Option<String>,
    tracker: Tracker,
    exiting: AtomicBool,
    /// Restored when the game exits (minimize/restore drops fullscreen).
    was_fullscreen: bool,
}

#[derive(Default)]
pub struct Launcher {
    current: Mutex<Option<Arc<Session>>>,
    /// KB's window before the manual came up over the game (position, size, fullscreen), restored after.
    before_manual: Arc<Mutex<Option<Before>>>,
}

type Before = (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>, bool);

/// The picture of the game's screen behind its manual (PNG), served as `/overlay-bg`.
pub static OVERLAY_BG: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// Counts manual openings over a game, so a late fallback doesn't act on a newer one.
static OVERLAY_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The game's whole screen at full size, as it is now (KB shows it sharp first, so coming up over the
/// game is seamless, then blurred); None when the capture is (nearly) black.
#[cfg(windows)]
fn game_backdrop(session: &Session) -> Option<Vec<u8>> {
    let hwnd = windows::Win32::Foundation::HWND(*session.tracker.windows().first()? as *mut std::ffi::c_void);
    let shot = window::capture(window::monitor_rect(hwnd)?)?;
    let small = image::imageops::thumbnail(&shot, 64, 64);
    let mean = small.pixels().map(|p| p.0[..3].iter().map(|&c| c as u64).sum::<u64>()).sum::<u64>() / (small.len() as u64 / 4).max(1);
    if mean < 6 {
        return None;
    }
    // Fast PNG: a 4K screen in a few dozen ms, sharp (it must match the game exactly).
    let mut out = Vec::new();
    let enc = image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Sub);
    image::ImageEncoder::write_image(enc, shot.as_raw(), shot.width(), shot.height(), image::ExtendedColorType::Rgba8).ok()?;
    Some(out)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    pub game_id: i64,
    pub title: String,
    /// The emulator running it.
    pub emulator: String,
    /// Why it isn't the emulator picked for the game (see `choose`).
    pub notice: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exited {
    pub game_id: i64,
    pub session_seconds: i64,
    /// Set when the emulator closed right away without running the game.
    pub failed: Option<String>,
}

/// Grace period between asking the emulator to close and force-killing it.
const EXIT_GRACE: Duration = Duration::from_secs(4);

impl Launcher {
    pub fn running(&self) -> Option<Started> {
        let cur = self.current.lock().unwrap_or_else(|e| e.into_inner());
        cur.as_ref().map(|s| Started {
            game_id: s.game_id,
            title: s.title.clone(),
            emulator: s.emulator.clone(),
            notice: s.notice.clone(),
        })
    }

    /// Select + △ held while a game runs: KB comes up over the game with its manual
    /// (page awake, always on top); held again, or the manual closed, KB goes back behind the game.
    pub fn toggle_manual(&self, app: &AppHandle) {
        if !app.state::<crate::commands::AppState>().input.overlay.load(Ordering::Acquire) {
            return self.set_manual(app, true);
        }
        // The page fades back to the game's picture first, then calls `close_manual`; closed anyway if it doesn't.
        let gen = OVERLAY_GEN.load(Ordering::Acquire);
        let _ = app.emit("manual-overlay-leave", ());
        let app = app.clone();
        let _ = std::thread::Builder::new().name("manual-leave".into()).spawn(move || {
            std::thread::sleep(Duration::from_millis(900));
            if OVERLAY_GEN.load(Ordering::Acquire) == gen {
                app.state::<crate::commands::AppState>().launcher.close_manual(&app);
            }
        });
    }

    /// The page has drawn the game's picture over the whole screen: KB can be seen now.
    pub fn manual_shown(&self, app: &AppHandle) {
        #[cfg(windows)]
        if let Some(w) = app.get_webview_window("main") {
            if app.state::<crate::commands::AppState>().input.overlay.load(Ordering::Acquire) {
                if let Ok(hwnd) = w.hwnd() {
                    window::set_cloaked(hwnd, false);
                }
            }
        }
    }

    pub fn close_manual(&self, app: &AppHandle) {
        if app.state::<crate::commands::AppState>().input.overlay.load(Ordering::Acquire) {
            self.set_manual(app, false);
        }
    }

    fn set_manual(&self, app: &AppHandle, open: bool) {
        let Some(session) = self.current.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return };
        if session.exiting.load(Ordering::Acquire) {
            return;
        }
        let input = app.state::<crate::commands::AppState>().input.clone();
        input.overlay.store(open, Ordering::Release);
        let gen = OVERLAY_GEN.fetch_add(1, Ordering::AcqRel) + 1;
        let Some(w) = app.get_webview_window("main") else { return };
        #[cfg(windows)]
        let hwnd = w.hwnd().ok();
        if open {
            // The game's screen as it is now, before KB covers it.
            #[allow(unused_mut, unused_assignments)]
            let mut backdrop = false;
            #[cfg(windows)]
            {
                let shot = game_backdrop(&session);
                backdrop = shot.is_some();
                *OVERLAY_BG.lock().unwrap_or_else(|e| e.into_inner()) = shot;
                window::set_webview_suspended(&w, false);
                window::set_webview_low_memory(&w, false);
                // Unseen while it moves into place: shown (`manual_shown`) once the page shows that picture.
                if let Some(h) = hwnd {
                    window::set_transitions(h, false);
                    window::set_cloaked(h, true);
                }
            }
            let _ = w.unminimize();
            let _ = w.show();
            // Fullscreen on the game's screen; put back as it was afterwards.
            let mut before = self.before_manual.lock().unwrap_or_else(|e| e.into_inner());
            if let (None, Ok(pos), Ok(size)) = (before.as_ref(), w.outer_position(), w.outer_size()) {
                *before = Some((pos, size, w.is_fullscreen().unwrap_or(false)));
            }
            drop(before);
            let _ = w.set_fullscreen(false);
            #[cfg(windows)]
            if let Some((x, y, _, _)) = session.tracker.windows().first().and_then(|h| window::window_rect(windows::Win32::Foundation::HWND(*h as *mut std::ffi::c_void))) {
                let _ = w.set_position(tauri::PhysicalPosition::new(x + 40, y + 40));
            }
            let _ = w.set_fullscreen(true);
            let _ = w.set_always_on_top(true);
            // In place: the page draws the manual over the game's picture, then asks to be seen.
            let _ = app.emit("manual-overlay", serde_json::json!({ "gameId": session.game_id, "open": true, "backdrop": backdrop }));
            #[cfg(windows)]
            if let Some(h) = hwnd {
                window::bring_to_front(h);
                // Shown anyway if the page never says it's ready.
                let h = h.0 as isize;
                let input = input.clone();
                let _ = std::thread::Builder::new().name("manual-show".into()).spawn(move || {
                    std::thread::sleep(Duration::from_millis(1200));
                    if OVERLAY_GEN.load(Ordering::Acquire) == gen && input.overlay.load(Ordering::Acquire) {
                        window::set_cloaked(windows::Win32::Foundation::HWND(h as *mut std::ffi::c_void), false);
                    }
                });
            }
        } else {
            // Gone in one frame (the page already faded to the game's picture), then put back behind the game unseen.
            #[cfg(windows)]
            if let Some(h) = hwnd {
                window::set_cloaked(h, true);
            }
            let _ = w.set_always_on_top(false);
            restore_window(&w, &self.before_manual);
            let _ = w.minimize();
            let _ = app.emit("manual-overlay", serde_json::json!({ "gameId": session.game_id, "open": false, "backdrop": false }));
            #[cfg(windows)]
            {
                window::set_webview_low_memory(&w, true);
                if let Some(h) = session.tracker.windows().first() {
                    window::focus_game(windows::Win32::Foundation::HWND(*h as *mut std::ffi::c_void));
                }
                let w = w.clone();
                let _ = std::thread::Builder::new().name("suspend-page".into()).spawn(move || {
                    // Uncloaked once it's down (a minimized window isn't seen), so the taskbar can show it again.
                    std::thread::sleep(Duration::from_millis(300));
                    if !input.overlay.load(Ordering::Acquire) {
                        if let Ok(h) = w.hwnd() {
                            window::set_cloaked(h, false);
                            window::set_transitions(h, true);
                        }
                    }
                    std::thread::sleep(Duration::from_millis(1200));
                    if input.game_running.load(Ordering::Acquire) && !input.overlay.load(Ordering::Acquire) {
                        window::set_webview_suspended(&w, true);
                    }
                });
            }
        }
    }
}

/// KB's window back to where and how it was before the manual came up.
fn restore_window(w: &tauri::WebviewWindow, before: &Mutex<Option<Before>>) {
    if let Some((pos, size, fs)) = before.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = w.set_fullscreen(false);
        let _ = w.set_position(pos);
        let _ = w.set_size(size);
        if fs {
            let _ = w.set_fullscreen(true);
        }
    }
}

impl Launcher {

    /// Starts a game with its own emulator pick, else the platform's default.
    pub fn launch(self: &Arc<Self>, app: &AppHandle, game_id: i64) -> Result<Started> {
        let state = app.state::<crate::commands::AppState>();
        let mut slot = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = slot.as_ref() {
            return Err(Error::msg(format!("{} is already running.", s.title)));
        }

        let (game, Choice { cfg, notice }, cores_dir) = {
            let conn = state.db();
            let game = queries::game_detail(&conn, game_id)?;
            let cores: Option<String> = queries::get_setting(&conn, "retroarchCoresDir")?;
            let cores = cores.map(PathBuf::from);
            let choice = choose(&conn, &game, cores.as_deref())?;
            (game, choice, cores)
        };
        if let Some(n) = &notice {
            log::info!("{}: {n}", game.title);
        }
        let playlist_dir = state.data_dir().join("playlists");
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
            emulator: cfg.emulator_name.clone(),
            notice: notice.clone(),
            bios: bios_warning(&cfg, &game.platform_id),
            tracker,
            exiting: AtomicBool::new(false),
            was_fullscreen,
        });
        *slot = Some(session.clone());
        drop(slot);

        queries::start_session(&state.db(), game_id, queries::now())?;
        state.input.game_running.store(true, Ordering::Release);
        let started = Started { game_id, title: game.title.clone(), emulator: cfg.emulator_name.clone(), notice };
        let _ = app.emit("game-started", &started);
        if let Some(w) = window {
            // Minimized, WebView2 stops rendering; it also gets out of the
            // emulator's way. (Hiding the window instead risks WebView2 not
            // repainting when it's shown again.)
            let _ = w.minimize();
            #[cfg(windows)]
            {
                window::set_webview_low_memory(&w, true);
                // Once the page has swapped the library for the "now playing"
                // screen, suspend it (unless the game is already over).
                let state_flag = app.state::<crate::commands::AppState>().input.clone();
                let _ = std::thread::Builder::new().name("suspend-page".into()).spawn(move || {
                    std::thread::sleep(Duration::from_millis(1500));
                    if state_flag.game_running.load(Ordering::Acquire) {
                        window::set_webview_suspended(&w, true);
                    }
                });
            }
        }

        // The emulator often opens behind everything (KB was just minimized and
        // this process no longer owns the foreground), so a controller-only
        // player would never see it: bring each new game window to the front.
        #[cfg(windows)]
        {
            let session = session.clone();
            let _ = std::thread::Builder::new().name("game-focus".into()).spawn(move || {
                let mut seen: Vec<isize> = Vec::new();
                for _ in 0..40 {
                    std::thread::sleep(Duration::from_millis(500));
                    if session.exiting.load(Ordering::Acquire) {
                        return;
                    }
                    let now = session.tracker.windows();
                    if now.is_empty() && !session.tracker.is_running() {
                        return;
                    }
                    // shortcut: raises every new window once, even a splash, for 20 s; a window the user raised themselves isn't tracked.
                    let fresh: Vec<isize> = now.into_iter().filter(|h| !seen.contains(h)).collect();
                    for h in fresh {
                        seen.push(h);
                        window::focus_game(windows::Win32::Foundation::HWND(h as *mut std::ffi::c_void));
                    }
                }
            });
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
        state.input.overlay.store(false, Ordering::Release);
        let secs = played.as_secs() as i64;
        // Closed on its own within seconds: the game never ran.
        let failed = (played < QUICK_EXIT && !session.exiting.load(Ordering::Acquire))
            .then(|| quick_exit_message(&session.title, &session.emulator, played.as_secs(), session.bios.as_deref()));
        let recorded = match &failed {
            Some(_) => queries::cancel_session(&state.db(), session.game_id),
            None => queries::end_session(&state.db(), session.game_id, secs, queries::now()),
        };
        if let Err(e) = recorded {
            log::error!("couldn't record play time: {e}");
        }
        match &failed {
            Some(m) => log::warn!("{m}"),
            None => log::info!("{} exited after {secs} s", session.title),
        }
        if let Some(w) = app.get_webview_window("main") {
            // The manual open (or still fading out) over the game: KB back as it was, solid again.
            let _ = w.set_always_on_top(false);
            restore_window(&w, &self.before_manual);
            *OVERLAY_BG.lock().unwrap_or_else(|e| e.into_inner()) = None;
            #[cfg(windows)]
            {
                if let Ok(hwnd) = w.hwnd() {
                    window::set_cloaked(hwnd, false);
                    window::set_transitions(hwnd, true);
                }
                // Awake before the page hears that the game exited.
                window::set_webview_suspended(&w, false);
                window::set_webview_low_memory(&w, false);
            }
            let _ = w.unminimize();
            let _ = w.show();
            // To the front first: its restore step, run after fullscreen was
            // re-applied, could drop the window back to windowed.
            #[cfg(windows)]
            if let Ok(hwnd) = w.hwnd() {
                window::bring_to_front(hwnd);
            }
            #[cfg(not(windows))]
            let _ = w.set_focus();
            if session.was_fullscreen {
                // Restoring from minimized leaves the window flagged fullscreen
                // but drawn windowed, so toggle to re-apply it.
                let _ = w.set_fullscreen(false);
                let _ = w.set_fullscreen(true);
                // Those run on the window's thread a moment later, and an emulator
                // that changed the screen mode can leave it wrong: check once
                // things have settled, and re-apply if it isn't covering its screen.
                let w = w.clone();
                let _ = std::thread::Builder::new().name("refullscreen".into()).spawn(move || {
                    std::thread::sleep(Duration::from_millis(700));
                    if !covers_its_screen(&w) {
                        log::info!("window wasn't fullscreen after the game: re-applying");
                        let _ = w.set_fullscreen(false);
                        let _ = w.set_fullscreen(true);
                    }
                });
            }
        }
        let _ = app.emit("game-exited", &Exited { game_id: session.game_id, session_seconds: secs, failed });
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
            file_title: "Final Fantasy VII".into(),
            file_variant: None,
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

    /// A PS1 game, DuckStation set up, RetroArch added with the SwanStation
    /// core (installed), all in a temp folder.
    fn ps1_setup() -> (tempfile::TempDir, rusqlite::Connection, GameDetail) {
        let t = tempfile::tempdir().unwrap();
        let conn = crate::db::test_db();
        for (id, exe) in [("duckstation", "duckstation.exe"), ("retroarch", "retroarch.exe")] {
            let p = t.path().join(exe);
            std::fs::write(&p, b"MZ").unwrap();
            queries::update_emulator(&conn, id, Some(p.to_string_lossy().into_owned()), None).unwrap();
        }
        std::fs::create_dir_all(t.path().join("cores")).unwrap();
        std::fs::write(t.path().join("cores").join("swanstation_libretro.dll"), b"MZ").unwrap();
        queries::add_platform_emulator(&conn, "ps1", "retroarch").unwrap();
        conn.execute(
            "INSERT INTO games (platform_id, title, sort_title, search_key, launch_path, added_at)
             VALUES ('ps1', 'Silent Hill', 'Silent Hill', 'silent hill', 'E:\\PS1\\Silent Hill.cue', 0)",
            [],
        )
        .unwrap();
        let game = queries::game_detail(&conn, conn.last_insert_rowid()).unwrap();
        (t, conn, game)
    }

    #[test]
    fn a_games_pick_wins_and_falls_back_to_the_default_with_a_notice() {
        let (t, conn, game) = ps1_setup();
        let cores = t.path().join("cores");
        let pick = || choose(&conn, &game, Some(&cores)).unwrap();
        // Nothing picked: the platform's default (the built-in DuckStation).
        assert_eq!(pick().cfg.emulator_id, "duckstation");
        // The game picks RetroArch: it gets RetroArch with the suggested core.
        queries::set_game_emulator(&conn, game.id, Some("retroarch")).unwrap();
        let c = pick();
        assert_eq!((c.cfg.emulator_id.as_str(), c.cfg.core.as_deref(), c.notice), ("retroarch", Some("swanstation"), None));
        // Its core goes missing: DuckStation plays it, and the notice says why.
        std::fs::remove_file(cores.join("swanstation_libretro.dll")).unwrap();
        let c = pick();
        assert_eq!(c.cfg.emulator_id, "duckstation");
        assert_eq!(c.notice.as_deref(), Some("RetroArch's swanstation core isn't installed. Playing with DuckStation instead."));
    }

    #[test]
    fn the_platform_default_and_removing_an_emulator() {
        let (t, conn, game) = ps1_setup();
        let cores = t.path().join("cores");
        queries::set_platform_default(&conn, "ps1", "retroarch").unwrap();
        assert_eq!(choose(&conn, &game, Some(&cores)).unwrap().cfg.emulator_id, "retroarch");
        // A game can still pick the built-in one.
        queries::set_game_emulator(&conn, game.id, Some("duckstation")).unwrap();
        assert_eq!(choose(&conn, &game, Some(&cores)).unwrap().cfg.emulator_id, "duckstation");
        queries::set_game_emulator(&conn, game.id, Some("retroarch")).unwrap();
        // Taking RetroArch off PS1: the default and the game's pick go back to DuckStation.
        queries::remove_platform_emulator(&conn, "ps1", "retroarch").unwrap();
        assert_eq!(queries::platform_default(&conn, "ps1").unwrap(), "duckstation");
        assert_eq!(queries::game_emulator_choice(&conn, game.id).unwrap(), None);
        let c = choose(&conn, &game, Some(&cores)).unwrap();
        assert_eq!((c.cfg.emulator_id.as_str(), c.notice), ("duckstation", None));
        // PCSX2 can't be added to PS1; RetroArch can't be added twice.
        assert!(queries::add_platform_emulator(&conn, "ps1", "pcsx2").is_err());
        queries::add_platform_emulator(&conn, "ps1", "retroarch").unwrap();
        assert!(queries::add_platform_emulator(&conn, "ps1", "retroarch").is_err());
    }

    #[test]
    fn each_emulator_keeps_its_own_core_and_template_on_a_platform() {
        let (_t, conn, _game) = ps1_setup();
        conn.execute("UPDATE platforms SET template_override = '\"{emulator}\" -fast \"{rom}\"' WHERE id = 'ps1'", []).unwrap();
        queries::update_platform_emulator(&conn, "ps1", "retroarch", Some("mednafen_psx_hw".into()), None).unwrap();
        let duck = queries::launch_config(&conn, "ps1", "duckstation").unwrap();
        let ra = queries::launch_config(&conn, "ps1", "retroarch").unwrap();
        assert_eq!(duck.template, r#""{emulator}" -fast "{rom}""#, "the platform's override is the built-in emulator's");
        assert_eq!((ra.template.as_str(), ra.core.as_deref()), (r#""{emulator}" -L "{core}" -f "{rom}""#, Some("mednafen_psx_hw")));
    }

    #[test]
    fn the_users_own_emulator_on_any_platform() {
        let (t, conn, game) = ps1_setup();
        let exe = t.path().join("PPSSPPWindows64.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let id = queries::add_custom_emulator(&conn, "PPSSPP (standalone)", Some(exe.to_string_lossy().into_owned()), r#""{emulator}" "{rom}""#, false)
            .unwrap();
        assert_eq!(id, "custom-ppsspp-standalone");
        let again = queries::add_custom_emulator(&conn, "PPSSPP (standalone)", None, r#""{emulator}" "{rom}""#, false).unwrap();
        assert_eq!(again, "custom-ppsspp-standalone-2", "ids stay unique");
        let addable = queries::platforms(&conn).unwrap().into_iter().find(|p| p.id == "psp").unwrap().addable_emulators;
        assert!(addable.contains(&id) && !addable.contains(&"retroarch".to_string()), "{addable:?}");
        queries::add_platform_emulator(&conn, "ps1", &id).unwrap();
        queries::set_game_emulator(&conn, game.id, Some(&id)).unwrap();
        let c = choose(&conn, &game, Some(&t.path().join("cores"))).unwrap();
        assert_eq!(c.cfg.emulator_name, "PPSSPP (standalone)");
        let p = prepare(&game, &c.cfg, None, Path::new("x")).unwrap();
        assert_eq!(p.argv, [exe.to_string_lossy().into_owned(), r"E:\PS1\Silent Hill.cue".to_string()]);
        // Removed: its platforms and games go back to their defaults.
        queries::remove_custom_emulator(&conn, &id).unwrap();
        assert_eq!(queries::game_emulator_choice(&conn, game.id).unwrap(), None);
        assert_eq!(queries::platform_emulator_ids(&conn, "ps1").unwrap(), ["duckstation", "retroarch"]);
        // Built-in emulators can't be removed.
        assert!(queries::remove_custom_emulator(&conn, "duckstation").is_err());
    }

    #[test]
    fn an_emulator_closing_right_away_is_explained() {
        let bios = "RetroArch's pcsx2 core needs a PS2 BIOS file in E:\\RA\\system\\pcsx2\\bios, which is empty.";
        assert_eq!(
            quick_exit_message("Darkwatch", "RetroArch", 1, Some(bios)),
            format!("RetroArch closed after 1 s without running Darkwatch. {bios}")
        );
        assert!(quick_exit_message("Darkwatch", "RetroArch", 0, None).contains("right away"));
        assert!(quick_exit_message("Jak 3", "PCSX2", 2, None).ends_with("Try starting the game in PCSX2 itself to see why."));

        // Settings: a core that isn't installed, and the PS2 core without a BIOS.
        let (t, _conn, _game) = ps1_setup();
        let cores = t.path().join("cores");
        let ra = t.path().join("retroarch.exe").to_string_lossy().into_owned();
        assert!(core_problem(Some("pcsx2"), "ps2", Some(&ra), Some(&cores)).unwrap().starts_with("Not installed"));
        std::fs::write(cores.join("pcsx2_libretro.dll"), b"MZ").unwrap();
        assert!(core_problem(Some("pcsx2"), "ps2", Some(&ra), Some(&cores)).unwrap().contains("PS2 BIOS"));
        assert_eq!(core_problem(None, "ps2", Some(&ra), Some(&cores)).as_deref(), Some("No core set."));
    }

    #[test]
    fn core_is_ignored_for_standalone_emulators() {
        let g = game(r"E:\3DS\x.cci", &[r"E:\3DS\x.cci"], false);
        // Even if a core value is set, a non-RetroArch template never gets one.
        let c = cfg("azahar", r#""{emulator}" -f "{rom}""#, Some("junk"), false);
        assert!(prepare(&g, &c, None, Path::new("x")).is_ok());
    }
}
