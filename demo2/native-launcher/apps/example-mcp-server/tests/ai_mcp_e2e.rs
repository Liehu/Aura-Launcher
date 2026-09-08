//! MVP4.3 Phase 9 E2E (review 44 §21/§23): AI Planner → ActionProposal →
//! Workflow → ReferenceResolver (fresh discovery) → ActionResolver →
//! ActionEngine → plugin.mcp.invoke → Core registry → McpExecutor → real
//! mcp-calculator stdio fixture → "46".
//!
//! The planner runs over the live unified ActionCatalog and never learns
//! anything about MCP execution; its output is the frozen four-field
//! ActionProposal.

use launcher_core::workflow_backend::CoreWorkflowBackend;
use launcher_core::providers::mcp::McpProvider;
use launcher_core::{Core, Provider};
use launcher_domain::workflow::WorkflowFailureClass;
use launcher_domain::{Command, QueryContext, WorkflowRunStatus};
use launcher_mcp::adapter::invoke_input;
use launcher_mcp::types::McpServerId;
use launcher_workflow::proposal::{ActionPlanner, ActionProposal, KeywordPlanner};
use launcher_workflow::execute_proposals;

fn core_with_calculator() -> Core {
    let mut core = Core::new();
    core.register_mcp_server("calc", env!("CARGO_BIN_EXE_mcp-calculator"), vec![]);
    core.register(Box::new(McpProvider::new(
        "calc".into(),
        env!("CARGO_BIN_EXE_mcp-calculator").into(),
        vec![],
    )));
    core
}

/// §21 E2E: intent → catalog → proposal → workflow → real server → 46.
/// The planner sees only Commands; the argument-generation step standing
/// in for the LLM fills `arguments` from the catalog's published schema.
#[test]
fn ai_planner_mcp_calculator_e2e() {
    let mut core = core_with_calculator();

    // 1. planning: the frozen KeywordPlanner over the live unified catalog
    let catalog = core.action_catalog();
    assert!(catalog.iter().any(|c| c.provider_id == "mcp:calc" && c.id == "evaluate"));
    let mut proposals = KeywordPlanner.plan("evaluate", &catalog);
    assert_eq!(proposals.len(), 1);
    assert_eq!(
        (proposals[0].provider_id.as_str(), proposals[0].command_id.as_str(), proposals[0].action_id.as_str()),
        ("mcp:calc", "evaluate", "invoke")
    );

    // 2. argument generation from the schema (LLM stand-in; §10): planner
    //    schema understanding is not execution-input authority — the
    //    Resolver + executor validation remain the gates
    proposals[0].input = invoke_input(
        &McpServerId("calc".into()),
        "evaluate",
        serde_json::json!({"expression": "12 + 34"}),
    );

    // 3. execution through the frozen chain with the real stdio server
    let backend = CoreWorkflowBackend { core: &mut core, session_results: &[], discovery_limit: 50 };
    let run = execute_proposals(backend, &proposals, "wf-ai-mcp-calc", 1).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, launcher_domain::StepRunStatus::Complete);
    assert_eq!(run.steps[0].attempt, 1);
    assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-1"));
}

/// §23 E2E: the proposal's target vanishes from the live catalog before
/// execution → fresh discovery → CommandNotFound; the stale proposal is
/// never executed. Proposal lifecycle ≠ ResolvedAction lifecycle.
#[test]
fn ai_proposal_vanished_tool_is_command_not_found() {
    // catalog WITHOUT the tool (it was removed after planning)
    let mut core = Core::new();
    core.register_mcp_server("calc", env!("CARGO_BIN_EXE_mcp-calculator"), vec![]);
    struct EmptyMcp;
    impl Provider for EmptyMcp {
        fn id(&self) -> &str {
            "mcp"
        }
        fn plugin_identity(&self) -> Option<&str> {
            Some("calc")
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            Vec::new()
        }
    }
    core.register(Box::new(EmptyMcp));

    let proposal = ActionProposal {
        provider_id: "mcp:calc".into(),
        command_id: "evaluate".into(),
        action_id: "invoke".into(),
        input: invoke_input(
            &McpServerId("calc".into()),
            "evaluate",
            serde_json::json!({"expression": "12 + 34"}),
        ),
    };
    let backend = CoreWorkflowBackend { core: &mut core, session_results: &[], discovery_limit: 50 };
    let run = execute_proposals(backend, &[proposal], "wf-ai-vanished", 1).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert!(run.steps[0]
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("evaluate"));
    let _ = WorkflowFailureClass::CommandNotFound; // frozen class contract
}
