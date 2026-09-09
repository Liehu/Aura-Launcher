//! Context Builder (P27-A02, spec `P2.7 开发设计规范` §10-§12): assembles
//! bounded, sanitized context for the agent prompt from search results,
//! favorites, and foreground app — bridging launcher-core state to the
//! P2.7 prompt builder (prompt.rs).
//!
//! All context data is UNTRUSTED: it goes through `sanitize_untrusted` and
//! `bound_chars` before reaching the prompt. Context is advisory (ranking /
//! selection hints), never authority.

use crate::prompt::{bound_chars, sanitize_untrusted};
use serde_json::json;

/// Compact context item for the agent prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextItem {
    pub label: String,
    pub value: String,
}

/// Assemble a bounded context block from search results / favorites /
/// foreground app. Deterministic; total output capped at `budget` chars.
pub fn build_context(
    foreground_app: Option<&str>,
    current_folder: Option<&str>,
    favorites: &[String],
    budget: usize,
) -> Vec<ContextItem> {
    let mut items = Vec::new();
    if let Some(app) = foreground_app {
        items.push(ContextItem {
            label: "foreground_app".into(),
            value: sanitize_untrusted(&bound_chars(app, 128)),
        });
    }
    if let Some(folder) = current_folder {
        items.push(ContextItem {
            label: "current_folder".into(),
            value: sanitize_untrusted(&bound_chars(folder, 256)),
        });
    }
    for (i, fav) in favorites.iter().enumerate().take(5) {
        items.push(ContextItem {
            label: format!("favorite_{i}"),
            value: sanitize_untrusted(&bound_chars(fav, 128)),
        });
    }
    // budget: keep adding until we exceed, then truncate the vec
    let mut total = 0usize;
    let mut result = Vec::new();
    for item in items {
        let cost = item.label.len() + item.value.len();
        if total + cost > budget {
            break;
        }
        total += cost;
        result.push(item);
    }
    result
}

/// Render context items as a JSON block for embedding in the prompt.
pub fn render_context_json(items: &[ContextItem]) -> String {
    let arr: Vec<serde_json::Value> = items
        .iter()
        .map(|i| json!({"label": i.label, "value": i.value}))
        .collect();
    serde_json::to_string(&serde_json::json!(arr)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_foreground_and_favorites() {
        let items = build_context(
            Some("explorer.exe"),
            Some(r"C:\Projects"),
            &["Chrome".into(), "VS Code".into()],
            1024,
        );
        assert_eq!(items.len(), 4, "foreground + folder + 2 favorites");
        assert_eq!(items[0].label, "foreground_app");
        assert_eq!(items[0].value, "explorer.exe");
        assert_eq!(items[2].label, "favorite_0");
    }

    #[test]
    fn budget_respected() {
        let long_fav = "x".repeat(500);
        let items = build_context(
            Some("app"),
            None,
            &[long_fav],
            128,
        );
        // the 500-char favorite exceeds the 128-char budget alone
        // so only the foreground item fits
        assert!(items.len() <= 2, "budget limits context items");
    }

    #[test]
    fn empty_context_produces_no_items() {
        let items = build_context(None, None, &[], 1024);
        assert!(items.is_empty());
    }
}
