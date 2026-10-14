//! API keys (IGDB client secret and token, SteamGridDB, RetroAchievements),
//! kept out of the database and the repo: in `keys.bin` in KB's data folder,
//! encrypted with the Windows user's login (DPAPI). The file can't be read
//! by another Windows user or on another PC; keys are re-entered there.
//! Elsewhere (non-Windows builds) it's a plain `secrets.local.json` readable
//! only by the user.
//!
//! Keys saved by earlier versions live in Windows Credential Manager
//! ("KaneegaBell:..." or, before the rename, "EmuFrontend:..."). The
//! installed app moves them into the file once (copied, read back, then
//! removed from Credential Manager); see `import_from_credential_manager`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::error::{Error, Result};

/// Every key name the app stores (for the one-time import).
const KNOWN: &[&str] = &["igdb-client-id", "igdb-client-secret", "igdb-token", "sgdb-key", "ra-user", "ra-web-api-key"];

static FILE: OnceLock<PathBuf> = OnceLock::new();
/// Serializes read-modify-write of the file.
static LOCK: Mutex<()> = Mutex::new(());

#[cfg(windows)]
const FILE_NAME: &str = "keys.bin";
#[cfg(not(windows))]
const FILE_NAME: &str = "secrets.local.json";

#[cfg(windows)]
mod crypt {
    //! DPAPI: encrypted for the current Windows user, with an app-specific
    //! extra secret so other apps' DPAPI blobs and ours don't mix.
    use crate::error::{Error, Result};
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    const ENTROPY: &[u8] = b"KaneegaBell keys v1";

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        // SAFETY: DPAPI allocated `out` with LocalAlloc; copied, then freed once.
        unsafe {
            let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(out.pbData as _)));
            v
        }
    }

    pub fn protect(plain: &[u8]) -> Result<Vec<u8>> {
        let mut out = CRYPT_INTEGER_BLOB::default();
        let (input, entropy) = (blob(plain), blob(ENTROPY));
        // SAFETY: the input blobs outlive the call; `out` is filled by DPAPI.
        unsafe { CryptProtectData(&input, None, Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out) }
            .map_err(|e| Error::msg(format!("couldn't encrypt the keys: {e}")))?;
        Ok(take(out))
    }

    pub fn unprotect(sealed: &[u8]) -> Result<Vec<u8>> {
        let mut out = CRYPT_INTEGER_BLOB::default();
        let (input, entropy) = (blob(sealed), blob(ENTROPY));
        // SAFETY: as above.
        unsafe { CryptUnprotectData(&input, None, Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out) }
            .map_err(|e| Error::msg(format!("couldn't decrypt the saved keys (made by another Windows user or PC?): {e}")))?;
        Ok(take(out))
    }
}

#[cfg(windows)]
mod credman {
    //! Windows Credential Manager, only read for the one-time import.
    use crate::error::{Error, Result};
    use windows::core::HSTRING;
    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{CredDeleteW, CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC};

    fn not_found(e: &windows::core::Error) -> bool {
        e.code() == ERROR_NOT_FOUND.to_hresult()
    }

    pub fn get(target: &str) -> Result<Option<String>> {
        let target = HSTRING::from(target);
        let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: CredReadW fills `cred` with a buffer we free with CredFree.
        match unsafe { CredReadW(&target, CRED_TYPE_GENERIC, None, &mut cred) } {
            Ok(()) => unsafe {
                let c = &*cred;
                let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
                let s = String::from_utf8_lossy(bytes).into_owned();
                CredFree(cred as *const _);
                Ok(Some(s))
            },
            Err(e) if not_found(&e) => Ok(None),
            Err(e) => Err(Error::msg(format!("Credential Manager: {e}"))),
        }
    }

    pub fn delete(target: &str) -> Result<()> {
        let target = HSTRING::from(target);
        match unsafe { CredDeleteW(&target, CRED_TYPE_GENERIC, None) } {
            Err(e) if !not_found(&e) => Err(Error::msg(format!("Credential Manager: {e}"))),
            _ => Ok(()),
        }
    }
}

/// Where the keys file lives (the data folder).
pub fn init(data_dir: &Path) {
    let _ = FILE.set(data_dir.join(FILE_NAME));
}

fn path() -> Result<&'static PathBuf> {
    FILE.get().ok_or_else(|| Error::msg("the keys file isn't set up yet"))
}

fn load_from(path: &Path) -> Result<BTreeMap<String, String>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e.into()),
    };
    #[cfg(windows)]
    let bytes = crypt::unprotect(&bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn save_to(path: &Path, map: &BTreeMap<String, String>) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if map.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    let plain = serde_json::to_vec(map)?;
    #[cfg(windows)]
    let bytes = crypt::protect(&plain)?;
    #[cfg(not(windows))]
    let bytes = plain;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn get(name: &str) -> Result<Option<String>> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    Ok(load_from(path()?)?.remove(name))
}

pub fn set(name: &str, value: &str) -> Result<()> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = path()?;
    let mut map = load_from(p)?;
    map.insert(name.to_string(), value.to_string());
    save_to(p, &map)
}

pub fn delete(name: &str) -> Result<()> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = path()?;
    let mut map = load_from(p)?;
    if map.remove(name).is_some() {
        save_to(p, &map)?;
    }
    Ok(())
}

/// One-time move of keys saved by earlier versions in Windows Credential
/// Manager into the keys file: copied, read back to check, then removed
/// from Credential Manager. Only for the installed app's own library (not a
/// development copy or a separate test folder). Returns how many moved.
#[cfg(windows)]
pub fn import_from_credential_manager() -> Result<usize> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = path()?;
    let mut map = load_from(p)?;
    let mut found = Vec::new();
    for name in KNOWN {
        let targets = [format!("KaneegaBell:{name}"), format!("EmuFrontend:{name}")];
        let mut value = None;
        for t in &targets {
            if let Some(v) = credman::get(t)? {
                value.get_or_insert(v);
            }
        }
        if let Some(v) = value {
            map.entry(name.to_string()).or_insert(v);
            found.push((name, targets));
        }
    }
    if found.is_empty() {
        return Ok(0);
    }
    save_to(p, &map)?;
    // Only remove the old copies once the file reads back with every key.
    let back = load_from(p)?;
    if found.iter().any(|(name, _)| back.get(**name) != map.get(**name)) {
        return Err(Error::msg("the keys file didn't read back correctly; the keys stay in Credential Manager"));
    }
    for (_, targets) in &found {
        for t in targets {
            credman::delete(t)?;
        }
    }
    Ok(found.len())
}

#[cfg(not(windows))]
pub fn import_from_credential_manager() -> Result<usize> {
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(FILE_NAME);
        assert!(load_from(&p).unwrap().is_empty(), "no file yet: no keys");
        let mut map = BTreeMap::new();
        map.insert("sgdb-key".to_string(), "s3cret ✓".to_string());
        save_to(&p, &map).unwrap();
        assert_eq!(load_from(&p).unwrap(), map);
        // Encrypted on Windows: the key isn't readable in the file.
        #[cfg(windows)]
        assert!(!String::from_utf8_lossy(&std::fs::read(&p).unwrap()).contains("s3cret"));
        // Removing the last key removes the file.
        save_to(&p, &BTreeMap::new()).unwrap();
        assert!(!p.exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_damaged_file_is_an_error_not_silently_empty() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(FILE_NAME);
        std::fs::write(&p, b"not a DPAPI blob").unwrap();
        assert!(load_from(&p).is_err());
    }
}
