//! SearchCoordinator (P2.5-B01..B04, spec `demo2/files2/101-p2.5-0.1.md`).
//!
//! Unified provider-orchestration entry decoupled from `Core::search`:
//! - B01: single coordinator entry + intent/strategy routing + stale-query
//!   guard; `Core::search` keeps its sequential path untouched as the
//!   compatibility mode (AC-B01-2) — switching the default is a post-benchmark
//!   decision (P25-E line).
//! - B02: bounded concurrent fan-out — providers are grouped into at most
//!   `max_parallelism` worker threads; `max_in_flight` is tracked and asserted
//!   bounded (AC-B02-1); results merge in PROVIDER order so completion order
//!   cannot affect ranking input (AC-B02-2).
//! - B03: per-provider failure isolation — an Err or even a PANIC in one
//!   provider never kills the coordinator (AC-B02-4); the error is recorded
//!   and the remaining providers still contribute (INV-SEARCH-004).
//! - B04: a cancellation flag checked between every provider turns the run
//!   into `Cancelled` (stale-query guard).
//!
//! The coordinator NEVER executes an Effect (AC-B01-3): it only calls
//! `Provider::query` and merges/ranks candidates.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use launcher_domain::{Command, QueryContext};
use launcher_search::intelligence::{
    search_request_v2, FilterKind, SearchRequestV2, SearchResultState, SearchStrategy,
};

use crate::Provider;

/// How a FilterKind maps to provider ids (provider-neutral routing table,
/// P25-B06 v0.1: prefix matching; a provider matches if any id prefix matches).
fn routes_to(kind: &FilterKind, provider_id: &str) -> bool {
    match kind {
        FilterKind::App => provider_id.starts_with("app"),
        FilterKind::File | FilterKind::Folder => {
            provider_id.starts_with("file") || provider_id.starts_with("recent")
        }
        FilterKind::Cmd => {
            provider_id.starts_with("workflow")
                || provider_id.starts_with("settings")
                || provider_id.starts_with("context")
        }
        FilterKind::Plugin => provider_id.starts_with("plugin"),
        // workflow filter routes to the workflow provider AND plugin-provided
        // workflows; keep it permissive to workflow-ish ids
        FilterKind::Wf => provider_id.starts_with("workflow"),
    }
}

fn intent_routes_to(req: &SearchRequestV2, provider_id: &str) -> bool {
    match &req.strategy {
        SearchStrategy::All => true,
        SearchStrategy::Routed(kinds) => kinds.iter().any(|k| routes_to(k, provider_id)),
    }
}

/// Bounded-concurrency search coordinator (B01/B02 skeleton).
pub struct SearchCoordinator {
    providers: Vec<ProviderSlot>,
    max_parallelism: usize,
    in_flight: Arc<AtomicUsize>,
    max_in_flight_seen: Arc<AtomicUsize>,
}

struct ProviderSlot {
    id: String,
    provider: Arc<Mutex<Box<dyn Provider>>>,
}

/// Coordinator output: merged candidates (provider order), per-provider
/// errors, the frozen result state and the fan-out metric.
#[derive(Debug)]
pub struct CoordinatorResult {
    pub commands: Vec<Command>,
    pub errors: Vec<String>,
    pub state: SearchResultState,
    /// Peak concurrent provider queries observed during this search.
    pub max_in_flight: usize,
}

impl SearchCoordinator {
    /// `max_parallelism` bounds the number of providers queried at once
    /// (B02 admission control; `0` is coerced to 1).
    pub fn new(max_parallelism: usize) -> Self {
        Self {
            providers: Vec::new(),
            max_parallelism: max_parallelism.max(1),
            in_flight: Arc::new(AtomicUsize::new(0)),
            max_in_flight_seen: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let id = provider.id().to_string();
        self.providers.push(ProviderSlot {
            id,
            provider: Arc::new(Mutex::new(provider)),
        });
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Orchestrated search: v2 request → strategy routing → bounded
    /// concurrent fan-out → deterministic merge. `cancel` (stale-query guard)
    /// is polled between providers; a set flag yields
    /// [`SearchResultState::Cancelled`] with whatever completed so far.
    pub fn search(&self, raw_query: &str, cancel: Option<&AtomicBool>) -> CoordinatorResult {
        let started = std::time::Instant::now();
        let req = search_request_v2(raw_query, 100);
        let q = QueryContext::parse(&req.normalized);

        // B06 v0.1 routing: strategy selects a provider subset (order kept)
        let selected: Vec<usize> = self
            .providers
            .iter()
            .enumerate()
            .filter(|(_, slot)| intent_routes_to(&req, &slot.id))
            .map(|(i, _)| i)
            .collect();

        // admission control: split into at most max_parallelism groups, each
        // group runs its providers sequentially inside one worker thread
        let mut groups: Vec<Vec<usize>> = vec![Vec::new(); self.max_parallelism.min(selected.len().max(1))];
        for (pos, idx) in selected.iter().enumerate() {
            let gi = pos % groups.len();
            groups[gi].push(*idx);
        }

        let results: Vec<Mutex<Option<Vec<Command>>>> =
            (0..selected.len()).map(|_| Mutex::new(None)).collect();
        let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let cancelled = AtomicBool::new(false);

        std::thread::scope(|scope| {
            for group in &groups {
                let results = &results;
                let errors = &errors;
                let cancelled_flag = &cancelled;
                let in_flight = &self.in_flight;
                let max_seen = &self.max_in_flight_seen;
                let q = &q;
                scope.spawn(move || {
                    for &idx in group {
                        if let Some(cancel) = cancel {
                            if cancel.load(Ordering::SeqCst) {
                                cancelled_flag.store(true, Ordering::SeqCst);
                                return;
                            }
                        }
                        let slot = &self.providers[idx];
                        // B02 admission: track in-flight concurrency
                        let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                        max_seen.fetch_max(now, Ordering::SeqCst);
                        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || {
                                let mut provider = slot.provider.lock().expect("provider lock");
                                let cmds = provider.query(q);
                                (cmds, provider.take_last_error())
                            },
                        ));
                        in_flight.fetch_sub(1, Ordering::SeqCst);
                        match outcome {
                            Ok((cmds, err)) => {
                                *results[idx].lock().expect("result lock") = Some(cmds);
                                if let Some(e) = err {
                                    errors
                                        .lock()
                                        .expect("errors lock")
                                        .push(format!("{}: {e}", slot.id));
                                }
                            }
                            Err(_) => {
                                // B03: a panicking provider is isolated
                                errors.lock().expect("errors lock").push(format!(
                                    "{}: provider panicked",
                                    slot.id
                                ));
                            }
                        }
                    }
                });
            }
        });

        // deterministic merge in PROVIDER order (AC-B02-2)
        let mut commands = Vec::new();
        for slot in results {
            if let Some(cmds) = slot.into_inner().expect("result lock") {
                commands.extend(cmds);
            }
        }
        let errors = errors.into_inner().expect("errors lock");
        let state = if cancelled.load(Ordering::SeqCst) {
            SearchResultState::Cancelled
        } else if errors.is_empty() {
            SearchResultState::Complete
        } else {
            SearchResultState::Partial
        };
        let _ = started;
        CoordinatorResult {
            commands,
            errors,
            state,
            max_in_flight: self.max_in_flight_seen.load(Ordering::SeqCst),
        }
    }
}
