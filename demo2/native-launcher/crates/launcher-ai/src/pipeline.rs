//! Agent Pipeline (P2.7-C03 groundwork, spec `P2.7 开发设计规范` §3/§16/§18/
//! §19): composes the P2.7 building blocks into one deterministic pipeline —
//! intent parse → clarification → prompt build → LLM call (injected as a
//! closure) → structured-output validation → per-step risk classification.
//!
//! The LLM call is a CLOSURE the host provides (Mock for tests, OpenAI-
//! compatible in production) — this module owns the ORCHESTRATION, not the
//! transport. The pipeline output is proposal DATA; execution still only
//! happens through the frozen Resolver → Engine chain.

use crate::agent_contract::{validate_proposal, AgentProposal, RiskLevel};
use crate::clarification::{needs_clarification, ClarificationPolicy};
use crate::intent::parse_intent;
use crate::risk_classifier::classify;
use crate::structured_output::validate_llm_output;

/// Pipeline outcome: either a validated proposal (risk-classified) or a
/// clarification request for the user.
#[derive(Debug, Clone, PartialEq)]
pub enum PipelineOutcome {
    Proposal {
        proposal: AgentProposal,
        /// Per-step risk levels, index-aligned with proposal.plan.
        risks: Vec<RiskLevel>,
        prompt: String,
    },
    Clarify {
        question: String,
        prompt: String,
    },
}

/// Run the pipeline.
///
/// * `llm` — injected LLM call: takes the assembled prompt, returns the raw
///   LLM text (mock in tests, OpenAI-compatible in production).
pub fn run_pipeline(
    input: &str,
    context: &str,
    catalog: &str,
    llm: &dyn Fn(String) -> Result<String, String>,
    policy: &ClarificationPolicy,
    context_budget: usize,
) -> Result<PipelineOutcome, String> {
    // 1. intent + entities (A03, deterministic rules)
    let parsed = parse_intent(input);
    // 2. clarification gate (A06) — before spending any LLM call
    //    (confidence unknown pre-LLM; v1 gates on ambiguity/empty only)
    if let Some(clar) = needs_clarification(parsed.intent, &parsed.entities, 1.0, policy) {
        return Ok(PipelineOutcome::Clarify {
            question: clar.question,
            prompt: String::new(),
        });
    }
    // 3. prompt assembly (A04/A02): sanitized, bounded, fenced
    let prompt = crate::prompt::build_agent_prompt(
        input,
        &parsed.entities,
        context,
        catalog,
        context_budget,
    );
    // 4. LLM call (injected) + 5. structured-output validation (A05)
    let raw = llm(prompt.clone()).map_err(|e| format!("LLM call failed: {e}"))?;
    let mut proposal = validate_llm_output(&raw)?;
    // intent consistency: the parser's deterministic intent wins on conflict
    // (the LLM refines HOW, never WHAT kind of request this is)
    proposal.intent = parsed.intent;
    // 6. per-step risk classification (B06)
    let risks: Vec<RiskLevel> = proposal
        .plan
        .iter()
        .map(|s| {
            let mut with_ref = s.clone();
            with_ref.action_ref = s.action_ref.clone();
            classify(&with_ref)
        })
        .collect();
    // approval flags follow the classification (can only add approval)
    for (step, risk) in proposal.plan.iter_mut().zip(&risks) {
        if risk.requires_approval() {
            step.requires_approval = true;
        }
    }
    validate_proposal(&proposal)?;
    Ok(PipelineOutcome::Proposal { proposal, risks, prompt })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_contract::{AgentIntent, RiskLevel};

    fn fake_llm(respond: &'static str) -> impl Fn(String) -> Result<String, String> {
        move |_prompt| Ok(respond.to_string())
    }

    const GOOD_PLAN: &str = r#"{
        "proposal_id": "p1", "session_id": "s1",
        "user_goal": "open the report",
        "intent": "open", "confidence": 0.95,
        "plan": [{"step_id": "s1", "action_ref": "command:app:report",
                  "input": {"target": "report"}}]
    }"#;

    /// Happy path: intent parse → prompt → LLM → validated, risk-classified
    /// proposal.
    #[test]
    fn pipeline_produces_classified_proposal() {
        let out = run_pipeline(
            "open the report",
            "recent: q4.xlsx",
            "1. command:app:report",
            &fake_llm(GOOD_PLAN),
            &ClarificationPolicy::default(),
            256,
        )
        .unwrap();
        match out {
            PipelineOutcome::Proposal { proposal, risks, prompt } => {
                assert_eq!(proposal.intent, AgentIntent::Open);
                assert_eq!(proposal.plan.len(), 1);
                assert_eq!(risks, vec![RiskLevel::L1ReadOnly]);
                assert!(!proposal.plan[0].requires_approval);
                assert!(prompt.contains("GOAL: open the report"));
            }
            other => panic!("expected proposal, got {other:?}"),
        }
    }

    /// Ambiguous input short-circuits BEFORE the LLM call (clarification
    /// gate): the LLM closure panics if invoked, proving the short-circuit.
    #[test]
    fn clarification_gates_before_llm() {
        let llm = |_p: String| -> Result<String, String> {
            panic!("LLM must not be called for ambiguous input");
        };
        let out = run_pipeline(
            "",
            "",
            "catalog",
            &llm,
            &ClarificationPolicy::default(),
            256,
        )
        .unwrap();
        match out {
            PipelineOutcome::Clarify { question, .. } => assert!(!question.is_empty()),
            other => panic!("expected clarify, got {other:?}"),
        }
    }

    /// LLM failure surfaces as an error (never a silent empty proposal).
    #[test]
    fn llm_failure_is_an_error() {
        let err = run_pipeline(
            "open the report",
            "",
            "catalog",
            &|_p| Err("provider down".into()),
            &ClarificationPolicy::default(),
            256,
        );
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("LLM call failed"));
    }

    /// High-risk steps get approval flags from the classifier (B06 + C 线).
    #[test]
    fn high_risk_steps_flagged_for_approval() {
        let plan = r#"{
            "proposal_id": "p2", "session_id": "s2",
            "user_goal": "clean up temp", "intent": "execute",
            "confidence": 0.9,
            "plan": [{"step_id": "s1", "action_ref": "shell:cleanup",
                      "input": {}}]
        }"#;
        let out = run_pipeline(
            "clean up temp",
            "",
            "catalog",
            &fake_llm_const(plan),
            &ClarificationPolicy::default(),
            256,
        )
        .unwrap();
        match out {
            PipelineOutcome::Proposal { proposal, .. } => {
                assert!(proposal.plan[0].requires_approval, "L3 forces approval");
            }
            other => panic!("expected proposal, got {other:?}"),
        }
    }

    fn fake_llm_const(respond: &'static str) -> impl Fn(String) -> Result<String, String> {
        move |_prompt| Ok(respond.to_string())
    }
}
