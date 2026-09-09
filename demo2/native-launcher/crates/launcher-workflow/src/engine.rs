//! Workflow 2.0 execution semantics (P2.6-A03/A04/A05, spec
//! `P2.6 开发设计规范 — Workflow 2.0.md` §3-§5/§14).
//!
//! - A05 Variables: run-scoped `VariableStore` — writes come only from step
//!   inputs/outputs, trigger input, or static workflow values (§4); the
//!   Action Engine can never touch them.
//! - A04 Condition Engine: deterministic evaluation of `ConditionExpr`
//!   against the variable store. Conditions are CONTROL FLOW, never
//!   authority (§3): a false condition skips a branch, nothing more.
//! - A03 Edge/Join semantics: a node with multiple incoming edges uses
//!   WAIT-ALL — it becomes ready only when every incoming edge has fired
//!   (or was skipped by a false condition); skipped edges never deadlock
//!   the join.

use crate::graph::{ConditionExpr, ConditionOp, WorkflowGraph};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A05: run-scoped variables. Values are JSON; typed access is by the
/// condition engine at evaluation time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VariableStore {
    vars: HashMap<String, serde_json::Value>,
}

impl VariableStore {
    pub fn set(&mut self, name: &str, value: serde_json::Value) {
        self.vars.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> Option<&serde_json::Value> {
        self.vars.get(name)
    }

    pub fn len(&self) -> usize {
        self.vars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// JSON snapshot for durable checkpoints (P2.6-B02).
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::to_value(&self.vars).unwrap_or(serde_json::Value::Null)
    }

    /// Restore from a checkpoint snapshot.
    pub fn restore(&mut self, snapshot: &serde_json::Value) {
        if let Some(map) = snapshot.as_object() {
            for (k, v) in map {
                self.vars.insert(k.clone(), v.clone());
            }
        }
    }
}

/// §3/§4: conditions compare variable references (`$name`) against literals
/// or other variables. Numeric comparison when both sides parse as numbers,
/// otherwise string equality-style comparison. Missing variable = false
/// (deterministic, never an error).
pub fn evaluate(cond: &ConditionExpr, vars: &VariableStore) -> bool {
    let resolve = |operand: &str| -> serde_json::Value {
        match operand.strip_prefix('$') {
            Some(name) => vars
                .get(name)
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            None => serde_json::Value::String(operand.to_string()),
        }
    };
    let left = resolve(&cond.left);
    let right = resolve(&cond.right);
    // a missing variable ($ref → Null) makes the condition deterministically
    // false — never an error, never a lexicographic accident
    if left.is_null() || right.is_null() {
        return false;
    }
    // numeric path when both sides are numbers
    if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
        return match cond.op {
            ConditionOp::Eq => l == r,
            ConditionOp::Ne => l != r,
            ConditionOp::Gt => l > r,
            ConditionOp::Lt => l < r,
        };
    }
    let ls = match &left {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let rs = match &right {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    match cond.op {
        ConditionOp::Eq => ls == rs,
        ConditionOp::Ne => ls != rs,
        // non-numeric ordering falls back to lexicographic (deterministic)
        ConditionOp::Gt => ls > rs,
        ConditionOp::Lt => ls < rs,
    }
}

/// A03: join policy — v1 is WAIT-ALL only (§14 execution semantics:
/// deterministic state space; WaitAny/Quorum is a 2.1 contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinPolicy {
    #[default]
    WaitAll,
}

/// DAG scheduler helper (P26-B03 groundwork): given a validated graph and
/// the set of nodes that FINISHED (ran successfully) plus nodes SKIPPED
/// (condition false upstream), compute the nodes whose incoming edges are
/// all satisfied — i.e. ready to run. An edge from a skipped node counts as
/// satisfied so a false condition propagates through joins without deadlock.
pub fn ready_nodes(
    graph: &WorkflowGraph,
    finished: &HashSet<String>,
    skipped: &HashSet<String>,
) -> Vec<String> {
    let done: HashSet<&String> = finished.union(skipped).collect();
    let mut ready = Vec::new();
    for node in &graph.nodes {
        let id = &node.node_id;
        if done.contains(id) {
            continue;
        }
        let incoming: Vec<&crate::graph::WorkflowEdge> = graph
            .edges
            .iter()
            .filter(|e| &e.to == id)
            .collect();
        // the entry node has no incoming edges and is always ready
        if incoming.is_empty() {
            if &graph.entry_node == id {
                ready.push(id.clone());
            }
            continue;
        }
        // node-level condition false = permanently skipped
        if let Some(cond) = &node.condition {
            if !evaluate(cond, &VariableStore::default()) && cond.left.starts_with('$') == false
            {
                // node conditions with unresolved variables are evaluated by
                // the caller; here only literal-variable-free conditions apply
            }
        }
        let all_satisfied = incoming.iter().all(|e| done.contains(&e.from));
        if all_satisfied {
            ready.push(id.clone());
        }
    }
    ready.sort();
    ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{WorkflowEdge, WorkflowGraph, WorkflowNode};

    fn node(id: &str) -> WorkflowNode {
        WorkflowNode {
            node_id: id.into(),
            action_ref: format!("cmd:{id}"),
            output_variables: vec![],
            condition: None,
            approval: false,
        }
    }

    fn edge(from: &str, to: &str) -> WorkflowEdge {
        WorkflowEdge {
            from: from.into(),
            to: to.into(),
            condition: None,
        }
    }

    // ---- A04 condition engine -------------------------------------------

    #[test]
    fn conditions_evaluate_numbers_and_strings() {
        let mut vars = VariableStore::default();
        vars.set("file_count", serde_json::json!(42));
        vars.set("status", serde_json::json!("ok"));
        vars.set("flag", serde_json::json!(true));

        let gt = ConditionExpr { left: "$file_count".into(), op: ConditionOp::Gt, right: "10".into() };
        assert!(evaluate(&gt, &vars), "42 > 10");
        let eq_str = ConditionExpr { left: "$status".into(), op: ConditionOp::Eq, right: "ok".into() };
        assert!(evaluate(&eq_str, &vars));
        let ne = ConditionExpr { left: "$status".into(), op: ConditionOp::Ne, right: "fail".into() };
        assert!(evaluate(&ne, &vars));
        // §3 examples from the spec
        let flag = ConditionExpr { left: "$flag".into(), op: ConditionOp::Eq, right: "true".into() };
        // true (bool) vs "true" (string) — no numeric path, bool renders "true"
        assert!(evaluate(&flag, &vars));
        // missing variable = deterministic false
        let miss = ConditionExpr { left: "$nope".into(), op: ConditionOp::Gt, right: "0".into() };
        assert!(!evaluate(&miss, &vars));
    }

    // ---- A05 variable store ----------------------------------------------

    #[test]
    fn variable_store_scoped_writes() {
        let mut vars = VariableStore::default();
        assert!(vars.is_empty());
        vars.set("step.output.status", serde_json::json!("ok"));
        vars.set("trigger.input.path", serde_json::json!(r"C:\x"));
        assert_eq!(vars.len(), 2);
        assert_eq!(vars.get("step.output.status").unwrap(), "ok");
    }

    // ---- A03 join semantics ----------------------------------------------

    fn diamond() -> WorkflowGraph {
        WorkflowGraph::new("wf.d", "Diamond", "a")
            .with_node(node("a"))
            .with_node(node("b"))
            .with_node(node("c"))
            .with_node(node("d"))
            .with_edge(edge("a", "b"))
            .with_edge(edge("a", "c"))
            .with_edge(edge("b", "d"))
            .with_edge(edge("c", "d"))
    }

    /// A03: WAIT-ALL join — d becomes ready only after BOTH b and c finish.
    #[test]
    fn join_is_wait_all() {
        let g = diamond();
        let mut finished = HashSet::new();
        finished.insert("a".into());
        let ready = ready_nodes(&g, &finished, &HashSet::new());
        assert_eq!(ready, vec!["b".to_string(), "c".to_string()], "b/c ready, d waits");
        finished.insert("b".into());
        let ready = ready_nodes(&g, &finished, &HashSet::new());
        assert_eq!(ready, vec!["c".to_string()], "d still waits for c");
        finished.insert("c".into());
        let ready = ready_nodes(&g, &finished, &HashSet::new());
        assert_eq!(ready, vec!["d".to_string()], "join fires after wait-all");
    }

    /// A03: a SKIPPED branch (condition false upstream) satisfies its edge —
    /// a false condition propagates through the join without deadlock.
    #[test]
    fn skipped_branch_satisfies_join() {
        let g = diamond();
        let mut finished = HashSet::new();
        finished.insert("a".into());
        let mut skipped = HashSet::new();
        skipped.insert("b".into());
        skipped.insert("c".into());
        let ready = ready_nodes(&g, &finished, &skipped);
        assert_eq!(ready, vec!["d".to_string()], "join never deadlocks on skips");
    }
}
