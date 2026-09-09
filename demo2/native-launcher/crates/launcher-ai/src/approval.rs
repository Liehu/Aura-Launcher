//! AI Approval (P27-D01/D03/D04/D05, spec `P2.7 开发设计规范` §19-§21):
//! the approval gate between a validated proposal and the executor.
//!
//! Boundary (§21, frozen): an approval is run-scoped, step-scoped and
//! event-scoped — approving one plan authorizes exactly THAT plan, never
//! future actions (`Approve ≠ mark all future Actions authorized`).
//! Fail-closed (D03): unknown / already-decided / expired request ids are
//! rejected; every request is single-use; expiry (D05) invalidates the
//! pending request.
//! Edit (D04): a decision may replace the plan with a host-validated
//! `PlanDocument` (B02 structural rules); the edited plan re-enters the
//! normal execution path with no extra authority.

use crate::agent_contract::PlanStep;
use crate::plan::{PlanDocument, PlanDocumentError};

/// Default pending-request time-to-live (D05): 5 minutes.
pub const DEFAULT_TTL_MS: i64 = 5 * 60 * 1000;

/// One pending approval: the plan exactly as it will execute if approved.
#[derive(Debug, Clone, PartialEq)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub session_id: String,
    pub user_goal: String,
    pub steps: Vec<PlanStep>,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
}

/// A decision on one request. `ApproveEdited` carries the replacement plan
/// (D04) — validated here before it may take effect.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Approve,
    ApproveEdited(Box<PlanDocument>),
    Reject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApprovalDecision {
    pub request_id: String,
    pub decision: Decision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    /// Unknown request id (never issued, or already finalized).
    Unknown,
    /// The request expired before the decision arrived (D05).
    Expired,
    /// Single-use violated: the request was already decided/cancelled.
    AlreadyDecided,
    /// The edited plan failed B02 structural validation.
    InvalidPlan(PlanDocumentError),
}

impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown approval request"),
            Self::Expired => write!(f, "approval request expired"),
            Self::AlreadyDecided => write!(f, "approval request already decided"),
            Self::InvalidPlan(e) => write!(f, "edited plan invalid: {e}"),
        }
    }
}

#[derive(Debug, Clone)]
struct Pending {
    request: ApprovalRequest,
    decided: bool,
}

/// The approval gate: mints requests, tracks expiry, enforces single use.
/// Owned by one agent run (run-scoped, §21).
#[derive(Debug)]
pub struct ApprovalGate {
    ttl_ms: i64,
    seq: u64,
    pending: Option<Pending>,
}

impl Default for ApprovalGate {
    fn default() -> Self {
        Self::new(DEFAULT_TTL_MS)
    }
}

impl ApprovalGate {
    pub fn new(ttl_ms: i64) -> Self {
        Self { ttl_ms, seq: 0, pending: None }
    }

    /// Mint a request when ANY step requires approval (§19/§20). Returns
    /// `None` when the plan needs no approval (auto-execute class).
    pub fn request(
        &mut self,
        session_id: &str,
        user_goal: &str,
        steps: &[PlanStep],
        now_ms: i64,
    ) -> Option<ApprovalRequest> {
        if !steps.iter().any(|s| s.requires_approval) {
            return None;
        }
        // one pending request at a time (run-scoped): a new request
        // supersedes a stale pending one (the old id dies with it)
        self.seq += 1;
        let request = ApprovalRequest {
            request_id: format!("apr-{session_id}-{}", self.seq),
            session_id: session_id.to_string(),
            user_goal: user_goal.to_string(),
            steps: steps.to_vec(),
            created_at_ms: now_ms,
            expires_at_ms: now_ms + self.ttl_ms,
        };
        self.pending = Some(Pending { request: request.clone(), decided: false });
        Some(request)
    }

    /// Apply a decision (fail-closed, single-use). Returns the effective
    /// plan to execute: the original steps on `Approve`, the validated
    /// replacement on `ApproveEdited`. `Reject`/`Expired` never execute.
    pub fn decide(
        &mut self,
        request_id: &str,
        decision: Decision,
        now_ms: i64,
    ) -> Result<Vec<PlanStep>, ApprovalError> {
        let Some(pending) = self.pending.as_mut() else {
            return Err(ApprovalError::Unknown);
        };
        if pending.request.request_id != request_id {
            return Err(ApprovalError::Unknown);
        }
        if pending.decided {
            return Err(ApprovalError::AlreadyDecided);
        }
        if now_ms > pending.request.expires_at_ms {
            pending.decided = true; // expired requests are dead, not reusable
            return Err(ApprovalError::Expired);
        }
        pending.decided = true;
        let steps = match decision {
            Decision::Approve => pending.request.steps.clone(),
            Decision::ApproveEdited(doc) => {
                doc.validate().map_err(ApprovalError::InvalidPlan)?;
                doc.steps.clone()
            }
            Decision::Reject => return Ok(Vec::new()),
        };
        Ok(steps)
    }

    /// D05: explicit cancellation without a decision (host timeout/Esc).
    /// The request becomes unconditionally unanswerable.
    pub fn cancel(&mut self, request_id: &str) -> Result<(), ApprovalError> {
        match self.pending.take() {
            Some(p) if p.request.request_id == request_id && !p.decided => Ok(()),
            _ => Err(ApprovalError::Unknown),
        }
    }

    /// Whether a live (unexpired, undecided) request exists.
    pub fn pending_request(&self, now_ms: i64) -> Option<&ApprovalRequest> {
        self.pending
            .as_ref()
            .filter(|p| !p.decided && now_ms <= p.request.expires_at_ms)
            .map(|p| &p.request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(id: &str, requires: bool) -> PlanStep {
        PlanStep {
            step_id: id.into(),
            action_ref: "p|c|a".into(),
            input: json!({}),
            rationale: None,
            requires_approval: requires,
        }
    }

    fn doc_from(steps: Vec<PlanStep>) -> PlanDocument {
        PlanDocument {
            schema_version: crate::plan::PLAN_SCHEMA_VERSION,
            proposal_id: "p1".into(),
            session_id: "s1".into(),
            user_goal: "g".into(),
            steps,
        }
    }

    /// D01: approval is required exactly when a step asks for it.
    #[test]
    fn request_only_when_required() {
        let mut gate = ApprovalGate::default();
        assert!(gate.request("s1", "g", &[step("a", false)], 0).is_none());
        let req = gate.request("s1", "g", &[step("a", true)], 0).unwrap();
        assert_eq!(req.request_id, "apr-s1-1");
        assert_eq!(gate.pending_request(0).map(|r| r.request_id.clone()), Some(req.request_id.clone()));
    }

    /// D03: single use; replay and unknown ids fail closed.
    #[test]
    fn decisions_are_single_use() {
        let mut gate = ApprovalGate::default();
        let req = gate.request("s1", "g", &[step("a", true)], 0).unwrap();
        let steps = gate.decide(&req.request_id, Decision::Approve, 1).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(
            gate.decide(&req.request_id, Decision::Approve, 2),
            Err(ApprovalError::AlreadyDecided)
        );
        assert_eq!(gate.decide("apr-nope", Decision::Approve, 2), Err(ApprovalError::Unknown));
    }

    /// D05: expiry and cancellation invalidate the pending request.
    #[test]
    fn expiry_and_cancel_fail_closed() {
        let mut gate = ApprovalGate::default();
        let req = gate.request("s1", "g", &[step("a", true)], 0).unwrap();
        assert!(gate.pending_request(1).is_some());
        assert!(gate.pending_request(req.expires_at_ms + 1).is_none());
        assert_eq!(
            gate.decide(&req.request_id, Decision::Approve, req.expires_at_ms + 1),
            Err(ApprovalError::Expired)
        );
        // cancelled requests cannot be answered either
        let req2 = gate.request("s1", "g", &[step("b", true)], 0).unwrap();
        gate.cancel(&req2.request_id).unwrap();
        assert_eq!(
            gate.decide(&req2.request_id, Decision::Approve, 1),
            Err(ApprovalError::Unknown)
        );
    }

    /// D04: an edited plan replaces the executed steps after validation;
    /// structurally invalid edits are refused.
    #[test]
    fn edited_plan_is_validated_then_used() {
        let mut gate = ApprovalGate::default();
        let req = gate.request("s1", "g", &[step("a", true)], 0).unwrap();
        let mut edited = doc_from(vec![step("b", false), step("c", false)]);
        edited.session_id = "s1".into();
        let steps = gate
            .decide(&req.request_id, Decision::ApproveEdited(Box::new(edited)), 1)
            .unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].step_id, "b");

        let req2 = gate.request("s1", "g", &[step("d", true)], 0).unwrap();
        let mut bad = doc_from(vec![]);
        bad.session_id = "s1".into();
        assert!(matches!(
            gate.decide(&req2.request_id, Decision::ApproveEdited(Box::new(bad)), 1),
            Err(ApprovalError::InvalidPlan(_))
        ));
    }

    /// D05: reject yields an empty (never-executed) plan.
    #[test]
    fn reject_never_executes() {
        let mut gate = ApprovalGate::default();
        let req = gate.request("s1", "g", &[step("a", true)], 0).unwrap();
        let steps = gate.decide(&req.request_id, Decision::Reject, 1).unwrap();
        assert!(steps.is_empty());
    }
}
