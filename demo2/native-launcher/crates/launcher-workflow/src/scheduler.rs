//! DAG Scheduler (P2.6-B02/B03, spec `P2.6 开发设计规范` §5/§14-§18):
//! the durable execution loop. Drives a validated graph to completion via
//! `ready_nodes`, checkpointing to the RunStore after EVERY node — a crash
//! at any point resumes from the last checkpoint in a fresh process.
//!
//! Boundaries: the scheduler ORCHESTRATES; it never executes effects. Steps
//! run through the `StepExecutor` trait (implemented by the host, whose
//! implementations resolve through the existing ReferenceResolver →
//! ActionResolver chain). Conditions/variables are control flow only.

use crate::durable::{RunCheckpoint, RunStatus, RunStore};
use crate::engine::{evaluate, ready_nodes, VariableStore};
use crate::graph::{ConditionExpr, WorkflowGraph, WorkflowNode};
use std::collections::HashSet;

/// Host-provided step runner. Receives the node and the current variables,
/// returns the node's output value (variable writes land in the store).
pub trait StepExecutor {
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
        // v1: sequential execution within ready set (B04 parallel lands later)
        for node in runnable {
            // B04 pause check between nodes: a cancel mid-batch pauses before
            // the next node, never mid-node
            if let Some(cancel) = cancel {
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    checkpoint(
                        store,
                        run_id,
                        graph,
                        RunStatus::Paused,
                        &finished,
                        &skipped,
                        &vars,
                    );
                    return SchedulerStop::Paused;
                }
            }
            match executor.execute(node, &vars) {
                Ok(output) => {
                    apply_outputs(&mut vars, node, &output);
                    finished.insert(node.node_id.clone());
                }
                Err(e) => {
                    // B05 v1 policy: no retry — fail the run at the checkpoint
                    skipped.insert(node.node_id.clone());
                    checkpoint(store, run_id, graph, RunStatus::Failed, &finished, &skipped, &vars);
                    return SchedulerStop::Failed {
                        node_id: node.node_id.clone(),
                        error: e,
                    };
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
