//! System Adapter: File/Clipboard (P2.9 Batch 2, spec
//! `P2.9 — System Integration & Automation 1.0 技术设计规范.md` §12-§19).
//!
//! Consolidates existing host-owned file/clipboard effects behind the
//! SystemCommand resolution contract (Batch 1). The adapter maps a VALIDATED
//! `SystemCommand` to an existing `Action` — the mapping is DATA-ONLY and the
//! returned Action still goes through launcher-action's validate → Policy →
//! Effect chain. No new execution path is introduced: unsupported operations
//! resolve to `None` (fail-closed).

use crate::system::{SystemCommand, SystemCapability, SystemTarget};
use crate::{Action, ActionKind, ActionPayload};

/// Map a resolved file/clipboard command to its host-owned Action.
/// Returns `None` when the operation is not (yet) an adapter operation —
/// callers treat `None` as "unsupported, deny" (fail-closed).
pub fn to_action(cmd: &SystemCommand) -> Option<Action> {
    if cmd.validate().is_err() {
        return None;
    }
    let path = match &cmd.target {
        SystemTarget::File { path } => path.clone(),
        _ => return None,
    };
    let kind = match (cmd.capability, cmd.operation.as_str()) {
        (SystemCapability::File, "open") => ActionKind::Open,
        (SystemCapability::File, "copy_path") => ActionKind::Copy,
        (SystemCapability::File, "reveal") => ActionKind::Reveal,
        (SystemCapability::File, "open_terminal") => ActionKind::OpenTerminalHere,
        _ => return None,
    };
    Some(Action {
        kind,
        payload: Some(ActionPayload::Path(path)),
        id: Some(format!("system.{}", cmd.operation)),
        title: None,
        disabled_reason: None,
        shortcut: None,
        confirmation_required: cmd.risk.requires_confirmation(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::SystemRisk;

    fn file_cmd(operation: &str, risk: SystemRisk) -> SystemCommand {
        SystemCommand {
            capability: SystemCapability::File,
            operation: operation.into(),
            target: SystemTarget::File {
                path: r"C:\a\report.txt".into(),
            },
            risk,
            origin: "search".into(),
        }
    }

    /// Batch 2 core: file operations map to existing host-owned actions.
    #[test]
    fn file_operations_map_to_actions() {
        let open = to_action(&file_cmd("open", SystemRisk::ReadOnly)).unwrap();
        assert!(matches!(open.kind, ActionKind::Open));
        assert!(matches!(
            open.payload,
            Some(ActionPayload::Path(ref p)) if p == r"C:\a\report.txt"
        ));

        let reveal = to_action(&file_cmd("reveal", SystemRisk::ReadOnly)).unwrap();
        assert!(matches!(reveal.kind, ActionKind::Reveal));

        let term = to_action(&file_cmd("open_terminal", SystemRisk::ReadOnly)).unwrap();
        assert!(matches!(term.kind, ActionKind::OpenTerminalHere));
    }

    /// Destructive file ops carry the confirmation flag (§35).
    #[test]
    fn destructive_ops_require_confirmation() {
        let del = to_action(&file_cmd("delete", SystemRisk::Destructive));
        // v0.1 adapter has no delete mapping (destructive ops land with the
        // File Transaction batch) — fail-closed None for now
        assert!(del.is_none(), "unmapped destructive op must not execute");
        // but a mapped destructive op would carry confirmation_required
        let copy = to_action(&file_cmd("copy_path", SystemRisk::Destructive)).unwrap();
        assert!(copy.confirmation_required);
    }

    /// Fail-closed: non-file targets and unknown operations → None.
    #[test]
    fn unsupported_resolves_to_none() {
        let mut uri = file_cmd("open", SystemRisk::ReadOnly);
        uri.capability = SystemCapability::Uri;
        uri.target = SystemTarget::Uri {
            scheme: "https".into(),
        };
        assert!(to_action(&uri).is_none());

        assert!(to_action(&file_cmd("format_disk", SystemRisk::Privileged)).is_none());

        let mut invalid = file_cmd("open", SystemRisk::ReadOnly);
        invalid.operation = "Open".into(); // not identifier-shaped
        assert!(to_action(&invalid).is_none(), "invalid command → None");
    }
}
