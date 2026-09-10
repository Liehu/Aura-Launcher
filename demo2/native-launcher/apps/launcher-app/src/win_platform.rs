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

/// P3.1-B2: ALL visible, titled top-level windows (target picker data).
/// Observation only — nothing is touched by listing them.
#[cfg(windows)]
pub fn enumerate_visible_windows() -> Vec<WindowInfo> {
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
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() || owner == 0 {
            return BOOL(1);
        }
        let mut buf = [0u16; 256];
        let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if len == 0 {
            return BOOL(1);
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
    out
}

#[cfg(not(windows))]
pub fn enumerate_visible_windows() -> Vec<WindowInfo> {
    Vec::new()
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

// ---- P3.1-B2: follow pin (target-anchored topmost) ----------------------
// "Pin above a specific app" WITHOUT touching that app: an out-of-context
// WinEventHook (EVENT_SYSTEM_FOREGROUND) observes foreground changes — a
// system broadcast, NOT an injection. When the TARGET becomes foreground,
// the pinned plugin window is brought to TOPMOST; otherwise (when it was
// only follow-pinned, not user-pinned) it is restored to NOTOPMOST.

use std::sync::Mutex as SyncMutex;

/// P3.1-B1/B2 pin table: hwnd → user-pinned (topmost via the Pin button).
/// Lives here so the follow hook can distinguish "user-pinned" (never
/// auto-restore) from "follow-pinned" (restore when target unfocused).
pub static PIN_TABLE: SyncMutex<Option<std::collections::HashMap<isize, bool>>> =
    SyncMutex::new(None);

pub fn pin_mark(hwnd: isize, on: bool) {
    if let Some(map) = PIN_TABLE.lock().ok().as_mut() {
        let map = map.get_or_insert_with(std::collections::HashMap::new);
        if on {
            map.insert(hwnd, true);
        } else {
            map.remove(&hwnd);
        }
    }
}

pub fn pin_is_marked(hwnd: isize) -> bool {
    PIN_TABLE
        .lock()
        .ok()
        .and_then(|m| m.as_ref().and_then(|m| m.get(&hwnd).copied()))
        .unwrap_or(false)
}

/// (pin_hwnd, target_hwnd, target_title) — at most one follow relation.
static FOLLOW: SyncMutex<Option<(isize, isize, String)>> = SyncMutex::new(None);

/// Pure predicate (FOL-1): re-assert topmost for the pinned window when the
/// TARGET became foreground.
pub fn follow_should_topmost(follow: Option<(isize, isize)>, foreground_hwnd: isize) -> bool {
    matches!(follow, Some((_, target)) if foreground_hwnd == target)
}

/// Pure predicate: restore (NOTOPMOST) when the target is no longer
/// foreground and the pin is not user-pinned.
pub fn follow_should_restore(
    follow: Option<(isize, isize)>,
    foreground_hwnd: isize,
    user_pinned: bool,
) -> bool {
    !user_pinned
        && matches!(follow, Some((_, target)) if foreground_hwnd != target)
}

pub fn follow_current() -> Option<(isize, isize, String)> {
    FOLLOW.lock().ok().and_then(|f| f.clone())
}

fn is_window_alive(hwnd: isize) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::IsWindow;
        unsafe { IsWindow(HWND(hwnd as *mut _)) }.as_bool()
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        true
    }
}

/// Register a follow relation and start the foreground hook thread
/// (idempotent — one hook for the process lifetime, event-driven).
pub fn follow_set(pin: isize, target: isize, target_title: String) -> Result<(), String> {
    // UIPI probe: we must be able to touch the pinned window NOW; a denial
    // fails loudly here instead of silently doing nothing later.
    set_topmost(pin, true)?;
    if let Ok(mut f) = FOLLOW.lock() {
        *f = Some((pin, target, target_title));
    }
    start_foreground_hook();
    Ok(())
}

/// Clear the follow relation for `pin` and restore NOTOPMOST (unless the
/// window is also user-pinned). Returns true when a relation was removed.
pub fn follow_clear(pin: isize) -> bool {
    let removed = FOLLOW
        .lock()
        .ok()
        .map(|mut f| {
            let hit = f.as_ref().map(|(p, _, _)| *p == pin).unwrap_or(false);
            if hit {
                *f = None;
            }
            hit
        })
        .unwrap_or(false);
    if removed && !pin_is_marked(pin) {
        let _ = set_topmost(pin, false);
    }
    removed
}

#[cfg(windows)]
static HOOK_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Start the (single) foreground hook thread. Event-driven: it blocks in
/// GetMessage between events, so idle cost is zero (FOL-5).
#[cfg(windows)]
fn start_foreground_hook() {
    use std::sync::atomic::Ordering;
    if HOOK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("follow-hook".into())
        .spawn(move || {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
            use windows::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, GetMessageW, MSG, EVENT_SYSTEM_FOREGROUND,
                WINEVENT_OUTOFCONTEXT,
            };
            unsafe extern "system" fn on_foreground(
                _hook: HWINEVENTHOOK,
                _event: u32,
                hwnd: HWND,
                _id_object: i32,
                _id_child: i32,
                _thread: u32,
                _time: u32,
            ) {
                let fg = hwnd.0 as isize;
                let follow = FOLLOW.lock().ok().and_then(|f| f.clone());
                let Some((pin, target, _)) = follow else {
                    return;
                };
                // auto-release: a dead target or dead pin ends the follow
                if !is_window_alive(target) || !is_window_alive(pin) {
                    if let Ok(mut f) = FOLLOW.lock() {
                        *f = None;
                    }
                    tracing::info!(target, pin, "follow.auto_released (window gone)");
                    return;
                }
                if follow_should_topmost(Some((pin, target)), fg) {
                    let _ = set_topmost(pin, true);
                    tracing::debug!(pin, target, "follow.reassert_topmost");
                } else if follow_should_restore(
                    Some((pin, target)),
                    fg,
                    pin_is_marked(pin),
                ) {
                    let _ = set_topmost(pin, false);
                }
            }
            unsafe {
                let hook = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    windows::Win32::Foundation::HMODULE::default(),
                    Some(on_foreground),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                if hook.is_invalid() {
                    tracing::error!("follow.hook_failed");
                    return;
                }
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                    let _ = DispatchMessageW(&msg);
                }
            }
        })
        .ok();
}

#[cfg(not(windows))]
fn start_foreground_hook() {}

#[cfg(test)]
mod p31_b2_tests {
    use super::*;

    /// FOL-1: the follow state machine predicates.
    #[test]
    fn follow_predicates() {
        let f = Some((10, 20));
        assert!(follow_should_topmost(f, 20), "target foreground -> topmost");
        assert!(!follow_should_topmost(f, 30));
        assert!(follow_should_restore(f, 30, false));
        assert!(
            !follow_should_restore(f, 30, true),
            "user-pinned windows are never auto-restored"
        );
        assert!(!follow_should_restore(f, 20, false));
        assert!(!follow_should_topmost(None, 20));
    }
}

// ---- P3.1-B3: desktop pin (WorkerW) — EXPERIMENTAL ----------------------
// Pins a plugin window BEHIND the desktop icons by re-parenting it to the
// WorkerW band Windows creates for wallpaper-style windows. Best effort:
// the framework compatibility matrix (tkinter/Electron/Qt) is a manual
// checklist (P3.1-B3); failures degrade loudly.

#[cfg(windows)]
pub fn pin_desktop(pin: isize) -> Result<(), String> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, FindWindowExW, FindWindowW, SendMessageW, SetParent,
    };
    use windows::core::{w, PCWSTR};

    if pin == 0 {
        return Err("invalid hwnd".into());
    }
    unsafe {
        // 1. ask Progman to spawn a WorkerW behind the icons (0x052C)
        let progman = FindWindowW(w!("Progman"), PCWSTR::null())
            .map_err(|e| format!("progman not found: {e}"))?;
        SendMessageW(progman, 0x052C, WPARAM(0), LPARAM(0));

        // 2. find the WorkerW that directly parents SHELLDLL_DefView (the
        //    desktop icon layer)
        let mut worker: Option<HWND> = None;
        unsafe extern "system" fn scan(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let out = unsafe { &mut *(lparam.0 as *mut Option<HWND>) };
            let shell = unsafe {
                FindWindowExW(hwnd, None, PCWSTR::null(), w!("SHELLDLL_DefView"))
            };
            if shell.is_ok() {
                *out = Some(hwnd);
                return BOOL(0);
            }
            BOOL(1)
        }
        let _ = EnumChildWindows(progman, Some(scan), LPARAM(&mut worker as *mut _ as isize));
        let Some(worker) = worker else {
            return Err("WorkerW band not found".into());
        };

        // 3. re-parent the plugin window behind the icon layer
        SetParent(HWND(pin as *mut _), worker).map_err(|e| format!("set_parent: {e}"))?;
    }
    tracing::info!(hwnd = pin, "desktop.pinned (WorkerW)");
    Ok(())
}

/// Unpin: restore the window to its original top-level position.
#[cfg(windows)]
pub fn unpin_desktop(pin: isize) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::SetParent;
    if pin == 0 {
        return Err("invalid hwnd".into());
    }
    unsafe {
        SetParent(HWND(pin as *mut _), None).map_err(|e| format!("set_parent(null): {e}"))?;
    }
    tracing::info!(hwnd = pin, "desktop.unpinned");
    Ok(())
}

#[cfg(not(windows))]
pub fn pin_desktop(_pin: isize) -> Result<(), String> {
    Err("unsupported platform".into())
}

#[cfg(not(windows))]
pub fn unpin_desktop(_pin: isize) -> Result<(), String> {
    Err("unsupported platform".into())
}
