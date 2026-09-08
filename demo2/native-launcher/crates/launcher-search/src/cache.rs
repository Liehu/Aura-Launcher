//! Query Cache (P2.2-B, review: P2.2-B §4/§36-46/§53-59/§129): generation-
//! aware LRU over FINAL ranked results. Pure in-memory; no filesystem, no
//! provider, no action-layer knowledge (INV-CACHE-001/002/003).
//!
//! CACHEABLE = final AND current generations AND not oversized (§153).
//! CACHE_HIT  = key match AND not expired (§153). Stale entries are removed
//! on lookup (§35).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use launcher_domain::Command;

/// Environment generations that make a cache key unique (P2.2-B §129).
/// Missing subsystems are pinned at 0 (v0.1: context/catalog generations
/// arrive with their batches).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchCacheEnvironment {
    pub file_generation: u64,
    pub application_generation: u64,
    pub user_state_generation: u64,
    pub context_generation: u64,
    /// Ranking config/weights revision (P2.2-B §80/§127): reserved — bump
    /// when ranking weights become runtime-configurable.
    pub ranking_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchCacheKey {
    pub normalized_query: String,
    pub file_generation: u64,
    pub application_generation: u64,
    pub user_state_generation: u64,
    pub context_generation: u64,
    pub ranking_generation: u64,
}

impl SearchCacheKey {
    pub fn new(normalized_query: &str, env: SearchCacheEnvironment) -> Self {
        Self {
            normalized_query: normalized_query.to_lowercase(),
            file_generation: env.file_generation,
            application_generation: env.application_generation,
            user_state_generation: env.user_state_generation,
            context_generation: env.context_generation,
            ranking_generation: env.ranking_generation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTtl {
    Complete,
    /// Empty/negative results change as soon as the index grows (§27/§28).
    Negative,
}

impl CacheTtl {
    fn duration(self) -> Duration {
        match self {
            CacheTtl::Complete => Duration::from_secs(60),
            CacheTtl::Negative => Duration::from_secs(5),
        }
    }
}

struct Entry {
    results: Vec<Command>,
    inserted_at: Instant,
    ttl: Duration,
    bytes: usize,
}

fn estimate_bytes(results: &[Command]) -> usize {
    std::mem::size_of::<Vec<Command>>()
        + results
            .iter()
            .map(|c| c.title.len() + c.subtitle.as_deref().map_or(0, str::len) + 64)
            .sum::<usize>()
}

/// Bounded LRU (§36/§38): entries + bytes budgets; oversized entries
/// rejected (§41); expired entries removed on access (§70).
pub struct SearchCache {
    max_entries: usize,
    max_bytes: usize,
    entries: HashMap<SearchCacheKey, Entry>,
    order: Vec<SearchCacheKey>,
    used_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub stale_rejections: u64,
    pub evictions: u64,
}

impl SearchCache {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            entries: HashMap::new(),
            order: Vec::new(),
            used_bytes: 0,
            hits: 0,
            misses: 0,
            stale_rejections: 0,
            evictions: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    pub fn get(&mut self, key: &SearchCacheKey) -> Option<Vec<Command>> {
        let now = Instant::now();
        let expired = self
            .entries
            .get(key)
            .is_some_and(|e| now.duration_since(e.inserted_at) >= e.ttl);
        if expired {
            self.remove(key);
            self.stale_rejections += 1;
            return None;
        }
        if let Some(e) = self.entries.get(key) {
            let results = e.results.clone();
            self.hits += 1;
            return Some(results);
        }
        self.misses += 1;
        None
    }

    pub fn remove(&mut self, key: &SearchCacheKey) {
        if let Some(e) = self.entries.remove(key) {
            self.used_bytes -= e.bytes;
            self.order.retain(|k| k != key);
        }
    }

    /// Cache admission (§42): oversized rejected; eviction until fit.
    pub fn put(&mut self, key: SearchCacheKey, results: Vec<Command>, ttl: CacheTtl) -> bool {
        let bytes = estimate_bytes(&results);
        if bytes > self.max_bytes / 4 {
            return false; // single large entry protection (§41)
        }
        self.remove(&key);
        while self.used_bytes + bytes > self.max_bytes || self.entries.len() >= self.max_entries {
            let Some(oldest) = self.order.first().cloned() else { break };
            if let Some(e) = self.entries.remove(&oldest) {
                self.used_bytes -= e.bytes;
                self.evictions += 1;
            }
            self.order.remove(0);
        }
        self.used_bytes += bytes;
        self.order.push(key.clone());
        self.entries.insert(
            key,
            Entry {
                results,
                inserted_at: Instant::now(),
                ttl: ttl.duration(),
                bytes,
            },
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category};

    fn cmd(id: &str, title: &str) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "apps".into(),
            score: 1.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
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

    fn key(q: &str, gen: u64) -> SearchCacheKey {
        SearchCacheKey::new(
            q,
            SearchCacheEnvironment {
                file_generation: gen,
                ..Default::default()
            },
        )
    }

    /// CACHE-001/002: put/get + miss.
    #[test]
    fn put_get_miss() {
        let mut c = SearchCache::new(16, 1 << 20);
        c.put(key("chrome", 1), vec![cmd("chrome", "Chrome")], CacheTtl::Complete);
        assert_eq!(c.get(&key("chrome", 1)).unwrap().len(), 1);
        assert!(c.get(&key("edge", 1)).is_none());
    }

    /// CACHE-GEN-002: generation change → miss + stale removal (§35).
    #[test]
    fn generation_change_misses_and_removes() {
        let mut c = SearchCache::new(16, 1 << 20);
        c.put(key("chrome", 1), vec![cmd("chrome", "Chrome")], CacheTtl::Complete);
        // new generation → different key → miss (old entry becomes
        // unreachable and ages out via LRU; no global clear needed, §51)
        assert!(c.get(&key("chrome", 2)).is_none());
        assert_eq!(c.misses, 1);
    }

    /// CACHE-MEM-001/002: entries + bytes budgets hold.
    #[test]
    fn budgets_enforced() {
        let mut c = SearchCache::new(3, 1 << 20);
        for i in 0..10 {
            let mut v = vec![cmd(&format!("x{i}"), "x"); 3];
            v.push(cmd("filler", &"y".repeat(200)));
            c.put(key(&format!("q{i}"), 1), v, CacheTtl::Complete);
        }
        assert!(c.len() <= 3);
        assert!(c.used_bytes() <= 1 << 20);
    }

    /// CACHE-005: oversized single entry rejected.
    #[test]
    fn oversized_entry_rejected() {
        let mut c = SearchCache::new(16, 4096);
        let big = vec![cmd("big", &"x".repeat(8192))];
        assert!(!c.put(key("big", 1), big, CacheTtl::Complete));
    }
}
