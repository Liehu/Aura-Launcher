//! DAG Scheduler (P2.6-B02/B03, spec `P2.6 开发设计规范` §5/§14-§18):
//! the durable execution loop. Drives a validated graph to completion via
//! `ready_nodes`, checkpointing to the RunStore after EVERY node — a crash
//! at any point resumes from the last checkpoint in a fresh process.
//!
//! Boundaries: the scheduler ORCHESTRATES; it never executes effects. Steps
//! run through the `StepExecutor` trait (implemented by the host, whose
//! implementations resolve through the existing ReferenceResolver →
//! ActionResolver chain). Conditions/variables are control flow only.

use crate::approval::ApprovalStatus;
use crate::durable::{RunCheckpoint, RunStatus, RunStore};
use crate::engine::{evaluate, ready_nodes, VariableStore};
use crate::graph::{ConditionExpr, WorkflowGraph, WorkflowNode};
use std::collections::HashSet;
use std::sync::Mutex;

/// Host-provided step runner. Receives the node and the current variables,
/// returns the node's output value (variable writes land in the store).
/// Host-provided step runner. `Send` is required so the ready set can be
/// executed concurrently in scoped threads (B04).
pub trait StepExecutor: Send {
    fn execute(&mut self, node: &WorkflowNode, vars: &VariableStore)
        -> Result<serde_json::Value, String>;
}

/// What the host must do next after a scheduler pass (§35 state machine).
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerStop {
    /// All reachable nodes finished.
    Finished,
    /// Cancel/pause requested — checkpoint written, resume later.
    Paused,
    /// P2.6-C: an approval-gated node is waiting — the run resumes via
    /// run_graph after an explicit approve (rejected nodes get skipped).
    AwaitingApproval { node_id: String },
    /// A step failed after retries; checkpoint written at the failure point.
    Failed { node_id: String, error: String },
}

/// Output variables a finished node writes back (§4: step output source).
fn apply_outputs(vars: &mut VariableStore, node: &WorkflowNode, output: &serde_json::Value) {
    for name in &node.output_variables {
        vars.set(name, output.clone());
    }
}

/// Node/edge conditions are evaluated against the CURRENT variables; a false
/// condition skips the node/branch. Returns (node_taken, edge_taken) pairs
/// are resolved inline by the loop below.
fn condition_holds(cond: &Option<ConditionExpr>, vars: &VariableStore) -> bool {
    match cond {
        Some(c) => evaluate(c, vars),
        None => true,
    }
}

/// Run (or resume) one graph to completion. Checkpoints after every node.
/// `cancel` polls between nodes (B04 pause semantics).
pub fn run_graph(
    graph: &WorkflowGraph,
    store: &RunStore,
    run_id: &str,
    executor: &mut dyn StepExecutor,
    cancel: Option<&AtomicBoolRef>,
    approvals: Option<&crate::approval::ApprovalStore>,
) -> SchedulerStop {
    let mut finished: HashSet<String> = HashSet::new();
    let mut skipped: HashSet<String> = HashSet::new();
    let mut vars = VariableStore::default();
    // resume from checkpoint if present (§18 recovery)
    if let Some(cp) = store.load(run_id).ok().flatten() {
        if cp.graph_id == graph.id && cp.graph_version == graph.version {
            finished = cp.finished.into_iter().collect();
            skipped = cp.skipped.into_iter().collect();
            if let serde_json::Value::Object(map) = &cp.variables {
                for (k, v) in map {
                    vars.set(k, v.clone());
                }
            }
        }
    }

    loop {
        if let Some(cancel) = cancel {
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                checkpoint(store, run_id, graph, RunStatus::Paused, &finished, &skipped, &vars);
                return SchedulerStop::Paused;
            }
        }
        // node conditions: evaluate now, permanently skipping false branches
        let ready = ready_nodes(graph, &finished, &skipped);
        let runnable: Vec<&WorkflowNode> = graph
            .nodes
            .iter()
            .filter(|n| {
                ready.contains(&n.node_id)
                    && !skipped.contains(&n.node_id)
                    && condition_holds(&n.condition, &vars)
            })
            .collect();
        // nothing runnable and nothing left to skip = done or stuck
        if runnable.is_empty() {
            let remaining: Vec<&str> = graph
                .nodes
                .iter()
                .map(|n| n.node_id.as_str())
                .filter(|id| !finished.contains(*id) && !skipped.contains(*id))
                .collect();
            // try skipping not-taken branch nodes (their incoming edge came
            // from a finished node but a node condition may have failed)
            let progressed = mark_condition_skips(graph, &mut skipped, &vars, &finished);
            if remaining.is_empty() {
                checkpoint(store, run_id, graph, RunStatus::Finished, &finished, &skipped, &vars);
                return SchedulerStop::Finished;
            }
            if !progressed {
                // runnable empty but remaining nodes exist: their join is
                // satisfied only by skipped propagation — mark and re-loop
                checkpoint(store, run_id, graph, RunStatus::Failed, &finished, &skipped, &vars);
                return SchedulerStop::Failed {
                    node_id: remaining[0].to_string(),
                    error: "no runnable node and no progress (scheduler stall)".into(),
                };
            }
            continue;
        }
        // v1 execution: approval gate → B04 concurrent batch → checkpoints.
        // B04: the ready set is executed CONCURRENTLY (independent nodes in
        // scoped threads; executor shared behind a Mutex). Outputs are
        // applied deterministically by node_id order AFTER the batch, so
        // variable writes stay reproducible. Approval-gated nodes pause the
        // run (AwaitingApproval) BEFORE any batch execution; rejected gated
        // nodes are skipped.
        let mut gated_pending: Option<String> = None;
        let mut to_execute: Vec<&WorkflowNode> = Vec::new();
        for node in &runnable {
            if node.approval {
                let decision = approvals
                    .and_then(|a| a.decision_for(run_id, &node.node_id).ok())
                    .flatten();
                match decision {
                    Some(ApprovalStatus::Approved) => to_execute.push(node),
                    Some(ApprovalStatus::Rejected) => {
                        skipped.insert(node.node_id.clone());
                    }
                    _ => {
                        if let Some(a) = approvals {
                            let _ = a.request(
                                &format!("{run_id}:{}", node.node_id),
                                run_id,
                                &node.node_id,
                                &node.action_ref,
                            );
                        }
                        gated_pending = Some(node.node_id.clone());
                    }
                }
            } else {
                to_execute.push(node);
            }
        }
        if let Some(node_id) = gated_pending {
            // P2.6-C: approval-gated node pauses the run (AwaitingApproval
            // checkpoint) until an explicit decision; rejected = skip node.
            checkpoint(
                store,
                run_id,
                graph,
                RunStatus::AwaitingApproval,
                &finished,
                &skipped,
                &vars,
            );
            return SchedulerStop::AwaitingApproval { node_id };
        }
        // B04 pause check before the batch: a cancel mid-run pauses between
        // batches, never mid-node
        if let Some(cancel) = cancel {
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                checkpoint(store, run_id, graph, RunStatus::Paused, &finished, &skipped, &vars);
                return SchedulerStop::Paused;
            }
        }
        let batch: Vec<(&WorkflowNode, Mutex<Option<(String, Result<serde_json::Value, String>)>>)> =
            to_execute
                .iter()
                .map(|n| (*n, Mutex::new(None)))
                .collect();
        {
            let ex = Mutex::new(&mut *executor);
            std::thread::scope(|scope| {
                for (node, slot) in &batch {
                    let ex = &ex;
                    let vars = &vars;
                    scope.spawn(move || {
                        let mut guard = ex.lock().expect("executor lock");
                        let out = guard.execute(node, vars);
                        *slot.lock().expect("slot lock") = Some((node.node_id.clone(), out));
                    });
                }
            });
        }
        // deterministic application: node_id order
        let mut batch_out: Vec<(String, Result<serde_json::Value, String>)> = batch
            .into_iter()
            .filter_map(|(_, slot)| slot.into_inner().expect("slot lock"))
            .collect();
        batch_out.sort_by(|a, b| a.0.cmp(&b.0));
        for (node_id, out) in batch_out {
            let node = graph
                .nodes
                .iter()
                .find(|n| n.node_id == node_id)
                .expect("batch node");
            match out {
                Ok(output) => {
                    apply_outputs(&mut vars, node, &output);
                    finished.insert(node_id);
                }
                Err(e) => {
                    // B05 v1 policy: no retry — fail the run at the checkpoint
                    checkpoint(
                        store,
                        run_id,
                        graph,
                        RunStatus::Failed,
                        &finished,
                        &skipped,
                        &vars,
                    );
                    return SchedulerStop::Failed { node_id, error: e };
                }
            }
            // checkpoint after EVERY node (B02): a crash here resumes exactly
            checkpoint(
                store,
                run_id,
                graph,
                RunStatus::Running,
                &finished,
                &skipped,
                &vars,
            );
        }
        // propagate skips: nodes whose incoming edges were all satisfied but
        // whose own condition is false are skipped instead of executed
        mark_condition_skips(graph, &mut skipped, &vars, &finished);
    }
}

/// Mark nodes as skipped when their own condition evaluates false against
/// current variables. Returns true if any new node was skipped.
fn mark_condition_skips(
    graph: &WorkflowGraph,
    skipped: &mut HashSet<String>,
    vars: &VariableStore,
    _finished: &HashSet<String>,
) -> bool {
    let mut progressed = false;
    for n in &graph.nodes {
        if skipped.contains(&n.node_id) {
            continue;
        }
        if let Some(cond) = &n.condition {
            if !condition_holds(&Some(cond.clone()), vars) {
                skipped.insert(n.node_id.clone());
                progressed = true;
            }
        }
    }
    progressed
}

fn checkpoint(
    store: &RunStore,
    run_id: &str,
    graph: &WorkflowGraph,
    status: RunStatus,
    finished: &HashSet<String>,
    skipped: &HashSet<String>,
    vars: &VariableStore,
) {
    let cp = RunCheckpoint {
        run_id: run_id.to_string(),
        graph_id: graph.id.clone(),
        graph_version: graph.version,
        status,
        finished: finished.iter().cloned().collect(),
        skipped: skipped.iter().cloned().collect(),
        variables: vars.snapshot(),
        updated_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
    };
    if let Err(e) = store.checkpoint(&cp) {
        tracing::warn!(error = %e, run_id = %run_id, "workflow checkpoint failed");
    }
}

/// Cancellation flag alias for call-site clarity (B04 pause semantics).
pub type AtomicBoolRef = std::sync::atomic::AtomicBool;
