//! System Integration contract base (P2.9 Batch 1, spec
//! `P2.9 — System Integration & Automation 1.0 技术设计规范.md` §4-§8/§31).
//!
//! Frozen v0.1 DTOs for the unified system capability layer. Boundaries
//! (§2, all five frozen "!="): a SystemCapability is NOT a capability grant;
//! the SystemResolver is NOT an effect authority — every effect still goes
//! through validate → Policy → Approval → launcher-action. Pure types:
//! zero OS calls (Native API stays inside Windows Adapters, later batches).

use serde::{Deserialize, Serialize};

/// §4/§5 capability taxonomy (v0.1 subset — remaining adapters add kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemCapability {
    Process,
    Window,
    File,
    Clipboard,
    Shell,
    Uri,
    Notification,
    Power,
    Settings,
}

/// §6 risk classes, aligned with P2.7 RiskLevel semantics: higher = more
/// confirmation. Data only — the actual gate stays Policy/launcher-action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemRisk {
    Info,
    ReadOnly,
    Reversible,
    Destructive,
    Privileged,
}

impl SystemRisk {
    /// Default confirmation requirement (§35): destructive/privileged need
    /// explicit confirmation; the Confirmation Agent owns the flow.
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, SystemRisk::Destructive | SystemRisk::Privileged)
    }
}

/// §8 SystemTarget: WHAT the command acts on, as a typed identity — never a
/// raw command string. Path targets are normalized upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SystemTarget {
    Process { pid: u32, name: String },
    Window { hwnd: u64, title: String },
    File { path: String },
    Uri { scheme: String },
    System { setting: String },
}

/// §7 SystemCommand: intent + target + payload — a RESOLUTION INPUT, not an
/// Effect. Execution is only legal via launcher-action after Policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemCommand {
    pub capability: SystemCapability,
    pub operation: String,
    pub target: SystemTarget,
    pub risk: SystemRisk,
    /// Origin for audit (§34): who asked for this — search/workflow/ai/plugin.
    pub origin: String,
}

impl SystemCommand {
    /// Deterministic, fail-closed validation: operation bounded and
    /// identifier-shaped, origin required, path targets non-empty.
    pub fn validate(&self) -> Result<(), String> {
        if self.operation.is_empty() || self.operation.len() > 64 {
            return Err("invalid operation".into());
        }
        if !self
            .operation
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
        {
            return Err(format!("invalid operation `{}`", self.operation));
        }
        if self.origin.is_empty() || self.origin.len() > 64 {
            return Err("invalid origin".into());
        }
        if let SystemTarget::File { path } = &self.target {
            if path.trim().is_empty() {
                return Err("empty file target".into());
            }
        }
        Ok(())
    }
}

/// §31 SystemResolver skeleton: resolves validated commands into a decision
/// record for Policy/launcher-action. v0.1 has NO adapters yet — the mock
/// resolution records what WOULD execute; adapters land in later batches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub approved_by_policy: bool,
    pub risk: SystemRisk,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct SystemResolver {
    /// Operations denied outright regardless of risk (policy v0.1).
    pub denied_operations: Vec<String>,
}

impl SystemResolver {
    /// Resolve a validated command. Denies: invalid commands, denied
    /// operations. Never executes anything.
    pub fn resolve(&self, cmd: &SystemCommand) -> Result<Resolution, String> {
        cmd.validate()?;
        if self.denied_operations.iter().any(|op| op == &cmd.operation) {
            return Ok(Resolution {
                approved_by_policy: false,
                risk: cmd.risk,
                reason: format!("operation `{}` denied by policy", cmd.operation),
            });
        }
        Ok(Resolution {
            approved_by_policy: true,
            risk: cmd.risk,
            reason: if cmd.risk.requires_confirmation() {
                "requires confirmation before effect".into()
            } else {
                "allowed".into()
            },
        })
    }
}

/// Mock provider for tests/diagnostics (§30): records resolved commands
/// without any OS interaction.
#[derive(Debug, Default)]
pub struct MockSystemProvider {
    pub seen: Vec<SystemCommand>,
}

impl MockSystemProvider {
    pub fn observe(&mut self, cmd: &SystemCommand) {
        self.seen.push(cmd.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(op: &str, risk: SystemRisk) -> SystemCommand {
        SystemCommand {
            capability: SystemCapability::File,
            operation: op.into(),
            target: SystemTarget::File {
                path: r"C:\a\report.txt".into(),
            },
            risk,
            origin: "search".into(),
        }
    }

    /// §2 boundaries: command validates and resolves, but resolution is a
    /// decision record — never an execution.
    #[test]
    fn resolve_allowed_and_denied() {
        let r = SystemResolver::default();
        let ok = r.resolve(&cmd("copy_path", SystemRisk::ReadOnly)).unwrap();
        assert!(ok.approved_by_policy);
        assert!(!ok.risk.requires_confirmation());

        let mut r = SystemResolver {
            denied_operations: vec!["format_disk".into()],
        };
        let denied = r.resolve(&cmd("format_disk", SystemRisk::Privileged)).unwrap();
        assert!(!denied.approved_by_policy);
        // invalid command rejected outright
        assert!(r.resolve(&cmd("Format-Disk", SystemRisk::Info)).is_err());
    }

    /// §6/§35: destructive/privileged require confirmation.
    #[test]
    fn destructive_requires_confirmation() {
        let r = SystemResolver::default();
        let res = r.resolve(&cmd("delete_file", SystemRisk::Destructive)).unwrap();
        assert!(res.risk.requires_confirmation());
        assert!(res.reason.contains("confirmation"));
    }

    /// Risk/order/DTO roundtrip + origin recorded for audit (§34).
    #[test]
    fn command_roundtrips_with_origin() {
        let c = cmd("copy_path", SystemRisk::ReadOnly);
        let json = serde_json::to_string(&c).unwrap();
        let back: SystemCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.origin, "search");

        let mut mock = MockSystemProvider::default();
        mock.observe(&back);
        assert_eq!(mock.seen.len(), 1);
        assert!(mock.seen[0].validate().is_ok());
    }
}
