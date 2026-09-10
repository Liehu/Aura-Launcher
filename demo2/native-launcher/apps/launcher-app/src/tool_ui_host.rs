//! P3-UI.1 Tool UI Host: renders a declarative UiSchema tree as Slint
//! components in the popup's ToolSurface, and routes user interactions
//! back to the plugin via `tool.event` IPC.
//!
//! Pure projection: the host flattens the `UiSchema` tree into a node list
//! for Slint; interaction callbacks route to the plugin via `tool.event`;
//! `tool.update` responses refresh the node list. No widget handles cross
//! the plugin boundary (P3UI-010).

use std::sync::Arc;
use std::sync::Mutex;

use launcher_domain::tool::UiSchema;
use slint::Weak;

/// Flatten a `UiSchema` tree into a depth-encoded node list for Slint.
fn flatten(schema: &UiSchema) -> Vec<launcher_ui::ToolNodeItem> {
    fn walk(
        node: &launcher_domain::tool::UiNode,
        depth: i32,
        out: &mut Vec<launcher_ui::ToolNodeItem>,
    ) {
        let title = node
            .title
            .clone()
            .or_else(|| node.text.clone())
            .unwrap_or_default();
        let value = match &node.value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        out.push(launcher_ui::ToolNodeItem {
            node_id: node.id.clone().into(),
            kind: node.kind.clone().into(),
            title: title.into(),
            value: value.into(),
            depth,
            readonly: node.readonly.unwrap_or(false),
        });
        for child in &node.children {
            walk(child, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(&schema.root, 0, &mut out);
    out
}

/// The active tool session (one at a time in v1).
pub struct ActiveToolSession {
    pub session_id: String,
    pub plugin_id: String,
    pub tool_id: String,
}

static SESSION: std::sync::OnceLock<Mutex<Option<ActiveToolSession>>> =
    std::sync::OnceLock::new();

fn session_slot() -> &'static Mutex<Option<ActiveToolSession>> {
    SESSION.get_or_init(|| Mutex::new(None))
}

/// Open a tool surface: sends `tool.open` to the plugin and renders the
/// returned UI schema on the popup's tool surface.
pub fn open_tool(
    handle: &mut launcher_plugin_host::PluginHandle,
    state: Arc<Mutex<AppState>>,
    ui_weak: Weak<launcher_ui::AppWindow>,
    tool_id: &str,
    initial_input: serde_json::Value,
) {
    let session_id = format!("ts-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0));
    let context = serde_json::json!({});
    match handle.tool_open(&session_id, tool_id, context, initial_input) {
        Ok(open) => {
            let schema = UiSchema {
                schema_version: launcher_domain::tool::UI_SCHEMA_VERSION,
                root: open_ui_root(&open.ui),
            };
            if let Err(e) = schema.validate() {
                tracing::warn!(error = %e, "tool ui schema invalid");
                crate::set_status(ui_weak, format!("⚠ Tool UI invalid: {e}"));
                return;
            }
            let nodes = flatten(&schema);
            let title = tool_id.to_string();
            if let Ok(mut s) = session_slot().lock() {
                *s = Some(ActiveToolSession {
                    session_id: session_id.clone(),
                    plugin_id: handle.manifest_id().to_string(),
                    tool_id: tool_id.to_string(),
                });
            }
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let _ = ui.show();
                    ui.set_tool_visible(true);
                    ui.set_tool_title(title.into());
                    ui.set_tool_status("ready".into());
                    ui.set_tool_nodes(slint::ModelRc::new(std::rc::Rc::new(
                        slint::VecModel::from(nodes),
                    )));
                    ui.invoke_focus_keys();
                }
            });
        }
        Err(e) => {
            tracing::warn!(error = %e, "tool.open failed");
            crate::set_status(ui_weak, format!("⚠ tool open failed: {e}"));
        }
    }
}

/// Extract the root node from a raw tool.open UI JSON value.
fn open_ui_root(ui: &serde_json::Value) -> launcher_domain::tool::UiNode {
    serde_json::from_value::<launcher_domain::tool::UiSchema>(ui.clone())
        .map(|s| s.root)
        .unwrap_or_else(|_| {
            // try wrapping as root if the plugin returned a bare node
            serde_json::from_value::<launcher_domain::tool::UiNode>(ui.clone())
                .unwrap_or(launcher_domain::tool::UiNode {
                    kind: "text".into(),
                    id: "fallback".into(),
                    title: Some("(no UI returned)".into()),
                    text: None,
                    value: serde_json::Value::Null,
                    readonly: None,
                    children: vec![],
                    properties: Default::default(),
                })
        })
}


/// Alias so the flatten fn signature works.

/// Relay a tool event to the plugin (called from the ToolSurface callback).
pub fn relay_event(
    _handle: &mut launcher_plugin_host::PluginHandle,
    ui_weak: Weak<launcher_ui::AppWindow>,
    node_id: &str,
    event: &str,
    value: &str,
) {
    let (Some(session), _) = (
        session_slot().lock().ok().and_then(|s| s.clone()),
        (),
    ) else { return };
    let e = launcher_domain::tool::UiEvent {
        session_id: session.session_id.clone(),
        event_id: format!("ev-{}-{}", node_id, std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0)),
        node_id: node_id.to_string(),
        event: event.to_string(),
        value: serde_json::Value::String(value.to_string()),
    };
    match handle.tool_event(&e) {
        Ok(result) => {
            // check for a UI update in the response
            if let Some(update) = result.get("ui") {
                let schema = UiSchema {
                    schema_version: 1,
                    root: open_ui_root(update),
                };
                let nodes = flatten(&schema);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_tool_nodes(slint::ModelRc::new(std::rc::Rc::new(
                            slint::VecModel::from(nodes),
                        )));
                    }
                });
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "tool.event failed");
        }
    }
}


