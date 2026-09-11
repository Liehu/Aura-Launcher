//! P3-UI.1 Tool UI Host: renders a declarative UiSchema tree as Slint
//! components in the popup's ToolSurface, and routes user interactions
//! back to the plugin via `tool.event` IPC.
//!
//! Pure projection: the host flattens the `UiSchema` tree into a node list
//! for Slint; interaction callbacks route to the plugin via `tool.event`;
//! UI updates refresh the node list. No widget handles cross the plugin
//! boundary (P3UI-010).

use launcher_domain::tool::{UiNode, UiSchema};
use slint::{ComponentHandle as _, Weak};

use launcher_domain::{Category, Command, QueryContext};
use launcher_plugin_host::PluginHandle;

/// Flatten a `UiSchema` tree into a depth-encoded node list for Slint.
fn flatten(schema: &UiSchema) -> Vec<launcher_ui::ToolNodeItem> {
    fn walk(node: &UiNode, depth: i32, out: &mut Vec<launcher_ui::ToolNodeItem>) {
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

/// Extract the root node from a raw tool.open UI JSON value.
fn open_ui_root(ui: &serde_json::Value) -> UiNode {
    serde_json::from_value::<launcher_domain::tool::UiSchema>(ui.clone())
        .map(|s| s.root)
        .unwrap_or_else(|_| {
            serde_json::from_value::<UiNode>(ui.clone()).unwrap_or(UiNode {
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

// ---- session state (UI-thread lifetime) ---------------------------------

struct ToolSession {
    handle: PluginHandle,
    session_id: String,
    /// Kept for future tool.update / tool.close references.
    #[allow(dead_code)]
    tool_id: String,
}

thread_local! {
    static TOOL_SESSION: std::cell::RefCell<Option<ToolSession>> =
        const { std::cell::RefCell::new(None) };
}

/// Open a tool: spawn the plugin, send `tool.open`, render the UI schema.
/// The manifest path must point to a `plugin.json` with `window: true`.
pub fn open_tool(
    manifest_path: &std::path::Path,
    tool_id: &str,
    ui_weak: Weak<launcher_ui::AppWindow>,
) {
    let manifest_json = match std::fs::read_to_string(manifest_path) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(error = %e, "tool manifest read failed");
            return;
        }
    };
    let manifest: launcher_domain::PluginManifest =
        serde_json::from_str(&manifest_json).unwrap_or_else(|e| {
            tracing::error!(error = %e, "tool manifest parse failed");
            launcher_domain::PluginManifest::parse("{}").unwrap_or_else(|_| {
                launcher_domain::PluginManifest::parse(
                    r#"{"id":"error","name":"error","api_version":"0.1","timeout_ms":1000}"#,
                )
                .expect("error manifest")
            })
        });
    let base_dir = manifest_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let tool_id = tool_id.to_string();
    let ui_weak = ui_weak.clone();

    std::thread::Builder::new()
        .name("tool-opener".into())
        .spawn(move || {
            let mut handle = match PluginHandle::spawn(manifest, &base_dir) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!(error = %e, "tool plugin spawn failed");
                    return;
                }
            };
            let session_id = format!(
                "ts-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            );
            match handle.tool_open(
                &session_id,
                &tool_id,
                serde_json::json!({}),
                serde_json::json!({}),
            ) {
                Ok(open) => {
                    let root = open_ui_root(&open.ui);
                    let nodes = flatten(&UiSchema { schema_version: 1, root });
                    TOOL_SESSION.with(|slot| {
                        *slot.borrow_mut() = Some(ToolSession {
                            handle,
                            session_id,
                            tool_id: tool_id.clone(),
                        });
                    });
                    let title2 = tool_id.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let _ = ui.show();
                            ui.set_tool_visible(true);
                            ui.set_tool_title(title2.into());
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
                }
            }
        })
        .expect("spawn tool-opener");
}

/// Relay a tool event to the plugin (called from the ToolSurface callback
/// on the UI thread; the actual IPC runs on the session handle).
pub fn relay_event(
    ui_weak: Weak<launcher_ui::AppWindow>,
    node_id: &str,
    event: &str,
    value: &str,
) {
    TOOL_SESSION.with(|slot| {
        let mut borrow = slot.borrow_mut();
        let Some(session) = borrow.as_mut() else { return };
        let e = launcher_domain::tool::UiEvent {
            session_id: session.session_id.clone(),
            event_id: format!(
                "ev-{}-{}",
                node_id,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_micros())
                    .unwrap_or(0)
            ),
            node_id: node_id.to_string(),
            event: event.to_string(),
            value: serde_json::Value::String(value.to_string()),
        };
        match session.handle.tool_event(&e) {
            Ok(result) => {
                if let Some(update) = result.get("ui") {
                    let root = open_ui_root(update);
                    let nodes = flatten(&UiSchema { schema_version: 1, root });
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_tool_nodes(slint::ModelRc::new(std::rc::Rc::new(
                                slint::VecModel::from(nodes),
                            )));
                        }
                    });
                }
            }
            Err(e) => tracing::warn!(error = %e, "tool.event failed"),
        }
    });
}

/// Close the active tool session (send tool.close + cleanup).
pub fn close_session(ui_weak: Weak<launcher_ui::AppWindow>) {
    TOOL_SESSION.with(|slot| {
        if let Some(mut session) = slot.borrow_mut().take() {
            let _ = session.handle.tool_close(&session.session_id);
        }
    });
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_tool_visible(false);
        }
    });
}

/// Searchable entry command (P3-UI.1): surfaces "Open Base64 Tool" when
/// the user searches base64/tool. The manifest path is resolved from the
/// example-testplugins directory.
pub struct ToolCommandProvider;

impl launcher_core::Provider for ToolCommandProvider {
    fn id(&self) -> &str {
        "tool-open"
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        let n = q.normalized.clone();
        let hit = n.contains("base64") || n.contains("tool");
        if !hit {
            return vec![];
        }
        // resolve the example tool path relative to the workspace
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("example-testplugins")
            .join("base64_tool_plugin.json");
        if !manifest.exists() {
            return vec![];
        }
        vec![Command {
            id: "tool:open:base64".into(),
            title: "🔐 Open Base64 Tool".into(),
            subtitle: Some("Interactive Base64 encode/decode (declarative UI)".into()),
            icon: None,
            provider_id: PROVIDER_TOOL_OPEN.into(),
            score: 0.9,
            keywords: vec!["base64".into(), "tool".into(), "encode".into(), "decode".into()],
            category: Category::Command,
            actions: vec![launcher_domain::Action {
                kind: launcher_domain::ActionKind::Execute,
                payload: None,
                id: Some("open".into()),
                title: Some("Open tool".into()),
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: Some(manifest.display().to_string()),
        }]
    }
}

pub const PROVIDER_TOOL_OPEN: &str = "tool-open";
