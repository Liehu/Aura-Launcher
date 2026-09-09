//! Structured Output Validator (P27-A05, spec §16): an LLM's raw text is
//! UNTRUSTED input. This module is the ONLY path from LLM output to an
//! `AgentProposal`: strip markdown fences → parse JSON → run the frozen
//! contract validation. Anything that fails validation is dropped with a
//! reason — a malformed LLM answer can never become a half-valid proposal.

use crate::agent_contract::{validate_proposal, AgentProposal};

/// Validate raw LLM output into a proposal. Accepts bare JSON or a single
/// ```json fenced block. Returns the proposal or a precise rejection reason.
pub fn validate_llm_output(raw: &str) -> Result<AgentProposal, String> {
    let json = extract_json(raw)?;
    let proposal: AgentProposal =
        serde_json::from_str(&json).map_err(|e| format!("not a valid proposal: {e}"))?;
    validate_proposal(&proposal)?;
    Ok(proposal)
}

/// Extract the first JSON object from LLM text: strips ``` fences if present
/// and falls back to the outermost `{...}` span (LLMs often add prose).
fn extract_json(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let without_fence = if let Some(start) = trimmed.find('{') {
        let end = trimmed.rfind('}').ok_or("no JSON object found")?;
        if end > start {
            trimmed[start..=end].to_string()
        } else {
            return Err("no JSON object found".into());
        }
    } else {
        return Err("no JSON object found".into());
    };
    // a fenced block's inner braces were already captured by {..} span; but
    // prose before the fence is excluded by starting at the first '{'
    let _ = without_fence.trim_start_matches("```json");
    Ok(without_fence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_contract::{PlanStep, RiskLevel};

    fn good_proposal_json() -> String {
        serde_json::json!({
            "proposal_id": "p1",
            "session_id": "s1",
            "user_goal": "open the report",
            "intent": "open",
            "confidence": 0.9,
            "plan": [{
                "step_id": "s1",
                "action_ref": "command:app:report",
                "input": {"target": "report"}
            }]
        })
        .to_string()
    }

    /// A clean LLM answer parses and passes the frozen contract.
    #[test]
    fn clean_json_accepted() {
        let p = validate_llm_output(&good_proposal_json()).unwrap();
        assert_eq!(p.intent, crate::agent_contract::AgentIntent::Open);
    }

    /// Realistic LLM output: prose + fenced JSON is accepted.
    #[test]
    fn fenced_json_with_prose_accepted() {
        let raw = format!("Here's my plan:\n```json\n{}\n```", good_proposal_json());
        validate_llm_output(&raw).unwrap();
    }

    /// Fail-closed: invalid contracts are rejected with a reason — a
    /// malformed LLM answer can never become a half-valid proposal.
    #[test]
    fn invalid_contracts_rejected() {
        // path-style action_ref (spec §8 guard)
        let mut bad = good_proposal_json();
        bad = bad.replace("command:app:report", "C:\\evil.exe");
        assert!(validate_llm_output(&bad).is_err());
        // no JSON at all
        assert!(validate_llm_output("I cannot help with that").is_err());
        // empty plan
        let empty = serde_json::json!({
            "proposal_id": "p", "session_id": "s", "user_goal": "g",
            "intent": "open", "confidence": 0.5, "plan": []
        });
        assert!(validate_llm_output(&empty.to_string()).is_err());
    }

    /// Risk default sanity: fresh steps carry no implicit approval.
    #[test]
    fn fresh_steps_have_no_implicit_approval() {
        let p = validate_llm_output(&good_proposal_json()).unwrap();
        assert!(!p.plan[0].requires_approval);
        assert_eq!(p.plan[0].requires_approval, false);
        let _ = RiskLevel::L1ReadOnly; // vocabulary available for D-line
    }

    /// PlanStep (§8) roundtrip for the validator's output shape.
    #[test]
    fn plan_step_roundtrip() {
        let step = PlanStep {
            step_id: "s".into(),
            action_ref: "command:app:x".into(),
            input: serde_json::json!({"k": "v"}),
            rationale: None,
            requires_approval: true,
        };
        let back: PlanStep = serde_json::from_str(&serde_json::to_string(&step).unwrap()).unwrap();
        assert_eq!(back, step);
    }
}
