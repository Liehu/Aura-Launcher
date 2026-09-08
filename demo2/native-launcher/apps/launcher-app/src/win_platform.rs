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
