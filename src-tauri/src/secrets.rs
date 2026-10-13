//! API credentials (IGDB client secret, tokens, other services' keys) kept
//! out of the database and out of the repo: in Windows Credential Manager on
//! Windows (encrypted for the user account; visible under Control Panel →
//! Credential Manager → Windows Credentials as "KaneegaBell:..."), elsewhere
//! in a file in the app's data dir.
//!
//! Keys saved before the KaneegaBell rename live under "EmuFrontend:...":
//! they're read as a fallback, copied to the new name on first use, and left
//! in place (removing a key removes both names).

use crate::error::Result;

const PREFIX: &str = "KaneegaBell:";
const LEGACY_PREFIX: &str = "EmuFrontend:";

#[cfg(windows)]
mod imp {
    use crate::error::{Error, Result};
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    fn not_found(e: &windows::core::Error) -> bool {
        e.code() == ERROR_NOT_FOUND.to_hresult()
    }

    pub fn get(target: &str) -> Result<Option<String>> {
        let target = HSTRING::from(target);
        let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: CredReadW fills `cred` with a buffer we free with CredFree.
        match unsafe { CredReadW(&target, CRED_TYPE_GENERIC, None, &mut cred) } {
            Ok(()) => {
                let value = unsafe {
                    let c = &*cred;
                    let blob = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
                    let s = String::from_utf8_lossy(blob).into_owned();
                    CredFree(cred as *const _);
                    s
                };
                Ok(Some(value))
            }
            Err(e) if not_found(&e) => Ok(None),
            Err(e) => Err(Error::msg(format!("Credential Manager: {e}"))),
        }
    }

    pub fn set(target: &str, value: &str) -> Result<()> {
        let mut target: Vec<u16> = target.encode_utf16().chain([0]).collect();
        let mut user: Vec<u16> = "KaneegaBell".encode_utf16().chain([0]).collect();
        let mut blob = value.as_bytes().to_vec();
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            UserName: PWSTR(user.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // SAFETY: every pointer in `cred` outlives the call.
        unsafe { CredWriteW(&cred, 0) }.map_err(|e| Error::msg(format!("Credential Manager: {e}")))
    }

    pub fn delete(target: &str) -> Result<()> {
        let target = HSTRING::from(target);
        match unsafe { CredDeleteW(&target, CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(e) if not_found(&e) => Ok(()),
            Err(e) => Err(Error::msg(format!("Credential Manager: {e}"))),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    //! Fallback: a JSON file next to the database (`secrets.local.json`,
    //! matched by .gitignore), readable only by the user.
    use crate::error::Result;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    pub static FILE: OnceLock<PathBuf> = OnceLock::new();

    fn load() -> HashMap<String, String> {
        FILE.get()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save(map: &HashMap<String, String>) -> Result<()> {
        let Some(p) = FILE.get() else { return Ok(()) };
        std::fs::write(p, serde_json::to_vec(map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn get(target: &str) -> Result<Option<String>> {
        Ok(load().get(target).cloned())
    }

    pub fn set(target: &str, value: &str) -> Result<()> {
        let mut m = load();
        m.insert(target.to_string(), value.to_string());
        save(&m)
    }

    pub fn delete(target: &str) -> Result<()> {
        let mut m = load();
        m.remove(target);
        save(&m)
    }
}

/// Where the non-Windows fallback keeps its file.
#[allow(unused_variables)]
pub fn init(data_dir: &std::path::Path) {
    #[cfg(not(windows))]
    let _ = imp::FILE.set(data_dir.join("secrets.local.json"));
}

/// A separate data folder (`KB_DATA_DIR`, e.g. the performance harness's
/// generated library) is a separate profile: it never sees or changes the
/// keys saved for the real library.
fn separate_profile() -> bool {
    crate::data_dir_override().is_some()
}

pub fn get(name: &str) -> Result<Option<String>> {
    if separate_profile() {
        return Ok(None);
    }
    if let Some(v) = imp::get(&format!("{PREFIX}{name}"))? {
        return Ok(Some(v));
    }
    // Saved before the rename: copy it to the new name (best effort).
    let legacy = imp::get(&format!("{LEGACY_PREFIX}{name}"))?;
    if let Some(v) = &legacy {
        if let Err(e) = imp::set(&format!("{PREFIX}{name}"), v) {
            log::warn!("couldn't copy the saved {name} to its new name: {e}");
        }
    }
    Ok(legacy)
}

pub fn set(name: &str, value: &str) -> Result<()> {
    if separate_profile() {
        return Err(crate::error::Error::msg("API keys can't be saved while using a separate data folder"));
    }
    imp::set(&format!("{PREFIX}{name}"), value)
}

/// Removes a key under both its current and pre-rename names (otherwise the
/// fallback would bring it back).
pub fn delete(name: &str) -> Result<()> {
    if separate_profile() {
        return Ok(());
    }
    imp::delete(&format!("{PREFIX}{name}"))?;
    imp::delete(&format!("{LEGACY_PREFIX}{name}"))
}

#[cfg(test)]
mod tests {
    /// Round trip through the real store (writes and removes test entries):
    /// `cargo test secrets -- --ignored`
    #[test]
    #[ignore]
    fn round_trip_and_legacy_fallback() {
        let name = "test-entry-please-ignore";
        super::set(name, "s3cret ✓").unwrap();
        assert_eq!(super::get(name).unwrap().as_deref(), Some("s3cret ✓"));
        super::delete(name).unwrap();
        assert_eq!(super::get(name).unwrap(), None);
        super::delete(name).unwrap(); // deleting twice is fine

        // A key saved under the old name is found, copied, and removed with it.
        super::imp::set(&format!("{}{name}", super::LEGACY_PREFIX), "old").unwrap();
        assert_eq!(super::get(name).unwrap().as_deref(), Some("old"));
        assert_eq!(super::imp::get(&format!("{}{name}", super::PREFIX)).unwrap().as_deref(), Some("old"));
        super::delete(name).unwrap();
        assert_eq!(super::get(name).unwrap(), None);
    }
}
