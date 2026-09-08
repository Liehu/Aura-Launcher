//! P1-C Workflow v0.2 tests (review 58 §41/§54/§67): variables, templates,
//! conditions, branching, graph validation and the security matrix
//! (WF-SEC-001..010). Deterministic scripted host — no real processes.

use std::sync::{Arc, Mutex};

use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{
    Action, ActionKind, ActionPayload, Category, Command, Condition, OutputBinding, StepRunStatus,
    WorkflowAction, WorkflowDefinition, WorkflowRunStatus, WorkflowStep,
};
use launcher_workflow::{ActionExecutor, CommandSource, ExecutionOutcome, WfFailure, WorkflowRunner};

// ---------- fake host ----------

type Calls = Arc<Mutex<Vec<(String, serde_json::Value)>>>;

#[derive(Default, Clone)]
struct FakeSource {
    session: Vec<Command>,
}

impl CommandSource for FakeSource {
    fn in_session(&self, p: &str, c: &str) -> Option<Command> {
        self.session
            .iter()
            .find(|cmd| cmd.provider_id == p && cmd.id == c)
            .cloned()
    }
    fn fresh_query(&mut self, p: &str) -> Result<Vec<Command>, (Class, String)> {
        Ok(self
            .session
            .iter()
            .filter(|cmd| cmd.provider_id == p)
            .cloned()
            .collect())
    }
}

struct FakeExecutor {
    calls: Calls,
    fail_ids: Vec<String>,
}


impl FakeExecutor {
    fn new(fail_ids: Vec<String>) -> (Self, Calls) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (Self { calls: calls.clone(), fail_ids }, calls)
    }
}

impl ActionExecutor for FakeExecutor {
    fn execute(
        &mut self,
        action: Action,
        _p: Option<String>,
        _g: u64,
        _confirmed: bool,
    ) -> Result<ExecutionOutcome, WfFailure> {
        let id = action.id.clone().unwrap_or_default();
        let payload = action.payload.clone().map(|p| match p {
            ActionPayload::Json(v) => v,
            _ => serde_json::Value::Null,
        }).unwrap_or(serde_json::Value::Null);
        self.calls.lock().unwrap().push((id.clone(), payload));
        if self.fail_ids.contains(&id) {
            return Err(WfFailure::new(Class::BusinessError, "scripted failure"));
        }
        // deterministic output: {"value": <len(id)>}
        Ok(ExecutionOutcome {
            execution_id: Some(format!("e-{}", id.len())),
            plugin_result: Some(serde_json::json!({ "value": id.len() })),
        })
    }
}

struct TestHost {
    source: FakeSource,
    executor: FakeExecutor,
}

impl CommandSource for TestHost {
    fn in_session(&self, p: &str, c: &str) -> Option<Command> {
        self.source.in_session(p, c)
    }
    fn fresh_query(&mut self, p: &str) -> Result<Vec<Command>, (Class, String)> {
        self.source.fresh_query(p)
    }
}
impl ActionExecutor for TestHost {
    fn execute(
        &mut self,
        a: Action,
        p: Option<String>,
        g: u64,
        confirmed: bool,
    ) -> Result<ExecutionOutcome, WfFailure> {
        self.executor.execute(a, p, g, confirmed)
    }
}

// ---------- builders ----------

fn action_ref(command: &str) -> WorkflowAction {
    WorkflowAction::Reference(launcher_domain::ActionReference {
        provider_id: "plugin:calc".into(),
        command_id: command.into(),
        // action id mirrors the command id so scripted failure matching and
        // the runner's identity checks stay aligned
        action_id: command.into(),
    })
}

#[allow(clippy::too_many_arguments)]
fn ref_step(
    id: &str,
    command: &str,
    condition: Option<&str>,
    output: Option<(&str, &str)>,
    on_success: Option<&str>,
    on_failure: Option<&str>,
    on_condition_false: Option<&str>,
) -> WorkflowStep {
    WorkflowStep {
        step_id: id.into(),
        action: action_ref(command),
        input: serde_json::json!({"arguments": {}}),
        failure_policy: Default::default(),
        condition: condition.map(|c| Condition::parse(c).unwrap()),
        output: output.map(|(t, s)| OutputBinding {
            target: t.into(),
            source: s.into(),
        }),
        on_success: on_success.map(str::to_string),
        on_failure: on_failure.map(str::to_string),
        on_condition_false: on_condition_false.map(str::to_string),
    }
}

fn run_workflow(host: TestHost, def: &WorkflowDefinition) -> launcher_domain::WorkflowRun {
    let mut runner = WorkflowRunner::new(host);
    runner.run(def, "wr-p1c".into(), 1).expect("definition valid")
}

fn seeded_source(ids: &[&str]) -> FakeSource {
    FakeSource {
        session: ids
            .iter()
            .map(|id| Command {
                id: id.to_string(),
                title: id.to_string(),
                subtitle: None,
                icon: None,
                provider_id: "plugin:calc".into(),
                score: 0.0,
                keywords: vec![],
                category: Category::Plugin,
                actions: vec![Action {
                    kind: ActionKind::PluginInvoke,
                    payload: Some(ActionPayload::Json(serde_json::json!({}))),
                    id: Some(id.to_string()),
                    title: Some(id.to_string()),
                    disabled_reason: None,
                    shortcut: None,
                    confirmation_required: false,
                }],
                target: None,
            })
            .collect(),
    }
}

// ---------- Branching (SS24-SS25) ----------

/// Branching: A (action, binds var.n from output) → B (cond var.n > 1) →
/// C (cond var.n > 100). B executes; C is skipped (BranchNotSelected).
#[test]
fn p1c_branch_selects_exactly_one_path() {
    let _ex = FakeExecutor::new(vec![]);
    let def = WorkflowDefinition {
        id: "wf-branch".into(),
        version: 2,
        name: "branch".into(),
        steps: vec![
            ref_step("a", "cmd_a", None, Some(("var.n", "value")), Some("b"), None, None),
            ref_step("b", "cmd_b", Some("var.n > 1"), None, None, None, None),
            ref_step("c", "cmd_c", Some("var.n > 100"), None, None, None, None),
        ],
        failure_policy: Default::default(),
        entry_step: None,
        variables: vec![launcher_domain::VariableDeclaration {
            name: "n".into(),
            value_type: "number".into(),
            default: Some(serde_json::json!(0)),
        }],
        inputs: Vec::new(),
    };
    let run = run_workflow(TestHost { source: seeded_source(&["cmd_a", "cmd_b", "cmd_c"]), executor: FakeExecutor::new(vec![]).0 }, &def);
    println!("BRANCHDBG status={:?} errs={:?}", run.status, run.steps.iter().map(|s| (&s.step_id, &s.status, &s.last_error, &s.output)).collect::<Vec<_>>());
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(run.steps[1].status, StepRunStatus::Complete);
    assert_eq!(run.steps[2].status, StepRunStatus::Skipped);
    // C's own condition var.n > 100 is false: ConditionFalse is the
    // accurate reason (it was evaluated and rejected)
    assert_eq!(
        run.steps[2].skip_reason.as_deref(),
        Some(launcher_domain::workflow::SKIP_CONDITION_FALSE)
    );
}

/// Condition FALSE with on_condition_false goto: control diverts to the
/// false-branch step (which executes); the true branch is never selected.
#[test]
fn p1c_condition_false_goto_executes_false_branch() {
    let (ex, calls) = FakeExecutor::new(vec![]);
    // a binds var.n = 5 (len "cmd_a"); gate's condition var.n > 100 is
    // FALSE -> goto "c"; the true branch "b" is never selected.
    let def = WorkflowDefinition {
        id: "wf-false-goto".into(),
        version: 2,
        name: "false-goto".into(),
        steps: vec![
            ref_step("a", "cmd_a", None, Some(("var.n", "value")), None, None, None),
            ref_step("gate", "cmd_gate", Some("var.n > 100"), None, Some("b"), None, Some("c")),
            ref_step("b", "cmd_b", None, None, None, None, None),
            ref_step("c", "cmd_c", None, None, None, None, None),
        ],
        failure_policy: Default::default(),
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    };
    let run = run_workflow(TestHost { source: seeded_source(&["cmd_a", "cmd_gate", "cmd_b", "cmd_c"]), executor: ex }, &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    let executed: Vec<String> = calls
        .lock()
        .unwrap()
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(executed, vec!["cmd_a".to_string(), "cmd_c".to_string()]);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(run.steps[1].status, StepRunStatus::Skipped);
    assert_eq!(
        run.steps[1].skip_reason.as_deref(),
        Some(launcher_domain::workflow::SKIP_CONDITION_FALSE)
    );
    assert_eq!(run.steps[2].status, StepRunStatus::Skipped);
    assert_eq!(run.steps[3].status, StepRunStatus::Complete);
}

/// Whole-field templates restore the referenced TYPE; embedded templates
/// interpolate as text; unknown refs are hard errors.
#[test]
fn wf_template_type_rules() {
    let store = launcher_workflow::v2::VariableStore::from_snapshot(Some(
        &serde_json::json!({"var": {"n": 42, "name": "backup"}}),
    ));
    let v = launcher_workflow::v2::materialize_input(
        &serde_json::json!("${var.n}"),
        &store,
    )
    .unwrap();
    assert_eq!(v.as_f64(), Some(42.0), "whole-field restores type");
    let v = launcher_workflow::v2::materialize_input(
        &serde_json::json!("backup-${var.name}"),
        &store,
    )
    .unwrap();
    assert_eq!(v, serde_json::json!("backup-backup"));
    assert!(launcher_workflow::v2::materialize_input(
        &serde_json::json!("${var.unknown}"),
        &store,
    )
    .is_err());
}

// ---------- Security ----------

/// WF-SEC-001/002: a variable template can never rebind canonical
/// identity (server_id/tool_name) — rejected at materialization.
#[test]
fn wf_sec001_002_identity_templates_rejected() {
    let store = launcher_workflow::v2::VariableStore::from_snapshot(Some(
        &serde_json::json!({"var": {"server": "evil", "tool": "delete_all"}}),
    ));
    for input in [
        serde_json::json!({"server_id": "${var.server}", "arguments": {}}),
        serde_json::json!({"tool_name": "${var.tool}", "arguments": {}}),
    ] {
        assert!(
            launcher_workflow::v2::materialize_input(&input, &store).is_err(),
            "identity template must be rejected: {input}"
        );
    }
}

/// WF-SEC-003: a variable can never construct an Effect — the proposal
/// type remains the frozen four-field shape regardless of content.
#[test]
fn wf_sec003_variable_cannot_create_effect() {
    let p = launcher_workflow::proposal::ActionProposal::from_json(&serde_json::json!({
        "provider_id": "mcp:calc", "command_id": "evaluate", "action_id": "invoke",
        "input": {"effect": "delete_everything"}
    }))
    .unwrap();
    let keys: Vec<String> = serde_json::to_value(&p)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, vec!["action_id", "command_id", "input", "provider_id"]);
}

/// WF-SEC-004: the condition evaluator is pure and strictly typed (§13).
#[test]
fn wf_sec004_condition_evaluator_pure_typed() {
    use launcher_domain::{evaluate, parse_expr, Value};
    let expr = parse_expr("var.x == \"yes\" and var.y > 10").unwrap();
    let r = evaluate(&expr, &|p| match p {
        "var.x" => Some(Value::String("yes".into())),
        "var.y" => Some(Value::Number(42.0)),
        _ => None,
    })
    .unwrap();
    assert_eq!(r, Value::Bool(true));
    // cross-type equality = false (no coercion)
    let eq = parse_expr("var.s == 10").unwrap();
    let r = evaluate(&eq, &|p| {
        if p == "var.s" {
            Some(Value::String("10".into()))
        } else {
            None
        }
    })
    .unwrap();
    assert_eq!(r, Value::Bool(false));
    // cross-type ordering = TypeMismatch
    let ord = parse_expr("var.s > 5").unwrap();
    assert!(evaluate(&ord, &|p| {
        if p == "var.s" {
            Some(Value::String("abc".into()))
        } else {
            None
        }
    })
    .is_err());
}

/// §59: expression depth is bounded.
#[test]
fn wf_limits_expression_depth_bounded() {
    let deep = format!("not {}", "not ".repeat(200) + "true");
    assert!(launcher_domain::parse_expr(&deep).is_err());
}

// ---------- Failure branch gated by FailurePolicy (SS32/SS33) ----------

fn stop_base() -> WorkflowDefinition {
    WorkflowDefinition {
        id: "wf-sec6".into(),
        version: 2,
        name: "wf-sec6".into(),
        steps: vec![
            ref_step("a", "failing", None, None, None, Some("cleanup"), None),
            ref_step("cleanup", "after", None, None, None, None, None),
        ],
        failure_policy: Default::default(), // BusinessError → Stop
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    }
}

/// WF-SEC-006: Stop policy — on_failure must NOT divert; cleanup never runs.
#[test]
fn wf_sec006_stop_policy_blocks_branch() {
    let (ex, calls) = FakeExecutor::new(vec!["failing".into()]);
    let run = run_workflow(TestHost { source: seeded_source(&["failing", "after"]), executor: ex }, &stop_base());
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(run.steps[0].status, StepRunStatus::Failed);
    // a Stop-class failure short-circuits before any branch/skip pass
    assert_eq!(run.steps[1].status, StepRunStatus::Pending);
    let executed: Vec<String> = calls.lock().unwrap().iter().map(|(id, _)| id.clone()).collect();
    assert!(!executed.contains(&"after".to_string()));
}

/// WF-SEC-006b: Skip policy permits the failure branch — cleanup executes
/// and the run succeeds.
#[test]
fn wf_sec006b_skip_policy_permits_branch() {
    use launcher_domain::{FailureAction, StepFailurePolicy};
    let mut def = stop_base();
    def.steps[0].failure_policy = StepFailurePolicy {
        business_error: Some(FailureAction::Skip),
        ..Default::default()
    };
    let (ex, calls) = FakeExecutor::new(vec!["failing".into()]);
    let run = run_workflow(TestHost { source: seeded_source(&["failing", "after"]), executor: ex }, &def);
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Skipped);
    let executed: Vec<String> = calls.lock().unwrap().iter().map(|(id, _)| id.clone()).collect();
    assert!(executed.contains(&"after".to_string()));
}

/// WF-SEC-009: a failed output binding commits NOTHING and fails the run.
#[test]
fn wf_sec009_failed_binding_commits_nothing() {
    let (ex, calls) = FakeExecutor::new(vec![]);
    let def = WorkflowDefinition {
        id: "wf-bind".into(),
        version: 2,
        name: "wf-bind".into(),
        steps: vec![ref_step(
            "a",
            "evaluate",
            None,
            Some(("var.result", "missing.path")),
            None,
            None,
            None,
        )],
        failure_policy: Default::default(),
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    };
    let run = run_workflow(TestHost { source: seeded_source(&["evaluate"]), executor: ex }, &def);
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert!(run.steps[0]
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("output path not found"));
    assert!(run.variables.is_none(), "no partial variable commit");
    assert_eq!(calls.lock().unwrap().len(), 1, "action did execute once");
}

// ---------- Graph validation (SS26-SS30) ----------

/// WF-SEC-010: a cyclic workflow is rejected by the validator BEFORE
/// execution.
#[test]
fn wf_sec010_cycle_rejected_before_execution() {
    let def = WorkflowDefinition {
        id: "wf-cycle".into(),
        version: 2,
        name: "cycle".into(),
        steps: vec![
            ref_step("a", "cmd_a", None, None, Some("b"), None, None),
            ref_step("b", "cmd_b", None, None, Some("a"), None, None),
        ],
        failure_policy: Default::default(),
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    };
    assert!(launcher_workflow::v2::validate_graph(&def).is_err());
    struct NeverHost;
    impl CommandSource for NeverHost {
        fn in_session(&self, _: &str, _: &str) -> Option<Command> {
            None
        }
        fn fresh_query(&mut self, _: &str) -> Result<Vec<Command>, (Class, String)> {
            Ok(Vec::new())
        }
    }
    impl ActionExecutor for NeverHost {
        fn execute(&mut self, _: Action, _: Option<String>, _: u64, _: bool) -> Result<ExecutionOutcome, WfFailure> {
            panic!("cycle must never execute")
        }
    }
    let mut runner = WorkflowRunner::new(NeverHost);
    assert!(runner.run(&def, "wr".into(), 1).is_err());
}

/// §29: unreachable steps are definition-invalid.
#[test]
fn unreachable_step_rejected() {
    let def = WorkflowDefinition {
        id: "wf-unreach".into(),
        version: 2,
        name: "unreach".into(),
        steps: vec![
            ref_step("a", "cmd_a", None, None, Some("b"), None, None),
            ref_step("b", "cmd_b", None, None, Some("end"), None, None),
            ref_step("c", "cmd_c", None, None, None, None, None), // no incoming
        ],
        failure_policy: Default::default(),
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    };
    let errs = launcher_workflow::v2::validate_graph(&def).unwrap_err();
    assert!(errs.iter().any(|e| matches!(
        e,
        launcher_workflow::v2::DefinitionError::UnreachableStep(s) if s == "c"
    )));
}

/// §28: unknown goto target is definition-invalid.
#[test]
fn unknown_goto_target_rejected() {
    let def = WorkflowDefinition {
        id: "wf-goto".into(),
        version: 2,
        name: "goto".into(),
        steps: vec![ref_step("a", "cmd_a", None, None, Some("nowhere"), None, None)],
        failure_policy: Default::default(),
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    };
    let errs = launcher_workflow::v2::validate_graph(&def).unwrap_err();
    assert!(errs.iter().any(|e| matches!(
        e,
        launcher_workflow::v2::DefinitionError::UnknownStep(_)
    )));
}
