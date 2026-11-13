//! Steam games are observed through exact image paths and process handles, never Job Objects.
use crate::{
    error::{Error, Result},
    scanner::path_key,
};
use std::collections::HashSet;
use std::path::PathBuf;

pub fn candidates(game: &crate::steam::Game) -> Result<HashSet<String>> {
    let paths = if let Some(chosen) = &game.tracking_exe {
        HashSet::from([path_key(&crate::steam::tracking_file(
            &game.install_dir,
            chosen,
        )?)])
    } else {
        // Optional launch variants can disappear after an update. Never substitute for a user's chosen file.
        game.executables
            .iter()
            .filter_map(|p| crate::steam::tracking_file(&game.install_dir, p).ok())
            .map(|p| path_key(&p))
            .collect()
    };
    if paths.is_empty() {
        return Err(Error::msg("Choose this Steam game's executable in its details before playing, so KB can track it safely."));
    }
    Ok(paths)
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::core::BOOL;
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, WAIT_TIMEOUT, WPARAM},
            System::{
                Diagnostics::ToolHelp::{
                    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                    TH32CS_SNAPPROCESS,
                },
                Threading::{
                    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
                    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
                },
            },
            UI::{
                Shell::ShellExecuteW,
                WindowsAndMessaging::{
                    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
                    SW_SHOWNORMAL, WM_CLOSE,
                },
            },
        },
    };
    struct Handle(HANDLE);
    unsafe impl Send for Handle {}
    unsafe impl Sync for Handle {}
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    struct Process {
        handle: Handle,
        pid: u32,
        created: u64,
    }
    impl Process {
        fn alive(&self) -> bool {
            unsafe { WaitForSingleObject(self.handle.0, 0) == WAIT_TIMEOUT }
        }
    }
    fn snapshot(paths: &HashSet<String>) -> Result<Vec<Process>> {
        let snapshot = Handle(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
                .map_err(|_| Error::msg("Windows couldn't list game processes."))?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut out = Vec::new();
        let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        while ok.is_ok() {
            if let Ok(raw) = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    false,
                    entry.th32ProcessID,
                )
            } {
                let handle = Handle(raw);
                let mut buffer = [0u16; 32768];
                let mut count = buffer.len() as u32;
                if unsafe {
                    QueryFullProcessImageNameW(
                        handle.0,
                        PROCESS_NAME_WIN32,
                        windows::core::PWSTR(buffer.as_mut_ptr()),
                        &mut count,
                    )
                }
                .is_ok()
                {
                    let file = PathBuf::from(String::from_utf16_lossy(&buffer[..count as usize]));
                    if let Ok(file) = std::fs::canonicalize(file) {
                        if paths.contains(&path_key(&file)) {
                            let (mut created, mut exit, mut kernel, mut user) = (
                                FILETIME::default(),
                                FILETIME::default(),
                                FILETIME::default(),
                                FILETIME::default(),
                            );
                            if unsafe {
                                GetProcessTimes(
                                    handle.0,
                                    &mut created,
                                    &mut exit,
                                    &mut kernel,
                                    &mut user,
                                )
                            }
                            .is_ok()
                            {
                                out.push(Process {
                                    handle,
                                    pid: entry.th32ProcessID,
                                    created: ((created.dwHighDateTime as u64) << 32)
                                        | created.dwLowDateTime as u64,
                                });
                            }
                        }
                    }
                }
            }
            ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
        Ok(out)
    }
    pub struct SteamTracker {
        processes: std::sync::Mutex<Vec<Process>>,
        paths: HashSet<String>,
        baseline: HashSet<(u32, u64)>,
    }
    impl SteamTracker {
        pub fn baseline(paths: &HashSet<String>) -> Result<HashSet<(u32, u64)>> {
            Ok(snapshot(paths)?
                .into_iter()
                .map(|p| (p.pid, p.created))
                .collect())
        }
        pub fn detect(
            paths: &HashSet<String>,
            baseline: &HashSet<(u32, u64)>,
            require_window: bool,
        ) -> Result<Option<Self>> {
            let processes = snapshot(paths)?
                .into_iter()
                .filter(|p| !baseline.contains(&(p.pid, p.created)) && p.alive())
                .collect::<Vec<_>>();
            if processes.is_empty() {
                return Ok(None);
            }
            let tracker = Self {
                processes: std::sync::Mutex::new(processes),
                paths: paths.clone(),
                baseline: baseline.clone(),
            };
            // Cached launch targets may be invisible bootstrappers. The player can explicitly
            // select a game executable for titles without a normal desktop window.
            if require_window && tracker.windows().is_empty() {
                return Ok(None);
            }
            Ok(Some(tracker))
        }

        pub fn is_running(&self) -> bool {
            let mut current = self.processes.lock().unwrap_or_else(|e| e.into_inner());
            if current.iter().any(Process::alive) {
                return true;
            }
            // Only enumerate again at a handoff, not on every running-game tick.
            if let Ok(next) = snapshot(&self.paths) {
                *current = next
                    .into_iter()
                    .filter(|p| !self.baseline.contains(&(p.pid, p.created)) && p.alive())
                    .collect();
            }
            current.iter().any(Process::alive)
        }
        pub fn wait(&self) {
            while self.is_running() {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
        pub fn windows(&self) -> Vec<isize> {
            struct Context {
                pids: Vec<u32>,
                windows: Vec<isize>,
            }
            unsafe extern "system" fn each(hwnd: HWND, param: LPARAM) -> BOOL {
                let c = unsafe { &mut *(param.0 as *mut Context) };
                let mut pid = 0;
                unsafe {
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                }
                if c.pids.contains(&pid) && unsafe { IsWindowVisible(hwnd) }.as_bool() {
                    c.windows.push(hwnd.0 as isize);
                }
                BOOL(1)
            }
            let mut c = Context {
                pids: self
                    .processes
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .filter(|p| p.alive())
                    .map(|p| p.pid)
                    .collect(),
                windows: Vec::new(),
            };
            unsafe {
                let _ = EnumWindows(Some(each), LPARAM(&mut c as *mut _ as isize));
            }
            c.windows
        }
        pub fn close_windows(&self) -> usize {
            let list = self.windows();
            for &h in &list {
                unsafe {
                    let hwnd = HWND(h as *mut _);
                    let mut pid = 0;
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                    if self
                        .processes
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .iter()
                        .any(|p| p.pid == pid && p.alive())
                    {
                        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }
            }
            list.len()
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        #[ignore = "controlled child used only by the exact-process test"]
        fn controlled_process_fixture() {
            if std::env::var_os("KB_STEAM_PROCESS_FIXTURE").is_some() {
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        }
        #[test]
        fn detects_only_new_exact_processes_and_observes_exit() {
            let exe = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
            let paths = HashSet::from([path_key(&exe)]);
            let baseline = SteamTracker::baseline(&paths).unwrap();
            assert!(!baseline.is_empty()); // The parent test runner is already present.
            assert!(SteamTracker::detect(&paths, &baseline, false)
                .unwrap()
                .is_none());
            let mut child = std::process::Command::new(&exe)
                .args([
                    "--exact",
                    "launcher::steam_process::win::tests::controlled_process_fixture",
                    "--ignored",
                ])
                .env("KB_STEAM_PROCESS_FIXTURE", "1")
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let tracker = SteamTracker::detect(&paths, &baseline, false)
                .unwrap()
                .unwrap();
            assert!(tracker.is_running());
            assert!(tracker.windows().is_empty());
            assert_eq!(tracker.close_windows(), 0); // No windows or close messages.
            assert!(child.wait().unwrap().success());
            assert!(!tracker.is_running()); // Never adopts the still-running parent.
            let other = HashSet::from([path_key(&exe.with_file_name("different.exe"))]);
            assert!(SteamTracker::detect(&other, &HashSet::new(), false)
                .unwrap()
                .is_none());
        }
    }
    pub fn open(id: u32) -> Result<()> {
        let uri = crate::steam::uri(id)?;
        let uri = uri.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let verb = "open\0".encode_utf16().collect::<Vec<_>>();
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(uri.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err(Error::msg("Windows couldn't open Steam. Make sure Steam is installed and its protocol is registered."));
        }
        Ok(())
    }
}
#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
pub struct SteamTracker;
#[cfg(not(windows))]
impl SteamTracker {
    pub fn baseline(_: &HashSet<String>) -> Result<HashSet<(u32, u64)>> {
        Err(Error::msg(
            "Steam integration currently supports Windows only.",
        ))
    }
    pub fn detect(_: &HashSet<String>, _: &HashSet<(u32, u64)>, _: bool) -> Result<Option<Self>> {
        Ok(None)
    }
    pub fn is_running(&self) -> bool {
        false
    }
    pub fn wait(&self) {}
    pub fn windows(&self) -> Vec<isize> {
        Vec::new()
    }
    pub fn close_windows(&self) -> usize {
        0
    }
}
#[cfg(not(windows))]
pub fn open(_: u32) -> Result<()> {
    Err(Error::msg(
        "Steam integration currently supports Windows only.",
    ))
}

/// Waiting never creates a session; only a confirmed tracker can reach that caller's success path.
pub fn wait_for_game(
    paths: &HashSet<String>,
    baseline: &HashSet<(u32, u64)>,
    require_window: bool,
    cancelled: &std::sync::atomic::AtomicBool,
    timeout: std::time::Duration,
) -> Result<SteamTracker> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(Error::msg("Stopped waiting for Steam. Steam or the game may still open; no play time was recorded."));
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::msg("Steam's game was not detected within 60 seconds. No play time was recorded. Check the game executable in its details."));
        }
        if let Some(tracker) = SteamTracker::detect(paths, baseline, require_window)? {
            if !cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return Ok(tracker);
            }
            continue;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_and_timeout_return_without_a_tracker() {
        use std::{
            sync::atomic::{AtomicBool, Ordering},
            time::{Duration, Instant},
        };
        let cancelled = AtomicBool::new(true);
        let started = Instant::now();
        let paths = HashSet::new();
        let baseline = HashSet::new();
        let err = wait_for_game(
            &paths,
            &baseline,
            false,
            &cancelled,
            Duration::from_secs(60),
        )
        .err()
        .unwrap();
        assert!(err.to_string().contains("Stopped waiting"));
        cancelled.store(false, Ordering::Release);
        let err = wait_for_game(&paths, &baseline, false, &cancelled, Duration::ZERO)
            .err()
            .unwrap();
        assert!(err.to_string().contains("No play time was recorded"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn tracking_uses_valid_exact_paths_and_never_replaces_an_explicit_choice() {
        let temp = tempfile::tempdir().unwrap();
        let game_dir = temp.path().join("Game");
        std::fs::create_dir(&game_dir).unwrap();
        let exe = game_dir.join("game.exe");
        std::fs::write(&exe, b"fixture, not executed").unwrap();
        let mut game = crate::steam::Game {
            app_id: 123,
            library_path: temp.path().into(),
            install_dir: game_dir.clone(),
            executables: vec![exe.clone(), game_dir.join("removed-variant.exe")],
            tracking_exe: None,
        };
        assert_eq!(
            candidates(&game).unwrap(),
            HashSet::from([path_key(&std::fs::canonicalize(&exe).unwrap())])
        );
        game.tracking_exe = Some(game_dir.join("missing-choice.exe"));
        assert!(candidates(&game).is_err());
        let outside = temp.path().join("other.exe");
        std::fs::write(&outside, b"fixture").unwrap();
        game.tracking_exe = Some(outside);
        assert!(candidates(&game).is_err());
        game.tracking_exe = Some(exe);
        assert!(candidates(&game).is_ok());
    }
}
