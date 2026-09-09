//! P2.9 Batch 3: Process/Window capability taxonomy + risk mapping (spec
//! `P2.9 — System Integration & Automation 1.0 技术设计规范.md` §4-§6/§21).
//!
//! Pure v0.1 layer: classifies process/window operations and builds
//! validated SystemCommands. The Windows Adapter (Native API, later batch)
//! consumes these commands; classification here can only make confirmation
//! MORE likely, never less. No OS calls.

use crate::system::{SystemCapability, SystemCommand, SystemRisk, SystemTarget};

/// Known process operations and their risk (§6 风险分类).
pub fn process_risk(operation: &str) -> Option<SystemRisk> {
    match operation {
        "list" => Some(SystemRisk::Info),
        "focus" => Some(SystemRisk::Reversible),
        "kill" | "terminate" => Some(SystemRisk::Destructive),
        _ => None, // unknown operation = unsupported (fail-closed)
    }
}

/// Known window operations and their risk (§21).
pub fn window_risk(operation: &str) -> Option<SystemRisk> {
    match operation {
        "list" | "enum" => Some(SystemRisk::Info),
        "focus" | "minimize" | "restore" => Some(SystemRisk::Reversible),
        "close" => Some(SystemRisk::Destructive),
        _ => None,
    }
}

/// Build a validated process command. `None` for unknown operations
/// (fail-closed: the Windows Adapter batch will extend the taxonomy).
pub fn process_command(pid: u32, name: &str, operation: &str, origin: &str) -> Option<SystemCommand> {
    Some(SystemCommand {
        capability: SystemCapability::Process,
        operation: operation.into(),
        target: SystemTarget::Process {
            pid,
            name: name.into(),
        },
        risk: process_risk(operation)?,
        origin: origin.into(),
    })
}

/// Build a validated window command (hwnd supplied by the host's window
/// enumeration — adapters own raw handles).
pub fn window_command(hwnd: u64, title: &str, operation: &str, origin: &str) -> Option<SystemCommand> {
    Some(SystemCommand {
        capability: SystemCapability::Window,
        operation: operation.into(),
        target: SystemTarget::Window {
            hwnd,
            title: title.into(),
        },
        risk: window_risk(operation)?,
        origin: origin.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §6: destructive ops (kill/close) classify high; info ops low.
    #[test]
    fn risk_classification_matches_taxonomy() {
        assert_eq!(process_risk("list"), Some(SystemRisk::Info));
        assert_eq!(process_risk("kill"), Some(SystemRisk::Destructive));
        assert_eq!(window_risk("focus"), Some(SystemRisk::Reversible));
        assert_eq!(window_risk("close"), Some(SystemRisk::Destructive));
        // unknown operations are unsupported, never guessed
        assert_eq!(process_risk("format_c"), None);
        assert_eq!(window_risk("steal"), None);
    }

    /// Commands built here pass the frozen validation and carry origin.
    #[test]
    fn built_commands_validate() {
        let c = process_command(4242, "chrome.exe", "kill", "test").unwrap();
        assert!(c.validate().is_ok());
        assert_eq!(c.risk, SystemRisk::Destructive);
        let w = window_command(0x1234, "Doc - Notepad", "focus", "test").unwrap();
        assert!(w.validate().is_ok());
        assert_eq!(w.risk, SystemRisk::Reversible);
    }

    /// Unknown operation → builder returns None (no command ever built).
    #[test]
    fn unknown_operation_builds_nothing() {
        assert!(process_command(1, "x", "possess", "test").is_none());
        assert!(window_command(1, "x", "possess", "test").is_none());
    }
}
