//! P3-UI.0-H contract test kit (`P3-UI.0 Contract Foundation` §14): reusable
//! fixtures for the plugin/tool contract matrix — valid, malformed, missing
//! required, wrong version, oversized, duplicate id, unknown kind.
//!
//! Fixtures are DATA ONLY: no IO, no process spawning. Consumers (host
//! integration tests) decide how to run them.

use launcher_domain::tool::{ToolDefinition, UiEvent, UiSchema};
use serde_json::{json, Value};

// ---- manifest fixtures ---------------------------------------------------

/// A valid v2 manifest for `id` with a process runtime (executable must
/// exist relative to the plugin dir at spawn time).
pub fn valid_manifest_json(id: &str) -> String {
    json!({
        "schema_version": 1,
        "id": id,
        "name": format!("Test {id}"),
        "version": "1.0.0",
        "api_version": "0.1",
        "runtime": { "type": "process", "executable": &format!("{id}.exe") },
        "capabilities": [],
        "timeout_ms": 3000,
        "idle_timeout_ms": 10000
    })
    .to_string()
}

/// Wrong api_version (host speaks 0.1).
pub fn wrong_version_manifest_json(id: &str) -> String {
    let mut v: Value = serde_json::from_str(&valid_manifest_json(id)).unwrap();
    v["api_version"] = json!("9.9");
    v.to_string()
}

/// Missing the required `id` field.
pub fn missing_id_manifest_json() -> String {
    let mut v: Value = serde_json::from_str(&valid_manifest_json("x")).unwrap();
    v.as_object_mut().unwrap().remove("id");
    v.to_string()
}

// ---- tool fixtures -------------------------------------------------------

/// A valid interactive tool definition.
pub fn valid_tool_definition(id: &str) -> ToolDefinition {
    serde_json::from_value(json!({
        "id": id,
        "name": format!("Tool {id}"),
        "schema_version": 1,
        "entry": { "type": "interactive" }
    }))
    .expect("valid tool definition")
}

/// Wrong schema_version (contract: only 1).
pub fn wrong_version_tool_definition(id: &str) -> ToolDefinition {
    let mut t = valid_tool_definition(id);
    t.schema_version = 9;
    t
}

// ---- UI schema fixtures --------------------------------------------------

/// §78 Base64-style layout: section > input + buttons + output.
pub fn base64_ui_schema() -> UiSchema {
    serde_json::from_value(json!({
        "schema_version": 1,
        "root": { "type": "section", "id": "main", "children": [
            { "type": "text_area", "id": "input" },
            { "type": "container", "id": "ops", "children": [
                { "type": "button", "id": "encode" },
                { "type": "button", "id": "decode" }
            ]},
            { "type": "text_area", "id": "output", "readonly": true }
        ]}
    }))
    .expect("valid ui schema")
}

/// Oversized tree: more nodes than the host limit.
pub fn oversized_ui_schema(max_nodes: usize) -> UiSchema {
    let mut kids = Vec::new();
    for i in 0..max_nodes {
        kids.push(json!({"type": "text", "id": format!("n{i}")}));
    }
    serde_json::from_value(json!({
        "schema_version": 1,
        "root": { "type": "container", "id": "root", "children": kids }
    }))
    .expect("valid json")
}

/// Unknown node kind (frozen primitive set violation).
pub fn unknown_kind_ui_schema() -> UiSchema {
    serde_json::from_value(json!({
        "schema_version": 1,
        "root": { "type": "hologram", "id": "a" }
    }))
    .expect("valid json")
}

// ---- event fixtures ------------------------------------------------------

pub fn click_event(session_id: &str, node_id: &str) -> UiEvent {
    UiEvent {
        session_id: session_id.into(),
        event_id: format!("ev-{node_id}"),
        node_id: node_id.into(),
        event: "click".into(),
        value: serde_json::Value::Null,
    }
}

// ---- rich result fixtures -------------------------------------------------

/// A valid rich payload (text + key/value + divider + table).
pub fn rich_payload() -> Value {
    json!({ "blocks": [
        { "type": "text", "text": "= 80", "emphasis": "strong" },
        { "type": "key_value", "rows": [
            { "key": "expression", "value": "12+34*2" },
            { "key": "result kind", "value": "integer" }
        ]},
        { "type": "divider" },
        { "type": "table", "headers": ["supported", "ops"], "rows": [["basic", "+ - * / % ^"]] }
    ]})
}

/// Malformed rich payload: unknown block kind only.
pub fn malformed_rich_payload() -> Value {
    json!({ "blocks": [ { "type": "hologram" } ] })
}

// ---- python process fixtures ---------------------------------------------

/// Compliant python plugin: speaks initialize/query/shutdown; query returns
/// one plain command.
pub fn toggler_python() -> &'static str {
    "import json, sys\nfor l in sys.stdin:\n    r=json.loads(l)\n    if r.get(\"method\")==\"initialize\":\n        out={\"protocol_version\":r[\"params\"][\"protocol_version\"]}\n    else:\n        out={\"query_id\":r[\"params\"].get(\"query_id\"),\"commands\":[{\"id\":\"toggle\",\"title\":\"toggle\",\"actions\":[]}]}\n    sys.stdout.write(json.dumps({\"jsonrpc\":\"2.0\",\"id\":r[\"id\"],\"result\":out})+\"\\n\"); sys.stdout.flush()\n"
}

/// Compliant python plugin whose query response carries a rich payload.
pub fn rich_python() -> &'static str {
    "import json, sys\nfor l in sys.stdin:\n    r=json.loads(l)\n    if r.get(\"method\")==\"initialize\":\n        out={\"protocol_version\":r[\"params\"][\"protocol_version\"]}\n    else:\n        out={\"query_id\":r[\"params\"][\"query_id\"],\"commands\":[{\"id\":\"rich\",\"title\":\"Rich\",\"actions\":[],\"rich\":{\"blocks\":[{\"type\":\"text\",\"text\":\"hello rich\"}]}}]}\n    sys.stdout.write(json.dumps({\"jsonrpc\":\"2.0\",\"id\":r[\"id\"],\"result\":out})+\"\\n\"); sys.stdout.flush()\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::rich::RichResult;

    /// The fixture matrix branches from spec §14 are exercisable.
    #[test]
    fn fixture_matrix() {
        assert!(serde_json::from_str::<Value>(&valid_manifest_json("a")).is_ok());
        let wrong: Value = serde_json::from_str(&wrong_version_manifest_json("a")).unwrap();
        assert_eq!(wrong["api_version"], "9.9");
        let missing: Value = serde_json::from_str(&missing_id_manifest_json()).unwrap();
        assert!(missing.get("id").is_none());

        valid_tool_definition("t").validate().unwrap();
        assert!(wrong_version_tool_definition("t").validate().is_err());

        base64_ui_schema().validate().unwrap();
        assert!(unknown_kind_ui_schema().validate().is_err());
        assert!(oversized_ui_schema(300).validate().is_err()); // > 256 node cap

        assert!(click_event("s", "encode").validate().is_ok());
    }

    /// Rich fixtures parse via the domain contract.
    #[test]
    fn rich_fixtures_parse() {
        assert!(RichResult::from_value(&rich_payload()).is_some());
        assert!(RichResult::from_value(&malformed_rich_payload()).is_none());
    }
}
