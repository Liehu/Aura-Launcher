//! E03/E04 binding proof: the GraphEditorSurface (now embedded as an
//! AppWindow mode) accepts the host-projected row model, exposes row data
//! to Slint, and the host-facing editor properties round-trip.

use launcher_ui::EditorRowItem;
use slint::{ComponentHandle as _, Model};

#[test]
fn editor_surface_binds_rows() {
    let ui = launcher_ui::AppWindow::new().unwrap();
    // host projection: rows from EditorSurface.rows (EditorRow DTOs)
    let rows = vec![
        EditorRowItem {
            node_id: "a".into(),
            action_ref: "cmd:a".into(),
            condition: String::new().into(),
        },
        EditorRowItem {
            node_id: "b".into(),
            action_ref: "cmd:b".into(),
            condition: "count > 3".into(),
        },
    ];
    let model = std::rc::Rc::new(slint::VecModel::from(rows.clone()));
    ui.set_editor_rows(slint::ModelRc::from(model.clone()));

    assert_eq!(ui.get_editor_rows().row_count(), 2, "row model bound");
    let row = ui.get_editor_rows().row_data(1).expect("row 1 exists");
    assert_eq!(row.node_id, "b");
    assert_eq!(row.condition, "count > 3");

    // mode flag + undo/redo flags round-trip through the AppWindow
    ui.set_editor_visible(true);
    assert!(ui.get_editor_visible());
    ui.set_editor_can_undo(true);
    ui.set_editor_can_redo(false);
    assert!(ui.get_editor_can_undo());
    assert!(!ui.get_editor_can_redo());
    ui.hide().unwrap();
}
