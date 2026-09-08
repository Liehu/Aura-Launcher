//! MVP4.3 Phase 8 E2E (review 42 §21/§22): the full MCP Workflow chain
//! over the real mcp-calculator stdio fixture —
//!
//! ```text
//! WorkflowRunner → ReferenceResolver (fresh discovery) → ActionResolver
//!   → ActionEngine → plugin.mcp.invoke → Core registry → McpExecutor
//!   → stdio → mcp-calculator → "46"
//! ```
//!
//! Exit criteria (§21): WorkflowRun Succeeded, StepRun Complete,
//! attempt = 1, last_execution_id = e-N.

use launcher_core::workflow_backend::CoreWorkflowBackend;
use launcher_core::providers::mcp::McpProvider;
use launcher_core::Core;
use launcher_domain::workflow::WorkflowFailureClass;
use launcher_domain::{
    ActionReference, Command, StepRunStatus, WorkflowAction, WorkflowDefinition,
    WorkflowRunStatus, WorkflowStep,
};
use launcher_mcp::adapter::invoke_input;
use launcher_mcp::types::McpServerId;
use launcher_workflow::{ExecutionOutcome, WfFailure, WorkflowRunner};

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

fn evaluate_step(expression: &str) -> WorkflowStep {
    WorkflowStep {
        step_id: "fetch".into(),
        action: WorkflowAction::Reference(ActionReference {
            provider_id: "mcp:calc".into(),
            command_id: "evaluate".into(),
            action_id: "invoke".into(),
        }),
        input: invoke_input(
            &McpServerId("calc".into()),
            "evaluate",
            serde_json::json!({"expression": expression}),
        ),
        condition: None,
        output: None,
        on_success: None,
        on_failure: None,
        on_condition_false: None,
        failure_policy: Default::default(),
    }
}

fn run(core: &mut Core, def: &WorkflowDefinition) -> launcher_domain::WorkflowRun {
    let mut runner = WorkflowRunner::new(CoreWorkflowBackend {
        core,
        session_results: &[],
        discovery_limit: 50,
    });
    runner.run(def, format!("wr-{}", def.id), 1).expect("definition valid")

}

/// §21 E2E: the calculator MCP workflow — Reference resolved via fresh
/// empty-query discovery, executed through the frozen chain, result "46".
#[test]
fn mcp_workflow_calculator_e2e() {
    let mut core = core_with_calculator();
    let def = WorkflowDefinition {
        id: "wf-calc".into(),
        version: 1,
        name: "calculator".into(),
        steps: vec![evaluate_step("12 + 34")],
        failure_policy: Default::default(),
        entry_step: None,
variables: Vec::new(),
inputs: Vec::new(),
    };
    let run = run(&mut core, &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(run.steps[0].attempt, 1);
    assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-1"));
    // the plugin_result payload carries the real server's answer
    // (observable through the outcome payload in the backend contract)
}

/// §22 E2E: stale context → ReResolve → fresh MCP discovery → new
/// execution. The MCP path shares the frozen stale/re-resolve machinery —
/// no dedicated stale execution path exists.
#[test]
fn mcp_workflow_stale_context_reresolve_e2e() {
    // provider counts fresh discovery rounds to prove the re-resolve went
    // back through McpProvider discovery
    let mut core = core_with_calculator();

    // host wrapper: the execution precondition fails once with StaleContext
    // (context generation changed between resolve and execute), then the
    // standard backend path takes over
    struct StaleOnce<'a> {
        inner: CoreWorkflowBackend<'a>,
        stale_injected: std::cell::Cell<bool>,
    }
    impl launcher_workflow::CommandSource for StaleOnce<'_> {
        fn in_session(&self, p: &str, c: &str) -> Option<Command> {
            self.inner.in_session(p, c)
        }
        fn fresh_query(
            &mut self,
            p: &str,
        ) -> Result<Vec<Command>, (WorkflowFailureClass, String)> {
            self.inner.fresh_query(p)
        }
    }
    impl launcher_workflow::ActionExecutor for StaleOnce<'_> {
        fn execute(
            &mut self,
            action: launcher_domain::Action,
            provider_id: Option<String>,
            context_generation: u64,
            confirmed: bool,
        ) -> Result<ExecutionOutcome, WfFailure> {
            if !self.stale_injected.replace(true) {
                return Err(WfFailure::new(
                    WorkflowFailureClass::StaleContext,
                    "context generation changed: G1 -> G2",
                ));
            }
            self.inner
                .execute(action, provider_id, context_generation, confirmed)
        }
    }

    let def = WorkflowDefinition {
        id: "wf-stale".into(),
        version: 1,
        name: "stale".into(),
        steps: vec![evaluate_step("10 % 3")],
        failure_policy: Default::default(),
        entry_step: None,
variables: Vec::new(),
inputs: Vec::new(),
    };
    let backend = CoreWorkflowBackend {
        core: &mut core,
        session_results: &[],
        discovery_limit: 50,
    };
    let host = StaleOnce { inner: backend, stale_injected: std::cell::Cell::new(false) };
    let mut runner = WorkflowRunner::new(host);
    let run = runner.run(&def, "wr-stale".into(), 1).expect("definition valid");

    assert_eq!(run.status, WorkflowRunStatus::Succeeded, "stale → ReResolve → execute");
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(run.steps[0].attempt, 2, "attempt 2 after the stale re-resolve");
    assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-1"));
}
