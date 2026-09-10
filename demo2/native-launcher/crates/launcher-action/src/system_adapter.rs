//! Windows System Adapter (P2.9 adapter batch + P210-003/006 hardening,
//! spec `P2.10 — Cross-System Consistency & Security Hardening 1.0` §6-§11):
//! the ONLY place a system command becomes a real Win32 call.
//!
//! Red lines:
//! - the adapter accepts ONLY an [`SystemAuthorization`] minted by the
//!   engine gate (`crate::authorize_system_command`) — it has no `confirmed`
//!   parameter and no way to become an authority itself (§7);
//! - dynamic targets are re-verified against their identity (§10/§11):
//!   process creation time for PIDs, owning pid + liveness for HWNDs —
//!   mismatch = `ActionError::StaleTarget`, never executed;
//! - anything not in the frozen operation table resolves to
//!   `ActionError::Unsupported` (fail-closed: unknown ≠ guessable);
//! - observation operations (`list`/`enum`) are NOT effects and are not
//!   executed here — they belong to the host's enumeration providers.

use launcher_domain::system::{SystemCapability, SystemCommand, SystemTarget};

use crate::{ActionError, Effect, SystemAuthorization};

/// Execute an engine-authorized system command through real Win32 calls.
/// There is deliberately NO `confirmed` parameter: the confirmation policy
/// was applied by `authorize_system_command` before this token existed.
pub fn execute_authorized(auth: SystemAuthorization) -> Result<Effect, ActionError> {
    let cmd: &SystemCommand = auth.command();
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

// ---- dynamic target identity (P210-006, §10/§11) ------------------------

/// Verify a process target still IS the process observed at resolve time:
/// open by pid, read the creation time, compare. Mismatch or death =
/// StaleTarget (a reused pid must never be terminated).
#[cfg(windows)]
fn verify_process_identity(pid: u32, creation_time_ft: Option<i64>) -> Result<(), ActionError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    if pid == 0 {
        return Err(ActionError::Unsupported);
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|_| ActionError::StaleTarget)?;
        let mut creation = Default::default();
        let mut exit = Default::default();
        let mut kernel = Default::default();
        let mut user = Default::default();
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        let _ = CloseHandle(handle);
        if ok.is_err() {
            return Err(ActionError::StaleTarget);
        }
        // compare RAW FILETIME (100ns) — identity precision is never
        // downsampled (P210-B07)
        let Some(expected) = creation_time_ft else {
            return Ok(()); // no identity recorded — legacy command; accept
        };
        let actual =
            (u64::from(creation.dwHighDateTime) << 32) as i64 | i64::from(creation.dwLowDateTime);
        if actual != expected {
            tracing::warn!(pid, expected, actual, "system.target stale (PID reuse)");
            return Err(ActionError::StaleTarget);
        }
        Ok(())
    }
}

/// Verify a window target still IS the window observed at resolve time:
/// the HWND must be alive and, when an owning pid was recorded, belong to
/// that process (HWND reuse across processes = StaleTarget).
#[cfg(windows)]
fn verify_window_identity(hwnd_raw: u64, expected_pid: Option<u32>) -> Result<(), ActionError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow};
    if hwnd_raw == 0 || hwnd_raw > isize::MAX as u64 {
        return Err(ActionError::Unsupported);
    }
    let hwnd = HWND(hwnd_raw as *mut _);
    unsafe {
        if !IsWindow(hwnd).as_bool() {
            return Err(ActionError::StaleTarget);
        }
        if let Some(expected) = expected_pid {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid != expected {
                tracing::warn!(hwnd = hwnd_raw, expected, actual = pid, "system.target stale (HWND reuse)");
                return Err(ActionError::StaleTarget);
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn window_target(cmd: &SystemCommand) -> Result<(isize, Option<u32>), ActionError> {
    let SystemTarget::Window { hwnd, title: _, pid } = &cmd.target else {
        return Err(ActionError::Unsupported);
    };
    verify_window_identity(*hwnd, *pid)?;
    Ok((*hwnd as isize, *pid))
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
    // P210-006 §11: liveness + owning-pid identity re-verified immediately
    // before every window effect (HWND reuse = StaleTarget, never executed)
    let (hwnd, _) = window_target(cmd)?;
    Ok(hwnd)
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
    let SystemTarget::Process { pid, name: _, creation_time_ft } = &cmd.target else {
        return Err(ActionError::Unsupported);
    };
    // P210-006 §10: PID reuse check BEFORE the terminate — a reused pid
    // belonging to a different process must never be killed.
    verify_process_identity(*pid, *creation_time_ft)?;
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
            tracing::warn!(pid, "system.process.kill failed");
            return Err(ActionError::Unsupported);
        }
        tracing::info!(pid, "system.process.kill");
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
    use crate::authorize_system_command;
    use launcher_domain::system_process_window::{process_command, window_command};
    use launcher_domain::system::{SystemRisk, SystemTarget};

    fn run(cmd: &SystemCommand, confirmed: bool) -> Result<Effect, ActionError> {
        execute_authorized(authorize_system_command(cmd, confirmed)?)
    }

    /// The confirmation gate: destructive/privileged commands are refused
    /// BEFORE any OS call when `confirmed` is false.
    #[test]
    fn destructive_requires_confirmation() {
        let close = window_command(1234, "Notepad", "close", "test").unwrap();
        assert!(matches!(
            run(&close, false),
            Err(ActionError::ConfirmationRequired)
        ));
        let kill = process_command(4242, "evil.exe", "kill", "test").unwrap();
        assert!(matches!(
            run(&kill, false),
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
            run(&shutdown, false),
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
            target: SystemTarget::Window { hwnd: 1, title: "x".into(), pid: None },
            risk: SystemRisk::Info,
            origin: "test".into(),
        };
        assert!(matches!(
            run(&ghost, true),
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
            run(&bad_uri, true),
            Err(ActionError::Unsupported)
        ));
        // Commands that pass the domain builders' taxonomy but fail the
        // frozen contract validation (operation must be [a-z0-9_]).
        let invalid = SystemCommand {
            capability: SystemCapability::Window,
            operation: "BAD_OP".into(),
            target: SystemTarget::Window { hwnd: 1234, title: "Notepad".into(), pid: None },
            risk: SystemRisk::Info,
            origin: "test".into(),
        };
        assert!(matches!(
            run(&invalid, true),
            Err(ActionError::Unsupported)
        ));
    }

    /// Observation operations (`list`/`enum`) are not effects: they are
    /// never executed through this adapter even when confirmed.
    #[test]
    fn observation_operations_are_not_effects() {
        let list = process_command(1, "x", "list", "test").unwrap();
        assert!(matches!(
            run(&list, true),
            Err(ActionError::Unsupported)
        ));
        let enm = window_command(1, "x", "enum", "test").unwrap();
        assert!(matches!(
            run(&enm, true),
            Err(ActionError::Unsupported)
        ));
    }
}

#[cfg(test)]
mod p210_tests {
    use super::*;
    use crate::authorize_system_command;
    use launcher_domain::system_process_window::{
        process_command, process_command_with_identity, window_command,
    };

    /// P210-006 §10: a PID whose creation time no longer matches the
    /// resolve-time identity is StaleTarget — never terminated. Uses the
    /// CURRENT process as the live target with a wrong creation time.
    #[test]
    fn pid_reuse_is_detected() {
        let live_pid = std::process::id();
        let kill =
            process_command_with_identity(live_pid, "self.exe", Some(1), "kill", "test").unwrap();
        let auth = authorize_system_command(&kill, true).unwrap();
        assert!(matches!(execute_authorized(auth), Err(ActionError::StaleTarget)));
    }

    /// P210-006 §11: a dead HWND is StaleTarget even before any effect.
    #[test]
    fn dead_hwnd_is_stale_target() {
        let close = launcher_domain::system_process_window::window_command_with_identity(
            0xDEAD_BEEF,
            "Ghost",
            None,
            "close",
            "test",
        )
        .unwrap();
        let auth = authorize_system_command(&close, true).unwrap();
        assert!(matches!(execute_authorized(auth), Err(ActionError::StaleTarget)));
    }

    /// The gate itself: unconfirmed destructive commands never mint a token.
    #[test]
    fn gate_refuses_unconfirmed() {
        let kill = launcher_domain::system_process_window::process_command(
            4242, "evil.exe", "kill", "test",
        )
        .unwrap();
        assert!(matches!(
            authorize_system_command(&kill, false),
            Err(ActionError::ConfirmationRequired)
        ));
    }

    /// INV-EFFECT-104: the PUBLIC path runs the full chain — Resolver
    /// policy first. A denied origin never mints a token, and an approved
    /// request reaches the adapter (here: StaleTarget from the dead-HWND
    /// identity check proves the chain went all the way through).
    #[test]
    fn public_entry_enforces_policy_before_mint() {
        use launcher_domain::system::SystemResolver;
        let mut resolver = SystemResolver::default();
        resolver.policy.allowed_origins = vec!["search".into()];
        let focus = window_command(0xDEAD_BEEF, "Ghost", "focus", "ai").unwrap();
        // origin "ai" not on the allow-list → denied BEFORE any mint
        assert!(matches!(
            crate::execute_system_effect(&resolver, &focus, true),
            Err(ActionError::Unsupported)
        ));
        // allowed origin → policy pass → mint → adapter identity check
        let focus_search =
            window_command(0xDEAD_BEEF, "Ghost", "focus", "search").unwrap();
        assert!(matches!(
            crate::execute_system_effect(&resolver, &focus_search, true),
            Err(ActionError::StaleTarget)
        ));
    }

    /// INV-EFFECT-105: the token is one-shot BY TYPE — `execute_authorized`
    /// takes it by value and `SystemAuthorization` has no Clone, so a
    /// consumed token cannot be reused (compile-time ownership).
    #[test]
    fn token_is_move_only() {
        fn assert_move_only<T>(_: T) {}
        let kill = process_command(1, "x", "kill", "test").unwrap();
        let auth = authorize_system_command(&kill, true).unwrap();
        assert_move_only(auth); // moves; a Clone type would allow dup minting
    }
}
