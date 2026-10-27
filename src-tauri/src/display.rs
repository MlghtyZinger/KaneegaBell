//! Windows and displays:
//! - KB remembers where its window was (position, size, maximized) and puts
//!   it back on start, unless that spot is no longer on any connected
//!   display (then Windows' default placement is kept).
//! - TV Mode can open on a chosen display (the TV) and returns the window to
//!   where it was afterwards.
//! - On big screens at low Windows scaling (a 4K TV at 100%), the page is
//!   zoomed so it lays out like 1080p instead of shrinking into a corner of
//!   tiny text. Screens at or below 1080p-equivalent are untouched.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::db::queries;
use crate::error::Result;

pub const PLACEMENT_KEY: &str = "windowPlacement";

/// The window's last normal (not fullscreen, not minimized) placement.
/// Position in physical desktop pixels, size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: f64,
    pub height: f64,
    pub maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Rect {
    fn of_work_area(m: &Monitor) -> Rect {
        let a = m.work_area();
        Rect { x: a.position.x, y: a.position.y, w: a.size.width as i32, h: a.size.height as i32 }
    }
    fn of_monitor(m: &Monitor) -> Rect {
        Rect { x: m.position().x, y: m.position().y, w: m.size().width as i32, h: m.size().height as i32 }
    }
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    fn overlap(&self, o: &Rect) -> i64 {
        let w = (self.x + self.w).min(o.x + o.w) - self.x.max(o.x);
        let h = (self.y + self.h).min(o.y + o.h) - self.y.max(o.y);
        if w > 0 && h > 0 {
            w as i64 * h as i64
        } else {
            0
        }
    }
}

/// Enough of the window's title bar (the top strip, at least 160 px of it)
/// is on some display that the window can be grabbed and moved.
fn grabbable(window: Rect, areas: &[Rect]) -> bool {
    let bar = Rect { x: window.x, y: window.y, w: window.w, h: 32.min(window.h) };
    let need = (160.min(window.w) as i64) * bar.h as i64;
    areas.iter().any(|a| a.overlap(&bar) >= need)
}

/// Page zoom for a window of this logical size: 1080p-equivalent layout on
/// bigger screens, never smaller than 1.
pub fn auto_zoom(width: f64, height: f64) -> f64 {
    let z = (height / 1080.0).min(width / 1920.0).max(1.0);
    (z * 100.0).round() / 100.0
}

/// Page zoom for the Interface size setting (owner, 2026-10-27: "like Windows"). Unset (or the old
/// "auto"/"fixed"): Windows' own display scaling, as Windows shows everything else. "fit": big screens
/// laid out like 1080p. A percentage ("150"): that scale, as if Windows were set to it.
pub fn zoom_for(setting: Option<&str>, width: f64, height: f64, windows_scale: f64) -> f64 {
    let zoom = match setting {
        Some("fit") => auto_zoom(width, height),
        Some(v) => v.parse::<f64>().map(|pct| pct / 100.0 / windows_scale.max(0.5)).unwrap_or(1.0),
        None => 1.0,
    };
    (zoom.clamp(0.25, 5.0) * 100.0).round() / 100.0
}

// ---------------------------------------------------------------- placement

/// Puts the window back where it was (called during startup, before the
/// first paint).
pub fn restore(window: &WebviewWindow, conn: &Connection) {
    let Some(p) = queries::get_setting::<Placement>(conn, PLACEMENT_KEY).ok().flatten() else { return };
    let monitors = window.available_monitors().unwrap_or_default();
    // The display the window's top-left corner lands on decides its scaling.
    let Some(target) = monitors.iter().find(|m| Rect::of_monitor(m).contains(p.x + 8, p.y + 8)) else {
        log::info!("the window's last display isn't connected: using the default placement");
        return;
    };
    let scale = target.scale_factor();
    let size = PhysicalSize::new((p.width * scale).round() as u32, (p.height * scale).round() as u32);
    let rect = Rect { x: p.x, y: p.y, w: size.width as i32, h: size.height as i32 };
    let areas: Vec<Rect> = monitors.iter().map(Rect::of_work_area).collect();
    if !grabbable(rect, &areas) {
        log::info!("the window's last position is off screen now: using the default placement");
        return;
    }
    let _ = window.set_position(PhysicalPosition::new(p.x, p.y));
    let _ = window.set_size(size);
    if p.maximized {
        let _ = window.maximize();
    }
}

static SAVE_SCHEDULED: AtomicBool = AtomicBool::new(false);
static LAST_CHANGE: Mutex<Option<Instant>> = Mutex::new(None);

/// Called on every move/resize: saves the placement once the window has
/// been still for a moment (one short-lived thread per drag, no polling).
pub fn schedule_save(app: &AppHandle) {
    *LAST_CHANGE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    if SAVE_SCHEDULED.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    let _ = std::thread::Builder::new().name("save-placement".into()).spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(700));
            let quiet = LAST_CHANGE
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(600));
            if quiet {
                break;
            }
        }
        SAVE_SCHEDULED.store(false, Ordering::Release);
        if let Err(e) = save(&app) {
            log::warn!("couldn't save the window placement: {e}");
        }
    });
}

/// Saves the placement now (also called when the window closes, so a move
/// right before closing isn't lost).
pub fn save(app: &AppHandle) -> Result<()> {
    let Some(window) = app.get_webview_window("main") else { return Ok(()) };
    let state = app.state::<crate::commands::AppState>();
    // Minimized (also while a game runs) and TV Mode on another display
    // aren't the desktop window's own placement.
    if window.is_minimized()? || state.input.game_running.load(Ordering::Acquire) || tv_session_active(app) {
        return Ok(());
    }
    let conn = state.db();
    if window.is_fullscreen()? {
        // Fullscreen has no size of its own, but the display counts ("Start
        // fullscreen" opens there next time): keep the windowed size, moved
        // onto this display if it was on another one.
        let Some(monitor) = window.current_monitor()? else { return Ok(()) };
        let previous = queries::get_setting::<Placement>(&conn, PLACEMENT_KEY)?;
        let Some(moved) = onto_monitor(previous, &monitor) else { return Ok(()) };
        return queries::set_setting(&conn, PLACEMENT_KEY, &serde_json::to_value(moved)?);
    }
    let placement = if window.is_maximized()? {
        // Keep the size it returns to when un-maximized.
        let previous = queries::get_setting::<Placement>(&conn, PLACEMENT_KEY)?;
        match previous {
            Some(p) => Placement { maximized: true, ..p },
            None => current_placement(&window, true)?,
        }
    } else {
        current_placement(&window, false)?
    };
    queries::set_setting(&conn, PLACEMENT_KEY, &serde_json::to_value(placement)?)
}

/// The saved placement moved onto `monitor` (centered in its work area, same
/// size, shrunk to fit), or None when it's already there.
fn onto_monitor(previous: Option<Placement>, monitor: &Monitor) -> Option<Placement> {
    let area = Rect::of_work_area(monitor);
    let screen = Rect::of_monitor(monitor);
    let p = previous.unwrap_or(Placement { x: 0, y: 0, width: 1600.0, height: 900.0, maximized: false });
    if previous.is_some() && screen.contains(p.x + 8, p.y + 8) {
        return None;
    }
    Some(centered(p, area, monitor.scale_factor()))
}

fn centered(p: Placement, area: Rect, scale: f64) -> Placement {
    let width = p.width.min(area.w as f64 / scale * 0.9);
    let height = p.height.min(area.h as f64 / scale * 0.9);
    Placement {
        x: area.x + ((area.w as f64 - width * scale) / 2.0).round() as i32,
        y: area.y + ((area.h as f64 - height * scale) / 2.0).round() as i32,
        width,
        height,
        maximized: p.maximized,
    }
}

fn current_placement(window: &WebviewWindow, maximized: bool) -> Result<Placement> {
    let pos = window.outer_position()?;
    let size = window.inner_size()?.to_logical::<f64>(window.scale_factor()?);
    Ok(Placement { x: pos.x, y: pos.y, width: size.width, height: size.height, maximized })
}

// ---------------------------------------------------------------- zoom

static ZOOM: AtomicU64 = AtomicU64::new(0);

/// Re-zooms the page for the window's current size and scaling (on start,
/// resize, display or scaling changes, and the Interface size setting).
pub fn apply_zoom(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) else { return };
    if size.width == 0 || size.height == 0 {
        return; // minimized
    }
    let setting = app
        .try_state::<crate::commands::AppState>()
        .and_then(|s| queries::get_setting::<String>(&s.db(), "interfaceSize").ok().flatten());
    let logical = size.to_logical::<f64>(scale);
    let zoom = zoom_for(setting.as_deref(), logical.width, logical.height, scale);
    if ZOOM.swap(zoom.to_bits(), Ordering::AcqRel) != zoom.to_bits() {
        log::info!("page zoom {zoom} for a {:.0}x{:.0} window", logical.width, logical.height);
        if let Err(e) = window.set_zoom(zoom) {
            log::warn!("couldn't zoom the page: {e}");
        }
    }
}

// ---------------------------------------------------------------- displays

/// A connected display, for the TV Mode display picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayRow {
    /// Stable id (the monitor's device path, which survives reboots and
    /// re-plugging; the GDI name like \\.\DISPLAY2 doesn't).
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub primary: bool,
    /// The window is on it now.
    pub current: bool,
}

struct OsName {
    gdi: String,
    friendly: String,
    path: String,
}

#[cfg(windows)]
fn os_display_names() -> Vec<OsName> {
    use windows::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
        DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    };
    use windows::Win32::Foundation::ERROR_SUCCESS;

    let wide = |s: &[u16]| String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]);
    let mut out = Vec::new();
    // SAFETY: buffers are sized by GetDisplayConfigBufferSizes and the request
    // packets carry their own sizes.
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) != ERROR_SUCCESS {
            return out;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None)
            != ERROR_SUCCESS
        {
            return out;
        }
        for p in &paths[..np as usize] {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            source.header.adapterId = p.sourceInfo.adapterId;
            source.header.id = p.sourceInfo.id;
            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            target.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            target.header.adapterId = p.targetInfo.adapterId;
            target.header.id = p.targetInfo.id;
            if DisplayConfigGetDeviceInfo(&mut source.header) == 0 && DisplayConfigGetDeviceInfo(&mut target.header) == 0 {
                out.push(OsName {
                    gdi: wide(&source.viewGdiDeviceName),
                    friendly: wide(&target.monitorFriendlyDeviceName),
                    path: wide(&target.monitorDevicePath),
                });
            }
        }
    }
    out
}

#[cfg(not(windows))]
fn os_display_names() -> Vec<OsName> {
    Vec::new()
}

/// Connected displays with their friendly names ("LG ULTRAGEAR", "SONY TV").
pub fn list(window: &WebviewWindow) -> Vec<DisplayRow> {
    let names = os_display_names();
    let primary = window.primary_monitor().ok().flatten().and_then(|m| m.name().cloned());
    let current = window.current_monitor().ok().flatten().and_then(|m| m.name().cloned());
    window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let gdi = m.name().cloned().unwrap_or_default();
            let os = names.iter().find(|n| n.gdi.eq_ignore_ascii_case(&gdi));
            DisplayRow {
                id: os.map(|n| n.path.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| gdi.clone()),
                name: os
                    .map(|n| n.friendly.clone())
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| format!("Display {}", i + 1)),
                width: m.size().width,
                height: m.size().height,
                scale: m.scale_factor(),
                primary: primary.as_deref() == Some(gdi.as_str()),
                current: current.as_deref() == Some(gdi.as_str()),
            }
        })
        .collect()
}

fn find_monitor(window: &WebviewWindow, id: &str) -> Option<Monitor> {
    let names = os_display_names();
    let gdi = names.iter().find(|n| n.path == id).map(|n| n.gdi.clone()).unwrap_or_else(|| id.to_string());
    window
        .available_monitors()
        .ok()?
        .into_iter()
        .find(|m| m.name().is_some_and(|n| n.eq_ignore_ascii_case(&gdi)))
}

// ---------------------------------------------------------------- TV Mode

/// Where the window was before TV Mode moved it to the TV.
#[derive(Debug, Clone, Copy)]
struct Before {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    maximized: bool,
    fullscreen: bool,
}

#[derive(Debug)]
struct TvSession {
    /// KB turned fullscreen on for TV Mode (turned off again on leaving).
    made_fullscreen: bool,
    /// Moved to the TV display from here.
    moved: Option<Before>,
    /// The chosen TV display wasn't connected (its saved id).
    missing: bool,
}

#[derive(Default)]
pub struct TvState(Mutex<Option<TvSession>>);

fn tv_session_active(app: &AppHandle) -> bool {
    app.try_state::<TvState>().is_some_and(|s| s.0.lock().unwrap_or_else(|e| e.into_inner()).is_some())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TvEntered {
    /// The chosen TV display isn't connected: TV Mode opened where the window is.
    pub display_missing: bool,
}

/// Enters TV Mode's window state: on the chosen display (if any) and
/// fullscreen. Calling it again while in TV Mode changes nothing.
pub fn tv_enter(app: &AppHandle, window: &WebviewWindow) -> Result<TvEntered> {
    let tv = app.state::<TvState>();
    let mut slot = tv.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = slot.as_ref() {
        return Ok(TvEntered { display_missing: s.missing });
    }
    let wanted = {
        let state = app.state::<crate::commands::AppState>();
        let conn = state.db();
        queries::get_setting::<String>(&conn, "tvDisplay")?.filter(|s| !s.is_empty())
    };
    let was_fullscreen = window.is_fullscreen()?;
    let mut session = TvSession { made_fullscreen: false, moved: None, missing: false };
    if let Some(id) = wanted {
        match find_monitor(window, &id) {
            None => session.missing = true,
            Some(m) => {
                let here = window.current_monitor()?.and_then(|c| c.name().cloned());
                if here.as_deref() != m.name().map(String::as_str) {
                    let before = Before {
                        position: window.outer_position()?,
                        size: window.inner_size()?,
                        maximized: window.is_maximized()?,
                        fullscreen: was_fullscreen,
                    };
                    if was_fullscreen {
                        window.set_fullscreen(false)?;
                    }
                    if before.maximized {
                        window.unmaximize()?;
                    }
                    let r = Rect::of_work_area(&m);
                    window.set_position(PhysicalPosition::new(r.x + r.w / 8, r.y + r.h / 8))?;
                    session.moved = Some(before);
                }
            }
        }
    }
    if session.moved.is_some() || !was_fullscreen {
        window.set_fullscreen(true)?;
        session.made_fullscreen = session.moved.map_or(true, |b| !b.fullscreen);
    }
    let entered = TvEntered { display_missing: session.missing };
    *slot = Some(session);
    Ok(entered)
}

/// Leaves TV Mode's window state: back to the display, size and fullscreen
/// state from before.
pub fn tv_leave(app: &AppHandle, window: &WebviewWindow) -> Result<()> {
    let Some(session) = app.state::<TvState>().0.lock().unwrap_or_else(|e| e.into_inner()).take() else { return Ok(()) };
    if let Some(b) = session.moved {
        window.set_fullscreen(false)?;
        window.set_position(b.position)?;
        window.set_size(b.size)?;
        if b.maximized {
            window.maximize()?;
        }
        if b.fullscreen {
            window.set_fullscreen(true)?;
        }
    } else if session.made_fullscreen {
        window.set_fullscreen(false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_lays_big_screens_out_like_1080p() {
        assert_eq!(auto_zoom(1920.0, 1080.0), 1.0);
        assert_eq!(auto_zoom(1707.0, 960.0), 1.0, "1440p at 150% stays as it is");
        assert_eq!(auto_zoom(3840.0, 2160.0), 2.0, "4K at 100%");
        assert_eq!(auto_zoom(2560.0, 1440.0), 1.33);
        assert_eq!(auto_zoom(5120.0, 1440.0), 1.33, "32:9 is limited by its height");
        assert_eq!(auto_zoom(1600.0, 1400.0), 1.0, "a tall window is limited by its width");
        assert_eq!(auto_zoom(960.0, 540.0), 1.0);
    }

    #[test]
    fn interface_size_works_like_windows_scaling() {
        // Default (and the old values): Windows' scaling as is.
        for old in [None, Some("auto"), Some("fixed")] {
            assert_eq!(zoom_for(old, 2560.0, 1440.0, 1.5), 1.0);
        }
        assert_eq!(zoom_for(Some("fit"), 3840.0, 2160.0, 1.0), 2.0);
        // A percentage replaces Windows' scale (here 4K at Windows 150%, then 1080p at 100%).
        assert_eq!(zoom_for(Some("300"), 2560.0, 1440.0, 1.5), 2.0);
        assert_eq!(zoom_for(Some("100"), 2560.0, 1440.0, 1.5), 0.67);
        assert_eq!(zoom_for(Some("150"), 1920.0, 1080.0, 1.0), 1.5);
    }

    /// Prints this PC's displays: `cargo test --lib os_names -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn os_names() {
        for n in os_display_names() {
            println!("{} = {:?} ({})", n.gdi, n.friendly, n.path);
        }
    }

    #[test]
    fn fullscreen_on_another_display_moves_the_windowed_size_there() {
        let left = Rect { x: -1920, y: 172, w: 1920, h: 1080 };
        let p = Placement { x: 100, y: 100, width: 1600.0, height: 900.0, maximized: true };
        let moved = centered(p, left, 1.0);
        assert_eq!(moved, Placement { x: -1760, y: 262, width: 1600.0, height: 900.0, maximized: true });
        // Too big for a smaller display: shrunk to 90% of it.
        let big = Placement { width: 2400.0, height: 1300.0, ..p };
        let m = centered(big, left, 1.0);
        assert_eq!((m.width, m.height), (1728.0, 972.0));
        assert!(grabbable(Rect { x: m.x, y: m.y, w: 1728, h: 972 }, &[left]));
    }

    #[test]
    fn a_window_must_keep_its_title_bar_on_a_display() {
        let main = Rect { x: 0, y: 0, w: 2560, h: 1400 };
        let left = Rect { x: -1920, y: 172, w: 1920, h: 1040 };
        let areas = [main, left];
        assert!(grabbable(Rect { x: 100, y: 100, w: 1600, h: 900 }, &areas));
        assert!(grabbable(Rect { x: -1800, y: 300, w: 1600, h: 900 }, &areas), "on the left display");
        // The left display is unplugged: the same spot is now nowhere.
        assert!(!grabbable(Rect { x: -1800, y: 300, w: 1600, h: 900 }, &[main]));
        // Mostly off the right edge, only a sliver of title bar left.
        assert!(!grabbable(Rect { x: 2500, y: 100, w: 1600, h: 900 }, &areas));
        // Title bar above the top of every display.
        assert!(!grabbable(Rect { x: 100, y: -200, w: 1600, h: 900 }, &areas));
    }
}
