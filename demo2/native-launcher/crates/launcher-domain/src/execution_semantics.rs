//! Execution Semantics v1 (P210-002/P210-005, spec
//! `P2.10 — Cross-System Consistency & Security Hardening 1.0` §15/§17-§20):
//! the frozen cross-phase vocabulary for command outcomes, effect
//! certainty, per-step records and retry decisions.
//!
//! The core rules (§19/§20):
//! - Timeout ≠ Failed: a started effect whose caller times out yields
//!   `CommandResult::Timeout` with `EffectState::Unknown`;
//! - `Unknown` + non-idempotent → no automatic retry;
//! - `Succeeded` → immutable success, never re-executed (replan safety);
//! - every retry carries a new execution id.

use serde::{Deserialize, Serialize};

/// §18: the unified command outcome across every phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandResult {
    Succeeded,
    Failed,
    Rejected,
    Cancelled,
    Timeout,
    Unknown,
}

/// §18: what is actually known about the OS-level effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectState {
    NotStarted,
    Started,
    Completed,
    Failed,
    Unknown,
}

/// §15: per-step execution status (replan safety vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Executing,
    Succeeded,
    Failed,
    Cancelled,
    Unknown,
    Skipped,
}

/// §14: one step's execution record — immutable success bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepExecutionRecord {
    pub step_id: String,
    pub execution_id: String,
    pub attempt: u32,
    pub status: StepStatus,
    pub effect_state: EffectState,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
}

/// §20: the retry decision derived from outcome + idempotency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    /// Retry permitted (new execution id REQUIRED).
    Allowed,
    /// Retry forbidden: duplicates an effect that already happened, or
    /// risks one whose outcome is unknown and cannot be made idempotent.
    Forbidden,
}

/// §19: the outcome/effect pair for a timed-out call. A started effect
/// whose caller gives up is NEVER recorded as Failed.
pub fn timeout_outcome() -> (CommandResult, EffectState) {
    (CommandResult::Timeout, EffectState::Unknown)
}

/// §20 retry rule (pure, deterministic):
/// - `Succeeded` → forbidden (no duplicate effect);
/// - `Unknown` (includes Timeout) → forbidden unless the step is declared
///   idempotent;
/// - `Failed`/`Cancelled`/`Rejected` → allowed (effect did not land);
/// - `Succeeded` is the only immutable-success state.
pub fn retry_decision(result: CommandResult, idempotent: bool) -> RetryDecision {
    match result {
        CommandResult::Succeeded => RetryDecision::Forbidden,
        CommandResult::Unknown | CommandResult::Timeout => {
            if idempotent {
                RetryDecision::Allowed
            } else {
                RetryDecision::Forbidden
            }
        }
        CommandResult::Failed
        | CommandResult::Rejected
        | CommandResult::Cancelled => RetryDecision::Allowed,
    }
}

/// §15 replan rule: which recorded steps may a replacement plan re-execute.
/// `Succeeded` steps are immutable; `Executing`/`Unknown` steps must NOT be
/// blindly re-run (their effect state is not settled) — callers promote
/// them via explicit recovery first.
pub fn replan_may_execute(status: StepStatus) -> bool {
    matches!(status, StepStatus::Pending | StepStatus::Failed | StepStatus::Skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §19: timeout maps to Unknown effect state, never Failed.
    #[test]
    fn timeout_is_not_failure() {
        let (result, effect) = timeout_outcome();
        assert_eq!(result, CommandResult::Timeout);
        assert_eq!(effect, EffectState::Unknown);
        assert_ne!(result, CommandResult::Failed);
    }

    /// §20: the retry matrix.
    #[test]
    fn retry_matrix() {
        assert_eq!(retry_decision(CommandResult::Succeeded, true), RetryDecision::Forbidden);
        assert_eq!(retry_decision(CommandResult::Succeeded, false), RetryDecision::Forbidden);
        assert_eq!(retry_decision(CommandResult::Unknown, false), RetryDecision::Forbidden);
        assert_eq!(retry_decision(CommandResult::Timeout, false), RetryDecision::Forbidden);
        assert_eq!(retry_decision(CommandResult::Unknown, true), RetryDecision::Allowed);
        assert_eq!(retry_decision(CommandResult::Timeout, true), RetryDecision::Allowed);
        assert_eq!(retry_decision(CommandResult::Failed, false), RetryDecision::Allowed);
        assert_eq!(retry_decision(CommandResult::Cancelled, false), RetryDecision::Allowed);
        assert_eq!(retry_decision(CommandResult::Rejected, false), RetryDecision::Allowed);
    }

    /// §15: replan must not re-execute succeeded (or unsettled) steps.
    #[test]
    fn replan_skips_immutable_success_and_unsettled() {
        assert!(replan_may_execute(StepStatus::Pending));
        assert!(replan_may_execute(StepStatus::Failed));
        assert!(replan_may_execute(StepStatus::Skipped));
        assert!(!replan_may_execute(StepStatus::Succeeded));
        assert!(!replan_may_execute(StepStatus::Executing));
        assert!(!replan_may_execute(StepStatus::Unknown));
        assert!(!replan_may_execute(StepStatus::Cancelled));
    }

    /// §14: records serialize (durable step bookkeeping contract).
    #[test]
    fn records_serialize() {
        let r = StepExecutionRecord {
            step_id: "a".into(),
            execution_id: "exec-1".into(),
            attempt: 2,
            status: StepStatus::Succeeded,
            effect_state: EffectState::Completed,
            started_at_ms: 1,
            finished_at_ms: Some(2),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: StepExecutionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }
}
