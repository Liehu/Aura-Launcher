//! Windows platform window tuning (WIN-TRAY v0.1, review 50-mvp6-0.6).
//!
//! The launcher popup is a *transient tool window*, not an app window:
//! `WS_EX_TOOLWINDOW` suppresses the taskbar button while keeping full
//! activation/focus behavior. Deliberately NOT `WS_EX_NOACTIVATE` (would
//! break keyboard input) and NOT AppUserModelID (that controls shell
//! identity/grouping, not taskbar suppression).

use slint::Window;

/// Apply `WS_EX_TOOLWINDOW` to the Slint window's native HWND. Idempotent;
/// safe to call again after the backend recreates the native window.
pub fn apply_tool_window(window: &Window) {
    #[cfg(windows)]
    unsafe {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_APPWINDOW,
            WS_EX_TOOLWINDOW,
        };
        let handle = window.window_handle();
        let Ok(handle) = handle.window_handle() else {
            tracing::warn!("tool_window: no native window handle yet");
            return;
        };
        let RawWindowHandle::Win32(win) = handle.as_raw() else {
            return; // non-Windows handle shapes never occur on this platform
        };
        let hwnd = windows::Win32::Foundation::HWND(win.hwnd.get() as *mut _);
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // WS_EX_APPWINDOW (which winit adds to visible top-level windows)
        // FORCES a taskbar button and overrides WS_EX_TOOLWINDOW - clear it.
        let updated = (style | WS_EX_TOOLWINDOW.0 as isize)
            & !(WS_EX_APPWINDOW.0 as isize);
        if style != updated {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated);
            tracing::info!(before = style, after = updated, "window.tool_window.applied");
        }
    }

    #[cfg(not(windows))]
    let _ = window;
}

/// WIN-TRAY acceptance gates (manual, review 50-mvp6-0.6 §8):
/// - WIN-TRAY-001 resident + hidden + tray present + taskbar button absent
/// - WIN-TRAY-002 visible (any mode) + taskbar button stays absent
/// - WIN-TRAY-003 Esc -> hidden, process resident, tray present
/// - WIN-TRAY-004 exit -> window destroyed, tray removed, process gone
/// - WIN-TRAY-005 Action/Workflow mode -> taskbar button stays absent
/// - WIN-TRAY-006 explorer.exe restart -> tray icon re-registered (P1:
///   verify Slint backend handles the TaskbarCreated broadcast; if not,
///   re-add the icon on a foreground window notification)
#[cfg(test)]
mod tests {
    // No headless test: style checks require a live HWND. Covered by the
    // manual WIN-TRAY checklist in docs/MVP3.1-ACCEPTANCE.md.
}

// ---- P3.1-B1: plugin window discovery + topmost -------------------------
// Strict pid boundary: only windows whose owning process is a plugin the
// launcher itself spawned are ever enumerated or manipulated. Target
// processes are NEVER touched by these functions.

/// One enumerable top-level window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub pid: u32,
}

/// Enumerate VISIBLE, titled top-level windows owned by `pid`.
#[cfg(windows)]
pub fn visible_windows_of_pid(pid: u32) -> Vec<WindowInfo> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = unsafe { &mut *(lparam.0 as *mut Vec<WindowInfo>) };
        let mut owner: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut owner));
        }
        let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
        if !visible || owner == 0 {
            return BOOL(1);
        }
        let mut buf = [0u16; 256];
        let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if len == 0 {
            return BOOL(1); // untitled windows are not manageable targets
        }
        out.push(WindowInfo {
            hwnd: hwnd.0 as isize,
            title: String::from_utf16_lossy(&buf[..len as usize]),
            pid: owner,
        });
        BOOL(1)
    }

    let mut out: Vec<WindowInfo> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut out as *mut Vec<WindowInfo> as isize),
        );
    }
    out.retain(|w| w.pid == pid);
    out
}

/// Toggle topmost for a window we own a pin on (fail-loud on UIPI denial).
#[cfg(windows)]
pub fn set_topmost(hwnd: isize, topmost: bool) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };
    if hwnd == 0 {
        return Err("invalid hwnd".into());
    }
    unsafe {
        SetWindowPos(
            HWND(hwnd as *mut _),
            if topmost { HWND_TOPMOST } else { HWND_NOTOPMOST },
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        )
        .map_err(|e| format!("UIPI/set_window_pos: {e}"))?;
    }
    tracing::debug!(hwnd, topmost, "window.topmost");
    Ok(())
}

#[cfg(not(windows))]
pub fn visible_windows_of_pid(_pid: u32) -> Vec<WindowInfo> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn set_topmost(_hwnd: isize, _topmost: bool) -> Result<(), String> {
    Err("unsupported platform".into())
}
