//! Bringing the frontend back to the foreground after a game exits.
//!
//! Windows only lets a process take the foreground if it received the last
//! input event; controller input (and the emulator's exit) doesn't count, so a
//! plain SetForegroundWindow often just flashes the taskbar button. A
//! synthetic Alt key press makes this process the last-input owner, the
//! standard workaround used by launchers.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow, ShowWindow, SW_RESTORE};

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
