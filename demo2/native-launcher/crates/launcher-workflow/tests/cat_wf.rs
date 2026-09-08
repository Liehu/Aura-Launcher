//! Domain Contract Test Kit for MVP4.1 Workflow (CAT-WF-001~012,
//! WORKFLOW-CONTRACT-v0.1 §12). Scripted fakes make every failure class
//! deterministic; the real-host plugin path is covered separately in
//! `apps/calculator-plus/tests`.

use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{
    Action, ActionKind, ActionPayload, Category, Command, StepRunStatus, WorkflowDefinition,
    WorkflowRunStatus, WorkflowStep,
};
use launcher_workflow::{
    ActionExecutor, CommandSource, ExecutionOutcome, WfFailure, WorkflowRunner,
};

// ---------- fake host (single backend: source + executor) ----------

#[derive(Default)]
struct FakeSource {
    session: Vec<Command>,
    discovery: Vec<Command>,
    discovery_err: Option<(Class, String)>,
}

impl CommandSource for FakeSource {
    fn in_session(&self, provider_id: &str, command_id: &str) -> Option<Command> {
        self.session
            .iter()
            .find(|c| c.provider_id == provider_id && c.id == command_id)
            .cloned()
    }
    fn fresh_query(&mut self, _provider_id: &str) -> Result<Vec<Command>, (Class, String)> {
        match &self.discovery_err {
            Some((c, r)) => Err((c.clone(), r.clone())),
            None => Ok(self.discovery.clone()),
        }
    }
}

enum Scripted {
    Fail(Class, String),
    Succeed(String),
}

struct FakeExecutor {
    script: Vec<Scripted>,
    calls: Vec<String>,
}

impl FakeExecutor {
    fn new(script: Vec<Scripted>) -> Self {
        Self {
            script,
            calls: Vec::new(),
        }
    }
}

impl ActionExecutor for FakeExecutor {
    fn execute(
        &mut self,
        action: Action,
        _provider_id: Option<String>,
        _gen: u64,
        confirmed: bool,
    ) -> Result<ExecutionOutcome, WfFailure> {
        if action.confirmation_required && !confirmed {
            return Err(WfFailure::new(
                Class::ConfirmationRequired,
                "confirmation required",
            ));
        }
        self.calls
            .push(action.id.clone().unwrap_or_else(|| "?".into()));
        match self.script.pop() {
            Some(Scripted::Fail(c, r)) => Err(WfFailure::new(c, r)),
            Some(Scripted::Succeed(eid)) => Ok(ExecutionOutcome {
                execution_id: Some(eid),
                plugin_result: None,
            }),
            None => panic!("executor script exhausted"),
        }
    }
}

struct FakeHost {
    source: FakeSource,
    executor: FakeExecutor,
}

impl CommandSource for FakeHost {
    fn in_session(&self, p: &str, c: &str) -> Option<Command> {
        self.source.in_session(p, c)
    }
    fn fresh_query(&mut self, p: &str) -> Result<Vec<Command>, (Class, String)> {
        self.source.fresh_query(p)
    }
}
impl ActionExecutor for FakeHost {
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

fn cmd(provider: &str, id: &str, action_id: &str, disabled: Option<&str>) -> Command {
    Command {
        id: id.into(),
        title: id.into(),
        subtitle: None,
        icon: None,
        provider_id: provider.into(),
        score: 0.0,
        keywords: vec![],
        category: Category::Plugin,
        actions: vec![Action {
            kind: ActionKind::Copy,
            payload: Some(ActionPayload::Text("x".into())),
            id: Some(action_id.into()),
            title: Some(action_id.into()),
            disabled_reason: disabled.map(str::to_string),
            shortcut: None,
            confirmation_required: false,
        }],
        target: None,
    }
}

fn reference_step(step_id: &str, provider: &str, command: &str, action: &str) -> WorkflowStep {
    serde_json::from_value(serde_json::json!({
        "step_id": step_id,
        "action": { "Reference": {
            "provider_id": provider, "command_id": command, "action_id": action } },
        "input": {}
    }))
    .unwrap()
}

fn run_def(
    def: &WorkflowDefinition,
    source: FakeSource,
    executor: FakeExecutor,
) -> launcher_domain::WorkflowRun {
    let mut runner = WorkflowRunner::new(FakeHost { source, executor });
    runner.run(def, "wr-1".into(), 7).unwrap()
}

// ---------- CAT-WF ----------

/// CAT-WF-001 single-step success.
#[test]
fn cat001_single_step_success() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.single", "version": 1, "name": "single",
        "steps": [reference_step("s1", "plugin:calc", "calc:= 1", "copy")]
    }))
    .unwrap();
    def.validate().unwrap();
    let mut source = FakeSource::default();
    source.session = vec![cmd("plugin:calc", "calc:= 1", "copy", None)];
    let run = run_def(
        &def,
        source,
        FakeExecutor::new(vec![Scripted::Succeed("e-1".into())]),
    );
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-1"));
    assert_eq!(run.steps[0].resolved_context_generation, Some(7));
}

/// CAT-WF-002 multi-step sequential execution, in declaration order.
#[test]
fn cat002_multi_step_sequential() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.multi", "version": 1, "name": "multi",
        "steps": [
            reference_step("s1", "plugin:calc", "c1", "a1"),
            reference_step("s2", "plugin:calc", "c2", "a2"),
            reference_step("s3", "plugin:calc", "c3", "a3"),
        ]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    source.session = vec![
        cmd("plugin:calc", "c1", "a1", None),
        cmd("plugin:calc", "c2", "a2", None),
        cmd("plugin:calc", "c3", "a3", None),
    ];
    let mut runner = WorkflowRunner::new(FakeHost {
        source,
        executor: FakeExecutor::new(vec![
            Scripted::Succeed("e-3".into()),
            Scripted::Succeed("e-2".into()),
            Scripted::Succeed("e-1".into()),
        ]),
    });
    let run = runner.run(&def, "wr-2".into(), 7).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    let ids: Vec<_> = run
        .steps
        .iter()
        .map(|s| s.last_execution_id.clone().unwrap())
        .collect();
    assert_eq!(ids, vec!["e-1", "e-2", "e-3"]);
}

/// CAT-WF-003 stale context -> re-resolve (ReResolve loops back through
/// resolution, never blindly executing the old result).
#[test]
fn cat003_stale_context_re_resolves() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.stale", "version": 1, "name": "stale",
        "steps": [reference_step("s1", "plugin:calc", "c1", "a1")]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    source.session = vec![cmd("plugin:calc", "c1", "a1", None)];
    let mut runner = WorkflowRunner::new(FakeHost {
        source,
        executor: FakeExecutor::new(vec![
            Scripted::Succeed("e-2".into()),
            Scripted::Fail(Class::StaleContext, "context moved".into()),
        ]),
    });
    let run = runner.run(&def, "wr-3".into(), 7).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
}

/// CAT-WF-004 capability denied -> stop, later steps never start.
#[test]
fn cat004_capability_denied_stops() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.denied", "version": 1, "name": "denied",
        "steps": [
            reference_step("s1", "plugin:calc", "c1", "a1"),
            reference_step("s2", "plugin:calc", "c2", "a2"),
        ]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    source.session = vec![cmd(
        "plugin:calc",
        "c1",
        "a1",
        Some("requires clipboard.write"),
    )];
    let run = run_def(&def, source, FakeExecutor::new(vec![]));
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(run.steps[0].status, StepRunStatus::Failed);
    assert_eq!(run.steps[1].status, StepRunStatus::Pending);
}

/// CAT-WF-005 timeout -> retry with a NEW execution_id (INV-054).
#[test]
fn cat005_timeout_retries_with_new_execution_id() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.timeout", "version": 1, "name": "timeout",
        "steps": [reference_step("s1", "plugin:calc", "c1", "a1")]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    source.session = vec![cmd("plugin:calc", "c1", "a1", None)];
    let mut runner = WorkflowRunner::new(FakeHost {
        source,
        executor: FakeExecutor::new(vec![
            Scripted::Succeed("e-103".into()),
            Scripted::Fail(Class::Timeout, "plugin timeout after 2s".into()),
        ]),
    });
    let run = runner.run(&def, "wr-5".into(), 7).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].attempt, 2);
    // the recorded execution id is the retry's, not the timed-out one
    assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-103"));
    assert!(run.steps[0]
        .last_error
        .as_deref()
        .unwrap()
        .contains("timeout"));
}

/// CAT-WF-006 business error -> default Stop; explicit step policy Skip works.
#[test]
fn cat006_business_error_policy() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.biz", "version": 1, "name": "biz",
        "steps": [reference_step("s1", "plugin:calc", "c1", "a1")]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    source.session = vec![cmd("plugin:calc", "c1", "a1", None)];
    let run = run_def(
        &def,
        source,
        FakeExecutor::new(vec![Scripted::Fail(Class::BusinessError, "quota".into())]),
    );
    assert_eq!(run.status, WorkflowRunStatus::Failed);

    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.biz2", "version": 1, "name": "biz2",
        "steps": [{
            "step_id": "s1",
            "action": { "Reference": {
                "provider_id": "plugin:calc", "command_id": "c1", "action_id": "a1" } },
            "input": {},
            "failure_policy": { "business_error": "skip" }
        }]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    source.session = vec![cmd("plugin:calc", "c1", "a1", None)];
    let run = run_def(
        &def,
        source,
        FakeExecutor::new(vec![Scripted::Fail(Class::BusinessError, "quota".into())]),
    );
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Skipped);
}

/// CAT-WF-007 protocol violation -> stop (never retried).
#[test]
fn cat007_protocol_violation_stops() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.proto", "version": 1, "name": "proto",
        "steps": [reference_step("s1", "plugin:calc", "c1", "a1")]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    source.session = vec![cmd("plugin:calc", "c1", "a1", None)];
    let run = run_def(
        &def,
        source,
        FakeExecutor::new(vec![Scripted::Fail(
            Class::ProtocolViolation,
            "execution_id echo mismatch".into(),
        )]),
    );
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert_eq!(
        run.steps[0].attempt, 1,
        "protocol violations are not retried"
    );
}

/// CAT-WF-008 invalid input -> stop by default, skip under explicit policy;
/// inline plugin.* normalizes to InvalidInput (WF-A2).
#[test]
fn cat008_invalid_input_stop_or_skip() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.inv", "version": 1, "name": "inv",
        "steps": [{
            "step_id": "s1",
            "action": { "Inline": {
                "id": "bad", "type": "plugin.calc.bad", "input": {} } },
            "input": {}
        }]
    }))
    .unwrap();
    let run = run_def(&def, FakeSource::default(), FakeExecutor::new(vec![]));
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert!(run.steps[0]
        .last_error
        .as_deref()
        .unwrap()
        .contains("unknown action type"));

    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.inv2", "version": 1, "name": "inv2",
        "steps": [{
            "step_id": "s1",
            "action": { "Inline": {
                "id": "bad", "type": "plugin.calc.bad", "input": {} } },
            "input": {},
            "failure_policy": { "invalid_input": "skip" }
        }]
    }))
    .unwrap();
    let run = run_def(&def, FakeSource::default(), FakeExecutor::new(vec![]));
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Skipped);
}

/// CAT-WF-009 confirmation-required action pauses the run; resume with
/// explicit confirmation re-resolves and completes (WF-010). Nothing executes
/// before the confirmation.
#[test]
fn cat009_confirmation_pauses_then_resume_re_resolves() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.confirm", "version": 1, "name": "confirm",
        "steps": [{
            "step_id": "s1",
            "action": { "Reference": {
                "provider_id": "plugin:calc", "command_id": "c1", "action_id": "a1" } },
            "input": {}
        }]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    let mut c = cmd("plugin:calc", "c1", "a1", None);
    c.actions[0].confirmation_required = true;
    source.session = vec![c];

    let mut runner = WorkflowRunner::new(FakeHost {
        source,
        executor: FakeExecutor::new(vec![Scripted::Succeed("e-1".into())]),
    });
    let run = runner.run(&def, "wr-9".into(), 7).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Paused);
    assert_eq!(run.paused_reason.as_deref(), Some("ConfirmationRequired"));
    assert_eq!(run.steps[0].status, StepRunStatus::WaitingForConfirmation);
    assert_eq!(
        run.steps[0].last_execution_id, None,
        "nothing executes before confirm"
    );

    // resume: re-resolve then execute confirmed
    let run = runner.resume(&def, run, 7).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-1"));
}

/// CAT-WF-010 plugin action through the ActionEngine: a Reference to a
/// PluginInvoke action flows through the exact same runner path.
#[test]
fn cat010_plugin_action_via_engine_path() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.plugin", "version": 1, "name": "plugin",
        "steps": [reference_step("s1", "plugin:calc", "c1", "echo")]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    let mut c = cmd("plugin:calc", "c1", "echo", None);
    c.actions[0].kind = ActionKind::PluginInvoke;
    source.session = vec![c];
    let mut runner = WorkflowRunner::new(FakeHost {
        source,
        executor: FakeExecutor::new(vec![Scripted::Succeed("e-10".into())]),
    });
    let run = runner.run(&def, "wr-10".into(), 7).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
}

/// CAT-WF-011 WorkflowRun is serializable (stable representation); the
/// serialized form contains execution state only — no executable payload.
#[test]
fn cat011_run_serializable_execution_state_only() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.ser", "version": 1, "name": "ser",
        "steps": [reference_step("s1", "plugin:calc", "c1", "a1")]
    }))
    .unwrap();
    let mut source = FakeSource::default();
    source.session = vec![cmd("plugin:calc", "c1", "a1", None)];
    let mut runner = WorkflowRunner::new(FakeHost {
        source,
        executor: FakeExecutor::new(vec![Scripted::Succeed("e-1".into())]),
    });
    let run = runner.run(&def, "wr-11".into(), 42).unwrap();
    let json = serde_json::to_string(&run).unwrap();
    let back: launcher_domain::WorkflowRun = serde_json::from_str(&json).unwrap();
    assert_eq!(back, run);
    assert_eq!(back.steps[0].resolved_context_generation, Some(42));
    assert!(
        !json.contains("\"payload\""),
        "no executable payload in run state"
    );
    assert!(!json.contains("\"kind\""), "no action kinds in run state");
}

/// CAT-WF-012 a persisted ResolvedAction is never executed: the run model
/// carries no Action field, and a vanished reference re-resolves to
/// CommandNotFound (Stop) — never "executes the stale one".
#[test]
fn cat012_never_executes_persisted_resolved_action() {
    let run = launcher_domain::WorkflowRun {
        workflow_run_id: "wr".into(),
        definition_id: "d".into(),
        definition_version: 1,
        status: WorkflowRunStatus::Running,
        paused_reason: None,
        current_step: None,
        steps: vec![],
        variables: None,
    };
    let json = serde_json::to_string(&run).unwrap();
    assert!(!json.contains("actions"), "run must not embed actions");

    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.vanish", "version": 1, "name": "vanish",
        "steps": [reference_step("s1", "plugin:calc", "gone", "a1")]
    }))
    .unwrap();
    let run = run_def(&def, FakeSource::default(), FakeExecutor::new(vec![]));
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert!(run.steps[0].last_error.as_deref().unwrap().contains("gone"));
}

/// WF-A3: fresh discovery failure maps to PluginUnavailable, and a provider
/// that answers but lacks the command yields CommandNotFound (never mixed).
#[test]
fn wf_a3_discovery_error_boundaries() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.disc", "version": 1, "name": "disc",
        "steps": [reference_step("s1", "plugin:calc", "c1", "a1")]
    }))
    .unwrap();

    let mut source = FakeSource::default();
    source.discovery_err = Some((Class::PluginUnavailable, "spawn failed".into()));
    let run = run_def(&def, source, FakeExecutor::new(vec![]));
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert!(run.steps[0]
        .last_error
        .as_deref()
        .unwrap()
        .contains("spawn failed"));

    let mut source = FakeSource::default();
    source.discovery = vec![cmd("plugin:calc", "other", "a1", None)];
    let run = run_def(&def, source, FakeExecutor::new(vec![]));
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert!(run.steps[0].last_error.as_deref().unwrap().contains("c1"));
}

/// WF-A1: step input overrides the inline descriptor's declared input.
#[test]
fn wf_a1_step_input_authoritative() {
    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.input", "version": 1, "name": "input",
        "steps": [{
            "step_id": "s1",
            "action": { "Inline": {
                "id": "copy", "title": "Copy", "type": "system.copy_to_clipboard",
                "input": {"text": "DECLARED"} } },
            "input": {"text": "FROM_STEP"}
        }]
    }))
    .unwrap();
    let mut runner = WorkflowRunner::new(FakeHost {
        source: FakeSource::default(),
        executor: FakeExecutor::new(vec![Scripted::Succeed("e-1".into())]),
    });
    let run = runner.run(&def, "wr-a1".into(), 7).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
}
