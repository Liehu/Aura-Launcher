//! Graph Editor state (P2.6-E01/E07, spec `P2.6 开发设计规范 — Workflow
//! 2.0.md` §25-§27): a pure, testable editor model over `WorkflowGraph` —
//! mutations push undo snapshots; undo/redo are bounded (100 levels).
//! The Slint canvas (E02-E05) is a VIEW over this state.

use crate::graph::{validate, ConditionExpr, WorkflowEdge, WorkflowGraph, WorkflowNode, NodeId};

const MAX_UNDO: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOp {
    AddNode { node_id: NodeId, action_ref: String },
    RemoveNode { node_id: NodeId },
    AddEdge { from: NodeId, to: NodeId },
    RemoveEdge { from: NodeId, to: NodeId },
    SetCondition { node_id: NodeId, condition: Option<ConditionExpr> },
}

/// Editor state: the working graph plus bounded undo/redo stacks (§27).
#[derive(Debug, Clone)]
pub struct GraphEditor {
    pub graph: WorkflowGraph,
    undo: Vec<WorkflowGraph>,
    redo: Vec<WorkflowGraph>,
}

impl GraphEditor {
    pub fn new(graph: WorkflowGraph) -> Self {
        Self {
            graph,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    fn push_undo(&mut self) {
        self.undo.push(self.graph.clone());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear(); // a new edit invalidates the redo branch (§27)
    }

    /// Apply an edit. Draft-state validation is deferred to `export_json`
    /// (§10: validation runs before SAVE/EXECUTE — intermediate drafts may
    /// hold temporarily unreachable nodes while the user is still wiring).
    fn edit(&mut self, op: EditOp) -> Result<(), String> {
        self.push_undo();
        let mut g = self.graph.clone();
        let result: Result<(), String> = (|| {
            match op.clone() {
                EditOp::AddNode { node_id, action_ref } => {
                    if g.nodes.iter().any(|n| n.node_id == node_id) {
                        return Err(format!("duplicate node: {node_id}"));
                    }
                    g.nodes.push(WorkflowNode {
                        node_id,
                        action_ref,
                        output_variables: vec![],
                        condition: None,
                        approval: false,
                    });
                    Ok(())
                }
                EditOp::RemoveNode { node_id } => {
                    g.nodes.retain(|n| n.node_id != node_id);
                    g.edges.retain(|e| e.from != node_id && e.to != node_id);
                    if g.entry_node == node_id {
                        return Err("cannot remove the entry node".into());
                    }
                    Ok(())
                }
                EditOp::AddEdge { from, to } => {
                    g.edges.push(WorkflowEdge {
                        from,
                        to,
                        condition: None,
                    });
                    Ok(())
                }
                EditOp::RemoveEdge { from, to } => {
                    g.edges
                        .retain(|e| !(e.from == from && e.to == to));
                    Ok(())
                }
                EditOp::SetCondition { node_id, condition } => {
                    if let Some(n) = g.nodes.iter_mut().find(|n| n.node_id == node_id) {
                        n.condition = condition;
                        Ok(())
                    } else {
                        Err(format!("unknown node: {node_id}"))
                    }
                }
            }
        })();
        result?;
        self.graph = g;
        Ok(())
    }

    pub fn add_node(&mut self, node_id: &str, action_ref: &str) -> Result<(), String> {
        self.edit(EditOp::AddNode {
            node_id: node_id.into(),
            action_ref: action_ref.into(),
        })
    }

    pub fn remove_node(&mut self, node_id: &str) -> Result<(), String> {
        self.edit(EditOp::RemoveNode {
            node_id: node_id.into(),
        })
    }

    pub fn add_edge(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.edit(EditOp::AddEdge {
            from: from.into(),
            to: to.into(),
        })
    }

    pub fn remove_edge(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.edit(EditOp::RemoveEdge {
            from: from.into(),
            to: to.into(),
        })
    }

    pub fn set_condition(
        &mut self,
        node_id: &str,
        condition: Option<ConditionExpr>,
    ) -> Result<(), String> {
        self.edit(EditOp::SetCondition {
            node_id: node_id.into(),
            condition,
        })
    }

    /// §27 undo: roll back one edit. Returns false when nothing to undo.
    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(self.graph.clone());
                self.graph = prev;
                true
            }
            None => false,
        }
    }

    /// §27 redo: re-apply the last undone edit. Returns false at the tip.
    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(self.graph.clone());
                self.graph = next;
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

/// E06: serialization of the edited graph for save/export (durable storage
/// contract — the same JSON the RunStore/durable definitions consume).
pub fn export_json(graph: &WorkflowGraph) -> Result<String, String> {
    validate(graph).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(graph).map_err(|e| e.to_string())
}

pub fn import_json(json: &str) -> Result<WorkflowGraph, String> {
    let g: WorkflowGraph = serde_json::from_str(json).map_err(|e| e.to_string())?;
    validate(&g).map_err(|e| e.to_string())?;
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> GraphEditor {
        GraphEditor::new(
            WorkflowGraph::new("wf.e", "Editor", "a")
                .with_node(WorkflowNode {
                    node_id: "a".into(),
                    action_ref: "cmd:a".into(),
                    output_variables: vec![],
                    condition: None,
                    approval: false,
                }),
        )
    }

    /// §27: undo/redo roundtrip with bounded stack.
    #[test]
    fn undo_redo_roundtrip_and_bound() {
        let mut e = base();
        assert!(!e.can_undo(), "fresh editor has no history");
        for i in 0..(MAX_UNDO + 10) {
            assert!(e.add_node(&format!("n{i}"), "cmd:x").is_ok());
        }
        assert_eq!(e.undo.len(), MAX_UNDO, "undo stack bounded at 100");
        // undo to the oldest TRACKED state (110 edits, 100 kept: the first
        // 10 nodes remain — the bound is the contract)
        while e.can_undo() {
            e.undo();
        }
        assert_eq!(e.graph.nodes.len(), 11, "oldest tracked state restored");
        // redo replays edits
        let mut redone = 0;
        while e.can_redo() {
            assert!(e.redo());
            redone += 1;
        }
        assert!(redone > 0);
        // draft state may hold unreachable nodes — the §10 gate is export
    }

    /// Draft edits allow transient cycles; the §10 validation gates EXPORT.
    #[test]
    fn cycles_gate_at_export() {
        let mut e = base();
        e.add_node("b", "cmd:b").unwrap();
        e.add_edge("a", "b").unwrap();
        e.add_edge("b", "a").unwrap(); // draft accepts; it's still wiring
        assert!(export_json(&e.graph).is_err(), "cycle rejected at export");
        e.remove_edge("b", "a").unwrap();
        assert!(export_json(&e.graph).is_ok());
        // removing the entry node is refused inline (structural rule)
        assert!(e.remove_node("a").is_err());
    }

    /// E06: export/import roundtrip; export refuses invalid graphs.
    #[test]
    fn export_import_roundtrip() {
        let mut e = base();
        e.add_node("b", "cmd:b").unwrap();
        e.add_edge("a", "b").unwrap();
        let json = export_json(&e.graph).unwrap();
        let back = import_json(&json).unwrap();
        assert_eq!(back, e.graph);
    }
}
