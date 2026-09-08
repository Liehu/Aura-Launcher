//! Explorer current-folder detection via IShellWindows COM (MVP2.1).
//!
//! MVP2.1 vertical slice: foreground Explorer window -> current folder ->
//! ContextSnapshot -> Context Suggestions / context commands -> Action.

use super::ContextSnapshot;
use std::time::{SystemTime, UNIX_EPOCH};

/// All open Explorer (file) windows: (hwnd, folder path). Best-effort —
/// any COM failure yields an empty list, never a panic.
#[cfg(windows)]
pub fn explorer_folders() -> Vec<(isize, String)> {
    use windows::core::{Interface, VARIANT};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellWindows, IWebBrowser2};

    // {9BA05972-F6A8-11CF-A442-00A0C90A8F39} CLSID_ShellWindows; the windows
    // crate does not export the constant, only the interface.
    const CLSID_SHELL_WINDOWS: windows::core::GUID =
        windows::core::GUID::from_u128(0x9ba05972_f6a8_11cf_a442_00a0c90a8f39);

    unsafe {
        // RPC_E_CHANGED_MODE means the thread already has COM in the other
        // apartment — that is fine, continue.
        let _hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let shell_windows: IShellWindows =
            match CoCreateInstance(&CLSID_SHELL_WINDOWS, None, CLSCTX_LOCAL_SERVER) {
                Ok(sw) => sw,
                Err(_) => return Vec::new(),
            };
        let count = shell_windows.Count().unwrap_or(0);
        let mut out = Vec::new();
        for i in 0..count {
            let Ok(dispatch) = shell_windows.Item(&VARIANT::from(i)) else {
                continue;
            };
            let Ok(browser) = dispatch.cast::<IWebBrowser2>() else {
                continue;
            };
            // only file-system Explorer windows carry a file:/// URL
            let Ok(url) = browser.LocationURL() else {
                continue;
            };
            let url = url.to_string();
            if url.is_empty() {
                continue;
            }
            let Some(path) = file_url_to_path(&url) else {
                continue;
            };
            let hwnd = browser.HWND().map(|h| h.0).unwrap_or(0);
            if !path.is_empty() {
                out.push((hwnd, path));
            }
        }
        out
    }
}

#[cfg(not(windows))]
pub fn explorer_folders() -> Vec<(isize, String)> {
    Vec::new()
}

/// `file:///C:/Users/a%20b` -> `C:\Users\a b` (percent-decoded, backslashes).
/// Non-file URLs (http://, special shell views) yield None.
pub fn file_url_to_path(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file:///")?;
    if rest.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(rest.len());
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = &rest[i + 1..i + 3];
                let v = u8::from_str_radix(hex, 16).ok()?;
                out.push(v as char);
                i += 3;
            }
            b'/' => {
                out.push('\\');
                i += 1;
            }
            c if c.is_ascii() => {
                out.push(c as char);
                i += 1;
            }
            _ => {
                // non-ASCII: fall back to pushing the raw char from str
                let ch = rest[i..].chars().next()?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// The folder of the Explorer window matching `foreground_hwnd` (or its root
/// owner). None when the foreground app is not Explorer or no window matches.
#[cfg(windows)]
pub fn folder_for_foreground(foreground_hwnd: isize) -> Option<String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GA_ROOTOWNER};
    let root = unsafe { GetAncestor(HWND(foreground_hwnd as *mut _), GA_ROOTOWNER) }.0 as isize;
    explorer_folders()
        .into_iter()
        .find(|(h, _)| *h == foreground_hwnd || *h == root)
        .map(|(_, p)| p)
}

#[cfg(not(windows))]
pub fn folder_for_foreground(_foreground_hwnd: isize) -> Option<String> {
    None
}

/// Full snapshot with Explorer folder (foreground app is resolved by the
/// caller's source). Duplicates ContextEngine::snapshot with the folder set
/// and a fresh timestamp.
pub fn snapshot_with_folder(mut snap: ContextSnapshot, folder: Option<String>) -> ContextSnapshot {
    snap.current_folder = folder;
    if snap.meta.is_none() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        snap.meta = Some(launcher_domain::ContextMeta {
            timestamp_ms: now,
            confidence: 1.0,
        });
    }
    snap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_parsing() {
        assert_eq!(
            file_url_to_path("file:///C:/Users/spence/Documents"),
            Some("C:\\Users\\spence\\Documents".to_string())
        );
        assert_eq!(
            file_url_to_path("file:///D:/work/my%20notes"),
            Some("D:\\work\\my notes".to_string())
        );
        assert_eq!(file_url_to_path("http://x/y"), None);
        assert_eq!(file_url_to_path("file:///"), None);
        assert_eq!(file_url_to_path(""), None);
    }

    #[cfg(windows)]
    #[test]
    fn explorer_enumeration_does_not_panic() {
        // functional on any desktop; must always return without panicking
        let _ = explorer_folders();
    }
}
