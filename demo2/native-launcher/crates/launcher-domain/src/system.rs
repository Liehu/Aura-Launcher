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
    /// §34 origin propagation: carried into the audit trail.
    pub origin: String,
}

#[derive(Debug, Default)]
pub struct SystemResolver {
    /// Operations denied outright regardless of risk (policy v0.1).
    pub denied_operations: Vec<String>,
    /// §32: per-origin policy (origin allow-list + risk ceilings).
    pub policy: SystemPolicy,
}

/// §32 System Policy: per-origin rules evaluated BEFORE risk. Fail-closed —
/// an origin absent from `allowed_origins` (when non-empty) is denied; a
/// ceiling can only LOWER the max risk an origin may request.
#[derive(Debug, Default)]
pub struct SystemPolicy {
    pub denied_operations: Vec<String>,
    /// Origins permitted to resolve commands. Empty = all allowed.
    pub allowed_origins: Vec<String>,
    /// Max risk per origin (ceiling).
    pub origin_risk_ceiling: Vec<(String, SystemRisk)>,
}

impl SystemPolicy {
    fn origin_allowed(&self, origin: &str) -> bool {
        self.allowed_origins.is_empty()
            || self.allowed_origins.iter().any(|o| o == origin)
    }

    fn ceiling(&self, origin: &str) -> Option<SystemRisk> {
        self.origin_risk_ceiling
            .iter()
            .find(|(o, _)| o == origin)
            .map(|(_, r)| *r)
    }
}

impl SystemResolver {
    /// Resolve a validated command. Denies: invalid commands, operations on
    /// the deny list, disallowed origins, and requests above the origin's
    /// risk ceiling. Never executes anything (§2: Resolver ≠ Authority).
    pub fn resolve(&self, cmd: &SystemCommand) -> Result<Resolution, String> {
        cmd.validate()?;
        if self.denied_operations.iter().any(|op| op == &cmd.operation) {
            return Ok(Resolution {
                approved_by_policy: false,
                risk: cmd.risk,
                reason: format!("operation `{}` denied by policy", cmd.operation),
                origin: cmd.origin.clone(),
            });
        }
        // §32 origin propagation + policy evaluation
        if !self.policy.origin_allowed(&cmd.origin) {
            return Ok(Resolution {
                approved_by_policy: false,
                risk: cmd.risk,
                reason: format!("origin `{}` not allowed to resolve commands", cmd.origin),
                origin: cmd.origin.clone(),
            });
        }
        if let Some(ceiling) = self.policy.ceiling(&cmd.origin) {
            if cmd.risk > ceiling {
                return Ok(Resolution {
                    approved_by_policy: false,
                    risk: cmd.risk,
                    reason: format!(
                        "risk {:?} exceeds origin ceiling {:?}",
                        cmd.risk, ceiling
                    ),
                    origin: cmd.origin.clone(),
                });
            }
        }
        let reason = if cmd.risk.requires_confirmation() {
            "requires confirmation before effect".into()
        } else {
            "allowed".into()
        };
        Ok(Resolution {
            approved_by_policy: true,
            risk: cmd.risk,
            reason,
            origin: cmd.origin.clone(),
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

        let r = SystemResolver {
            denied_operations: vec!["format_disk".into()],
            policy: SystemPolicy::default(),
        };
        let denied = r.resolve(&cmd("format_disk", SystemRisk::Privileged)).unwrap();
        assert!(!denied.approved_by_policy);
        // invalid command rejected outright
        assert!(r.resolve(&cmd("Format-Disk", SystemRisk::Info)).is_err());
    }

    /// P2.9 Batch 5: origin allow-list + risk ceiling policy (§32).
    #[test]
    fn origin_policy_and_risk_ceiling() {
        let mut r = SystemResolver::default();
        r.policy.allowed_origins = vec!["search".into()];
        r.policy.origin_risk_ceiling = vec![("search".into(), SystemRisk::Reversible)];

        // disallowed origin denied outright, origin propagated
        let mut c = cmd("copy_path", SystemRisk::ReadOnly);
        c.origin = "plugin".into();
        let res = r.resolve(&c).unwrap();
        assert!(!res.approved_by_policy, "disallowed origin denied");
        assert_eq!(res.origin, "plugin");

        // allowed origin within ceiling -> allowed, origin carried
        let ok = r.resolve(&cmd("copy_path", SystemRisk::ReadOnly)).unwrap();
        assert!(ok.approved_by_policy);
        assert_eq!(ok.origin, "search");

        // above the origin's risk ceiling -> denied
        let mut heavy = cmd("delete_file", SystemRisk::Destructive);
        heavy.origin = "search".into();
        let res = r.resolve(&heavy).unwrap();
        assert!(!res.approved_by_policy, "risk above ceiling denied");
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
