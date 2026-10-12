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
