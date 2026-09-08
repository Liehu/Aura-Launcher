//! Action Engine (design spec 5.2): the only path from a Command to an effect.
//!
//! Actions are executed off the UI thread by the caller. On non-Windows
//! platforms (tests/CI) effects can be dry-run validated without executing.

use std::path::Path;

use launcher_domain::{Action, ActionKind, ActionPayload};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ActionError {
    #[error("no target to act on")]
    MissingTarget,
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported on this platform")]
    Unsupported,
    #[error("action disabled: {0}")]
    Disabled(String),
    #[error("confirmation required")]
    ConfirmationRequired,
    #[error("clipboard error: {0}")]
    Clipboard(String),
}

/// Write text to the Windows clipboard (CF_UNICODETEXT). MVP3.1: ActionKind::Copy
/// previously returned Effect::Copied without touching the real clipboard.
#[cfg(windows)]
// `cargo test` must never touch the real system clipboard (the Copy unit
// test otherwise overwrites whatever the user had there).
#[cfg(all(windows, not(test)))]
fn copy_to_clipboard(text: &str) -> Result<(), ActionError> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    if text.is_empty() {
        return Err(ActionError::Clipboard("empty text".into()));
    }
    unsafe {
        // the clipboard can be locked by another process briefly; retry a few times
        let mut opened = Err(windows::core::Error::empty());
        for _ in 0..5 {
            opened = OpenClipboard(None);
            if opened.is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        opened.map_err(|e| ActionError::Clipboard(format!("open: {e}")))?;
        let result = (|| -> Result<(), ActionError> {
            EmptyClipboard().map_err(|e| ActionError::Clipboard(format!("empty: {e}")))?;
            let mut units: Vec<u16> = text.encode_utf16().collect();
            units.push(0); // NUL terminator
            let bytes = units.len() * 2;
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes)
                .map_err(|e| ActionError::Clipboard(format!("alloc: {e}")))?;
            let dst = GlobalLock(handle);
            if dst.is_null() {
                return Err(ActionError::Clipboard("global lock failed".into()));
            }
            std::ptr::copy_nonoverlapping(units.as_ptr(), dst as *mut u16, units.len());
            let _ = GlobalUnlock(handle);
            if SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(handle.0)).is_err() {
                return Err(ActionError::Clipboard("set data failed".into()));
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(any(not(windows), test))]
fn copy_to_clipboard(_text: &str) -> Result<(), ActionError> {
    Ok(())
}

/// Outcome of executing an action.
#[derive(Debug, PartialEq)]
pub enum Effect {
    Launched(String),
    Copied(String),
    Revealed(String),
    Pasted,
    /// MVP4.0: the descriptor was validated by the resolver and accepted by
    /// the engine; the caller routes it to the PluginBroker, which is the
    /// effect executor for `plugin.*` (INV-047: never an independent gateway).
    PluginInvoked {
        action_id: Option<String>,
        input: serde_json::Value,
    },
    Skipped,
}

/// Normalize and reject clearly invalid paths (security baseline, spec 16).
pub fn normalize_path(raw: &str) -> Result<std::path::PathBuf, ActionError> {
    if raw.trim().is_empty() {
        return Err(ActionError::InvalidPath(raw.into()));
    }
    if raw.contains('\0') {
        return Err(ActionError::InvalidPath(raw.into()));
    }
    Ok(Path::new(raw).to_path_buf())
}

/// Validate an action without performing effects (used by tests / dry-run).
/// Disabled actions (capability-denied at resolution time, INV-027) are
/// presentable but never executable here — the single execution gate.
pub fn validate(action: &Action) -> Result<(), ActionError> {
    if let Some(reason) = &action.disabled_reason {
        return Err(ActionError::Disabled(reason.clone()));
    }
    // Execution policy gate (MVP3.2-B, INV-041): an unconfirmed action is
    // refused here — the single choke point in front of every Effect.
    if action.confirmation_required {
        return Err(ActionError::ConfirmationRequired);
    }
    match action.kind {
        ActionKind::Paste => Ok(()),
        ActionKind::Open
        | ActionKind::Execute
        | ActionKind::Reveal
        | ActionKind::RunAsAdmin
        | ActionKind::OpenTerminalHere => match action.payload.as_ref() {
            Some(ActionPayload::Path(p)) | Some(ActionPayload::CommandLine(p)) => {
                normalize_path(p).map(|_| ())
            }
            _ => Err(ActionError::MissingTarget),
        },
        ActionKind::Copy => match action.payload.as_ref() {
            Some(ActionPayload::Text(_) | ActionPayload::Path(_)) => Ok(()),
            _ => Err(ActionError::MissingTarget),
        },
        ActionKind::PluginInvoke => match action.payload.as_ref() {
            Some(ActionPayload::Json(v)) if v.is_object() => Ok(()),
            _ => Err(ActionError::MissingTarget),
        },
    }
}

/// Execute on Windows via ShellExecuteW (no console window flash).
#[cfg(windows)]
/// MVP3.2 `system.paste`: restore focus to the recorded pre-popup window
/// (the effect's own precondition), then simulate Ctrl+V there.
#[cfg(windows)]
fn send_paste(target: Option<&ActionPayload>) -> Result<Effect, ActionError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    if let Some(ActionPayload::Hwnd(h)) = target {
        if *h != 0 {
            let hwnd = HWND(*h as *mut _);
            unsafe {
                if IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                let ok = SetForegroundWindow(hwnd).as_bool();
                let _ = SetFocus(hwnd);
                tracing::debug!(set_foreground = ok, "paste.focus_restored");
            }
        }
    }
    // let the OS settle the focus switch before injecting the keystroke
    std::thread::sleep(std::time::Duration::from_millis(120));
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL,
        VK_V,
    };
    let key = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                ..Default::default()
            },
        },
    };
    unsafe {
        let inputs = [
            key(VK_CONTROL.0, false),
            key(VK_V.0, false),
            key(VK_V.0, true),
            key(VK_CONTROL.0, true),
        ];
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent != inputs.len() as u32 {
            return Err(ActionError::Unsupported);
        }
    }
    tracing::info!("paste.sent");
    Ok(Effect::Pasted)
}

pub fn execute(action: &Action) -> Result<Effect, ActionError> {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    validate(action)?;
    // Paste carries no payload; it sends the keystroke after focus restore
    // and must be dispatched before the generic target extraction.
    if action.kind == ActionKind::Paste {
        return send_paste(action.payload.as_ref());
    }
    // MVP4.0: engine validates + policy-checks; the PluginBroker (caller)
    // produces the actual effect via the plugin's execute_action RPC.
    if action.kind == ActionKind::PluginInvoke {
        let input = match action.payload.as_ref() {
            Some(ActionPayload::Json(v)) => v.clone(),
            _ => serde_json::Value::Null,
        };
        tracing::info!(action = ?action.id, "plugin_action.validated");
        return Ok(Effect::PluginInvoked {
            action_id: action.id.clone(),
            input,
        });
    }
    let target = match action.payload.as_ref() {
        Some(ActionPayload::Path(p)) | Some(ActionPayload::CommandLine(p)) => p.clone(),
        Some(ActionPayload::Text(t)) => t.clone(),
        _ => return Err(ActionError::MissingTarget),
    };
    match action.kind {
        ActionKind::Paste => send_paste(action.payload.as_ref()),
        ActionKind::PluginInvoke => match action.payload.as_ref() {
            Some(ActionPayload::Json(v)) => Ok(Effect::PluginInvoked {
                action_id: action.id.clone(),
                input: v.clone(),
            }),
            _ => Err(ActionError::MissingTarget),
        },
        ActionKind::Copy => {
            copy_to_clipboard(&target)?;
            tracing::info!(chars = target.len(), "clipboard.copied");
            Ok(Effect::Copied(target))
        }
        ActionKind::RunAsAdmin => {
            let path = normalize_path(&target)?;
            let file = HSTRING::from(path.as_os_str());
            let result = unsafe {
                ShellExecuteW(None, &HSTRING::from("runas"), &file, None, None, SW_SHOWNORMAL)
            };
            if result.0 as isize <= 32 {
                return Err(ActionError::Unsupported);
            }
            tracing::info!(path = %target, "launched as admin");
            Ok(Effect::Launched(target))
        }
        ActionKind::Open | ActionKind::Execute => {
            let path = normalize_path(&target)?;
            // Use ShellExecute so documents open with their default handler.
            let file = HSTRING::from(path.as_os_str());
            let result = unsafe {
                ShellExecuteW(
                    None,
                    &HSTRING::from("open"),
                    &file,
                    None,
                    None,
                    SW_SHOWNORMAL,
                )
            };
            if result.0 as isize <= 32 {
                // ShellExecuteW failure: fall back to direct spawn for exes
                let status = std::process::Command::new(&path).spawn()?;
                tracing::info!(pid = status.id(), "spawned via fallback");
            }
            Ok(Effect::Launched(target))
        }
        ActionKind::Reveal => {
            let path = normalize_path(&target)?;
            let parent = path.parent().unwrap_or(Path::new("."));
            let status = std::process::Command::new("explorer").arg(parent).spawn()?;
            tracing::info!(pid = status.id(), "revealed");
            Ok(Effect::Revealed(target))
        }
        ActionKind::OpenTerminalHere => {
            let path = normalize_path(&target)?;
            let status = std::process::Command::new("cmd")
                .args(["/C", "start", "cmd"])
                .current_dir(path)
                .spawn()?;
            tracing::info!(pid = status.id(), "opened terminal");
            Ok(Effect::Launched(target))
        }
    }
}

/// Non-Windows (CI): validate only, no side effects.
#[cfg(not(windows))]
pub fn execute(action: &Action) -> Result<Effect, ActionError> {
    validate(action)?;
    match action.kind {
        ActionKind::Paste => Ok(Effect::Pasted),
        ActionKind::Copy => match action.payload.as_ref() {
            Some(ActionPayload::Text(t)) => Ok(Effect::Copied(t.clone())),
            Some(ActionPayload::Path(p)) => Ok(Effect::Copied(p.clone())),
            _ => Err(ActionError::MissingTarget),
        },
        _ => Ok(Effect::Skipped),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(path: &str) -> Action {
        Action {
            kind: ActionKind::Open,
            payload: Some(ActionPayload::Path(path.into())),
            id: None,
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        }
    }

    #[test]
    fn validate_requires_target() {
        assert!(validate(&Action {
            kind: ActionKind::Open,
            payload: None,
            id: None,
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        })
        .is_err());
        assert!(validate(&open("C:\\Windows\\notepad.exe")).is_ok());
        assert!(validate(&open("")).is_err());
        assert!(validate(&open("bad\0path")).is_err());
    }

    #[test]
    fn copy_validates_text() {
        let a = Action {
            kind: ActionKind::Copy,
            payload: Some(ActionPayload::Text("hi".into())),

            id: None,
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        };
        assert!(validate(&a).is_ok());
        match execute(&a) {
            Ok(Effect::Copied(t)) => assert_eq!(t, "hi"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn disabled_actions_are_never_executable() {
        let mut a = Action {
            kind: ActionKind::Copy,
            payload: Some(ActionPayload::Text("x".into())),
            id: None,
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        };
        assert!(validate(&a).is_ok());
        a.disabled_reason = Some("capability denied: clipboard.write".into());
        assert!(matches!(validate(&a), Err(ActionError::Disabled(_))));
        assert!(matches!(execute(&a), Err(ActionError::Disabled(_))));
    }

    #[test]
    fn unconfirmed_actions_are_refused_until_host_confirms() {
        let mut a = Action {
            kind: ActionKind::Execute,
            payload: Some(ActionPayload::CommandLine("cmd".into())),
            id: None,
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: true,
        };
        assert!(matches!(
            execute(&a),
            Err(ActionError::ConfirmationRequired)
        ));
        // host confirmation = clearing the policy flag (INV-042)
        a.confirmation_required = false;
        let _ = execute(&a); // the effect itself is CI-environment dependent
    }

    #[test]
    fn normalize_rejects_empty() {
        assert!(normalize_path("   ").is_err());
        assert!(normalize_path("C:\\x").is_ok());
    }
}
