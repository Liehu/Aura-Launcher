//! MVP4.3 Phase 8 — Workflow Integration tests (review 42, MCP-WF-001..016
//! + MCP-ARCH-006).
//!
//! Architecture success criterion (§24): the WorkflowRunner and
//! ReferenceResolver contain ZERO MCP-specific code. Every test here runs
//! the frozen orchestration path — ReferenceResolver → ActionResolver →
//! ActionEngine → Core effect registry — with an MCP-shaped provider and
//! the Phase 6/7 executor; no MCP branch exists anywhere in the chain.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use launcher_core::workflow_backend::CoreWorkflowBackend;
use launcher_core::{Core, Provider};
use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{
    Action, ActionKind, ActionPayload, ActionReference, Category, Command, QueryContext,
    StepRunStatus, WorkflowAction, WorkflowDefinition, WorkflowRunStatus, WorkflowStep,
};
use launcher_mcp::adapter::{invoke_input, tool_to_command_with_grants};
use launcher_mcp::executor::{McpExecutor, McpInvokeInput, McpToolResult};
use launcher_mcp::types::{McpServerId, McpTool};
use launcher_workflow::WorkflowRunner;

// ---------- fixtures ----------

fn ok_result(text: &str) -> McpToolResult {
    McpToolResult {
        content: vec![launcher_mcp::executor::McpContent {
            kind: "text".into(),
            text: Some(text.into()),
        }],
        structured_content: None,
        is_error: false,
    }
}

/// Scripted MCP executor: pops one scripted outcome per call; when the
/// script runs dry the LAST outcome repeats forever.
struct ScriptedExecutor {
    outcomes: Mutex<Vec<Result<McpToolResult, launcher_mcp::McpError>>>,
    calls: Mutex<Vec<(String, String, String)>>,
}

impl ScriptedExecutor {
    /// Always succeeds (default text "46").
    fn ok() -> Self {
        Self::scripted(vec![Ok(ok_result("46"))])
    }

    /// Repeats one failing outcome forever.
    fn always(err: launcher_mcp::McpError) -> Self {
        Self::scripted(vec![Err(err)])
    }

    /// Fails once with `err`, then succeeds forever.
    fn fail_then(err: launcher_mcp::McpError) -> Self {
        Self::scripted(vec![Err(err), Ok(ok_result("46"))])
    }

    fn scripted(outcomes: Vec<Result<McpToolResult, launcher_mcp::McpError>>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    fn ids(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().map(|c| c.2.clone()).collect()
    }

    fn tools(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().map(|c| c.1.clone()).collect()
    }

    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl McpExecutor for ScriptedExecutor {
    fn execute(
        &self,
        input: &McpInvokeInput,
        execution_id: &str,
    ) -> Result<McpToolResult, launcher_mcp::McpError> {
        self.calls
            .lock()
            .unwrap()
            .push((input.server_id.to_string(), input.tool_name.clone(), execution_id.into()));
        let mut q = self.outcomes.lock().unwrap();
        match q.len() {
            0 => Ok(ok_result("46")),
            1 => q[0].clone(),
            _ => q.remove(0),
        }
    }
}

/// MCP-shaped tool projected exactly as McpProvider serves it, without
/// spawning a process.
fn evaluate_tool() -> McpTool {
    McpTool {
        server_id: McpServerId("calc".into()),
        name: "evaluate".into(),
        title: Some("Evaluate arithmetic".into()),
        description: None,
        input_schema: serde_json::json!({"type": "object"}),
        annotations: serde_json::json!({}),
    }
}

/// Provider over a projected MCP catalog; counts fresh discovery rounds.
struct FakeMcpProvider {
    commands: Vec<Command>,
    fresh_queries: Arc<AtomicUsize>,
}

impl FakeMcpProvider {
    fn evaluate_command() -> Command {
        tool_to_command_with_grants(
            &evaluate_tool(),
            &McpServerId("calc".into()),
            &[launcher_domain::Capability::McpInvoke],
        )
    }

    /// The same tool projected WITHOUT the host `mcp.invoke` grant.
    fn denied_command(annotations: serde_json::Value) -> Command {
        let mut t = evaluate_tool();
        t.annotations = annotations;
        tool_to_command_with_grants(&t, &McpServerId("calc".into()), &[])
    }
}

impl Provider for FakeMcpProvider {
    fn id(&self) -> &str {
        "mcp"
    }
    fn plugin_identity(&self) -> Option<&str> {
        Some("calc")
    }
    fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
        self.fresh_queries.fetch_add(1, Ordering::SeqCst);
        self.commands.clone()
    }
}

fn core_with(executor: Arc<ScriptedExecutor>) -> (Core, Arc<AtomicUsize>) {
    let mut core = Core::new();
    core.register_mcp_server("calc", "mcp-calculator", vec![]);
    core.set_mcp_executor(executor);
    let counter = Arc::new(AtomicUsize::new(0));
    core.register(Box::new(FakeMcpProvider {
        commands: vec![FakeMcpProvider::evaluate_command()],
        fresh_queries: counter.clone(),
    }));
    (core, counter)
}

fn reference_step(provider: &str, command: &str, input: serde_json::Value) -> WorkflowStep {
    WorkflowStep {
        step_id: "fetch".into(),
        action: WorkflowAction::Reference(ActionReference {
            provider_id: provider.into(),
            command_id: command.into(),
            action_id: "invoke".into(),
        }),
        input,
        condition: None,
        output: None,
        on_success: None,
        on_failure: None,
        on_condition_false: None,
        failure_policy: Default::default(),
    }
}

fn calc_step_input() -> serde_json::Value {
    invoke_input(
        &McpServerId("calc".into()),
        "evaluate",
        serde_json::json!({"expression": "12 + 34"}),
    )
}

fn definition(step: WorkflowStep) -> WorkflowDefinition {
    WorkflowDefinition {
        id: "wf-mcp".into(),
        version: 1,
        name: "wf-mcp".into(),
        steps: vec![step],
        failure_policy: Default::default(),
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    }
}

fn run_workflow(
    core: &mut Core,
    session: Vec<Command>,
    def: &WorkflowDefinition,
) -> launcher_domain::WorkflowRun {
    let mut runner = WorkflowRunner::new(CoreWorkflowBackend {
        core,
        session_results: &session,
        discovery_limit: 50,
    });
    runner
        .run(def, format!("wr-{}", def.id), 1)
        .expect("definition valid")

}

// ---------- Discovery ----------

/// MCP-WF-001: the reference resolves from the current popup session —
/// no fresh discovery round is spent (R1 priority, §5).
#[test]
fn mcp_wf001_resolves_from_session_results() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, counter) = core_with(ex.clone());
    let cmd = FakeMcpProvider::evaluate_command();
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![cmd], &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(counter.load(Ordering::SeqCst), 0, "session hit: no fresh discovery");
}

/// MCP-WF-002: with an empty session the reference resolves via fresh
/// empty-query discovery against the MCP provider (§4: DISCOVERY-TODO-001
/// MCP half closed as a real executable loop).
#[test]
fn mcp_wf002_resolves_via_fresh_discovery() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, counter) = core_with(ex.clone());
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(counter.load(Ordering::SeqCst), 1, "exactly one fresh discovery");
    assert_eq!(ex.call_count(), 1, "exactly one MCP execution");
}

/// MCP-WF-003: the MCP provider participates through the plain Provider
/// abstraction — resolution never reaches the executor; the only MCP-aware
/// component in the whole chain is the Phase 7 registry (§3/§6).
#[test]
fn mcp_wf003_no_workflow_special_case() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex.clone());
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    // the executor sees exactly ONE call (the engine's effect), never a
    // resolution probe; the resolver touched no MCP transport at all
    assert_eq!(ex.call_count(), 1);
    assert_eq!(ex.tools(), vec!["evaluate".to_string()]);
}

/// MCP-WF-004: a missing MCP tool (deleted server/tool, absent from the
/// fresh catalog) is CommandNotFound → Stop. No fuzzy fallback (§15/§16).
#[test]
fn mcp_wf004_missing_tool_command_not_found() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex.clone());
    let def = definition(reference_step("mcp:calc", "vanished_tool", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(run.steps[0].status, StepRunStatus::Failed);
    assert_eq!(ex.call_count(), 0, "never executes a stale descriptor");

    // resolver-level: exact identity only
    let mut backend = CoreWorkflowBackend {
        core: &mut core,
        session_results: &[],
        discovery_limit: 50,
    };
    let mut resolver = launcher_workflow::ReferenceResolver::new(&mut backend);
    let err = match resolver.resolve(&ActionReference {
        provider_id: "mcp:calc".into(),
        command_id: "vanished_tool".into(),
        action_id: "invoke".into(),
    }) {
        Err(e) => e,
        Ok(_) => panic!("exact identity only: must not resolve"),
    };
    assert_eq!(err.class, Class::CommandNotFound);
}

/// MCP-WF-016b (§16): a replaced tool is NOT fuzzy-matched — after the
/// server renames `evaluate` → `calculate`, the persisted Reference for
/// `evaluate` is CommandNotFound even though `calculate` exists.
#[test]
fn mcp_wf016b_replaced_tool_not_fuzzy_matched() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let mut core = Core::new();
    core.register_mcp_server("calc", "mcp-calculator", vec![]);
    core.set_mcp_executor(ex.clone());
    let mut renamed = evaluate_tool();
    renamed.name = "calculate".into();
    core.register(Box::new(FakeMcpProvider {
        commands: vec![tool_to_command_with_grants(
            &renamed,
            &McpServerId("calc".into()),
            &[launcher_domain::Capability::McpInvoke],
        )],
        fresh_queries: Arc::new(AtomicUsize::new(0)),
    }));
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed, "no fuzzy match to `calculate`");
    assert_eq!(ex.call_count(), 0, "the old descriptor is never executed");
}

// ---------- Authorization ----------

/// MCP-WF-005: tool exists but `mcp.invoke` denied → CapabilityDenied →
/// Stop (the frozen failure matrix; no new MCP class, §8).
#[test]
fn mcp_wf005_denied_capability() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex.clone());
    let denied = FakeMcpProvider::denied_command(serde_json::json!({}));
    assert!(denied.actions[0].disabled_reason.is_some());
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![denied], &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(run.steps[0].status, StepRunStatus::Failed);
    assert_eq!(ex.call_count(), 0, "denied tools never execute");
}

/// MCP-WF-006: workflow input can never be a cross-server confused deputy —
/// Reference says mcp:calc, step.input says server_id = jira → rejected (§18).
#[test]
fn mcp_wf006_provider_server_mismatch_rejected() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex.clone());
    let input = invoke_input(
        &McpServerId("jira".into()),
        "evaluate",
        serde_json::json!({"expression": "1 + 1"}),
    );
    let def = definition(reference_step("mcp:calc", "evaluate", input));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(ex.call_count(), 0, "mismatched identity never executes");
}

/// MCP-WF-007: tool metadata (annotations) cannot grant capability even
/// through a workflow — the resolver denies, the runner sees only the
/// frozen CapabilityDenied class (§7/§8).
#[test]
fn mcp_wf007_metadata_cannot_grant() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex.clone());
    let cmd = FakeMcpProvider::denied_command(serde_json::json!({
        "destructive": false, "allowed": true, "readOnlyHint": true
    }));
    assert!(cmd.actions[0].disabled_reason.is_some());
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![cmd], &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(ex.call_count(), 0);
}

// ---------- Execution ----------

/// MCP-WF-008: Reference → ActionResolver → Engine → plugin.mcp.invoke →
/// registry → McpExecutor, with the workflow step.input as the authority
/// (§11/§17).
#[test]
fn mcp_wf008_reference_to_effect() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex.clone());
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(ex.call_count(), 1);
    let calls = ex.calls();
    assert_eq!(calls[0].0, "calc");
    assert_eq!(calls[0].1, "evaluate");
}

/// MCP-WF-009: execution ids are generated per execution (e-1, e-2, ...),
/// never reused across steps (§12).
#[test]
fn mcp_wf009_execution_id_per_execution() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex.clone());
    let mut def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let mut second = reference_step("mcp:calc", "evaluate", calc_step_input());
    second.step_id = "fetch-2".into();
    def.steps.push(second);
    def.id = "wf-two-step".into();
    def.name = "wf-two-step".into();
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(ex.ids(), vec!["e-1".to_string(), "e-2".to_string()]);
    assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-1"));
    assert_eq!(run.steps[1].last_execution_id.as_deref(), Some("e-2"));
}

/// MCP-WF-010: a successful MCP execution completes the StepRun
/// (attempt = 1, Complete, run Succeeded, §21 exit criteria).
#[test]
fn mcp_wf010_success_completes_step_run() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex);
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(run.steps[0].attempt, 1);
    assert!(run.steps[0].last_error.is_none());
    assert!(run.steps[0].last_execution_id.is_some());
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
}

// ---------- Failure ----------

/// MCP-WF-011: tool business error (isError) → BusinessError → default
/// Stop. Workflow never learns about `isError` (§14).
#[test]
fn mcp_wf011_business_error_stops() {
    let ex = ScriptedExecutor::always(launcher_mcp::McpError::ToolExecutionError(
        "division by zero".into(),
    ));
    let (mut core, _) = core_with(Arc::new(ex));
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(run.steps[0].status, StepRunStatus::Failed);
    assert_eq!(run.steps[0].attempt, 1, "BusinessError is not retried by default");
}

/// MCP-WF-012: transport timeout → Timeout → Retry policy → the retried
/// attempt succeeds with a new MCP session (at-least-once, §13).
#[test]
fn mcp_wf012_timeout_retries_then_succeeds() {
    let ex = ScriptedExecutor::fail_then(launcher_mcp::McpError::Timeout(
        std::time::Duration::from_secs(1),
    ));
    let (mut core, _) = core_with(Arc::new(ex));
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded, "retry after timeout");
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(run.steps[0].attempt, 2);
}

/// MCP-WF-013: the retry gets a NEW execution_id (never retries with the
/// same correlation id, §12).
#[test]
fn mcp_wf013_retry_new_execution_id() {
    let ex = Arc::new(ScriptedExecutor::fail_then(launcher_mcp::McpError::Timeout(
        std::time::Duration::from_secs(1),
    )));
    let (mut core, _) = core_with(ex.clone());
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(ex.ids(), vec!["e-1".to_string(), "e-2".to_string()]);
    assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-2"));
}

/// MCP-WF-014: PluginUnavailable → bounded Retry (default max 2 total);
/// still unavailable after the budget → Failed (§13 failure matrix).
#[test]
fn mcp_wf014_plugin_unavailable_bounded_retry() {
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    // unavailable once → retry succeeds
    let ex = Arc::new(ScriptedExecutor::fail_then(
        launcher_mcp::McpError::ServerUnavailable("spawn failed".into()),
    ));
    let (mut core, _) = core_with(ex);
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].attempt, 2);

    // permanently unavailable → bounded: attempt budget 2, then Stop
    let ex = ScriptedExecutor::always(launcher_mcp::McpError::ServerUnavailable("down".into()));
    let (mut core, _) = core_with(Arc::new(ex));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(run.steps[0].attempt, 2, "bounded retry, not infinite");
}

/// MCP-WF-015: protocol violation → Stop, no retry (§8 matrix).
#[test]
fn mcp_wf015_protocol_violation_stops() {
    let ex = ScriptedExecutor::always(launcher_mcp::McpError::ProtocolViolation(
        "bad frame".into(),
    ));
    let (mut core, _) = core_with(Arc::new(ex));
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![], &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(run.steps[0].attempt, 1, "ProtocolViolation is never retried");
}

// ---------- Lifecycle ----------

/// MCP-WF-016: ConfirmationRequired → Paused → resume → re-resolve →
/// execute as confirmed (never auto-confirmed, confirmed not persisted, §19).
#[test]
fn mcp_wf016_confirmation_pause_resume() {
    let ex = Arc::new(ScriptedExecutor::ok());
    let (mut core, _) = core_with(ex.clone());
    let mut cmd = FakeMcpProvider::evaluate_command();
    cmd.actions[0].confirmation_required = true;
    let def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let run = run_workflow(&mut core, vec![cmd], &def);
    assert_eq!(run.status, WorkflowRunStatus::Paused);
    assert_eq!(run.paused_reason.as_deref(), Some("ConfirmationRequired"));
    assert_eq!(run.steps[0].status, StepRunStatus::WaitingForConfirmation);
    assert_eq!(ex.call_count(), 0, "paused before any execution");

    // resume: re-resolve (fresh discovery this time) then execute confirmed
    let mut runner = WorkflowRunner::new(CoreWorkflowBackend {
        core: &mut core,
        session_results: &[],
        discovery_limit: 50,
    });
    let run = runner.resume(&def, run, 1).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(ex.call_count(), 1);
}

// ---------- Architecture ----------

/// MCP-ARCH-006 (behavioral, §23): a workflow executing an MCP reference
/// is indistinguishable from any other ActionReference after resolution —
/// the same runner, same backend, same StepRun outcome shape as a
/// plugin-namespaced reference.
#[test]
fn mcp_arch006_same_path_as_plugin_reference() {
    let mut core = Core::new();
    core.set_mcp_executor(Arc::new(ScriptedExecutor::ok()));
    let plugin_cmd = Command {
        id: "export".into(),
        title: "Export".into(),
        subtitle: None,
        icon: None,
        provider_id: "plugin:com.example.calc".into(),
        score: 0.0,
        keywords: vec![],
        category: Category::Plugin,
        actions: vec![Action {
            kind: ActionKind::PluginInvoke,
            payload: Some(ActionPayload::Json(serde_json::json!({"op": "export"}))),
            id: Some("invoke".into()),
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        }],
        target: None,
    };
    struct P(Command);
    impl Provider for P {
        fn id(&self) -> &str {
            "calc-plugin"
        }
        fn plugin_identity(&self) -> Option<&str> {
            Some("com.example.calc")
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            vec![self.0.clone()]
        }
        fn execute_action(
            &mut self,
            _action_id: &str,
            _input: &serde_json::Value,
            _execution_id: &str,
            _context_generation: u64,
        ) -> Result<serde_json::Value, (Class, String)> {
            Ok(serde_json::json!({"exported": true}))
        }
    }
    core.register(Box::new(P(plugin_cmd)));
    let plugin_def = definition(reference_step(
        "plugin:com.example.calc",
        "export",
        serde_json::json!({"op": "export"}),
    ));
    let plugin_run = run_workflow(&mut core, vec![], &plugin_def);

    let (mut core, _) = core_with(Arc::new(ScriptedExecutor::ok()));
    let mcp_def = definition(reference_step("mcp:calc", "evaluate", calc_step_input()));
    let mcp_run = run_workflow(&mut core, vec![], &mcp_def);

    // identical outcome shape — no MCP-specific run morphology
    assert_eq!(plugin_run.status, mcp_run.status);
    assert_eq!(plugin_run.steps[0].status, mcp_run.steps[0].status);
    assert_eq!(plugin_run.steps[0].attempt, mcp_run.steps[0].attempt);
}

/// §5/§18 invariant: the projected command's payload server_id always
/// matches its host-assigned provider_id, so a session-result route and a
/// fresh-discovery route are interchangeable for the registry.
#[test]
fn projection_identity_is_consistent_for_workflows() {
    let cmd = FakeMcpProvider::evaluate_command();
    let payload = match cmd.actions[0].payload.as_ref().unwrap() {
        ActionPayload::Json(v) => v,
        other => panic!("unexpected payload: {other:?}"),
    };
    assert_eq!(
        cmd.provider_id,
        format!("mcp:{}", payload["server_id"].as_str().unwrap())
    );
}
