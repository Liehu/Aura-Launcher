//! AI Contract v1 (P27-001, spec `P2.7 开发设计规范` §7/§8/§9/§17/§19).
//!
//! Everything here is PROPOSAL DATA. The invariant of the whole phase:
//! AI can understand/plan/propose but NEVER holds execution authority —
//! `action_ref` is a LOGICAL reference resolved later by the existing
//! ReferenceResolver → ActionResolver chain; a proposal can never become
//! an execution without user approval (L3/L4) and Resolver validation.

use serde::{Deserialize, Serialize};

/// §9 AgentIntent: what the user is trying to accomplish (deterministic
/// classification; the LLM only fills it in, policy never derives from it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentIntent {
    Search,
    Open,
    Execute,
    Workflow,
    Explain,
    Unknown,
}

/// §19 risk levels (L0–L4) with the DEFAULT approval policy per level.
/// The actual gate stays ActionResolver/Policy — risk level is an AI-side
/// hint that can only make approval MORE likely, never less.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    L0Informational,
    L1ReadOnly,
    L2Reversible,
    L3Destructive,
    L4Sensitive,
}

impl RiskLevel {
    /// §19 table: L0/L1 auto, L2 configurable, L3 approval, L4 forced.
    pub fn requires_approval(&self) -> bool {
        matches!(self, RiskLevel::L3Destructive | RiskLevel::L4Sensitive)
    }
}

/// §8 plan step: a LOGICAL action reference (never a ResolvedAction),
/// optional rationale, and the AI-side approval flag (belt to the risk
/// classification, which the Resolver still overrides).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_id: String,
    /// Logical reference, e.g. `command:app:chrome` / `action:copypath`.
    pub action_ref: String,
    /// JSON payload for the step input.
    pub input: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default)]
    pub requires_approval: bool,
}

/// §7 proposal: a full plan for one user goal. DATA, never authorization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentProposal {
    pub proposal_id: String,
    pub session_id: String,
    pub user_goal: String,
    pub intent: AgentIntent,
    pub plan: Vec<PlanStep>,
    /// §17 model confidence in [0,1].
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

/// Contract validation (P27-001, deterministic, fail-closed):
/// non-empty plan, unique step ids, bounded ids/goal, confidence in [0,1],
/// action_ref must be a logical reference (never empty, no path separators
/// masquerading as ids).
pub fn validate_proposal(p: &AgentProposal) -> Result<(), String> {
    if p.proposal_id.is_empty() || p.proposal_id.len() > 128 {
        return Err("invalid proposal_id".into());
    }
    if p.session_id.is_empty() || p.session_id.len() > 128 {
        return Err("invalid session_id".into());
    }
    if p.user_goal.trim().is_empty() || p.user_goal.len() > 4096 {
        return Err("invalid user_goal".into());
    }
    if !(0.0..=1.0).contains(&p.confidence) {
        return Err("confidence must be in [0,1]".into());
    }
    if p.plan.is_empty() {
        return Err("plan must have at least one step".into());
    }
    let mut seen = std::collections::HashSet::new();
    for step in &p.plan {
        if step.step_id.is_empty()
            || step.step_id.len() > 128
            || !seen.insert(step.step_id.clone())
        {
            return Err(format!("invalid or duplicate step_id: {}", step.step_id));
        }
        if step.action_ref.is_empty()
            || step.action_ref.contains('/')
            || step.action_ref.contains('\\')
        {
            return Err(format!("invalid action_ref: {}", step.action_ref));
        }
        if step.input.as_str().map(|s| s.len()).unwrap_or(0) > 64 * 1024 {
            return Err(format!("step {} input too large", step.step_id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal() -> AgentProposal {
        AgentProposal {
            proposal_id: "p1".into(),
            session_id: "s1".into(),
            user_goal: "find my quarterly report".into(),
            intent: AgentIntent::Search,
            plan: vec![PlanStep {
                step_id: "s1".into(),
                action_ref: "command:file:quarterly".into(),
                input: serde_json::json!({"query": "quarterly report"}),
                rationale: Some("user asked for the report".into()),
                requires_approval: false,
            }],
            confidence: 0.9,
            explanation: None,
        }
    }

    /// Valid proposal passes and roundtrips stably (frozen contract).
    #[test]
    fn valid_proposal_roundtrips() {
        let p = proposal();
        validate_proposal(&p).unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let back: AgentProposal = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    /// Fail-closed validation: empty plan, duplicate step ids, confidence
    /// out of range, invalid action_ref.
    #[test]
    fn validation_fails_closed() {
        let mut p = proposal();
        p.plan.clear();
        assert!(validate_proposal(&p).is_err());
        let mut p = proposal();
        let dup = p.plan[0].clone();
        p.plan.push(dup);
        assert!(validate_proposal(&p).is_err());
        p.confidence = 1.5;
        assert!(validate_proposal(&p).is_err());
        p.confidence = 0.5;
        p.plan[0].action_ref = "C:\\evil\\tool.exe".into();
        assert!(validate_proposal(&p).is_err(), "path-like ref rejected");
    }

    /// §19: risk default policy — L0/L1 auto, L3/L4 forced approval.
    #[test]
    fn risk_levels_default_policy() {
        assert!(!RiskLevel::L0Informational.requires_approval());
        assert!(!RiskLevel::L1ReadOnly.requires_approval());
        assert!(RiskLevel::L3Destructive.requires_approval());
        assert!(RiskLevel::L4Sensitive.requires_approval());
        // ordering L0 < L1 < L2 < L3 < L4
        assert!(RiskLevel::L0Informational < RiskLevel::L4Sensitive);
    }

    /// The phase invariant, structurally pinned: the proposal JSON never
    /// contains capability/authority/resolved vocabulary.
    #[test]
    fn proposal_is_authority_free() {
        let json = serde_json::to_string(&proposal()).unwrap();
        assert!(!json.contains("capabilit"));
        assert!(!json.contains("authorit"));
        assert!(!json.contains("resolved"));
        assert!(!json.contains("ResolvedAction"));
    }
}
