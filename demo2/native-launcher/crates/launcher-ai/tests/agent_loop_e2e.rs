//! P1-FIX-03 E2E (review 64 §7): the bounded agent loop must form a real
//! Observe → Plan → Execute → Observation → Replan cycle, where the planner
//! SEES what failed and never re-proposes it, and every execution attempt
//! carries its own execution_id.

use launcher_ai::agent::{
    run_bounded_agent, AgentExecutionHost, AgentLimits, AgentRunStatus, ExecutionRecord,
};
use launcher_domain::{Action, ActionKind, Category, Command};
use launcher_workflow::proposal::{ActionPlanner, ActionProposal, ReplanContext};

fn catalog_command(id: &str, title: &str) -> Command {
    Command {
        id: id.into(),
        title: title.into(),
        subtitle: None,
        icon: None,
        provider_id: "plugin:test".into(),
        score: 0.0,
        keywords: vec![],
        category: Category::Plugin,
        actions: vec![Action {
            kind: ActionKind::PluginInvoke,
            payload: Some(launcher_domain::ActionPayload::Json(serde_json::json!({}))),
            id: Some("invoke".into()),
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        }],
        target: None,
    }
}

fn proposal(command_id: &str) -> ActionProposal {
    ActionProposal {
        provider_id: "plugin:test".into(),
        command_id: command_id.into(),
        action_id: "invoke".into(),
        input: serde_json::json!({}),
    }
}

/// Fixed catalog; `scan` fails, `report` succeeds.
struct FakeHost {
    calls: usize,
}

impl AgentExecutionHost for FakeHost {
    fn catalog(&mut self) -> Vec<Command> {
        self.calls += 1;
        vec![
            catalog_command("scan", "Scan files"),
            catalog_command("report", "Report results"),
        ]
    }

    fn execute(&mut self, proposals: &[ActionProposal]) -> Vec<ExecutionRecord> {
        proposals
            .iter()
            .map(|p| ExecutionRecord {
                execution_id: Some(format!("E-{}", (self.calls * 10) + p.command_id.len())),
                action_id: p.action_id.clone(),
                ok: p.command_id != "scan",
                error: (p.command_id == "scan").then(|| "scan failed".into()),
                result: None,
            })
            .collect()
    }
}

/// Plans [scan, report] on the first call. The default `replan` filters the
/// failed scan out, so turn 2 must only see `report`.
struct ScanFirstPlanner {
    calls: usize,
    seen_failed: Vec<Vec<(String, String)>>,
}

impl ActionPlanner for ScanFirstPlanner {
    fn plan(&mut self, _input: &str, _catalog: &[Command]) -> Vec<ActionProposal> {
        self.calls += 1;
        vec![proposal("scan"), proposal("report")]
    }

    fn replan(
        &mut self,
        _input: &str,
        _catalog: &[Command],
        observation: &ReplanContext,
    ) -> Vec<ActionProposal> {
        self.seen_failed.push(observation.failed.clone());
        self.calls += 1;
        // a result-aware planner drops what it observed to have failed
        let failed = &observation.failed;
        let proposals = vec![proposal("scan"), proposal("report")];
        proposals
            .into_iter()
            .filter(|p| !failed.contains(&(p.command_id.clone(), p.action_id.clone())))
            .collect()
    }
}

/// PRODUCT-E2E (review 64 §28): goal → plan A fails → observation carries
/// the failure → replan proposes only B → complete.
#[test]
fn agent_replan_sees_failures_and_completes() {
    let mut host = FakeHost { calls: 0 };
    let mut planner = ScanFirstPlanner {
        calls: 0,
        seen_failed: Vec::new(),
    };
    let outcome = run_bounded_agent(
        &mut host,
        &mut planner,
        "scan and report",
        &AgentLimits::default(),
    );

    // loop shape: 2 turns, 1 replan; 3 execution ATTEMPTS (scan fails once,
    // report runs in both turns), 1 failure, 2 distinct successful ids
    assert_eq!(outcome.status, AgentRunStatus::Completed);
    assert_eq!(outcome.turns, 2);
    assert_eq!(outcome.replans, 1, "replan counter must be single-sourced");
    assert_eq!(outcome.executions, 3);
    assert_eq!(outcome.failures, 1);

    // the planner OBSERVED the failure before replanning
    assert_eq!(
        planner.seen_failed,
        vec![vec![("scan".to_string(), "invoke".to_string())]],
        "replan must receive the failed (command, action) pairs"
    );

    // each execution attempt kept its own execution_id (E1 != E2)
    let executed: Vec<_> = outcome.turn_records
        .iter()
        .flat_map(|t| t.executed.clone())
        .collect();
    assert_eq!(executed.len(), 2);
    assert_ne!(executed[0], executed[1], "E1 != E2: ids are never reused");
}

/// Default `replan` semantics: plan() minus everything that failed — a
/// planner that keeps re-proposing the failure cannot loop forever.
#[test]
fn default_replan_filters_failed_proposals() {
    struct StubbornPlanner;
    impl ActionPlanner for StubbornPlanner {
        fn plan(&mut self, _input: &str, _catalog: &[Command]) -> Vec<ActionProposal> {
            vec![proposal("scan")]
        }
    }

    struct FailingHost;
    impl AgentExecutionHost for FailingHost {
        fn catalog(&mut self) -> Vec<Command> {
            vec![catalog_command("scan", "Scan files")]
        }
        fn execute(&mut self, proposals: &[ActionProposal]) -> Vec<ExecutionRecord> {
            proposals
                .iter()
                .map(|p| ExecutionRecord {
                    execution_id: Some("E-x".into()),
                    action_id: p.action_id.clone(),
                    ok: false,
                    error: Some("always fails".into()),
                    result: None,
                })
                .collect()
        }
    }

    let limits = AgentLimits {
        max_turns: 5,
        max_failures: 5,
        max_replans: 5,
        ..Default::default()
    };
    let outcome = run_bounded_agent(&mut FailingHost, &mut StubbornPlanner, "goal", &limits);
    // turn 1 executes and fails; the filtered replan proposes nothing, the
    // loop never re-executes the same failing proposal
    assert_eq!(outcome.executions, 1, "failed proposal must not be re-proposed");
    assert_eq!(outcome.status, AgentRunStatus::Completed);
}
