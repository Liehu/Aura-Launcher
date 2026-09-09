//! Windows System Adapter (P2.9 — the deferred "Windows Adapter" batch,
//! spec `P2.9 — System Integration & Automation 1.0 技术设计规范.md` §21):
//! the ONLY place a validated [`SystemCommand`] becomes a real Win32 call.
//!
//! Red lines kept intact:
//! - the command must pass `SystemCommand::validate()` (frozen contract);
//! - destructive/privileged operations additionally require the caller's
//!   `confirmed = true` — the adapter is downstream of Policy, never a
//!   policy authority of its own (mirrors `Action`'s INV-041 gate);
//! - anything not in the frozen operation table resolves to
//!   `ActionError::Unsupported` (fail-closed: unknown ≠ guessable);
//! - observation operations (`list`/`enum`) are NOT effects and are not
//!   executed here — they belong to the host's enumeration providers.

use launcher_domain::system::{SystemCapability, SystemCommand, SystemTarget};

use crate::{ActionError, Effect};

/// Execute a validated system command through real Win32 calls.
/// `confirmed` must carry the host's confirmation decision; destructive
/// and privileged operations are refused without it.
pub fn execute_system_command(
    cmd: &SystemCommand,
    confirmed: bool,
) -> Result<Effect, ActionError> {
    if cmd.validate().is_err() {
        return Err(ActionError::Unsupported);
    }
    // The confirmation gate mirrors launcher-action's INV-041 choke point:
    // an unconfirmed destructive command is refused BEFORE any OS call.
    if cmd.risk.requires_confirmation() && !confirmed {
        return Err(ActionError::ConfirmationRequired);
    }
    match (cmd.capability, cmd.operation.as_str()) {
        (SystemCapability::Window, "focus") => window_focus(cmd),
        (SystemCapability::Window, "minimize") => window_show_minimize(cmd),
        (SystemCapability::Window, "restore") => window_show_restore(cmd),
        (SystemCapability::Window, "close") => window_close(cmd),
        (SystemCapability::Process, "kill" | "terminate") => process_kill(cmd),
        (SystemCapability::Power, "lock") => power_lock(),
        (SystemCapability::Power, "sleep") => power_sleep(),
        (SystemCapability::Power, "restart") => power_restart(),
        (SystemCapability::Power, "shutdown") => power_shutdown(),
        (SystemCapability::Uri, "open_uri") | (SystemCapability::Shell, "open_uri") => {
            open_uri(cmd)
        }
        // Everything else — including observation ops (list/enum) and the
        // deliberately-unwired shell.run_command / notify — fails closed.
        _ => Err(ActionError::Unsupported),
    }
}

// ---- uri ----------------------------------------------------------------

/// ShellExecute an `open` on the scheme URI. The frozen target carries the
/// scheme only; it is strictly validated before it may reach the shell.
fn open_uri(cmd: &SystemCommand) -> Result<Effect, ActionError> {
    let SystemTarget::Uri { scheme } = &cmd.target else {
        return Err(ActionError::Unsupported);
    };
    let scheme_ok = !scheme.is_empty()
        && scheme.len() <= 32
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && scheme.chars().next().is_some_and(|c| c.is_ascii_alphanumeric());
    if !scheme_ok {
        return Err(ActionError::Unsupported);
    }
    let uri = format!("{scheme}:");
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let r = unsafe {
        ShellExecuteW(
            None,
            &HSTRING::from("open"),
            &HSTRING::from(&uri),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if r.0 as isize > 32 {
        Ok(Effect::SystemApplied(format!("uri.open:{uri}")))
    } else {
        Err(ActionError::Unsupported)
    }
}

// ---- window -------------------------------------------------------------

fn window_hwnd(cmd: &SystemCommand) -> Result<isize, ActionError> {
    let SystemTarget::Window { hwnd, .. } = &cmd.target else {
        return Err(ActionError::Unsupported);
    };
    // HWND values are pointer-sized; anything above that range is corrupt
    // input from upstream enumeration and is refused.
    if *hwnd == 0 || *hwnd > isize::MAX as u64 {
        return Err(ActionError::Unsupported);
    }
    Ok(*hwnd as isize)
}

fn window_focus(cmd: &SystemCommand) -> Result<Effect, ActionError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };
    let hwnd_raw = window_hwnd(cmd)?;
    let hwnd = HWND(hwnd_raw as *mut _);
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        let ok = SetForegroundWindow(hwnd).as_bool();
        let _ = SetFocus(hwnd);
        tracing::info!(hwnd = hwnd_raw, foreground = ok, "system.window.focus");
    }
    Ok(Effect::SystemApplied("window.focus".into()))
}

fn window_show_minimize(cmd: &SystemCommand) -> Result<Effect, ActionError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MINIMIZE};
    let hwnd = HWND(window_hwnd(cmd)? as *mut _);
    unsafe {
        let _ = ShowWindow(hwnd, SW_MINIMIZE);
    }
    tracing::info!(hwnd = window_hwnd(cmd)?, "system.window.minimize");
    Ok(Effect::SystemApplied("window.minimize".into()))
}

fn window_show_restore(cmd: &SystemCommand) -> Result<Effect, ActionError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_RESTORE};
    let hwnd = HWND(window_hwnd(cmd)? as *mut _);
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
    }
    tracing::info!(hwnd = window_hwnd(cmd)?, "system.window.restore");
    Ok(Effect::SystemApplied("window.restore".into()))
}

fn window_close(cmd: &SystemCommand) -> Result<Effect, ActionError> {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let hwnd_raw = window_hwnd(cmd)?;
    let hwnd = HWND(hwnd_raw as *mut _);
    unsafe {
        let posted = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
        if posted.is_ok() {
            tracing::info!(hwnd = hwnd_raw, "system.window.close (WM_CLOSE posted)");
        }
    }
    Ok(Effect::SystemApplied("window.close".into()))
}

// ---- process ------------------------------------------------------------

fn process_kill(cmd: &SystemCommand) -> Result<Effect, ActionError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, TerminateProcess, PROCESS_TERMINATE,
    };
    let SystemTarget::Process { pid, name } = &cmd.target else {
        return Err(ActionError::Unsupported);
    };
    if *pid == 0 {
        return Err(ActionError::Unsupported);
    }
    unsafe {
        // Open the process by pid; the pid came from the host's enumeration
        // snapshot and may be stale — a failed open is a clean error, never
        // a retry with looser rights.
        let opened = OpenProcess(PROCESS_TERMINATE, false, *pid);
        let Ok(handle) = opened else {
            return Err(ActionError::Unsupported);
        };
        let terminated = TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);
        if terminated.is_err() {
            tracing::warn!(pid, name, "system.process.kill failed");
            return Err(ActionError::Unsupported);
        }
        tracing::info!(pid, name, "system.process.kill");
    }
    Ok(Effect::SystemApplied("process.kill".into()))
}

// ---- power --------------------------------------------------------------

fn power_lock() -> Result<Effect, ActionError> {
    use windows::Win32::System::Shutdown::LockWorkStation;
    unsafe {
        LockWorkStation().map_err(|_| ActionError::Unsupported)?;
    }
    Ok(Effect::SystemApplied("power.lock".into()))
}

fn power_sleep() -> Result<Effect, ActionError> {
    unsafe {
        let ok = windows::Win32::System::Power::SetSuspendState(false, false, false);
        if !ok.as_bool() {
            return Err(ActionError::Unsupported);
        }
    }
    Ok(Effect::SystemApplied("power.sleep".into()))
}

fn power_restart() -> Result<Effect, ActionError> {
    use windows::Win32::System::Shutdown::{ExitWindowsEx, EWX_REBOOT, SHUTDOWN_REASON};
    unsafe {
        ExitWindowsEx(EWX_REBOOT, SHUTDOWN_REASON(0))
            .map_err(|_| ActionError::Unsupported)?;
    }
    Ok(Effect::SystemApplied("power.restart".into()))
}

fn power_shutdown() -> Result<Effect, ActionError> {
    use windows::Win32::System::Shutdown::{ExitWindowsEx, EWX_SHUTDOWN, SHUTDOWN_REASON};
    unsafe {
        ExitWindowsEx(EWX_SHUTDOWN, SHUTDOWN_REASON(0))
            .map_err(|_| ActionError::Unsupported)?;
    }
    Ok(Effect::SystemApplied("power.shutdown".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::system_process_window::{process_command, window_command};
    use launcher_domain::system::{SystemRisk, SystemTarget};

    /// The confirmation gate: destructive/privileged commands are refused
    /// BEFORE any OS call when `confirmed` is false.
    #[test]
    fn destructive_requires_confirmation() {
        let close = window_command(1234, "Notepad", "close", "test").unwrap();
        assert!(matches!(
            execute_system_command(&close, false),
            Err(ActionError::ConfirmationRequired)
        ));
        let kill = process_command(4242, "evil.exe", "kill", "test").unwrap();
        assert!(matches!(
            execute_system_command(&kill, false),
            Err(ActionError::ConfirmationRequired)
        ));
        let shutdown = SystemCommand {
            capability: SystemCapability::Power,
            operation: "shutdown".into(),
            target: SystemTarget::System { setting: "system".into() },
            risk: SystemRisk::Destructive,
            origin: "test".into(),
        };
        assert!(matches!(
            execute_system_command(&shutdown, false),
            Err(ActionError::ConfirmationRequired)
        ));
    }

    /// Fail-closed: unknown operations and invalid commands never reach the
    /// OS — regardless of confirmation state.
    #[test]
    fn unknown_operations_fail_closed() {
        let ghost = SystemCommand {
            capability: SystemCapability::Window,
            operation: "teleport".into(),
            target: SystemTarget::Window { hwnd: 1, title: "x".into() },
            risk: SystemRisk::Info,
            origin: "test".into(),
        };
        assert!(matches!(
            execute_system_command(&ghost, true),
            Err(ActionError::Unsupported)
        ));
        let bad_uri = SystemCommand {
            capability: SystemCapability::Uri,
            operation: "open_uri".into(),
            target: SystemTarget::Uri { scheme: "bad scheme!".into() },
            risk: SystemRisk::Info,
            origin: "test".into(),
        };
        assert!(matches!(
            execute_system_command(&bad_uri, true),
            Err(ActionError::Unsupported)
        ));
        // Commands that pass the domain builders' taxonomy but fail the
        // frozen contract validation (operation must be [a-z0-9_]).
        let invalid = SystemCommand {
            capability: SystemCapability::Window,
            operation: "BAD_OP".into(),
            target: SystemTarget::Window { hwnd: 1234, title: "Notepad".into() },
            risk: SystemRisk::Info,
            origin: "test".into(),
        };
        assert!(matches!(
            execute_system_command(&invalid, true),
            Err(ActionError::Unsupported)
        ));
    }

    /// Observation operations (`list`/`enum`) are not effects: they are
    /// never executed through this adapter even when confirmed.
    #[test]
    fn observation_operations_are_not_effects() {
        let list = process_command(1, "x", "list", "test").unwrap();
        assert!(matches!(
            execute_system_command(&list, true),
            Err(ActionError::Unsupported)
        ));
        let enm = window_command(1, "x", "enum", "test").unwrap();
        assert!(matches!(
            execute_system_command(&enm, true),
            Err(ActionError::Unsupported)
        ));
    }
}
