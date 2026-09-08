//! Context-aware Provider (MVP2.1): Context Suggestions + context commands.
//!
//! `Context -> Context-aware Provider -> Command -> Action` chain: the app
//! refreshes the command list from a fresh ContextSnapshot on every popup
//! open via `ContextHandle::replace`; empty-query results (Context
//! Suggestions) are served through `context_suggestions()` because ranking
//! drops zero-score items.

use std::sync::{Arc, Mutex};

use launcher_domain::{Command, QueryContext};

use crate::Provider;

/// Presentation bound for Context Suggestions (empty-query list).
///
/// This is a UX/latency cap applied by Core/presentation, NOT a Provider API
/// limit: `Provider::query` may return any number of commands, and ranking
/// decides what is displayed (≤ 15 here for the empty-query suggestions).
pub const MAX_CONTEXT_SUGGESTIONS: usize = 15;

#[derive(Clone, Default)]
pub struct ContextHandle(Arc<Mutex<Vec<Command>>>);

impl ContextHandle {
    pub fn replace(&self, commands: Vec<Command>) {
        *self.0.lock().expect("context commands lock") = commands;
    }

    pub fn is_empty(&self) -> bool {
        self.0.lock().expect("context commands lock").is_empty()
    }
}

pub struct ContextProvider {
    commands: ContextHandle,
}

impl ContextProvider {
    pub fn new() -> (Self, ContextHandle) {
        let handle = ContextHandle::default();
        (
            Self {
                commands: handle.clone(),
            },
            handle,
        )
    }

    /// Context Suggestions list: served for an empty query on popup open,
    /// insertion order (freshness = context recency), bounded.
    pub fn context_suggestions(&self, limit: usize) -> Vec<Command> {
        self.commands
            .0
            .lock()
            .expect("context commands lock")
            .iter()
            .take(limit)
            .cloned()
            .collect()
    }
}

impl Provider for ContextProvider {
    fn id(&self) -> &str {
        "context"
    }

    /// Non-empty queries: context commands compete via normal ranking
    /// (keywords make "term" match "Open Terminal Here").
    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if q.normalized.is_empty() {
            return Vec::new();
        }
        self.commands
            .0
            .lock()
            .expect("context commands lock")
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category};

    fn cmd(id: &str, title: &str, keywords: &[&str]) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "context".into(),
            score: 0.0,
            keywords: keywords.iter().map(|s| s.to_string()).collect(),
            category: Category::Command,
            actions: vec![Action {
                kind: ActionKind::Execute,
                payload: None,

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        }
    }

    #[test]
    fn context_suggestions_are_bounded_insertion_order() {
        let (provider, handle) = ContextProvider::new();
        assert!(provider.context_suggestions(10).is_empty());

        handle.replace(
            (0..30)
                .map(|i| cmd(&format!("c{i}"), &format!("Item{i}"), &[]))
                .collect(),
        );
        let qs = provider.context_suggestions(MAX_CONTEXT_SUGGESTIONS);
        assert_eq!(qs.len(), MAX_CONTEXT_SUGGESTIONS);
        assert_eq!(qs[0].title, "Item0"); // insertion order preserved
    }

    #[test]
    fn query_serves_context_commands_but_not_on_empty() {
        let (mut provider, handle) = ContextProvider::new();
        handle.replace(vec![cmd("t", "Open Terminal Here", &["terminal"])]);

        assert!(provider.query(&QueryContext::parse("")).is_empty());
        let hits = provider.query(&QueryContext::parse("term"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Open Terminal Here");
        // unrelated query still returns the catalog; ranking in core decides
        assert_eq!(provider.query(&QueryContext::parse("zzz")).len(), 1);
    }

    #[test]
    fn handle_replaces_across_clones() {
        let (_provider, handle) = ContextProvider::new();
        let h2 = handle.clone();
        handle.replace(vec![cmd("a", "A", &[])]);
        assert!(!h2.is_empty());
    }
}
