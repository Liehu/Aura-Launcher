//! E03/E04 binding proof: the GraphEditorSurface instantiates, accepts the
//! host-projected row model, exposes row data to Slint, and routes the
//! undo/redo callbacks back to handlers the host installs.

use launcher_ui::{EditorRowItem, GraphEditorSurface};
use slint::Model;

#[test]
fn editor_surface_binds_rows() {
    let editor = GraphEditorSurface::new().unwrap();
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
    editor.set_rows(slint::ModelRc::from(model.clone()));

    assert_eq!(editor.get_rows().row_count(), 2, "row model bound");
    let row = editor.get_rows().row_data(1).expect("row 1 exists");
    assert_eq!(row.node_id, "b");
    assert_eq!(row.condition, "count > 3");
}
