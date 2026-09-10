//! Rich results (P3.2-B0, `docs/contracts/RICH-RESULT-v1.md`): read-only
//! structured content a plugin may attach to a result item. Rendered in the
//! detail pane; NEVER authority (renderers do not parse semantics, unknown
//! block types are skipped).
//!
//! Transport: plugin result items carry a `rich` JSON value; `from_value`
//! parses + validates + sanitizes leniently — ANY failure drops the rich
//! payload and keeps the plain item (fault containment, INV-031 family).
//!
//! Storage: a bounded process-local registry keyed by the stable command id
//! (the `Command` type stays frozen; rich rides a side channel like the
//! P3.1 pin table). Entries evict FIFO when the cap is hit.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub const MAX_BLOCKS: usize = 64;
pub const MAX_TABLE_ROWS: usize = 200;
pub const MAX_TEXT_CHARS: usize = 4_096;
/// Total rendered-character budget (mirrors the plugin frame cap scale).
pub const MAX_TOTAL_CHARS: usize = 256 * 1024;
const REGISTRY_CAP: usize = 512;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emphasis {
    #[default]
    None,
    Strong,
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValueRow {
    pub key: String,
    pub value: String,
}

/// Read-only content blocks (§2 of the contract). Unknown block types in
/// incoming JSON are SKIPPED (forward compatibility), never errors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RichBlock {
    Text {
        text: String,
        #[serde(default)]
        emphasis: Emphasis,
    },
    KeyValue { rows: Vec<KeyValueRow> },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Divider,
}

/// A validated rich payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichResult {
    pub blocks: Vec<RichBlock>,
}

/// Neutralize control characters and bound length (same policy family as
/// the P2.7 privacy sanitizer — content is untrusted plugin output).
fn clean(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    out.retain(|c| c == '\n' || c == '\r' || c == '\t' || !c.is_control());
    out
}

impl RichResult {
    /// Lenient parse from an untrusted JSON value: unknown block types are
    /// skipped, malformed entries drop the WHOLE payload (keep the plain
    /// item), and every string is sanitized + bounded. `None` = no/invalid
    /// rich content.
    pub fn from_value(v: &Value) -> Option<Self> {
        let raw = v.as_object()?.get("blocks")?.as_array()?;
        let mut blocks = Vec::new();
        let mut total = 0usize;
        for b in raw.iter().take(MAX_BLOCKS * 2) {
            let block = match serde_json::from_value::<RichBlock>(b.clone()) {
                Ok(b) => b,
                Err(_) => continue, // unknown type: skip, keep going
            };
            match &block {
                RichBlock::Text { text, .. } => total += text.chars().count(),
                RichBlock::KeyValue { rows } => {
                    total += rows.iter().map(|r| r.key.chars().count() + r.value.chars().count()).sum::<usize>();
                }
                RichBlock::Table { headers, rows } => {
                    total += headers.iter().map(|h| h.chars().count()).sum::<usize>();
                    total += rows.iter().flatten().map(|c| c.chars().count()).sum::<usize>();
                }
                RichBlock::Divider => {}
            }
            blocks.push(block);
            if blocks.len() >= MAX_BLOCKS || total > MAX_TOTAL_CHARS {
                break;
            }
        }
        if blocks.is_empty() || total > MAX_TOTAL_CHARS {
            return None;
        }
        // sanitize every stored string (control chars + bounds)
        for b in &mut blocks {
            match b {
                RichBlock::Text { text, .. } => *text = clean(text, MAX_TEXT_CHARS),
                RichBlock::KeyValue { rows } => {
                    for r in rows {
                        r.key = clean(&r.key, 256);
                        r.value = clean(&r.value, MAX_TEXT_CHARS);
                    }
                }
                RichBlock::Table { headers, rows } => {
                    for h in headers {
                        *h = clean(h, 256);
                    }
                    for row in rows {
                        for c in row {
                            *c = clean(c, 1_024);
                        }
                    }
                }
                RichBlock::Divider => {}
            }
        }
        Some(Self { blocks })
    }

    /// Structural validation (cap echoes from_value's limits).
    pub fn validate(&self) -> Result<(), String> {
        if self.blocks.is_empty() {
            return Err("empty rich payload".into());
        }
        if self.blocks.len() > MAX_BLOCKS {
            return Err(format!("too many blocks (max {MAX_BLOCKS})"));
        }
        let mut total = 0usize;
        for b in &self.blocks {
            match b {
                RichBlock::Text { text, .. } => total += text.chars().count(),
                RichBlock::KeyValue { rows } => {
                    if rows.len() > MAX_TABLE_ROWS {
                        return Err("too many key/value rows".into());
                    }
                    total += rows.iter().map(|r| r.key.chars().count() + r.value.chars().count()).sum::<usize>();
                }
                RichBlock::Table { headers, rows } => {
                    if rows.len() > MAX_TABLE_ROWS {
                        return Err("too many table rows".into());
                    }
                    total += headers.iter().map(|h| h.chars().count()).sum::<usize>();
                    total += rows.iter().flatten().map(|c| c.chars().count()).sum::<usize>();
                }
                RichBlock::Divider => {}
            }
        }
        if total > MAX_TOTAL_CHARS {
            return Err("rich payload exceeds character budget".into());
        }
        Ok(())
    }

    /// Plain-text projection for the detail pane (one line per visual row).
    /// The Slint renderer consumes these lines verbatim — no semantics.
    pub fn render_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for b in &self.blocks {
            match b {
                RichBlock::Text { text, .. } => out.push(text.clone()),
                RichBlock::Divider => out.push("────────".into()),
                RichBlock::KeyValue { rows } => {
                    for r in rows {
                        out.push(format!("{}: {}", r.key, r.value));
                    }
                }
                RichBlock::Table { headers, rows } => {
                    out.push(headers.join(" | "));
                    for row in rows {
                        out.push(row.join(" | "));
                    }
                }
            }
        }
        out
    }
}

// ---- process-local registry (bounded FIFO, keyed by stable command id) --

static REGISTRY: std::sync::Mutex<Option<HashMap<String, RichResult>>> =
    std::sync::Mutex::new(None);

/// Store/replace the rich payload for a command id (bounded FIFO).
pub fn store(cmd_id: &str, rich: RichResult) {
    let mut reg = REGISTRY.lock().expect("rich registry lock");
    let map = reg.get_or_insert_with(HashMap::new);
    if map.len() >= REGISTRY_CAP && !map.contains_key(cmd_id) {
        if let Some(oldest) = map.keys().next().cloned() {
            map.remove(&oldest);
        }
    }
    map.insert(cmd_id.to_string(), rich);
}

/// Look up the rich payload for a command id.
pub fn lookup(cmd_id: &str) -> Option<RichResult> {
    REGISTRY
        .lock()
        .ok()
        .and_then(|m| m.as_ref().and_then(|m| m.get(cmd_id).cloned()))
}

/// Drop one entry (e.g. when its plugin is disabled).
pub fn remove(cmd_id: &str) {
    if let Some(m) = REGISTRY.lock().ok().as_mut() {
        if let Some(m) = m.as_mut() {
            m.remove(cmd_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Happy path: text/keyvalue/table/divider parse, sanitize, render.
    #[test]
    fn parse_and_render() {
        let v = json!({
            "blocks": [
                {"type": "text", "text": "Result 1+2*3 = 7"},
                {"type": "key_value", "rows": [{"key": "engine", "value": "expr"}]},
                {"type": "divider"},
                {"type": "table", "headers": ["a", "b"], "rows": [["1", "2"]]}
            ]
        });
        let r = RichResult::from_value(&v).expect("valid rich");
        assert_eq!(r.blocks.len(), 4);
        let lines = r.render_lines();
        assert!(lines.contains(&"Result 1+2*3 = 7".to_string()));
        assert!(lines.contains(&"engine: expr".to_string()));
        assert!(lines.iter().any(|l| l.contains("a | b")));
    }

    /// Unknown block types are skipped; a payload of only unknowns is None.
    #[test]
    fn unknown_blocks_skipped() {
        let v = json!({"blocks": [{"type": "hologram"}, {"type": "text", "text": "ok"}]});
        let r = RichResult::from_value(&v).expect("mixed payload");
        assert_eq!(r.blocks.len(), 1);
        let v = json!({"blocks": [{"type": "hologram"}]});
        assert!(RichResult::from_value(&v).is_none());
    }

    /// Bounds: block cap, row cap, char budget.
    #[test]
    fn bounds_enforced() {
        let big: Vec<Value> = (0..MAX_BLOCKS + 10)
            .map(|i| json!({"type": "text", "text": format!("t{i}")}))
            .collect();
        let r = RichResult::from_value(&json!({"blocks": big})).unwrap();
        assert_eq!(r.blocks.len(), MAX_BLOCKS);

        let rows: Vec<Vec<String>> = (0..MAX_TABLE_ROWS + 5)
            .map(|i| vec![format!("r{i}")])
            .collect();
        let r = RichResult::from_value(&json!({
            "blocks": [{"type": "table", "headers": ["h"], "rows": rows}]
        }))
        .unwrap();
        assert!(r.validate().is_err(), "row cap enforced at validate");
    }

    /// Sanitization: control characters stripped, long text bounded.
    #[test]
    fn sanitize_strips_control_and_bounds() {
        let v = json!({"blocks": [{"type": "text",
            "text": format!("a\u{7}b{}", "x".repeat(MAX_TEXT_CHARS + 100))}]});
        let r = RichResult::from_value(&v).unwrap();
        match &r.blocks[0] {
            RichBlock::Text { text, .. } => {
                assert!(!text.contains('\u{7}'));
                assert!(text.chars().count() <= MAX_TEXT_CHARS);
            }
            _ => panic!("wrong block"),
        }
    }

    /// Registry: store/lookup/evict at cap.
    #[test]
    fn registry_store_lookup_evict() {
        let r = RichResult::from_value(&json!({"blocks": [{"type": "divider"}]})).unwrap();
        for i in 0..(REGISTRY_CAP + 5) {
            let id = format!("cmd{i}");
            store(&id, r.clone());
            lookup(&id).expect("just stored");
            if i >= REGISTRY_CAP {
                remove(&format!("cmd{}", i - REGISTRY_CAP));
            }
        }
        assert!(lookup("cmd0").is_none(), "oldest evicted");
    }
}
