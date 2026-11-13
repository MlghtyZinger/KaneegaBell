//! Native desktop picker; the frontend retains its browser for controllers.

#[tauri::command]
pub async fn pick_path(
    window: tauri::WebviewWindow,
    title: String,
    start: Option<String>,
    mode: String,
    extensions: Vec<String>,
) -> Result<Option<String>, String> {
    if mode != "file" && mode != "folder" {
        return Err("Unknown picker mode".into());
    }
    #[cfg(windows)]
    {
        let owner = window.hwnd().map_err(|e| e.to_string())?.0 as usize;
        tauri::async_runtime::spawn_blocking(move || {
            show(
                owner,
                &title,
                start.as_deref(),
                mode == "folder",
                &extensions,
            )
            .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = (window, title, start, extensions);
        Err("Native file selection is supported on Windows".into())
    }
}

#[cfg(windows)]
fn show(
    owner: usize,
    title: &str,
    start: Option<&str>,
    folder: bool,
    extensions: &[String],
) -> windows::core::Result<Option<String>> {
    use windows::{
        core::{HSTRING, PCWSTR},
        Win32::{
            Foundation::{ERROR_CANCELLED, HWND},
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
            },
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog, IShellItem,
                SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
                SIGDN_FILESYSPATH,
            },
        },
    };
    // The blocking worker owns the COM apartment and all dialog interfaces.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        let mut options = dialog.GetOptions()? | FOS_FORCEFILESYSTEM;
        if folder {
            options |= FOS_PICKFOLDERS;
        }
        dialog.SetOptions(options)?;
        dialog.SetTitle(&HSTRING::from(title))?;
        let patterns = HSTRING::from(
            extensions
                .iter()
                .map(|e| format!("*.{}", e.trim_start_matches('.')))
                .collect::<Vec<_>>()
                .join(";"),
        );
        let label = HSTRING::from("Supported files");
        if !folder && !extensions.is_empty() {
            dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: PCWSTR(label.as_ptr()),
                pszSpec: PCWSTR(patterns.as_ptr()),
            }])?;
        }
        if let Some(dir) = start.and_then(crate::fsbrowse::nearest_existing_dir) {
            if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                &HSTRING::from(dir.to_string_lossy().as_ref()),
                None,
            ) {
                let _ = dialog.SetFolder(&item);
            }
        }
        if let Err(e) = dialog.Show(Some(HWND(owner as *mut _))) {
            if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(e);
        }
        let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let result = path.to_string();
        CoTaskMemFree(Some(path.0.cast()));
        Ok(Some(result?))
    }
}
