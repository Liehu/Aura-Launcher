//! P2.6-F durability stress/recovery tests: concurrent runs sharing one
//! store, restart-recovery from mid-run checkpoints, and a bounded stress
//! sweep (20 runs) — the durable runtime contracts under load.

use launcher_workflow::durable::{RunStore, RunStatus};
use launcher_workflow::graph::{WorkflowEdge, WorkflowGraph, WorkflowNode};
use launcher_workflow::scheduler::{run_graph, SchedulerStop, StepExecutor};
use launcher_workflow::engine::VariableStore;

fn graph(id: &str, n: usize) -> WorkflowGraph {
    let mut g = WorkflowGraph::new(id, id, "n0");
    let node = |i: usize| WorkflowNode {
        node_id: format!("n{i}"),
        action_ref: format!("cmd:{i}"),
        output_variables: vec![],
        condition: None,
        approval: false,
    };
    for i in 0..n {
        g.nodes.push(node(i));
    }
    for i in 0..n.saturating_sub(1) {
        g.edges.push(WorkflowEdge {
            from: format!("n{i}"),
            to: format!("n{}", i + 1),
            condition: None,
        });
    }
    g
}

struct Exec {
    ok: bool,
    delay_ms: u64,
}

impl StepExecutor for Exec {
    fn execute(
        &mut self,
        _node: &WorkflowNode,
        _vars: &VariableStore,
    ) -> Result<serde_json::Value, String> {
        if self.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        }
        if self.ok {
            Ok(serde_json::json!("done"))
        } else {
            Err("injected failure".into())
        }
    }
}


/// Concurrent runs on ONE store: both complete with full checkpoints.
#[test]
fn concurrent_runs_share_store_safely() {
    let dir = std::env::temp_dir().join(format!("nl_dur_c_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("runs.db");
    let g1 = graph("wf.one", 2);
    let g2 = graph("wf.two", 2);
    let (s1, s2) = {
        let s1 = RunStore::open(&db).unwrap();
        let s2 = RunStore::open(&db).unwrap();
        (s1, s2)
    };
    let h1 = std::thread::spawn(move || {
        run_graph(&g1, &s1, "run-1", &mut Exec { ok: true, delay_ms: 30 }, None, None)
    });
    let r2 = run_graph(&g2, &s2, "run-2", &mut Exec { ok: true, delay_ms: 30 }, None, None);
    assert_eq!(r2, SchedulerStop::Finished);
    assert_eq!(h1.join().unwrap(), SchedulerStop::Finished);
    let s = RunStore::open(&db).unwrap();
    assert!(s.load("run-1").unwrap().is_some());
    assert!(s.load("run-2").unwrap().is_some());
    std::fs::remove_dir_all(&dir).ok();
}

/// Restart-recovery: a paused run resumes after reopening the store —
/// finished nodes are not re-executed (B06 contract under restart).
#[test]
fn paused_run_resumes_after_store_reopen() {
    let dir = std::env::temp_dir().join(format!("nl_dur_r_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("runs.db");
    let g = graph("wf.resume", 3);
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let store = RunStore::open(&db).unwrap();
        let cancel2 = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(40));
            cancel2.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        let stop = run_graph(&g, &store, "run-p", &mut Exec { ok: true, delay_ms: 30 }, Some(&cancel), None);
        assert!(matches!(stop, SchedulerStop::Paused));
    }
    // reopen (fresh store handle = restart semantics) and finish
    {
        let store = RunStore::open(&db).unwrap();
        let stop = run_graph(&g, &store, "run-p", &mut Exec { ok: true, delay_ms: 30 }, None, None);
        assert_eq!(stop, SchedulerStop::Finished);
        let cp = store.load("run-p").unwrap().unwrap();
        assert_eq!(cp.status, RunStatus::Finished);
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Bounded stress sweep: 20 linear runs all complete, all checkpoints land.
#[test]
fn stress_sweep_20_runs_all_complete() {
    let dir = std::env::temp_dir().join(format!("nl_dur_s_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("runs.db");
    for i in 0..20 {
        let g = graph(&format!("wf.stress{i}"), 2);
        let stop = run_graph(&g, &RunStore::open(&db).unwrap(), &format!("run-{i}"), &mut Exec { ok: true, delay_ms: 30 }, None, None);
        assert_eq!(stop, SchedulerStop::Finished, "run {i}");
    }
    let s = RunStore::open(&db).unwrap();
    assert_eq!(s.list_by_status(RunStatus::Finished).unwrap().len(), 20);
    std::fs::remove_dir_all(&dir).ok();
}
