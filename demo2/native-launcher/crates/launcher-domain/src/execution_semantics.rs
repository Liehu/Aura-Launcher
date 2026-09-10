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

/// §20 retry rule (pure, deterministic). P210-D07: the decision takes BOTH
/// the command result AND the effect certainty — `Cancelled` alone is not
/// "safe to retry" (a cancel during execution may still have landed an
/// effect):
/// - `effect_state == Completed` → Forbidden (duplicate effect);
/// - `effect_state == Started` → Forbidden (in-flight, not settled);
/// - `effect_state == Unknown` (with result Timeout/Unknown/Cancelled) →
///   idempotency-gated;
/// - otherwise (effect NotStarted/Failed) → retry allowed; a `Succeeded`
///   result is forbidden regardless.
pub fn retry_decision(
    result: CommandResult,
    effect_state: EffectState,
    idempotent: bool,
) -> RetryDecision {
    match effect_state {
        EffectState::Completed | EffectState::Started => RetryDecision::Forbidden,
        EffectState::Unknown => {
            if idempotent {
                RetryDecision::Allowed
            } else {
                RetryDecision::Forbidden
            }
        }
        EffectState::NotStarted | EffectState::Failed => {
            if result == CommandResult::Succeeded {
                RetryDecision::Forbidden
            } else {
                RetryDecision::Allowed
            }
        }
    }
}

/// P210-D08 (spec review §8): the recovery route for an unsettled effect.
/// An effect whose outcome is Unknown must go through explicit recovery —
/// never an automatic blind re-run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryRoute {
    /// The effect can be queried (probe API) to determine the actual result.
    Queryable,
    /// Idempotent effect: policy MAY retry (new execution id required).
    PolicyRetry,
    /// Non-idempotent and unqueryable: manual / explicit recovery only —
    /// the runtime MUST NOT re-execute.
    ExplicitRecoveryRequired,
}

/// §8 recovery routing (pure): where does an Unknown-effect step go?
pub fn recovery_route(idempotent: bool, queryable: bool) -> RecoveryRoute {
    if queryable {
        RecoveryRoute::Queryable
    } else if idempotent {
        RecoveryRoute::PolicyRetry
    } else {
        RecoveryRoute::ExplicitRecoveryRequired
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

    /// §20: the retry matrix — result AND effect state together
    /// (P210-D07: Cancelled+Unknown is NOT a safe retry).
    #[test]
    fn retry_matrix() {
        use CommandResult::*;
        use EffectState::*;
        #[allow(unused_imports)]
        use EffectState::Unknown as EUnknown;
        #[allow(unused_imports)]
        use CommandResult::Unknown as RUnknown;
        // settled success: forbidden regardless
        assert_eq!(retry_decision(Succeeded, EffectState::Completed, true), RetryDecision::Forbidden);
        assert_eq!(retry_decision(Succeeded, EffectState::NotStarted, false), RetryDecision::Forbidden);
        // unknown effect: idempotency-gated (incl. Cancelled+Unknown!)
        assert_eq!(retry_decision(RUnknown, EUnknown, false), RetryDecision::Forbidden);
        assert_eq!(retry_decision(Timeout, EUnknown, false), RetryDecision::Forbidden);
        assert_eq!(retry_decision(Cancelled, EUnknown, false), RetryDecision::Forbidden);
        assert_eq!(retry_decision(RUnknown, EUnknown, true), RetryDecision::Allowed);
        assert_eq!(retry_decision(Timeout, EUnknown, true), RetryDecision::Allowed);
        assert_eq!(retry_decision(Cancelled, EUnknown, true), RetryDecision::Allowed);
        // in-flight: never re-run
        assert_eq!(retry_decision(CommandResult::Failed, EffectState::Started, true), RetryDecision::Forbidden);
        // effect did not land: allowed
        assert_eq!(retry_decision(CommandResult::Failed, EffectState::Failed, false), RetryDecision::Allowed);
        assert_eq!(retry_decision(Cancelled, EffectState::NotStarted, false), RetryDecision::Allowed);
        assert_eq!(retry_decision(Rejected, EffectState::NotStarted, false), RetryDecision::Allowed);
    }

    /// P210-D08: recovery routing for unsettled effects.
    #[test]
    fn recovery_routing() {
        assert_eq!(recovery_route(false, true), RecoveryRoute::Queryable);
        assert_eq!(recovery_route(true, false), RecoveryRoute::PolicyRetry);
        assert_eq!(
            recovery_route(false, false),
            RecoveryRoute::ExplicitRecoveryRequired
        );
        // queryable wins even when idempotent (determine before re-run)
        assert_eq!(recovery_route(true, true), RecoveryRoute::Queryable);
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
