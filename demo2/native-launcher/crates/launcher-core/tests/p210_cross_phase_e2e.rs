//! P210-G16 Cross-Phase E2E (spec `P2.10` §33): the three golden paths that
//! compose P2.6–P2.9 subsystems, verified end-to-end in one suite.
#![cfg(windows)]

use launcher_ai::agent_loop::{
    run_agent, ApprovalSink, LoopStop, NoApprovalSink, TurnExecutor,
};
use launcher_ai::agent_session::AgentSession;
use launcher_ai::approval::{ApprovalDecision, ApprovalGate, Decision};
use launcher_ai::clarification::ClarificationPolicy;
use launcher_ai::pipeline::{run_pipeline, PipelineOutcome};
use launcher_ai::plan_validator::validate_plan;
use launcher_ai::telemetry::{AgentEvent, Telemetry};
use launcher_ai::tool_catalog::ToolCatalog;
use launcher_domain::system_process_window::{process_command_with_identity, window_command};
use std::sync::atomic::{AtomicBool, Ordering};

// ---- Path 1: Search → AI Proposal → Approval → Plan Validation → Effect →
//              Audit -------------------------------------------------------

struct EffectAuditHost {
    effects: Vec<String>,
}

impl TurnExecutor for EffectAuditHost {
    fn execute(
        &mut self,
        action_ref: &str,
        _input: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.effects.push(action_ref.into());
        Ok(serde_json::json!({"ok": true}))
    }
}

#[test]
fn path1_search_to_ai_to_approval_to_effect_to_audit() {
    // SEARCH: user intent (deterministic parse)
    let catalog_items: Vec<launcher_workflow::proposal::ActionCatalogItem> = Vec::new();
    let catalog = ToolCatalog::project(&catalog_items, &[]);
    let (catalog_json, _) = catalog.render_json(&Default::default());

    // AI: pipeline → validated proposal (mock LLM, structured output)
    let llm_text = r#"{
        "proposal_id": "p1", "session_id": "e2e1",
        "user_goal": "evaluate 1+1", "intent": "execute",
        "confidence": 0.95,
        "plan": [{"step_id": "a", "action_ref": "mcp:calc|evaluate|invoke", "input": {"expr": "1+1"}}]
    }"#;
    let llm = |_p: String| -> Result<String, String> { Ok(llm_text.into()) };
    let outcome = run_pipeline(
        "run the calculator", "", &catalog_json, &llm,
        &ClarificationPolicy::default(), 4096,
    )
    .expect("pipeline");
    let proposal = match outcome {
        PipelineOutcome::Proposal { proposal, .. } => proposal,
        other => panic!("unexpected pipeline outcome: {other:?}"),
    };

    // PLAN VALIDATION (B03) against the Tool Catalog — fail-closed refs
    assert!(
        validate_plan(&launcher_ai::plan::PlanDocument::from_proposal(&proposal), &catalog)
            .is_err(),
        "a plan referencing an un-published tool must fail catalog validation"
    );

    // APPROVAL (D-line, single-use gate) then EXECUTE through the engine
    // stand-in host; the audit trail records the full arc.
    let llm_ok = |_p: String| -> Result<String, String> {
        Ok(r#"{
            "proposal_id": "p1", "session_id": "e2e1",
            "user_goal": "g", "intent": "execute", "confidence": 0.9,
            "plan": [{"step_id": "a", "action_ref": "calc|evaluate|invoke", "input": {}}]
        }"#
        .into())
    };
    let catalog_json2 = r#"[{"ref":"calc|evaluate|invoke","title":"Evaluate"}]"#;
    struct ApprovingSink;
    impl ApprovalSink for ApprovingSink {
        fn request_approval(
            &mut self,
            r: &launcher_ai::approval::ApprovalRequest,
        ) -> Option<ApprovalDecision> {
            Some(ApprovalDecision {
                request_id: r.request_id.clone(),
                decision: Decision::Approve,
            })
        }
    }
    let mut session = AgentSession::new("e2e1", 8);
    let mut host = EffectAuditHost { effects: vec![] };
    let cancel = AtomicBool::new(false);
    let mut telemetry = Telemetry::default();
    let stop = run_agent(
        &mut session, "run the calculator", "", catalog_json2, &llm_ok,
        &mut host, &cancel, &ClarificationPolicy::default(), 256,
        &mut ApprovingSink, &mut telemetry,
    );
    assert_eq!(stop, LoopStop::Completed);
    assert_eq!(host.effects, vec!["calc|evaluate|invoke"]);

    // AUDIT: the §37 event trail covers plan → approval → step → run
    let events: Vec<AgentEvent> = telemetry.log.records().iter().map(|r| r.event).collect();
    for expected in [
        AgentEvent::PlanGenerated,
        AgentEvent::ApprovalRequested,
        AgentEvent::ApprovalReceived,
        AgentEvent::StepStarted,
        AgentEvent::StepCompleted,
        AgentEvent::RunSucceeded,
    ] {
        assert!(events.contains(&expected), "missing {expected:?}");
    }
}

// ---- Path 2: Workflow → System Target → Target Changes → StaleTarget ----

#[test]
fn path2_workflow_origin_stale_system_target() {
    // a workflow-originated system command whose target CHANGED after
    // resolve (pid reused by another process identity) must be refused
    let live = std::process::id();
    let wrong_identity = process_command_with_identity(
        live, "reused.exe", Some(12345), "kill", "workflow",
    )
    .unwrap();
    let resolver = launcher_domain::system::SystemResolver {
        policy: launcher_domain::system::SystemPolicy {
            allowed_origins: vec!["workflow".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    // policy passes (workflow origin allowed), but the ADAPTER's identity
    // check refuses the stale target before any Win32 call
    assert!(matches!(
        launcher_action::execute_system_effect(&resolver, &wrong_identity, true),
        Err(launcher_action::ActionError::StaleTarget)
    ));
    // a well-identity-checked window command on a dead hwnd is also stale
    let stale_window = window_command(0xDEAD_0001, "Ghost", "close", "workflow").unwrap();
    assert!(matches!(
        launcher_action::execute_system_effect(&resolver, &stale_window, true),
        Err(launcher_action::ActionError::StaleTarget)
    ));
}

// ---- Path 3: AI → Effect Timeout → Unknown → Replan → no duplicate ------

#[test]
fn path3_timeout_unknown_replan_never_duplicates() {
    const PLAN: &str = r#"{
        "proposal_id": "p", "session_id": "e2e3",
        "user_goal": "g", "intent": "execute",
        "confidence": 0.9,
        "plan": [{"step_id": "a", "action_ref": "command:app:thing", "input": {}}]
    }"#;
    fn llm(_p: String) -> Result<String, String> {
        Ok(PLAN.into())
    }
    /// The executor TIMES OUT: the effect state is Unknown, not Failed.
    struct TimeoutHost {
        started: u32,
    }
    impl TurnExecutor for TimeoutHost {
        fn execute(
            &mut self,
            _action_ref: &str,
            _input: &serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            self.started += 1;
            Err("timed out".into())
        }
        fn effect_state(
            &self,
            _action_ref: &str,
        ) -> launcher_domain::execution_semantics::EffectState {
            launcher_domain::execution_semantics::EffectState::Unknown
        }
    }
    let mut session = AgentSession::new("e2e3", 8);
    let mut host = TimeoutHost { started: 0 };
    let cancel = AtomicBool::new(false);
    let mut telemetry = Telemetry::default();
    let stop = run_agent(
        &mut session, "g", "", "[]", &llm, &mut host, &cancel,
        &ClarificationPolicy::default(), 256, &mut NoApprovalSink, &mut telemetry,
    );
    // an unsettled effect goes to RECOVERY, not to an automatic replan
    assert!(matches!(stop, LoopStop::Failed { .. }));
    assert_eq!(
        host.started, 1,
        "Unknown effect must not be blindly re-executed"
    );
    assert_eq!(telemetry.metrics.replan_count, 0);
}
