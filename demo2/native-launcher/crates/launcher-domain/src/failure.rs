//! C1 Failure Taxonomy (P2.3-C, review 87 §2): a unified failure
//! classification that maps every crate-specific error into a small,
//! actionable set. Each variant determines the recovery policy.

use serde::{Deserialize, Serialize};

/// Unified failure classification for all Launcher subsystems.
/// Every error must map to exactly one variant — no `Other`/`Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Input validation failed (bad path, malformed JSON, missing field).
    InvalidInput,
    /// Capability was requested but not granted by host policy.
    CapabilityDenied,
    /// User must confirm before the action proceeds.
    ConfirmationRequired,
    /// Referenced command/action/tool does not exist.
    NotFound,
    /// The providing subsystem (plugin/MCP/provider) is not reachable.
    ProviderUnavailable,
    /// A timeout occurred while waiting for a response.
    Timeout,
    /// The external process crashed or exited unexpectedly.
    ProcessCrashed,
    /// The external process violated its communication protocol.
    ProtocolViolation,
    /// A business-level error from the plugin/MCP tool (not a bug).
    BusinessError,
    /// A persistence layer (SQLite/file) failed.
    PersistenceFailure,
    /// Configuration was missing, corrupt, or invalid.
    ConfigurationFailure,
    /// Internal invariant was violated (bug — should never happen).
    Internal,
}

impl FailureClass {
    /// C1 policy: what the system should do when this failure occurs.
    /// Deterministic — no randomness, no hidden state.
    pub fn recovery_policy(&self) -> RecoveryPolicy {
        match self {
            Self::InvalidInput => RecoveryPolicy::Stop,
            Self::CapabilityDenied => RecoveryPolicy::Stop,
            Self::ConfirmationRequired => RecoveryPolicy::Pause,
            Self::NotFound => RecoveryPolicy::Stop,
            Self::ProviderUnavailable => RecoveryPolicy::Report,
            Self::Timeout => RecoveryPolicy::RetryBounded,
            Self::ProcessCrashed => RecoveryPolicy::Restart,
            Self::ProtocolViolation => RecoveryPolicy::Quarantine,
            Self::BusinessError => RecoveryPolicy::Report,
            Self::PersistenceFailure => RecoveryPolicy::Repair,
            Self::ConfigurationFailure => RecoveryPolicy::Repair,
            Self::Internal => RecoveryPolicy::Stop,
        }
    }
}

/// What the system should do after classification (C1 policy layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPolicy {
    /// Stop the current operation; do not retry.
    Stop,
    /// Pause and wait for user confirmation.
    Pause,
    /// Report to diagnostics; continue operation.
    Report,
    /// Retry with bounded backoff; do not loop forever.
    RetryBounded,
    /// Restart the failed component (process/session).
    Restart,
    /// Quarantine the failed component (block new invocations).
    Quarantine,
    /// Repair the damaged data (delete/rebuild derived state).
    Repair,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C1: every failure class has a well-defined recovery policy.
    #[test]
    fn all_classes_have_policies() {
        let all = [
            FailureClass::InvalidInput,
            FailureClass::CapabilityDenied,
            FailureClass::ConfirmationRequired,
            FailureClass::NotFound,
            FailureClass::ProviderUnavailable,
            FailureClass::Timeout,
            FailureClass::ProcessCrashed,
            FailureClass::ProtocolViolation,
            FailureClass::BusinessError,
            FailureClass::PersistenceFailure,
            FailureClass::ConfigurationFailure,
            FailureClass::Internal,
        ];
        for fc in &all {
            let _ = fc.recovery_policy(); // must not panic
        }
    }
}
