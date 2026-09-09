//! Action Proposal Builder (P27-B04, spec `P2.7 开发设计规范` §7): bridges
//! deterministic intent parsing (A03) and the frozen proposal contract
//! (P27-001) — builds a validated `AgentProposal` from parsed user input.
//!
//! This is the ONLY sanctioned path from natural language to a proposal:
//! parse → validate → build → contract-validate. The builder never creates
//! execution authority; the output is DATA for the Resolver/Policy chain.

use crate::agent_contract::{validate_proposal, AgentIntent, AgentProposal, PlanStep};
use crate::intent::ParsedRequest;
use serde_json::json;

/// Build an `AgentProposal` from a parsed request. The proposal_id is
/// derived from a session id + a monotonic counter the caller maintains.
pub fn build_proposal(
    session_id: &str,
    proposal_seq: u64,
    parsed: &ParsedRequest,
    confidence: f32,
) -> Result<AgentProposal, String> {
    let proposal_id = format!("{session_id}-p{proposal_seq}");
    let action_ref = action_ref_for(parsed);
    let step_id = format!("{proposal_id}-s1");
    let input = input_for(parsed);
    let proposal = AgentProposal {
        proposal_id,
        session_id: session_id.to_string(),
        user_goal: user_goal(parsed),
        intent: parsed.intent,
        plan: vec![PlanStep {
            step_id,
            action_ref: action_ref.clone(),
            input,
            rationale: Some(rationale_for(parsed)),
            requires_approval: requires_approval_for(&action_ref),
        }],
        confidence: confidence.clamp(0.0, 1.0),
        explanation: Some(explanation_for(parsed)),
    };
    validate_proposal(&proposal)?;
    Ok(proposal)
}

fn user_goal(parsed: &ParsedRequest) -> String {
    if let Some(q) = &parsed.entities.query {
        return format!("{}: {q}", intent_label(parsed.intent));
    }
    if let Some(t) = &parsed.entities.target {
        return format!("{}: {t}", intent_label(parsed.intent));
    }
    if let Some(w) = &parsed.entities.workflow_id {
        return format!("run workflow: {w}");
    }
    "unspecified".into()
}

fn intent_label(intent: AgentIntent) -> &'static str {
    match intent {
        AgentIntent::Search => "search",
        AgentIntent::Open => "open",
        AgentIntent::Execute => "execute",
        AgentIntent::Workflow => "workflow",
        AgentIntent::Explain => "explain",
        AgentIntent::Unknown => "unknown",
    }
}

fn action_ref_for(parsed: &ParsedRequest) -> String {
    match parsed.intent {
        AgentIntent::Search => "search:query".into(),
        AgentIntent::Open => "command:open".into(),
        AgentIntent::Execute => "command:execute".into(),
        AgentIntent::Workflow => "workflow:run".into(),
        AgentIntent::Explain => "explain:context".into(),
        AgentIntent::Unknown => "noop".into(),
    }
}

fn input_for(parsed: &ParsedRequest) -> serde_json::Value {
    if let Some(q) = &parsed.entities.query {
        return json!({"query": q});
    }
    if let Some(t) = &parsed.entities.target {
        return json!({"target": t});
    }
    if let Some(w) = &parsed.entities.workflow_id {
        return json!({"workflow_id": w});
    }
    json!({})
}

fn rationale_for(parsed: &ParsedRequest) -> String {
    format!("derived from {} intent via deterministic rules", intent_label(parsed.intent))
}

fn explanation_for(parsed: &ParsedRequest) -> String {
    format!(
        "proposal built from parsed intent `{}` with deterministic rules",
        intent_label(parsed.intent)
    )
}

/// Fail-closed approval: only read-only operations skip approval.
fn requires_approval_for(action_ref: &str) -> bool {
    !(action_ref.starts_with("search:") || action_ref.starts_with("explain:"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::parse_intent;

    #[test]
    fn search_proposal_builds_and_validates() {
        let parsed = parse_intent("find quarterly report");
        let p = build_proposal("s1", 1, &parsed, 0.9).unwrap();
        assert_eq!(p.intent, AgentIntent::Search);
        assert_eq!(p.plan.len(), 1);
        assert_eq!(p.plan[0].action_ref, "search:query");
        assert!(!p.plan[0].requires_approval, "search is L0/L1, no approval");
        validate_proposal(&p).unwrap();
    }

    #[test]
    fn open_proposal_requires_approval() {
        let parsed = parse_intent("open chrome");
        let p = build_proposal("s1", 2, &parsed, 0.9).unwrap();
        assert_eq!(p.intent, AgentIntent::Open);
        assert!(p.plan[0].requires_approval, "open is not read-only");
    }

    #[test]
    fn workflow_proposal_has_workflow_id() {
        let parsed = parse_intent("workflow nightly-backup");
        let p = build_proposal("s1", 3, &parsed, 0.9).unwrap();
        assert_eq!(p.intent, AgentIntent::Workflow);
        assert_eq!(p.plan[0].input["workflow_id"], "nightly-backup");
    }

    #[test]
    fn confidence_clamped() {
        let parsed = parse_intent("find stuff");
        let p = build_proposal("s1", 4, &parsed, 5.0).unwrap();
        assert_eq!(p.confidence, 1.0, "clamped to [0,1]");
    }

    /// Deterministic: same input → same proposal.
    #[test]
    fn deterministic() {
        let parsed = parse_intent("open chrome");
        let a = build_proposal("s1", 1, &parsed, 0.9).unwrap();
        let b = build_proposal("s1", 1, &parsed, 0.9).unwrap();
        assert_eq!(a, b);
    }
}
