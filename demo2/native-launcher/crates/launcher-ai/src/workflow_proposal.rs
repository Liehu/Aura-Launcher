//! Workflow Proposal Builder (P27-B05): builds an `AgentProposal` targeting
//! a workflow run — the bridge between intent parsing and the Durable
//! Runtime (P2.6-B). The proposal is DATA; the actual run starts via
//! TriggerQueue → start_workflow (P2.6-D).

use crate::agent_contract::{validate_proposal, AgentIntent, AgentProposal, PlanStep};

/// Build a workflow-start proposal for the given workflow id.
pub fn build_workflow_proposal(
    session_id: &str,
    seq: u64,
    workflow_id: &str,
    variables: &serde_json::Value,
    confidence: f32,
) -> Result<AgentProposal, String> {
    if workflow_id.trim().is_empty() {
        return Err("workflow_id must not be empty".into());
    }
    let proposal_id = format!("{session_id}-wf{seq}");
    let proposal = AgentProposal {
        proposal_id: proposal_id.clone(),
        session_id: session_id.to_string(),
        user_goal: format!("run workflow: {workflow_id}"),
        intent: AgentIntent::Workflow,
        confidence: confidence.clamp(0.0, 1.0),
        explanation: Some(format!("workflow `{workflow_id}` triggered by agent")),
        plan: vec![PlanStep {
            step_id: format!("{proposal_id}-run"),
            action_ref: format!("workflow:{workflow_id}"),
            input: variables.clone(),
            rationale: Some(format!("user requested workflow `{workflow_id}`")),
            requires_approval: false, // workflow runs go through the Durable Runtime's own approval flow
        }],
    };
    validate_proposal(&proposal)?;
    Ok(proposal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn workflow_proposal_builds_and_validates() {
        let p = build_workflow_proposal("s1", 1, "nightly-backup", &json!({"target": "/tmp"}), 0.9)
            .unwrap();
        assert_eq!(p.intent, AgentIntent::Workflow);
        assert_eq!(p.plan.len(), 1);
        assert_eq!(p.plan[0].action_ref, "workflow:nightly-backup");
        validate_proposal(&p).unwrap();
    }

    #[test]
    fn invalid_workflow_id_rejected() {
        let result = build_workflow_proposal("s1", 2, "", &json!({}), 0.9);
        assert!(result.is_err(), "empty workflow id rejected");
    }

    #[test]
    fn confidence_clamped() {
        let p = build_workflow_proposal("s1", 3, "wf", &json!({}), 5.0).unwrap();
        assert_eq!(p.confidence, 1.0);
    }
}
