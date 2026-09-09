//! Tool Catalog Projection (P27-B01, spec `P2.7 开发设计规范` §13/§14):
//! the single planner-facing view over every executable tool the host
//! publishes — actions AND workflows — as neutral, bounded, deterministic
//! entries keyed by a stable `ref` the model echoes back in plan steps.
//!
//! Red lines (§13): catalog visibility ≠ capability grant. The projection
//! drops all authority state (disabled reasons, confirmation flags,
//! resolution caches); presence in this catalog says NOTHING about
//! authorization — the Resolver/Policy chain still gates every execution.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use launcher_workflow::proposal::ActionCatalogItem;

/// The `ref` separator. Provider ids may contain `:` (e.g. `mcp:calc`) but
/// never `|`, so refs split unambiguously.
pub const REF_SEP: char = '|';

/// One planner-facing tool. `ref` is the identity the plan schema (B02)
/// and validator (B03) speak in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCatalogEntry {
    /// Stable routing key: `provider|command|action` (actions) or
    /// `workflow|<definition id>` (workflows).
    pub r#ref: String,
    pub kind: ToolKind,
    pub title: String,
 #[serde(default)]
    pub description: Option<String>,
    /// Semantic routing type of actions (`system.open`, `plugin.invoke`, …).
 #[serde(default)]
    pub action_type: Option<String>,
    /// JSON Schema of the action input when the provider publishes one.
    /// Schema aids argument generation; it is NEVER execution-input
    /// authority (§10) — the Resolver + Executor input validation remain
    /// the only gates.
 #[serde(default)]
    pub input_schema: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Action,
    Workflow,
}

/// The full projected catalog. A BTreeMap keyed by ref: deterministic
/// iteration order regardless of provider query order, and duplicate refs
/// collapse (last wins — providers are deduped upstream by identity).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolCatalog {
    entries: BTreeMap<String, ToolCatalogEntry>,
}

/// Projection caps (§13 bounded output): max entries and max serialized
/// characters; truncation drops whole entries, never emits partial JSON.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionLimits {
    pub max_entries: usize,
    pub max_chars: usize,
}

impl Default for ProjectionLimits {
    fn default() -> Self {
        Self { max_entries: 200, max_chars: 16_000 }
    }
}

/// Build the `ref` for one action catalog item (the same key the host's
/// TurnExecutor resolves back to an ActionProposal).
pub fn action_ref(provider_id: &str, command_id: &str, action_id: &str) -> String {
    format!("{provider_id}{REF_SEP}{command_id}{REF_SEP}{action_id}")
}

/// Build the `ref` for an installed workflow.
pub fn workflow_ref(definition_id: &str) -> String {
    format!("workflow{REF_SEP}{definition_id}")
}

impl ToolCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, r#ref: &str) -> Option<&ToolCatalogEntry> {
        self.entries.get(r#ref)
    }

    /// Deterministic iteration (sorted by ref).
    pub fn entries(&self) -> impl Iterator<Item = &ToolCatalogEntry> {
        self.entries.values()
    }

    pub fn insert(&mut self, entry: ToolCatalogEntry) {
        self.entries.insert(entry.r#ref.clone(), entry);
    }

    /// B01: project action catalog items + installed workflow summaries
    /// into one catalog. Deterministic: the BTreeMap absorbs provider
    /// query order.
    pub fn project(
        actions: &[ActionCatalogItem],
        workflows: &[WorkflowToolSummary],
    ) -> Self {
        let mut cat = Self::new();
        for a in actions {
            cat.insert(ToolCatalogEntry {
                r#ref: action_ref(&a.provider_id, &a.command_id, &a.action_id),
                kind: ToolKind::Action,
                title: a.title.clone(),
                description: a.description.clone(),
                action_type: Some(a.action_type.clone()),
                input_schema: a.input_schema.clone(),
            });
        }
        for w in workflows {
            cat.insert(ToolCatalogEntry {
                r#ref: workflow_ref(&w.definition_id),
                kind: ToolKind::Workflow,
                title: w.name.clone(),
                description: w.description.clone(),
                action_type: None,
                input_schema: None,
            });
        }
        cat
    }

    /// Bounded serialization for the prompt (§14 catalog-bound selection):
    /// entries sorted by ref, truncated to the limits. Returns the JSON
    /// string plus how many entries fit.
    pub fn render_json(&self, limits: &ProjectionLimits) -> (String, usize) {
        let mut kept: Vec<&ToolCatalogEntry> = Vec::new();
        let mut chars = 2usize; // "[]"
        for e in self.entries() {
            let Ok(json) = serde_json::to_string(e) else { continue };
            let extra = if kept.is_empty() { json.len() } else { json.len() + 1 };
            if kept.len() >= limits.max_entries || chars + extra > limits.max_chars {
                break;
            }
            chars += extra;
            kept.push(e);
        }
        let items: Vec<serde_json::Value> = kept
            .iter()
            .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
            .collect();
        let rendered = serde_json::to_string(&items).unwrap_or_else(|_| "[]".into());
        (rendered, kept.len())
    }
}

/// Workflow summary the host feeds into the projection (id/name only —
/// the definition body is host-side, never shipped to the model).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowToolSummary {
    pub definition_id: String,
    pub name: String,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category, Command};

    fn item(provider: &str, cmd: &str, action: &str) -> ActionCatalogItem {
        let c = Command {
            id: cmd.into(),
            title: format!("cmd {cmd}"),
            subtitle: None,
            icon: None,
            provider_id: provider.into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Command,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: Some(action.into()),
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        };
        ActionCatalogItem::items_from_command(&c).remove(0)
    }

    /// B01: refs are stable and round-trip through the catalog.
    #[test]
    fn refs_are_stable_and_lookup_works() {
        let a = item("mcp:calc", "evaluate", "invoke");
        let cat = ToolCatalog::project(&[a], &[WorkflowToolSummary {
            definition_id: "wf.demo".into(),
            name: "Demo".into(),
            description: None,
        }]);
        assert_eq!(cat.len(), 2);
        let r = action_ref("mcp:calc", "evaluate", "invoke");
        assert_eq!(r, "mcp:calc|evaluate|invoke");
        let e = cat.get(&r).expect("action entry");
        assert_eq!(e.kind, ToolKind::Action);
        assert_eq!(e.action_type.as_deref(), Some("system.open"));
        let w = cat.get(&workflow_ref("wf.demo")).expect("workflow entry");
        assert_eq!(w.kind, ToolKind::Workflow);
        assert_eq!(w.title, "Demo");
    }

    /// B01: projection is deterministic and bounded (char cap drops whole
    /// entries, never emits partial JSON).
    #[test]
    fn render_is_bounded_and_deterministic() {
        let actions: Vec<ActionCatalogItem> =
            (0..50).map(|i| item("p", &format!("c{i}"), "run")).collect();
        let cat = ToolCatalog::project(&actions, &[]);
        let limits = ProjectionLimits { max_entries: 200, max_chars: 600 };
        let (json1, n1) = cat.render_json(&limits);
        let (json2, n2) = cat.render_json(&limits);
        assert_eq!(json1, json2);
        assert!(n1 < 50, "char budget must truncate: {n1}");
        assert!(json1.len() <= 600 + 2);
        assert!(serde_json::from_str::<Value>(&json1).is_ok(), "partial JSON leaked");
        // sorted-by-ref determinism
        assert!(json1.contains("c0") || json1.contains("c1"));
    }
}
