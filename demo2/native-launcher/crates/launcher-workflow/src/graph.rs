//! Workflow 2.0 Graph Model + Validator (P2.6-A01/A02, spec
//! `demo2/files2/P2.6 开发设计规范 — Workflow 2.0.md` §7-§11).
//!
//! Contract points:
//! - DAG ONLY (§11): arbitrary cycles are rejected in P2.6 v1 — this bounds
//!   the durable scheduler's state space. Loops are a 2.1/3.x contract.
//! - Conditions/variables are DATA/CONTROL FLOW (§3/§4), never authority:
//!   an edge condition can route execution but can never grant an Action
//!   capability that the Resolver would deny.
//! - Validation runs before save AND before execute (§10): duplicate/missing
//!   node ids, dangling edges, multiple entry, unreachable nodes, cycles,
//!   self-loops.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub type NodeId = String;

/// Deterministic condition expression (§3): `left OP right` where operands
/// are variable references (`$name`, `step.output.field`) or literals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionExpr {
    pub left: String,
    pub op: ConditionOp,
    pub right: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOp {
    Eq,
    Ne,
    Gt,
    Lt,
}

/// One DAG node: an action reference plus control metadata (§8). `action_ref`
/// is a stable action/command id — resolution stays in the existing
/// ReferenceResolver/ActionResolver chain (no new execution path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub node_id: NodeId,
    pub action_ref: String,
    /// Run-scoped variable writes from this node's output (§4), if any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_variables: Vec<String>,
    /// Node-level condition: node is skipped (branch not taken) when false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionExpr>,
    /// §19/§20 Human Approval: the node pauses the run for explicit user
    /// approval before executing (P2.6-C). False = no approval needed.
    #[serde(default)]
    pub approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from: NodeId,
    pub to: NodeId,
    /// Edge condition: branch taken only when true (§9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionExpr>,
}

/// Workflow 2.0 definition (§7): a validated DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowGraph {
    pub id: String,
    pub version: u64,
    pub name: String,
    pub entry_node: NodeId,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

impl WorkflowGraph {
    pub fn new(id: &str, name: &str, entry_node: &str) -> Self {
        Self {
            id: id.to_string(),
            version: 1,
            name: name.to_string(),
            entry_node: entry_node.to_string(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn with_node(mut self, node: WorkflowNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn with_edge(mut self, edge: WorkflowEdge) -> Self {
        self.edges.push(edge);
        self
    }
}

/// Validation failure kinds (§10 checklist), each carrying the offending id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    DuplicateNode(NodeId),
    MissingEntryNode(NodeId),
    MultipleEntry(Vec<NodeId>),
    DanglingEdge { from: NodeId, to: NodeId },
    SelfLoop(NodeId),
    Cycle(Vec<NodeId>),
    UnreachableNode(NodeId),
    Empty,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::DuplicateNode(id) => write!(f, "duplicate node id: {id}"),
            GraphError::MissingEntryNode(id) => write!(f, "entry node not found: {id}"),
            GraphError::MultipleEntry(ids) => write!(f, "multiple entry candidates: {ids:?}"),
            GraphError::DanglingEdge { from, to } => {
                write!(f, "dangling edge: {from} -> {to}")
            }
            GraphError::SelfLoop(id) => write!(f, "self-loop on node: {id}"),
            GraphError::Cycle(ids) => write!(f, "cycle detected (DAG-only policy): {ids:?}"),
            GraphError::UnreachableNode(id) => write!(f, "unreachable node: {id}"),
            GraphError::Empty => write!(f, "graph has no nodes"),
        }
    }
}

/// §10 DAG validation. Deterministic order: cheap structural checks first,
/// then reachability, then cycle detection (DFS with colors).
pub fn validate(graph: &WorkflowGraph) -> Result<(), GraphError> {
    if graph.nodes.is_empty() {
        return Err(GraphError::Empty);
    }
    let mut ids: HashSet<&NodeId> = HashSet::new();
    for n in &graph.nodes {
        if !ids.insert(&n.node_id) {
            return Err(GraphError::DuplicateNode(n.node_id.clone()));
        }
    }
    if !ids.contains(&graph.entry_node) {
        return Err(GraphError::MissingEntryNode(graph.entry_node.clone()));
    }
    let mut adjacency: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    for e in &graph.edges {
        if !ids.contains(&e.from) || !ids.contains(&e.to) {
            return Err(GraphError::DanglingEdge {
                from: e.from.clone(),
                to: e.to.clone(),
            });
        }
        if e.from == e.to {
            return Err(GraphError::SelfLoop(e.from.clone()));
        }
        adjacency.entry(&e.from).or_default().push(&e.to);
    }
    // reachability from entry — checked BEFORE multiple-entry: any
    // disconnected subgraph root is precisely an UnreachableNode (its own
    // entry), which is the more actionable diagnosis (§10 order adapted).
    let mut reachable: HashSet<NodeId> = HashSet::new();
    let mut stack = vec![graph.entry_node.clone()];
    while let Some(id) = stack.pop() {
        if reachable.insert(id.clone()) {
            if let Some(nexts) = adjacency.get(&id) {
                for n in nexts {
                    stack.push((*n).clone());
                }
            }
        }
    }
    for n in &graph.nodes {
        if !reachable.contains(&n.node_id) {
            return Err(GraphError::UnreachableNode(n.node_id.clone()));
        }
    }
    // cycle detection: iterative DFS with colors (§11 DAG-only policy)
    let mut color: HashMap<&NodeId, u8> = HashMap::new(); // 0=visiting 1=done
    let node_ids: Vec<&NodeId> = graph.nodes.iter().map(|n| &n.node_id).collect();
    for start in node_ids {
        let mut stack: Vec<(&NodeId, bool)> = vec![(start, false)];
        while let Some((id, processed)) = stack.pop() {
            if processed {
                color.insert(id, 1);
                continue;
            }
            match color.get(id) {
                Some(1) => continue,
                Some(0) => {
                    // re-entry on a visiting node = cycle along the path
                    let path: Vec<String> =
                        reachable.iter().map(|s| s.to_string()).collect();
                    return Err(GraphError::Cycle(path));
                }
                _ => color.insert(id, 0),
            };
            stack.push((id, true));
            if let Some(nexts) = adjacency.get(id) {
                for n in nexts {
                    stack.push((n, false));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn linear() -> WorkflowGraph {
        WorkflowGraph::new("wf.linear", "Linear", "a")
            .with_node(node("a"))
            .with_node(node("b"))
            .with_node(node("c"))
            .with_edge(edge("a", "b"))
            .with_edge(edge("b", "c"))
    }

    /// §2.1: the diamond (branch + join) is a valid DAG.
    #[test]
    fn diamond_branch_join_is_valid() {
        let g = WorkflowGraph::new("wf.d", "Diamond", "a")
            .with_node(node("a"))
            .with_node(node("b"))
            .with_node(node("c"))
            .with_node(node("d"))
            .with_edge(edge("a", "b"))
            .with_edge(edge("a", "c"))
            .with_edge(edge("b", "d"))
            .with_edge(edge("c", "d"));
        assert_eq!(validate(&g), Ok(()));
    }

    /// §11: A→B→C→A cycles are rejected (DAG-only policy).
    #[test]
    fn cycle_rejected() {
        let g = WorkflowGraph::new("wf.cycle", "Cycle", "a")
            .with_node(node("a"))
            .with_node(node("b"))
            .with_node(node("c"))
            .with_edge(edge("a", "b"))
            .with_edge(edge("b", "c"))
            .with_edge(edge("c", "a"));
        assert!(matches!(validate(&g), Err(GraphError::Cycle(_))));
    }

    #[test]
    fn self_loop_rejected() {
        let g = WorkflowGraph::new("wf.self", "Self", "a")
            .with_node(node("a"))
            .with_edge(edge("a", "a"));
        assert!(matches!(validate(&g), Err(GraphError::SelfLoop(_))));
    }

    #[test]
    fn dangling_edge_rejected() {
        let g = linear().with_edge(edge("c", "ghost"));
        assert!(matches!(
            validate(&g),
            Err(GraphError::DanglingEdge { to, .. }) if to == "ghost"
        ));
    }

    #[test]
    fn duplicate_node_rejected() {
        let g = linear().with_node(node("a"));
        assert!(matches!(validate(&g), Err(GraphError::DuplicateNode(_))));
    }

    #[test]
    fn missing_entry_rejected() {
        let mut g = linear();
        g.entry_node = "nope".into();
        assert!(matches!(validate(&g), Err(GraphError::MissingEntryNode(_))));
    }

    /// §10: two nodes without incoming edges = multiple entry.
    /// §10 multiple-entry manifests as disconnected roots, diagnosed as
    /// UnreachableNode (the entry beyond the first is unreachable).
    #[test]
    fn multiple_entry_rejected() {
        let g = WorkflowGraph::new("wf.multi", "Multi", "a")
            .with_node(node("a"))
            .with_node(node("b"))
            .with_node(node("c"))
            .with_edge(edge("b", "c"));
        assert!(matches!(validate(&g), Err(GraphError::UnreachableNode(_))));
    }

    #[test]
    fn unreachable_node_rejected() {
        let g = WorkflowGraph::new("wf.unreach", "Unreachable", "a")
            .with_node(node("a"))
            .with_node(node("b"))
            .with_node(node("orphan"))
            .with_edge(edge("a", "b"));
        assert!(matches!(validate(&g), Err(GraphError::UnreachableNode(_))));
    }

    #[test]
    fn empty_rejected() {
        assert!(matches!(
            validate(&WorkflowGraph::new("wf.e", "E", "a")),
            Err(GraphError::Empty)
        ));
    }

    /// Contract: the graph DTO roundtrips stably (durable-run store input).
    #[test]
    fn graph_roundtrips() {
        let g = linear();
        let json = serde_json::to_string(&g).unwrap();
        let back: WorkflowGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
    }

    /// §3: conditions are control flow, serialized as plain data — no
    /// capability/authority vocabulary may enter the model.
    #[test]
    fn conditions_are_authority_free() {
        let g = WorkflowGraph::new("wf.cond", "Cond", "a")
            .with_node(node("a"))
            .with_node(node("b"))
            .with_node(node("c"))
            .with_edge(WorkflowEdge {
                from: "a".into(),
                to: "b".into(),
                condition: Some(ConditionExpr {
                    left: "$file_count".into(),
                    op: ConditionOp::Gt,
                    right: "10".into(),
                }),
            })
            .with_edge(edge("a", "c"));
        assert_eq!(validate(&g), Ok(()));
        let json = serde_json::to_string(&g).unwrap();
        assert!(!json.contains("capabilit"));
        assert!(!json.contains("authority"));
    }
}
