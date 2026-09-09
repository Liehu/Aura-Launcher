//! Editor surface projection (P2.6-E02 host binding, review 60 §58 ViewModel
//! pattern): the host projects `GraphEditor` state into plain DTO rows for
//! the Slint surface, and routes surface callbacks back into the editor.
//! launcher-ui never depends on launcher-workflow (UI dependency guard).
//!
//! Wired to the Slint surface in E03-E05 (next slice); until then only the
//! tests consume it, hence the dead_code allowance.

#![allow(dead_code)]

use launcher_workflow::editor::GraphEditor;
use launcher_workflow::graph::ConditionOp;

/// One row of the editor surface (plain strings — UI-renderable DTO).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorRow {
    pub node_id: String,
    pub action_ref: String,
    /// Human-readable condition ("" = unconditional).
    pub condition: String,
}

/// Host-side editor surface: owns the `GraphEditor` and the projected rows.
/// Every mutation refreshes `rows` so the VIEW is always a pure projection.
pub struct EditorSurface {
    pub editor: GraphEditor,
    pub rows: Vec<EditorRow>,
}

impl EditorSurface {
    pub fn new(editor: GraphEditor) -> Self {
        let mut s = Self {
            editor,
            rows: Vec::new(),
        };
        s.refresh();
        s
    }

    /// GraphEditor → rows (§25 VIEW projection; sorted for stability).
    pub fn refresh(&mut self) {
        let mut rows: Vec<EditorRow> = self
            .editor
            .graph
            .nodes
            .iter()
            .map(|n| EditorRow {
                node_id: n.node_id.clone(),
                action_ref: n.action_ref.clone(),
                condition: n
                    .condition
                    .as_ref()
                    .map(|c| format!("{} {} {}", c.left, debug_op(c.op), c.right))
                    .unwrap_or_default(),
            })
            .collect();
        rows.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        self.rows = rows;
    }

    pub fn add_node(&mut self, node_id: &str, action_ref: &str) -> Result<(), String> {
        self.editor.add_node(node_id, action_ref)?;
        self.refresh();
        Ok(())
    }

    pub fn remove_node(&mut self, node_id: &str) -> Result<(), String> {
        self.editor.remove_node(node_id)?;
        self.refresh();
        Ok(())
    }

    pub fn add_edge(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.editor.add_edge(from, to)?;
        self.refresh();
        Ok(())
    }

    pub fn remove_edge(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.editor.remove_edge(from, to)?;
        self.refresh();
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let changed = self.editor.undo();
        if changed {
            self.refresh();
        }
        changed
    }

    pub fn redo(&mut self) -> bool {
        let changed = self.editor.redo();
        if changed {
            self.refresh();
        }
        changed
    }
}

fn debug_op(op: ConditionOp) -> &'static str {
    match op {
        ConditionOp::Eq => "==",
        ConditionOp::Ne => "!=",
        ConditionOp::Gt => ">",
        ConditionOp::Lt => "<",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_workflow::graph::WorkflowGraph;

    fn surface() -> EditorSurface {
        EditorSurface::new(GraphEditor::new(
            WorkflowGraph::new("wf.e", "E", "a").with_node(launcher_workflow::graph::WorkflowNode {
                node_id: "a".into(),
                action_ref: "cmd:a".into(),
                output_variables: vec![],
                condition: None,
                approval: false,
            }),
        ))
    }

    /// E02: every mutation refreshes the projected rows (sorted).
    #[test]
    fn rows_are_refreshed_projection() {
        let mut s = surface();
        assert_eq!(s.rows.len(), 1);
        s.add_node("b", "cmd:b").unwrap();
        s.add_edge("a", "b").unwrap();
        assert_eq!(s.rows.len(), 2);
        assert_eq!(s.rows[0].node_id, "a");
        assert_eq!(s.rows[1].node_id, "b");
    }

    /// E07: undo/redo update the projection too.
    #[test]
    fn undo_redo_refresh_rows() {
        let mut s = surface();
        s.add_node("b", "cmd:b").unwrap();
        assert_eq!(s.rows.len(), 2);
        assert!(s.undo());
        assert_eq!(s.rows.len(), 1, "undo updates the VIEW");
        assert!(s.redo());
        assert_eq!(s.rows.len(), 2, "redo updates the VIEW");
    }

    /// E05 验证面前身：编辑器状态非法（如暂不可达节点）时投影可携带错误。
    #[test]
    fn invalid_draft_is_detectable() {
        let mut s = surface();
        s.add_node("orphan", "cmd:o").unwrap();
        // the draft is allowed, but the host can detect invalidity for the
        // validation surface (E05) by validating the graph
        let valid = launcher_workflow::graph::validate(&s.editor.graph).is_ok();
        assert!(!valid, "draft with unreachable node is detectable");
    }
}
