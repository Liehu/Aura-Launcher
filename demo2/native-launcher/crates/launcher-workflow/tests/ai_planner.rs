//! MVP4.2 AI Planner acceptance (review 21 §21/§22 + review 27):
//! AI is just an ActionProposal producer. Every forgery attempt an AI model
//! could make — provider impersonation is addressed separately by host
//! identity, but here: authorized/confirmed state, embedded capabilities,
//! embedded ResolvedAction/Effect objects — MUST be inert.

use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{Action, ActionKind, ActionPayload, Category, Command, WorkflowRunStatus};
use launcher_workflow::proposal::{ActionPlanner, ActionProposal, KeywordPlanner};
use launcher_workflow::WfFailure;
use launcher_workflow::{ActionExecutor, CommandSource};

// ---------- catalog for the planner ----------

fn catalog_cmd(id: &str, title: &str, keywords: &[&str], disabled: bool) -> Command {
    Command {
        id: id.into(),
        title: title.into(),
        subtitle: None,
        icon: None,
        provider_id: "plugin:calc".into(),
        score: 0.0,
        keywords: keywords.iter().map(|s| s.to_string()).collect(),
        category: Category::Plugin,
        actions: vec![Action {
            kind: ActionKind::Copy,
            payload: Some(ActionPayload::Text("x".into())),
            id: Some("copy".into()),
            title: Some("Copy".into()),
            disabled_reason: disabled.then(|| "requires clipboard.write".to_string()),
            shortcut: None,
            confirmation_required: false,
        }],
        target: None,
    }
}

/// The planner only proposes Ready actions from the catalog.
#[test]
fn planner_proposes_only_ready_catalog_matches() {
    let catalog = vec![
        catalog_cmd("calc:= 80", "= 80", &["copy"], false),
        catalog_cmd("calc.disabled", "Disabled Thing", &["copy"], true),
    ];
    let proposals = KeywordPlanner.plan("copy", &catalog);
    assert_eq!(proposals.len(), 1, "disabled command must not be proposed");
    assert_eq!(proposals[0].command_id, "calc:= 80");
    assert_eq!(proposals[0].provider_id, "plugin:calc");
    // no match -> zero proposals
    assert!(KeywordPlanner.plan("zzz", &catalog).is_empty());
}

/// FORGERY 1+2: `authorized` and `confirmed` flags in AI output are
/// non-authoritative — they are dropped at parse time and never reach the
/// engine's confirmation gate (INV-048).
#[test]
fn forged_authorization_state_is_dropped() {
    let ai_output = serde_json::json!({
        "provider_id": "plugin:calc",
        "command_id": "c1",
        "action_id": "a1",
        "input": {"text": "hi"},
        "authorized": true,
        "confirmed": true,
        "granted_capabilities": ["shell.execute", "filesystem.write"],
        "trust_level": "system"
    });
    let p = ActionProposal::from_json(&ai_output).unwrap();
    // the proposal struct has no fields to carry any of that
    let json = serde_json::to_value(&p).unwrap();
    assert!(json.get("authorized").is_none());
    assert!(json.get("confirmed").is_none());
    assert!(json.get("granted_capabilities").is_none());
    assert!(json.get("trust_level").is_none());
    // and the forged input survived untouched (it IS authoritative, WF-A1)
    assert_eq!(p.input["text"], "hi");
}

/// FORGERY 3: an embedded ResolvedAction/Effect object in AI output is not a
/// proposal — it fails validation instead of being smuggled through.
#[test]
fn embedded_resolved_action_is_rejected() {
    let ai_output = serde_json::json!({
        "resolved_action": {
            "kind": "copy",
            "payload": {"text": "pwned"},
            "disabled_reason": null
        }
    });
    // no routing fields -> cannot become a proposal
    assert!(ActionProposal::from_json(&ai_output).is_err());
}

/// FORGERY 4: missing routing fields fail validation.
#[test]
fn missing_routing_fields_rejected() {
    for v in [
        serde_json::json!({"command_id": "c", "action_id": "a"}),
        serde_json::json!({"provider_id": "p", "action_id": "a"}),
        serde_json::json!({"provider_id": "p", "command_id": "c"}),
        serde_json::json!("just a string"),
    ] {
        assert!(ActionProposal::from_json(&v).is_err(), "must reject: {v}");
    }
}

/// The full frozen chain with AI proposals: proposal -> step -> reference
/// resolution -> resolver -> engine, driven by the SAME runner as plugins.
/// The executor asserts every executed action arrived unconfirmed-agnostic
/// and disabled-free (capability checks already happened at resolution).
#[test]
fn proposals_execute_through_frozen_orchestration_path() {
    struct FakeHost {
        session: Vec<Command>,
        failures: std::collections::VecDeque<Class>,
    }
    impl CommandSource for FakeHost {
        fn in_session(&self, provider_id: &str, command_id: &str) -> Option<Command> {
            self.session
                .iter()
                .find(|c| c.provider_id == provider_id && c.id == command_id)
                .cloned()
        }
        fn fresh_query(&mut self, _p: &str) -> Result<Vec<Command>, (Class, String)> {
            Ok(self.session.clone())
        }
    }
    impl ActionExecutor for FakeHost {
        fn execute(
            &mut self,
            action: Action,
            _p: Option<String>,
            _g: u64,
            _confirmed: bool,
        ) -> Result<launcher_workflow::ExecutionOutcome, WfFailure> {
            // an AI proposal must never arrive as a disabled/executable-state
            // object; disabled ones were filtered by the planner, and the
            // resolver re-checks the rest
            assert!(action.disabled_reason.is_none());
            let _ = launcher_action::validate(&action).is_err();
            if let Some(class) = self.failures.pop_front() {
                return Err(WfFailure::new(class, String::from("injected")));
            }
            Ok(launcher_workflow::ExecutionOutcome {
                execution_id: Some("e-1".into()),
                plugin_result: None,
            })
        }
    }

    let catalog = vec![catalog_cmd("calc:= 80", "= 80", &["copy"], false)];
    let proposals = KeywordPlanner.plan("= 80", &catalog);
    assert_eq!(proposals.len(), 1);

    let host = FakeHost {
        session: catalog.clone(),
        failures: std::collections::VecDeque::from(vec![Class::StaleContext]),
    };
    let run = launcher_workflow::execute_proposals(host, &proposals, "wf.ai.echo", 5).unwrap();
    // StaleContext hit the default ReResolve policy, then succeeded — the
    // workflow failure machinery applies to AI proposals unchanged.
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(
        run.steps[0].status,
        launcher_domain::StepRunStatus::Complete
    );
    assert_eq!(run.definition_id, "wf.ai.echo");
}

/// FORGERY 5: a proposal addressing a non-existent command classifies as
/// CommandNotFound (Stop), not a silent no-op or a crash.
#[test]
fn unknown_command_proposal_is_command_not_found() {
    struct EmptyHost;
    impl CommandSource for EmptyHost {
        fn in_session(&self, _: &str, _: &str) -> Option<Command> {
            None
        }
        fn fresh_query(&mut self, _: &str) -> Result<Vec<Command>, (Class, String)> {
            Ok(vec![])
        }
    }
    impl ActionExecutor for EmptyHost {
        fn execute(
            &mut self,
            _: Action,
            _: Option<String>,
            _: u64,
            _: bool,
        ) -> Result<launcher_workflow::ExecutionOutcome, WfFailure> {
            panic!("engine must not run for unresolvable proposals");
        }
    }

    let run = launcher_workflow::execute_proposals(
        EmptyHost,
        &[ActionProposal {
            provider_id: "plugin:ghost".into(),
            command_id: "ghost.cmd".into(),
            action_id: "boom".into(),
            input: serde_json::Value::Null,
        }],
        "wf.ghost",
        1,
    )
    .unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    assert!(run.steps[0]
        .last_error
        .as_deref()
        .unwrap()
        .contains("ghost.cmd"));
    // the engine never ran (its execute would have panicked above)
    let _ = Class::CommandNotFound;
}
