//! Global hotkey support (Windows `RegisterHotKey`), ported from demo1.
//!
//! Hotkey string parsing (`"Alt+Space"` -> virtual key code) is a pure
//! function with full unit-test coverage; registration runs on a dedicated
//! message-window thread.

use std::sync::mpsc::Sender;

use thiserror::Error;
use tracing::{error, info};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW, RegisterClassW,
    TranslateMessage, PM_REMOVE, WINDOW_EX_STYLE, WM_HOTKEY, WNDCLASSW,
};

const HOTKEY_ID: i32 = 0xB00C;

// The WNDPROC callback cannot carry Rust closure context; the notify sender
// is stored in a thread-local only touched by the hotkey thread.
thread_local! {
    static HOTKEY_TX: std::cell::RefCell<Option<Sender<()>>> =
        const { std::cell::RefCell::new(None) };
}

#[derive(Debug, Error)]
pub enum HotkeyError {
    #[error("invalid hotkey string: {0:?}")]
    InvalidHotkey(String),
    #[error("RegisterHotKey failed (hotkey may be in use by another app)")]
    RegisterFailed,
}

/// Parsed hotkey: modifier mask + primary key virtual code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedHotkey {
    pub modifiers: HOT_KEY_MODIFIERS,
    pub vk: u32,
}

/// Pure function: parse `"Alt+Space"` style strings into modifier mask and
/// virtual key code. Modifiers `Ctrl`/`Control`/`Alt`/`Shift`/`Win` in any
/// order; primary key supports A-Z/0-9, F1-F24 and common named keys.
pub fn parse_hotkey(s: &str) -> Result<ParsedHotkey, HotkeyError> {
    let mut modifiers = HOT_KEY_MODIFIERS(0);
    let mut key_part: Option<String> = None;
    for part in s.split('+') {
        let part = part.trim();
        if part.is_empty() {
            return Err(HotkeyError::InvalidHotkey(s.to_string()));
        }
        let lower = part.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" => modifiers |= MOD_ALT,
            "shift" => modifiers |= MOD_SHIFT,
            "win" | "super" => modifiers |= MOD_WIN,
            _ => {
                if key_part.is_some() {
                    return Err(HotkeyError::InvalidHotkey(s.to_string()));
                }
                key_part = Some(lower);
            }
        }
    }
    let key = key_part
        .as_deref()
        .ok_or_else(|| HotkeyError::InvalidHotkey(s.to_string()))?;
    let vk = named_key_vk(key).ok_or_else(|| HotkeyError::InvalidHotkey(s.to_string()))?;
    Ok(ParsedHotkey { modifiers, vk })
}

/// Key name -> virtual key code (pure function).
fn named_key_vk(key: &str) -> Option<u32> {
    match key {
        "space" => Some(0x20),
        "enter" | "return" => Some(0x0D),
        "escape" | "esc" => Some(0x1B),
        "tab" => Some(0x09),
        "backspace" => Some(0x08),
        "delete" | "del" => Some(0x2E),
        "insert" | "ins" => Some(0x2D),
        "home" => Some(0x24),
        "end" => Some(0x23),
        "pageup" => Some(0x21),
        "pagedown" => Some(0x22),
        "up" => Some(0x26),
        "down" => Some(0x28),
        "left" => Some(0x25),
        "right" => Some(0x27),
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
            s.chars().next().map(|c| c.to_ascii_uppercase() as u32)
        }
        s if s.len() >= 2 && s.starts_with('f') && s[1..].chars().all(|c| c.is_ascii_digit()) => {
            let n: u32 = s[1..].parse().ok()?;
            ((1..=24).contains(&n)).then(|| 0x6F + n)
        }
        _ => None,
    }
}

/// Global hotkey session: runs a Windows message loop on a dedicated thread
/// and sends `()` on `tx` per hotkey press. Dropping the guard unregisters.
pub struct GlobalHotkey {
    stop: Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl GlobalHotkey {
    pub fn spawn(
        hotkey: ParsedHotkey,
        tx: Sender<()>,
        result_tx: Sender<Result<(), HotkeyError>>,
    ) -> Self {
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("hotkey".into())
            .spawn(move || run_message_loop(hotkey, tx, result_tx, stop_rx))
            .map_err(|e| error!(error = %e, "failed to spawn hotkey thread"))
            .ok();
        Self {
            stop: stop_tx,
            thread,
        }
    }
}

impl Drop for GlobalHotkey {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

unsafe extern "system" fn hotkey_wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_HOTKEY {
        HOTKEY_TX.with(|t| {
            if let Some(tx) = t.borrow().as_ref() {
                let _ = tx.send(());
            }
        });
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn run_message_loop(
    hotkey: ParsedHotkey,
    tx: Sender<()>,
    result_tx: Sender<Result<(), HotkeyError>>,
    stop_rx: std::sync::mpsc::Receiver<()>,
) {
    HOTKEY_TX.with(|t| *t.borrow_mut() = Some(tx));

    let hinstance =
        HINSTANCE::from(unsafe { GetModuleHandleW(PCWSTR::null()) }.unwrap_or_default());
    let class_name = w!("LauncherHotkeyClass");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(hotkey_wndproc),
        hInstance: hinstance,
        lpszClassName: class_name,
        ..Default::default()
    };
    // class_name is a NUL-terminated static wide string (w!) and the wndproc
    // is a 'static system fn, satisfying RegisterClassW's safety contract.
    let atom = unsafe { RegisterClassW(&wc) };
    if atom == 0 {
        let _ = result_tx.send(Err(HotkeyError::RegisterFailed));
        return;
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(atom as *const u16),
            w!("launcher-hotkey"),
            Default::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(&hinstance),
            None,
        )
    }
    .unwrap_or_default();
    if hwnd.is_invalid() {
        let _ = result_tx.send(Err(HotkeyError::RegisterFailed));
        return;
    }

    if unsafe { RegisterHotKey(hwnd, HOTKEY_ID, hotkey.modifiers, hotkey.vk) }.is_err() {
        let _ = result_tx.send(Err(HotkeyError::RegisterFailed));
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return;
    }
    info!(
        vk = hotkey.vk,
        modifiers = hotkey.modifiers.0,
        "global hotkey registered"
    );
    let _ = result_tx.send(Ok(()));

    // Non-blocking pump + 50ms polling so Drop's stop signal is honored.
    let mut msg = Default::default();
    loop {
        if stop_rx.try_recv().is_ok() {
            unsafe {
                let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
                let _ = DestroyWindow(hwnd);
            }
            return;
        }
        unsafe {
            while PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_alt_space() {
        let hk = parse_hotkey("Alt+Space").expect("Alt+Space must parse");
        assert_eq!(hk.modifiers, MOD_ALT);
        assert_eq!(hk.vk, 0x20);
    }

    #[test]
    fn parse_ctrl_space() {
        let hk = parse_hotkey("Ctrl+Space").expect("Ctrl+Space must parse");
        assert_eq!(hk.modifiers, MOD_CONTROL);
        assert_eq!(hk.vk, 0x20);
    }

    #[test]
    fn parse_modifiers_any_order_and_case() {
        let a = parse_hotkey("ctrl+shift+p").expect("ctrl+shift+p must parse");
        let b = parse_hotkey("SHIFT + CTRL + P").expect("uppercase must parse");
        assert_eq!(a, b);
        assert_eq!(a.modifiers, MOD_CONTROL | MOD_SHIFT);
        assert_eq!(a.vk, b'P' as u32);
    }

    #[test]
    fn parse_function_keys() {
        assert_eq!(parse_hotkey("Ctrl+F1").expect("F1").vk, 0x70);
        assert_eq!(parse_hotkey("F12").expect("F12 without modifier").vk, 0x7B);
    }

    #[test]
    fn parse_errors_on_garbage() {
        assert!(parse_hotkey("").is_err());
        assert!(parse_hotkey("Alt+").is_err());
        assert!(parse_hotkey("Alt+Space+X").is_err());
        assert!(parse_hotkey("Alt+NotAKey").is_err());
        assert!(
            parse_hotkey("Space").is_ok(),
            "bare key without modifier is allowed"
        );
    }

    #[test]
    fn parse_win_key() {
        let hk = parse_hotkey("Win+D").expect("Win+D must parse");
        assert_eq!(hk.modifiers, MOD_WIN);
        assert_eq!(hk.vk, b'D' as u32);
    }
}
