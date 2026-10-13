//! API credentials (IGDB client secret, tokens) kept out of the database and
//! out of the repo: in Windows Credential Manager on Windows (encrypted for
//! the user account; visible under Control Panel → Credential Manager →
//! Windows Credentials as "EmuFrontend:..."), elsewhere in a file in the
//! app's data dir.

use crate::error::Result;

const PREFIX: &str = "EmuFrontend:";

#[cfg(windows)]
mod imp {
    use super::PREFIX;
    use crate::error::{Error, Result};
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    fn not_found(e: &windows::core::Error) -> bool {
        e.code() == ERROR_NOT_FOUND.to_hresult()
    }

    pub fn get(name: &str) -> Result<Option<String>> {
        let target = HSTRING::from(format!("{PREFIX}{name}"));
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

    pub fn set(name: &str, value: &str) -> Result<()> {
        let mut target: Vec<u16> = format!("{PREFIX}{name}").encode_utf16().chain([0]).collect();
        let mut user: Vec<u16> = "EmuFrontend".encode_utf16().chain([0]).collect();
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

    pub fn delete(name: &str) -> Result<()> {
        let target = HSTRING::from(format!("{PREFIX}{name}"));
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
    use super::PREFIX;
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

    pub fn get(name: &str) -> Result<Option<String>> {
        Ok(load().get(&format!("{PREFIX}{name}")).cloned())
    }

    pub fn set(name: &str, value: &str) -> Result<()> {
        let mut m = load();
        m.insert(format!("{PREFIX}{name}"), value.to_string());
        save(&m)
    }

    pub fn delete(name: &str) -> Result<()> {
        let mut m = load();
        m.remove(&format!("{PREFIX}{name}"));
        save(&m)
    }
}

/// Where the non-Windows fallback keeps its file.
#[allow(unused_variables)]
pub fn init(data_dir: &std::path::Path) {
    #[cfg(not(windows))]
    let _ = imp::FILE.set(data_dir.join("secrets.local.json"));
}

/// A separate data folder (`EMU_FRONTEND_DATA_DIR`, e.g. the performance
/// harness's generated library) is a separate profile: it never sees or
/// changes the keys saved for the real library.
fn separate_profile() -> bool {
    std::env::var_os("EMU_FRONTEND_DATA_DIR").is_some_and(|d| !d.is_empty())
}

pub fn get(name: &str) -> Result<Option<String>> {
    if separate_profile() {
        return Ok(None);
    }
    imp::get(name)
}

pub fn set(name: &str, value: &str) -> Result<()> {
    if separate_profile() {
        return Err(crate::error::Error::msg("API keys can't be saved while using a separate data folder"));
    }
    imp::set(name, value)
}

pub fn delete(name: &str) -> Result<()> {
    if separate_profile() {
        return Ok(());
    }
    imp::delete(name)
}

#[cfg(test)]
mod tests {
    /// Round trip through the real store (writes and removes a test entry):
    /// `cargo test secrets -- --ignored`
    #[test]
    #[ignore]
    fn round_trip() {
        let name = "test-entry-please-ignore";
        super::set(name, "s3cret ✓").unwrap();
        assert_eq!(super::get(name).unwrap().as_deref(), Some("s3cret ✓"));
        super::delete(name).unwrap();
        assert_eq!(super::get(name).unwrap(), None);
        super::delete(name).unwrap(); // deleting twice is fine
    }
}
