//! P3-UI contract domain types (P3UI-A/C, spec `P3-UI.0 Contract
//! Foundation` §3-§4/§25-§30): Tool / ToolFunction / ToolReference /
//! ToolSession / UiSchema / UiNode / UiEvent — pure model, no IO, no UI,
//! no effects.
//!
//! Invariants (spec §4, verbatim):
//! - Tool is an interactive capability, not an Effect (P3UI-001);
//! - ToolFunction is a logical callable capability (P3UI-002);
//! - ToolSession is UI/session state, not authorization (P3UI-003);
//! - UiEvent is input, not Effect (P3UI-005);
//! - UI node identity is session-scoped (P3UI-006);
//! - Workflow references ToolFunction, never UiNode (P3UI-007);
//! - every externally supplied UI object is validated before rendering
//!   (P3UI-013) — see `UiSchema::validate`.
//!
//! Rendering is host-side (Slint); these types never carry widget handles.

use serde::{Deserialize, Serialize};

pub const UI_SCHEMA_VERSION: u32 = 1;
pub const MAX_UI_NODES: usize = 256;
pub const MAX_UI_DEPTH: usize = 16;
pub const MAX_TEXT_CHARS: usize = 8_192;
pub const MAX_ID_CHARS: usize = 128;

/// Validated tool id: `[a-zA-Z0-9._-]+`, ≤128 chars (spec §33).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(String);

impl ToolId {
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.is_empty() || raw.chars().count() > MAX_ID_CHARS {
            return Err(format!("invalid tool id length: {raw:?}"));
        }
        if !raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(format!("invalid tool id characters: {raw:?}"));
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Function id within a tool (≤128 chars, fail-closed on empty).
pub type ToolFunctionId = String;

/// P3UI-G: what a Workflow stores to reference a tool function — logical
/// identity only, never a session/node/effect (P3UI-007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolReference {
    pub plugin_id: String,
    pub tool_id: String,
    pub function_id: ToolFunctionId,
}

/// One interactive tool declared by a plugin manifest (spec §33 schema).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub id: ToolId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub schema_version: u32,
    /// v1: only `interactive` entry types exist.
    #[serde(default = "default_entry_type")]
    pub entry_type: String,
}

fn default_entry_type() -> String {
    "interactive".into()
}

impl ToolDefinition {
    /// Deterministic, fail-closed validation (spec §33).
    pub fn validate(&self) -> Result<(), String> {
        // serde-derived ToolId bypasses parse() — re-validate here (fail closed)
        ToolId::parse(&self.id.0)?;
        if self.name.is_empty() || self.name.chars().count() > 256 {
            return Err("invalid tool name".into());
        }
        if self.description.as_deref().map(|d| d.chars().count() > 4_096).unwrap_or(false) {
            return Err("description too long".into());
        }
        if self.schema_version != 1 {
            return Err(format!("unsupported tool schema_version {}", self.schema_version));
        }
        if self.entry_type != "interactive" {
            return Err(format!("unknown entry_type {}", self.entry_type));
        }
        Ok(())
    }
}

/// Tool function declaration (spec §34): a logical callable with
/// input/output schemas and purity metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolFunction {
    pub id: ToolFunctionId,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub deterministic: bool,
    pub side_effect: bool,
    pub required_capabilities: Vec<String>,
}

impl ToolFunction {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.id.chars().count() > MAX_ID_CHARS {
            return Err("invalid function id".into());
        }
        if !self.input_schema.is_object() || !self.output_schema.is_object() {
            return Err("schemas must be objects".into());
        }
        Ok(())
    }
}

// ---- UI model -------------------------------------------------------------

/// UI node kinds frozen for P3-UI 1.0 (spec §36). Unknown kinds from
/// plugins are REJECTED at validation (fail closed), not skipped — the
/// primitive set is frozen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiNodeKind {
    Container,
    Section,
    Text,
    Markdown,
    Icon,
    Image,
    TextField,
    PasswordField,
    TextArea,
    Dropdown,
    Checkbox,
    Radio,
    Toggle,
    NumberInput,
    Button,
    ActionButton,
    List,
    ListItem,
    Detail,
    KeyValue,
    Progress,
    Status,
    Badge,
    Separator,
    Spacer,
}

fn node_kind_parse(s: &str) -> Option<UiNodeKind> {
    Some(match s {
        "container" => UiNodeKind::Container,
        "section" => UiNodeKind::Section,
        "text" => UiNodeKind::Text,
        "markdown" => UiNodeKind::Markdown,
        "icon" => UiNodeKind::Icon,
        "image" => UiNodeKind::Image,
        "text_field" => UiNodeKind::TextField,
        "password_field" => UiNodeKind::PasswordField,
        "text_area" => UiNodeKind::TextArea,
        "dropdown" => UiNodeKind::Dropdown,
        "checkbox" => UiNodeKind::Checkbox,
        "radio" => UiNodeKind::Radio,
        "toggle" => UiNodeKind::Toggle,
        "number_input" => UiNodeKind::NumberInput,
        "button" => UiNodeKind::Button,
        "action_button" => UiNodeKind::ActionButton,
        "list" => UiNodeKind::List,
        "list_item" => UiNodeKind::ListItem,
        "detail" => UiNodeKind::Detail,
        "key_value" => UiNodeKind::KeyValue,
        "progress" => UiNodeKind::Progress,
        "status" => UiNodeKind::Status,
        "badge" => UiNodeKind::Badge,
        "separator" => UiNodeKind::Separator,
        "spacer" => UiNodeKind::Spacer,
        _ => return None,
    })
}

/// One declarative UI node. Properties ride `properties` (spec allows
/// additionalProperties); identity is `id`, nesting via `children`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNode {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub value: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readonly: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<UiNode>,
    #[serde(flatten)]
    pub properties: serde_json::Map<String, serde_json::Value>,
}

/// A declarative UI tree (spec §35): version + root node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiSchema {
    pub schema_version: u32,
    pub root: UiNode,
}

impl UiSchema {
    /// P3UI-E: host-side validation BEFORE rendering (P3UI-013). Fails
    /// closed on: unknown node kinds, node count, depth, text length,
    /// duplicate sibling ids.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != UI_SCHEMA_VERSION {
            return Err(format!("unsupported ui schema_version {}", self.schema_version));
        }
        let mut ids = std::collections::HashSet::new();
        validate_node(&self.root, &mut ids, 1, &mut 0usize)?;
        Ok(())
    }
}

fn validate_node(
    node: &UiNode,
    ids: &mut std::collections::HashSet<String>,
    depth: usize,
    count: &mut usize,
) -> Result<(), String> {
    if depth > MAX_UI_DEPTH {
        return Err(format!("ui depth exceeds {MAX_UI_DEPTH}"));
    }
    *count += 1;
    if *count > MAX_UI_NODES {
        return Err(format!("ui node count exceeds {MAX_UI_NODES}"));
    }
    if node.kind.is_empty() || node.id.is_empty() || node.id.chars().count() > MAX_ID_CHARS {
        return Err(format!("invalid node id {:?}", node.id));
    }
    if node_kind_parse(&node.kind).is_none() {
        return Err(format!("unknown ui node kind {:?}", node.kind));
    }
    if !ids.insert(node.id.clone()) {
        // session-scoped uniqueness (P3UI-006): reject duplicate ids
        return Err(format!("duplicate ui node id {:?}", node.id));
    }
    if let Some(t) = &node.text {
        if t.chars().count() > MAX_TEXT_CHARS {
            return Err("node text too long".into());
        }
    }
    for child in &node.children {
        validate_node(child, ids, depth + 1, count)?;
    }
    Ok(())
}

// ---- events ---------------------------------------------------------------

/// Frozen event vocabulary (spec §37).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEventKind {
    Click,
    Change,
    Submit,
    Select,
    Focus,
    Blur,
    Refresh,
    Copy,
    Paste,
    Close,
}

pub fn event_kind_parse(s: &str) -> Option<UiEventKind> {
    Some(match s {
        "click" => UiEventKind::Click,
        "change" => UiEventKind::Change,
        "submit" => UiEventKind::Submit,
        "select" => UiEventKind::Select,
        "focus" => UiEventKind::Focus,
        "blur" => UiEventKind::Blur,
        "refresh" => UiEventKind::Refresh,
        "copy" => UiEventKind::Copy,
        "paste" => UiEventKind::Paste,
        "close" => UiEventKind::Close,
        _ => return None,
    })
}

/// A UI event relayed to the plugin (§28): host rejects duplicates, events
/// for closed sessions, and events for unknown nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiEvent {
    pub session_id: String,
    pub event_id: String,
    pub node_id: String,
    pub event: String,
    #[serde(default)]
    pub value: serde_json::Value,
}

impl UiEvent {
    pub fn validate(&self) -> Result<UiEventKind, String> {
        if self.session_id.is_empty()
            || self.event_id.is_empty()
            || self.event_id.chars().count() > MAX_ID_CHARS
            || self.node_id.is_empty()
        {
            return Err("invalid ui event identity".into());
        }
        event_kind_parse(&self.event).ok_or_else(|| format!("unknown event {}", self.event))
    }
}

// ---- session identity + state machine (P3UI-C, §25/§26/§29) --------------

/// Host-generated session identity (§29): the plugin NEVER chooses the
/// authoritative session id.
pub type ToolSessionId = String;

/// UI node id — session-scoped (§30). Workflows must never persist this id
/// (P3UI-006).
pub type UiNodeId = String;

/// §25 ToolSession lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSessionState {
    Created,
    Starting,
    Ready,
    Active,
    Idle,
    Closing,
    Closed,
    Failed,
    Crashed,
    Timeout,
}

impl ToolSessionState {
    /// §26 legal transitions (fail-closed). `Closed` is terminal.
    pub fn can_transition_to(&self, to: ToolSessionState) -> bool {
        use ToolSessionState::*;
        if to == Closed {
            return !matches!(self, Closed);
        }
        matches!(
            (self, to),
            (Created, Starting)
                | (Starting, Ready)
                | (Starting, Failed)
                | (Starting, Crashed)
                | (Starting, Timeout)
                | (Ready, Active)
                | (Ready, Closed)
                | (Active, Idle)
                | (Active, Failed)
                | (Active, Crashed)
                | (Active, Timeout)
                | (Active, Closing)
                | (Idle, Active)
                | (Idle, Closing)
                | (Idle, Closed)
                | (Closing, Closed)
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, ToolSessionState::Closed)
    }
}

/// §27: ui_generation rules — accepted update +1, rejected/stale unchanged,
/// closed session takes no more updates.
#[derive(Debug, Clone)]
pub struct UiGeneration {
    value: u64,
    closed: bool,
}

impl Default for UiGeneration {
    fn default() -> Self {
        Self::new()
    }
}

impl UiGeneration {
    pub fn new() -> Self {
        Self { value: 0, closed: false }
    }

    pub fn get(&self) -> u64 {
        self.value
    }

    /// Accept an update carrying `base_generation`. Stale/mismatched bases
    /// and closed sessions are rejected (§27/§46: no implicit merge).
    pub fn accept_update(&mut self, base_generation: u64) -> Result<u64, String> {
        if self.closed {
            return Err("session closed".into());
        }
        if base_generation != self.value {
            return Err(format!(
                "stale update: base {base_generation} != current {}",
                self.value
            ));
        }
        self.value += 1;
        Ok(self.value)
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool() -> ToolDefinition {
        serde_json::from_value(json!({
            "id": "base64",
            "name": "Base64",
            "schema_version": 1,
            "entry": { "type": "interactive" }
        }))
        .unwrap()
    }

    /// §33: valid tool schema parses and validates.
    #[test]
    fn tool_definition_valid() {
        let t = tool();
        t.validate().unwrap();
        assert_eq!(t.id.as_str(), "base64");
    }

    /// §33 fail-closed: wrong version, unknown entry, bad id characters.
    #[test]
    fn tool_definition_fail_closed() {
        let mut t = tool();
        t.schema_version = 2;
        assert!(t.validate().is_err());
        let mut t = tool();
        t.entry_type = "daemon".into();
        assert!(t.validate().is_err());
        let t: ToolDefinition =
            serde_json::from_value(json!({"id": "bad id!", "name": "x", "schema_version": 1, "entry": {"type": "interactive"}}))
                .unwrap();
        assert!(t.validate().is_err());
    }

    fn node(kind: &str, id: &str, children: Vec<UiNode>) -> UiNode {
        UiNode {
            kind: kind.into(),
            id: id.into(),
            title: None,
            text: None,
            value: serde_json::Value::Null,
            readonly: None,
            children,
            properties: Default::default(),
        }
    }

    fn schema(root: UiNode) -> UiSchema {
        UiSchema { schema_version: 1, root }
    }

    /// P3UI-E: unknown node kind fails closed (frozen primitive set).
    #[test]
    fn ui_schema_rejects_unknown_kind() {
        let s = schema(node("hologram", "a", vec![]));
        assert!(s.validate().is_err());
        let s = schema(node("text", "a", vec![node("button", "b", vec![])]));
        s.validate().unwrap();
    }

    /// §17: node count and depth limits are enforced.
    #[test]
    fn ui_schema_bounds() {
        // depth: chain of MAX_UI_DEPTH+1 nodes
        let mut n = node("text", "leaf", vec![]);
        for i in 0..MAX_UI_DEPTH + 1 {
            n = node("container", &format!("c{i}"), vec![n]);
        }
        assert!(schema(n).validate().is_err());
        // count: fan-out beyond MAX_UI_NODES
        let mut kids = Vec::new();
        for i in 0..MAX_UI_NODES + 1 {
            kids.push(node("text", &format!("n{i}"), vec![]));
        }
        assert!(schema(node("container", "root", kids)).validate().is_err());
    }

    /// Duplicate node ids within one schema are rejected (P3UI-006 scoping).
    #[test]
    fn ui_schema_duplicate_ids_rejected() {
        let s = schema(node("container", "root", vec![
            node("text", "dup", vec![]),
            node("button", "dup", vec![]),
        ]));
        assert!(s.validate().is_err());
    }

    /// §28: event identity + vocabulary validation.
    #[test]
    fn ui_event_validation() {
        let e = UiEvent {
            session_id: "ts-1".into(),
            event_id: "ev-1".into(),
            node_id: "encode".into(),
            event: "click".into(),
            value: serde_json::Value::Null,
        };
        assert_eq!(e.validate().unwrap(), UiEventKind::Click);
        let bad = UiEvent { event: "explode".into(), ..e };
        assert!(bad.validate().is_err());
    }
}
