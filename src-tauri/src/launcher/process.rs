//! Process tracking for a launched emulator.
//!
//! Windows: the emulator is placed in a Job Object whose completion port
//! reports `ACTIVE_PROCESS_ZERO` when the last process in the job (including
//! any children it spawned) exits. A thread blocks on that port, so there is
//! no polling. The job is *not* kill-on-close: if the frontend crashes, the
//! game keeps running.
//!
//! Exiting a game: first WM_CLOSE to the job's visible top-level windows (lets
//! the emulator flush saves), then `TerminateJobObject` after a grace period.

use std::process::Child;

use crate::error::{Error, Result};

#[cfg(windows)]
pub use windows_impl::Tracker;

#[cfg(not(windows))]
pub use portable::Tracker;

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::sync::Mutex;

    use windows::core::BOOL;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WPARAM};
    use windows::Win32::System::IO::{CreateIoCompletionPort, GetQueuedCompletionStatus, OVERLAPPED};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectAssociateCompletionPortInformation,
        JobObjectBasicProcessIdList, QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        JOBOBJECT_ASSOCIATE_COMPLETION_PORT, JOBOBJECT_BASIC_PROCESS_ID_LIST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
    };

    const JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO: u32 = 4;
    const INFINITE: u32 = u32::MAX;

    struct Owned(HANDLE);
    // Kernel handles are process-wide and safe to use from any thread.
    unsafe impl Send for Owned {}
    unsafe impl Sync for Owned {}
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    pub struct Tracker {
        job: Owned,
        port: Owned,
        /// Set when the process couldn't be put in the job (it exited instantly,
        /// or assignment was refused); then we wait on the process itself.
        fallback: Mutex<Option<Child>>,
        pub pid: u32,
    }

    impl Tracker {
        pub fn attach(child: Child) -> Result<Tracker> {
            let pid = child.id();
            unsafe {
                let job = Owned(CreateJobObjectW(None, None).map_err(win)?);
                let port = Owned(CreateIoCompletionPort(INVALID_HANDLE_VALUE, None, 0, 1).map_err(win)?);
                let assoc = JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
                    CompletionKey: job.0 .0,
                    CompletionPort: port.0,
                };
                SetInformationJobObject(
                    job.0,
                    JobObjectAssociateCompletionPortInformation,
                    &assoc as *const _ as *const c_void,
                    std::mem::size_of::<JOBOBJECT_ASSOCIATE_COMPLETION_PORT>() as u32,
                )
                .map_err(win)?;
                let process = HANDLE(child.as_raw_handle() as *mut c_void);
                let assigned = AssignProcessToJobObject(job.0, process);
                let fallback = match assigned {
                    // Children are tracked through the job from here on; the
                    // Child handle is no longer needed.
                    Ok(()) => None,
                    Err(e) => {
                        log::warn!("couldn't assign pid {pid} to a job ({e}); tracking the process only");
                        Some(child)
                    }
                };
                Ok(Tracker { job, port, fallback: Mutex::new(fallback), pid })
            }
        }

        /// Blocks until every process of the game has exited.
        pub fn wait(&self) {
            if let Some(mut child) = self.fallback.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = child.wait();
                return;
            }
            // The process may have exited before it was assigned (no message
            // would ever arrive), so check the job's population first.
            if self.pids().is_empty() {
                return;
            }
            loop {
                let mut code = 0u32;
                let mut key = 0usize;
                let mut overlapped: *mut OVERLAPPED = std::ptr::null_mut();
                let ok = unsafe { GetQueuedCompletionStatus(self.port.0, &mut code, &mut key, &mut overlapped, INFINITE) };
                if ok.is_err() {
                    log::warn!("job completion port failed; assuming the game exited");
                    return;
                }
                if code == JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO {
                    return;
                }
            }
        }

        /// PIDs currently in the job.
        pub fn pids(&self) -> Vec<u32> {
            // Header + room for 64 ids is plenty for an emulator's process tree.
            #[repr(C)]
            struct Buf {
                list: JOBOBJECT_BASIC_PROCESS_ID_LIST,
                more: [usize; 63],
            }
            let mut buf: Buf = unsafe { std::mem::zeroed() };
            let ok = unsafe {
                QueryInformationJobObject(
                    Some(self.job.0),
                    JobObjectBasicProcessIdList,
                    &mut buf as *mut _ as *mut c_void,
                    std::mem::size_of::<Buf>() as u32,
                    None,
                )
            };
            if ok.is_err() {
                return Vec::new();
            }
            let n = buf.list.NumberOfProcessIdsInList as usize;
            let ids = unsafe { std::slice::from_raw_parts(buf.list.ProcessIdList.as_ptr(), n.min(64)) };
            ids.iter().map(|&id| id as u32).collect()
        }

        pub fn is_running(&self) -> bool {
            match &mut *self.fallback.lock().unwrap_or_else(|e| e.into_inner()) {
                Some(child) => matches!(child.try_wait(), Ok(None)),
                None => !self.pids().is_empty(),
            }
        }

        /// Asks the game's windows to close. Returns how many were asked.
        pub fn close_windows(&self) -> usize {
            let mut pids = self.pids();
            if pids.is_empty() {
                pids.push(self.pid);
            }
            struct Ctx {
                pids: Vec<u32>,
                found: Vec<isize>,
            }
            unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
                let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
                let mut pid = 0u32;
                unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
                if ctx.pids.contains(&pid) && unsafe { IsWindowVisible(hwnd) }.as_bool() {
                    ctx.found.push(hwnd.0 as isize);
                }
                BOOL(1)
            }
            let mut ctx = Ctx { pids, found: Vec::new() };
            unsafe {
                let _ = EnumWindows(Some(each), LPARAM(&mut ctx as *mut Ctx as isize));
                for &h in &ctx.found {
                    let _ = PostMessageW(Some(HWND(h as *mut c_void)), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            ctx.found.len()
        }

        pub fn terminate(&self) {
            if let Some(child) = &mut *self.fallback.lock().unwrap_or_else(|e| e.into_inner()) {
                let _ = child.kill();
                return;
            }
            unsafe {
                let _ = TerminateJobObject(self.job.0, 1);
            }
        }
    }

    fn win(e: windows::core::Error) -> Error {
        Error::msg(format!("Windows error: {e}"))
    }
}

#[cfg(not(windows))]
mod portable {
    use super::*;
    use std::sync::Mutex;

    /// Fallback: tracks the launched process only (children are not followed).
    pub struct Tracker {
        child: Mutex<Child>,
        pub pid: u32,
    }

    impl Tracker {
        pub fn attach(child: Child) -> Result<Tracker> {
            Ok(Tracker { pid: child.id(), child: Mutex::new(child) })
        }
        pub fn wait(&self) {
            loop {
                if !matches!(self.child.lock().unwrap().try_wait(), Ok(None)) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
        pub fn is_running(&self) -> bool {
            matches!(self.child.lock().unwrap().try_wait(), Ok(None))
        }
        pub fn close_windows(&self) -> usize {
            0
        }
        pub fn terminate(&self) {
            let _ = self.child.lock().unwrap().kill();
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn waits_for_child_processes_spawned_by_the_launched_process() {
        // cmd starts a grandchild (`ping` for ~2 s) and exits immediately: the
        // tracker must wait for the grandchild, not just cmd.
        let child = Command::new("cmd")
            .args(["/C", "start", "/B", "ping", "-n", "3", "127.0.0.1", ">NUL"])
            .spawn()
            .unwrap();
        let t = Tracker::attach(child).unwrap();
        let started = Instant::now();
        t.wait();
        let took = started.elapsed();
        assert!(took >= Duration::from_millis(1500), "returned after {took:?}, before the grandchild exited");
        assert!(!t.is_running());
    }

    #[test]
    fn terminate_ends_the_whole_tree() {
        let child = Command::new("cmd").args(["/C", "ping", "-n", "30", "127.0.0.1", ">NUL"]).spawn().unwrap();
        let t = Tracker::attach(child).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert!(t.pids().len() >= 2, "cmd + ping should both be in the job: {:?}", t.pids());
        let started = Instant::now();
        t.terminate();
        t.wait();
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(!t.is_running());
    }

    #[test]
    fn already_exited_process_does_not_hang() {
        let mut child = Command::new("cmd").args(["/C", "exit", "0"]).spawn().unwrap();
        let _ = child.try_wait();
        std::thread::sleep(Duration::from_millis(300));
        let t = Tracker::attach(child).unwrap();
        let started = Instant::now();
        t.wait();
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
