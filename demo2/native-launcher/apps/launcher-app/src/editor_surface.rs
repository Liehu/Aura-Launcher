//! Editor surface projection (P2.6-E02 host binding, review 60 §58 ViewModel
//! pattern): the host projects `GraphEditor` state into plain DTO rows for
//! the Slint surface, and routes surface callbacks back into the editor.
//! launcher-ui never depends on launcher-workflow (UI dependency guard).
//!
//! Wired to the Slint surface in E03-E05 (next slice); until then only the
//! tests consume it, hence the dead_code allowance.

#![allow(dead_code)]

use std::sync::Mutex;

use slint::ComponentHandle as _;
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

/// E05 验证面: validate the draft graph and produce a human-readable
/// status line for the editor surface (empty = valid).
pub fn status_text(graph: &launcher_workflow::graph::WorkflowGraph) -> String {
    match launcher_workflow::graph::validate(graph) {
        Ok(()) => String::new(),
        Err(e) => format!("INVALID: {e}"),
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

// ---- E03–E05: Slint surface host binding --------------------------------

/// The session-scoped editor instance. One editor at a time (v1); the draft
/// survives closing the surface within the process lifetime.
static EDITOR: std::sync::OnceLock<Mutex<Option<EditorSurface>>> = std::sync::OnceLock::new();

fn editor_slot() -> &'static Mutex<Option<EditorSurface>> {
    EDITOR.get_or_init(|| Mutex::new(None))
}

fn draft_path() -> std::path::PathBuf {
    crate::data_dir().join("editor-draft.json")
}

/// Ensure the editor exists: import the last draft when present, else a
/// fresh single-node graph.
fn ensure_editor() {
    let mut slot = editor_slot().lock().expect("editor lock");
    if slot.is_some() {
        return;
    }
    let graph = std::fs::read_to_string(draft_path())
        .ok()
        .and_then(|raw| launcher_workflow::editor::import_json(&raw).ok())
        .unwrap_or_else(|| {
            launcher_workflow::graph::WorkflowGraph::new("wf.edited", "Edited Workflow", "start")
                .with_node(launcher_workflow::graph::WorkflowNode {
                    node_id: "start".into(),
                    action_ref: "cmd:open".into(),
                    output_variables: vec![],
                    condition: None,
                    approval: false,
                })
        });
    *slot = Some(EditorSurface::new(GraphEditor::new(graph)));
}

/// Push the current projection to the UI (rows/status/undo-redo flags).
fn push(ui: &slint::Weak<launcher_ui::AppWindow>) {
    let ui = ui.clone();
    let slot = editor_slot().lock().expect("editor lock");
    let Some(s) = slot.as_ref() else { return };
    let rows: Vec<launcher_ui::EditorRowItem> = s
        .rows
        .iter()
        .map(|r| launcher_ui::EditorRowItem {
            node_id: r.node_id.clone().into(),
            action_ref: r.action_ref.clone().into(),
            condition: r.condition.clone().into(),
        })
        .collect();
    let status = status_text(&s.editor.graph);
    let (can_undo, can_redo) = (s.editor.can_undo(), s.editor.can_redo());
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.upgrade() {
            ui.set_editor_rows(slint::ModelRc::new(std::rc::Rc::new(
                slint::VecModel::from(rows),
            )));
            ui.set_editor_status(status.into());
            ui.set_editor_can_undo(can_undo);
            ui.set_editor_can_redo(can_redo);
        }
    });
}

/// E03 entry: open the editor mode (importing the draft when one exists).
pub fn open_editor(ui_weak: slint::Weak<launcher_ui::AppWindow>) {
    ensure_editor();
    let ui2 = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui2.upgrade() {
            let _ = ui.show();
            ui.set_editor_visible(true);
            ui.invoke_focus_keys();
        }
    });
    push(&ui_weak);
}

/// E04: host-routed surface callbacks — every mutation re-projects.
pub fn add_node(node_id: &str, action_ref: &str, ui_weak: slint::Weak<launcher_ui::AppWindow>) {
    if let Ok(mut slot) = editor_slot().lock() {
        if let Some(s) = slot.as_mut() {
            let _ = s.add_node(node_id, action_ref);
        }
    }
    push(&ui_weak);
}

pub fn remove_node(node_id: &str, ui_weak: slint::Weak<launcher_ui::AppWindow>) {
    if let Ok(mut slot) = editor_slot().lock() {
        if let Some(s) = slot.as_mut() {
            let _ = s.remove_node(node_id);
        }
    }
    push(&ui_weak);
}

pub fn undo(ui_weak: slint::Weak<launcher_ui::AppWindow>) {
    if let Ok(mut slot) = editor_slot().lock() {
        if let Some(s) = slot.as_mut() {
            s.undo();
        }
    }
    push(&ui_weak);
}

pub fn redo(ui_weak: slint::Weak<launcher_ui::AppWindow>) {
    if let Ok(mut slot) = editor_slot().lock() {
        if let Some(s) = slot.as_mut() {
            s.redo();
        }
    }
    push(&ui_weak);
}

/// E05/E06: validate + persist the draft (same JSON contract the durable
/// definition store consumes); invalid graphs refuse to save (validate
/// runs before save, §10).
pub fn export_draft(ui_weak: slint::Weak<launcher_ui::AppWindow>) {
    let result: Result<String, String> = (|| {
        let slot = editor_slot().lock().expect("editor lock");
        let s = slot.as_ref().ok_or("editor not open")?;
        let json = launcher_workflow::editor::export_json(&s.editor.graph)?;
        std::fs::write(draft_path(), json).map_err(|e| e.to_string())?;
        Ok(format!("saved to {}", draft_path().display()))
    })();
    let status = match result {
        Ok(msg) => msg,
        Err(e) => format!("INVALID: {e}"),
    };
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_editor_status(status.into());
        }
    });
}
