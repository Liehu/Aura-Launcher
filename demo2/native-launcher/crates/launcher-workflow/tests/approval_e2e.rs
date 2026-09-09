//! P2.6-C integration tests: approval-gated nodes pause the durable run
//! (AwaitingApproval), an explicit approve resumes and executes the node,
//! a rejection skips it. Uses the REAL RunStore + ApprovalStore.

use launcher_workflow::approval::{ApprovalStore, ApprovalStatus};
use launcher_workflow::durable::{RunStore, RunStatus};
use launcher_workflow::graph::{WorkflowGraph, WorkflowNode, WorkflowEdge};
use launcher_workflow::scheduler::{run_graph, SchedulerStop, StepExecutor};
use launcher_workflow::engine::VariableStore;
use std::path::PathBuf;

fn node(id: &str, approval: bool) -> WorkflowNode {
    WorkflowNode {
        node_id: id.into(),
        action_ref: format!("cmd:{id}"),
        output_variables: vec![],
        condition: None,
        approval,
    }
}

fn edge(from: &str, to: &str) -> WorkflowEdge {
    WorkflowEdge {
        from: from.into(),
        to: to.into(),
        condition: None,
    }
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "nl_appr_e2e_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Recording {
    executed: Vec<String>,
}

impl StepExecutor for Recording {
    fn execute(
        &mut self,
        node: &WorkflowNode,
        _vars: &VariableStore,
    ) -> Result<serde_json::Value, String> {
        self.executed.push(node.node_id.clone());
        Ok(serde_json::json!("out"))
    }
}

/// C01/C03 happy path: approval node pauses → approve → resume executes it.
#[test]
fn approval_pauses_then_approve_resumes() {
    let dir = scratch("approve");
    let g = WorkflowGraph::new("wf.appr", "Approval", "a")
        .with_node(node("a", false))
        .with_node(node("deploy", true))
        .with_edge(edge("a", "deploy"));
    let store = RunStore::open(&dir.join("runs.db")).unwrap();
    let approvals = ApprovalStore::open(&dir.join("approvals.db")).unwrap();
    let mut ex = Recording { executed: vec![] };

    // first pass: a runs, deploy pauses for approval
    let stop = run_graph(&g, &store, "run-1", &mut ex, None, Some(&approvals));
    assert_eq!(
        stop,
        SchedulerStop::AwaitingApproval {
            node_id: "deploy".into()
        }
    );
    assert_eq!(ex.executed, vec!["a"], "gated node NOT executed pre-approval");
    let cp = store.load("run-1").unwrap().unwrap();
    assert_eq!(cp.status, RunStatus::AwaitingApproval);
    let pending = approvals.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].node_id, "deploy");

    // approve via the store (host approval UI surface), resume
    approvals
        .decide(&format!("run-1:deploy"), true)
        .unwrap();
    let stop = run_graph(&g, &store, "run-1", &mut ex, None, Some(&approvals));
    assert_eq!(stop, SchedulerStop::Finished);
    assert_eq!(ex.executed, vec!["a", "deploy"], "approved node executed once");
    std::fs::remove_dir_all(&dir).ok();
}

/// C01: a REJECTED approval skips the gated node; the run still finishes.
#[test]
fn rejection_skips_gated_node() {
    let dir = scratch("reject");
    let g = WorkflowGraph::new("wf.rej", "Reject", "a")
        .with_node(node("a", false))
        .with_node(node("deploy", true))
        .with_edge(edge("a", "deploy"));
    let store = RunStore::open(&dir.join("runs.db")).unwrap();
    let approvals = ApprovalStore::open(&dir.join("approvals.db")).unwrap();
    let mut ex = Recording { executed: vec![] };

    let stop = run_graph(&g, &store, "run-2", &mut ex, None, Some(&approvals));
    assert!(matches!(stop, SchedulerStop::AwaitingApproval { .. }));
    approvals
        .decide(&format!("run-2:deploy"), false)
        .unwrap();
    let stop = run_graph(&g, &store, "run-2", &mut ex, None, Some(&approvals));
    assert_eq!(stop, SchedulerStop::Finished);
    assert_eq!(ex.executed, vec!["a"], "rejected node skipped, not executed");
    let cp = store.load("run-2").unwrap().unwrap();
    assert_eq!(cp.status, RunStatus::Finished);
    assert!(
        cp.skipped.contains(&"deploy".to_string()),
        "rejected node recorded as skipped"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Persistence: the pending approval survives a store reopen (restart-safe).
#[test]
fn pending_approval_survives_restart() {
    let dir = scratch("restart");
    let g = WorkflowGraph::new("wf.r", "R", "a")
        .with_node(node("a", false))
        .with_node(node("b", true))
        .with_edge(edge("a", "b"));
    {
        let store = RunStore::open(&dir.join("runs.db")).unwrap();
        let approvals = ApprovalStore::open(&dir.join("approvals.db")).unwrap();
        let mut ex = Recording { executed: vec![] };
        let stop = run_graph(&g, &store, "run-3", &mut ex, None, Some(&approvals));
        assert!(matches!(stop, SchedulerStop::AwaitingApproval { .. }));
    }
    // "restart": fresh store handles, decision, resume
    {
        let store = RunStore::open(&dir.join("runs.db")).unwrap();
        let approvals = ApprovalStore::open(&dir.join("approvals.db")).unwrap();
        assert_eq!(approvals.pending().unwrap().len(), 1, "pending survives");
        approvals.decide(&"run-3:b".replace("b", "b"), true).unwrap();
        let mut ex = Recording { executed: vec![] };
        let stop = run_graph(&g, &store, "run-3", &mut ex, None, Some(&approvals));
        assert_eq!(stop, SchedulerStop::Finished);
        assert_eq!(ex.executed, vec!["b"]);
    }
    std::fs::remove_dir_all(&dir).ok();
}
