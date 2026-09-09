//! P2.6-B02/B03/B05/B06 tests: the durable scheduler — per-node
//! checkpointing, resumable runs, failure stops at the checkpoint, and
//! pause/resume through the cancellation flag.

use launcher_workflow::durable::{RunStore, RunStatus};
use launcher_workflow::graph::{WorkflowGraph, WorkflowNode};
use launcher_workflow::scheduler::{run_graph, SchedulerStop, StepExecutor};
use launcher_workflow::engine::VariableStore;
use std::collections::HashSet;

fn node(id: &str) -> WorkflowNode {
    WorkflowNode {
        node_id: id.into(),
        action_ref: format!("cmd:{id}"),
        output_variables: vec![],
        condition: None,
        approval: false,
    }
}

fn linear_graph() -> WorkflowGraph {
    WorkflowGraph::new("wf.linear", "Linear", "a")
        .with_node(node("a"))
        .with_node(node("b"))
        .with_node(node("c"))
        .with_edge(launcher_workflow::graph::WorkflowEdge {
            from: "a".into(),
            to: "b".into(),
            condition: None,
        })
        .with_edge(launcher_workflow::graph::WorkflowEdge {
            from: "b".into(),
            to: "c".into(),
            condition: None,
        })
}

struct Recording {
    executed: Vec<String>,
    fail_on: Option<String>,
}

impl StepExecutor for Recording {
    fn execute(
        &mut self,
        node: &WorkflowNode,
        _vars: &VariableStore,
    ) -> Result<serde_json::Value, String> {
        if self.fail_on.as_deref() == Some(node.node_id.as_str()) {
            return Err(format!("step {} failed", node.node_id));
        }
        self.executed.push(node.node_id.clone());
        Ok(serde_json::json!("out"))
    }
}

fn store() -> (RunStore, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "nl_sched_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    (RunStore::open(&dir.join("runs.db")).unwrap(), dir)
}

/// B02/B03: a linear graph runs to completion, checkpointing after every
/// node — the final checkpoint shows all finished, status Finished.
#[test]
fn b03_runs_to_completion_with_checkpoints() {
    let (store, dir) = store();
    let g = linear_graph();
    let mut ex = Recording {
        executed: vec![],
        fail_on: None,
    };
    let stop = run_graph(&g, &store, "run-ok", &mut ex, None, None);
    assert_eq!(stop, SchedulerStop::Finished);
    assert_eq!(ex.executed, vec!["a", "b", "c"]);
    let cp = store.load("run-ok").unwrap().unwrap();
    assert_eq!(cp.status, RunStatus::Finished);
    assert_eq!(cp.finished.len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

/// B05 v1: a failing step stops the run at the checkpoint with Failed.
#[test]
fn b05_failure_stops_at_checkpoint() {
    let (store, dir) = store();
    let g = linear_graph();
    let mut ex = Recording {
        executed: vec![],
        fail_on: Some("b".into()),
    };
    let stop = run_graph(&g, &store, "run-fail", &mut ex, None, None);
    match stop {
        SchedulerStop::Failed { node_id, .. } => assert_eq!(node_id, "b"),
        other => panic!("expected Failed, got {other:?}"),
    }
    let cp = store.load("run-fail").unwrap().unwrap();
    assert_eq!(cp.status, RunStatus::Failed);
    std::fs::remove_dir_all(&dir).ok();
}

/// B02/B06: a pause mid-run leaves a Running checkpoint; resuming with a
/// fresh executor continues EXACTLY where it left off (no re-execution of
/// finished nodes) — the durable recovery contract.
#[test]
fn b06_pause_resume_continues_from_checkpoint() {
    let (store, dir) = store();
    let g = linear_graph();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // cancel fires once "a" has executed: implement via an executor that
    // sets the flag after the first node
    struct CancelAfter {
        ran: usize,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    }
    use std::sync::Arc;
    impl StepExecutor for CancelAfter {
        fn execute(
            &mut self,
            node: &WorkflowNode,
            _vars: &VariableStore,
        ) -> Result<serde_json::Value, String> {
            self.ran += 1;
            if self.ran >= 2 {
                // simulate a supersede arriving during the 2nd node
                self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            let _ = node;
            Ok(serde_json::json!("out"))
        }
    }
    let mut ex = CancelAfter {
        ran: 0,
        cancel: cancel.clone(),
    };
    let stop = run_graph(&g, &store, "run-pause", &mut ex, Some(cancel.as_ref()), None);
    assert_eq!(stop, SchedulerStop::Paused);
    let cp = store.load("run-pause").unwrap().unwrap();
    assert_eq!(cp.status, RunStatus::Paused);
    assert!(cp.finished.contains(&"a".to_string()));
    assert!(cp.finished.contains(&"b".to_string()), "b completed before pause");

    // RESUME: cancel flag clear, fresh executor — remaining nodes run only
    let mut ex2 = Recording {
        executed: vec![],
        fail_on: None,
    };
    let stop = run_graph(&g, &store, "run-pause", &mut ex2, None, None);
    assert_eq!(stop, SchedulerStop::Finished);
    assert_eq!(ex2.executed, vec!["c"], "finished nodes NOT re-executed");
    std::fs::remove_dir_all(&dir).ok();
}

/// Variables written by a step's output_variables flow into the store and
/// survive checkpoints (A05 + B02 integration).
#[test]
fn step_outputs_flow_into_variables() {
    let (store, dir) = store();
    let mut g = WorkflowGraph::new("wf.vars", "Vars", "a");
    let mut a = node("a");
    a.output_variables = vec!["status".into()];
    g.nodes.push(a);
    g.nodes.push(node("b"));
    g.edges.push(launcher_workflow::graph::WorkflowEdge {
        from: "a".into(),
        to: "b".into(),
        condition: None,
    });
    struct Outputter;
    impl StepExecutor for Outputter {
        fn execute(
            &mut self,
            _node: &WorkflowNode,
            _vars: &VariableStore,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"status": "ok"}))
        }
    }
    let stop = run_graph(&g, &store, "run-vars", &mut Outputter, None, None);
    assert_eq!(stop, SchedulerStop::Finished);
    let cp = store.load("run-vars").unwrap().unwrap();
    assert_eq!(cp.variables["status"]["status"], "ok");
    std::fs::remove_dir_all(&dir).ok();
}
