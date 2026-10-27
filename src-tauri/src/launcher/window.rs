//! Bringing the frontend back to the foreground after a game exits.
//!
//! Windows only lets a process take the foreground if it received the last
//! input event; controller input (and the emulator's exit) doesn't count, so a
//! plain SetForegroundWindow often just flashes the taskbar button. A
//! synthetic Alt key press makes this process the last-input owner, the
//! standard workaround used by launchers.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_TRANSITIONS_FORCEDISABLED};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};

fn key(flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_MENU, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

/// While a game runs, asks WebView2 to keep its memory low: it frees caches
/// (decoded covers, GPU tiles) it would otherwise hold while minimized.
/// Back to normal when the game exits. Older WebView2 runtimes without the
/// setting are left alone.
pub fn set_webview_low_memory(window: &tauri::WebviewWindow, low: bool) {
    let _ = window.with_webview(move |wv| {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows::core::Interface;
        let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
        // SAFETY: COM calls on the webview's own (main) thread, where with_webview runs this.
        unsafe {
            let Ok(core) = wv.controller().CoreWebView2() else { return };
            match core.cast::<ICoreWebView2_19>() {
                Ok(c) => {
                    if let Err(e) = c.SetMemoryUsageTargetLevel(level) {
                        log::warn!("WebView2 memory target: {e}");
                    }
                }
                Err(_) => log::info!("WebView2 runtime too old for a memory target"),
            }
        }
    });
}

/// While a game runs, the minimized page is hidden from WebView2 and
/// suspended: its renderer stops running script and timers, and WebView2
/// trims more memory than the low memory target alone. Resumed and shown
/// again when the game exits. Best effort: WebView2 declines to suspend in
/// some cases (for example while audio plays), and any script KB sends the
/// page (an event) wakes it up early. `KB_NO_SUSPEND=1` turns this off, to
/// compare with and without it.
pub fn set_webview_suspended(window: &tauri::WebviewWindow, suspended: bool) {
    if suspended && std::env::var_os("KB_NO_SUSPEND").is_some_and(|v| v == "1") {
        return;
    }
    let _ = window.with_webview(move |wv| {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
        use webview2_com::TrySuspendCompletedHandler;
        use windows::core::Interface;
        let controller = wv.controller();
        // SAFETY: COM calls on the webview's own (main) thread, where with_webview runs this.
        unsafe {
            let Ok(core) = controller.CoreWebView2() else { return };
            let Ok(core) = core.cast::<ICoreWebView2_3>() else {
                log::info!("WebView2 runtime too old to suspend");
                return;
            };
            if suspended {
                let _ = controller.SetIsVisible(false);
                let done = TrySuspendCompletedHandler::create(Box::new(|result, ok| {
                    match result {
                        Ok(()) => log::info!("page suspended while the game runs: {ok}"),
                        Err(e) => log::info!("page not suspended: {e}"),
                    }
                    Ok(())
                }));
                if let Err(e) = core.TrySuspend(&done) {
                    log::info!("page not suspended: {e}");
                }
            } else {
                if let Err(e) = core.Resume() {
                    log::warn!("couldn't resume the page: {e}");
                }
                let _ = controller.SetIsVisible(true);
            }
        }
    });
}

/// Gives a just-opened emulator window the foreground without un-maximizing it
/// (only a minimized one is restored), so a fullscreen game isn't left behind.
pub fn focus_game(hwnd: HWND) {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd {
            return;
        }
        let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        let _ = SetForegroundWindow(hwnd);
    }
}

/// The game window's place on screen (physical pixels): left, top, width, height.
pub fn window_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    let mut r = windows::Win32::Foundation::RECT::default();
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut r).ok()? };
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    (w > 0 && h > 0).then_some((r.left, r.top, w, h))
}

/// The whole screen the game window is on (physical pixels): left, top, width, height.
pub fn monitor_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::Graphics::Gdi::*;
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: plain calls with a valid out-struct.
    unsafe { GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info).ok().ok()? };
    let r = info.rcMonitor;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    (w > 0 && h > 0).then_some((r.left, r.top, w, h))
}

/// What's on screen in this area, as RGBA (the game behind its manual). Copied from the screen, so a
/// game in exclusive fullscreen may come out black (the caller checks).
pub fn capture((x, y, w, h): (i32, i32, i32, i32)) -> Option<image::RgbaImage> {
    use windows::Win32::Graphics::Gdi::*;
    // SAFETY: plain GDI calls on handles created and released here.
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp.into());
        let copied = BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY).is_ok();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut px = vec![0u8; (w * h * 4) as usize];
        let rows = if copied { GetDIBits(mem, bmp, 0, h as u32, Some(px.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS) } else { 0 };
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        if rows == 0 {
            return None;
        }
        for p in px.chunks_exact_mut(4) {
            p.swap(0, 2); // BGRA -> RGBA
            p[3] = 255;
        }
        image::RgbaImage::from_raw(w as u32, h as u32, px)
    }
}

/// Hides the window from the screen while it keeps drawing (DWM cloak): KB moves and resizes over a
/// game unseen, then shows up in one frame.
pub fn set_cloaked(hwnd: HWND, cloaked: bool) {
    let v = cloaked as i32;
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &v as *const i32 as *const std::ffi::c_void, 4);
    }
}

/// Windows' own zoom when the window is minimized or restored; off while KB comes up over a game.
pub fn set_transitions(hwnd: HWND, on: bool) {
    let off = (!on) as i32;
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, &off as *const i32 as *const std::ffi::c_void, 4);
    }
}

pub fn bring_to_front(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        if SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd {
            return;
        }
        let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        let _ = SetForegroundWindow(hwnd);
    }
}
