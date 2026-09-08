//! P2.5-B tests (spec `demo2/files2/101-p2.5-0.1.md` §B): SearchCoordinator
//! skeleton (B01), bounded concurrent fan-out (B02), provider failure/panic
//! isolation (B03) and the stale-query cancellation guard (B04).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use launcher_core::search_coordinator::SearchCoordinator;
use launcher_core::Provider;
use launcher_domain::{Category, Command, QueryContext};
use launcher_search::intelligence::SearchResultState;

struct MockProvider {
    id: String,
    count: usize,
    delay_ms: u64,
    panic_on_query: bool,
    err_on_query: bool,
    live: Arc<AtomicUsize>,
}

impl Provider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
        let _now = self.live.fetch_add(1, Ordering::SeqCst) + 1;
        if self.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        }
        self.live.fetch_sub(1, Ordering::SeqCst);
        if self.panic_on_query {
            panic!("mock provider exploded");
        }
        if self.err_on_query {
            return Vec::new();
        }
        (0..self.count)
            .map(|i| Command {
                id: format!("{}:{i}", self.id),
                title: format!("{} result {i}", self.id),
                subtitle: None,
                icon: None,
                provider_id: self.id.clone(),
                score: 0.0,
                keywords: vec![],
                category: Category::Application,
                actions: vec![],
                target: None,
            })
            .collect()
    }
}

impl MockProvider {
}

fn coord(max_parallelism: usize) -> SearchCoordinator {
    SearchCoordinator::new(max_parallelism)
}

/// B01: unified entry, happy path — all providers contribute, state Complete.
#[test]
fn b01_happy_path_complete() {
    let mut c = coord(4);
    c.register(Box::new(MockProvider {
        id: "app-registry".into(),
        count: 3,
        delay_ms: 0,
        panic_on_query: false,
        err_on_query: false,
        live: Arc::new(AtomicUsize::new(0)),
    }));
    c.register(Box::new(MockProvider {
        id: "files".into(),
        count: 2,
        delay_ms: 0,
        panic_on_query: false,
        err_on_query: false,
        live: Arc::new(AtomicUsize::new(0)),
    }));
    let r = c.search("chrome", None);
    assert_eq!(r.commands.len(), 5, "all providers contribute");
    assert_eq!(r.state, SearchResultState::Complete);
    assert!(r.errors.is_empty());
    // deterministic provider-order merge
    assert!(r.commands[0].id.starts_with("app-registry:"));
    assert!(r.commands[3].id.starts_with("files:"));
}

/// B02: 100 providers through a parallelism-4 coordinator — every provider
/// contributes exactly once (no orphans), in-flight is bounded, and the
/// merged order is identical across repeated runs (AC-B02-1/2/3).
#[test]
fn b02_bounded_fanout_deterministic() {
    let live = Arc::new(AtomicUsize::new(0));
    let mut c = coord(4);
    for i in 0..100 {
        c.register(Box::new(MockProvider {
            id: format!("files-{i}"),
            count: 1,
            delay_ms: 5,
            panic_on_query: false,
            err_on_query: false,
            live: live.clone(),
        }));
    }
    let r1 = c.search("query", None);
    assert_eq!(r1.commands.len(), 100, "no orphan tasks: all 100 contributed");
    assert!(r1.max_in_flight <= 4, "AC-B02-1: in-flight bounded by limit");
    let ids: Vec<String> = r1.commands.iter().map(|c| c.id.clone()).collect();
    for _ in 0..2 {
        let r = c.search("query", None);
        let again: Vec<String> = r.commands.iter().map(|c| c.id.clone()).collect();
        assert_eq!(ids, again, "AC-B02-2: completion order cannot affect merge");
    }
}

/// B03: a PANICKING provider is isolated — remaining providers still
/// contribute and the state degrades to Partial, never a crash (AC-B02-4).
#[test]
fn b03_panicking_provider_isolated() {
    let mut c = coord(2);
    c.register(Box::new(MockProvider {
        id: "app-registry".into(),
        count: 2,
        delay_ms: 0,
        panic_on_query: false,
        err_on_query: false,
        live: Arc::new(AtomicUsize::new(0)),
    }));
    c.register(Box::new(MockProvider {
        id: "files".into(),
        count: 1,
        delay_ms: 0,
        panic_on_query: true,
        err_on_query: false,
        live: Arc::new(AtomicUsize::new(0)),
    }));
    c.register(Box::new(MockProvider {
        id: "recent-files".into(),
        count: 1,
        delay_ms: 0,
        panic_on_query: false,
        err_on_query: false,
        live: Arc::new(AtomicUsize::new(0)),
    }));
    let r = c.search("anything", None);
    assert_eq!(r.commands.len(), 3, "healthy providers survive the panic");
    assert_eq!(r.state, SearchResultState::Partial);
    assert!(r.errors.iter().any(|e| e.contains("files") && e.contains("panicked")));
}

/// B06 v0.1 routing: an explicit `app:` filter routes ONLY to matching
/// providers (non-matching providers are not queried at all).
#[test]
fn b06_filter_routes_to_matching_providers_only() {
    let mut c = coord(4);
    c.register(Box::new(MockProvider {
        id: "app-registry".into(),
        count: 1,
        delay_ms: 0,
        panic_on_query: false,
        err_on_query: false,
        live: Arc::new(AtomicUsize::new(0)),
    }));
    struct Counting {
        id: String,
        hits: Arc<AtomicUsize>,
    }
    impl Provider for Counting {
        fn id(&self) -> &str {
            &self.id
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            self.hits.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        }
    }
    // the NON-routed provider's counter must stay at zero after the search
    let workflow_hits = Arc::new(AtomicUsize::new(0));
    c.register(Box::new(Counting {
        id: "workflow".into(),
        hits: workflow_hits.clone(),
    }));
    let r = c.search("app:chrome", None);
    assert_eq!(r.state, SearchResultState::Complete);
    // app-registry ran (its command is present); workflow never ran
    assert_eq!(r.commands.len(), 1, "routed provider contributed");
    assert_eq!(workflow_hits.load(Ordering::SeqCst), 0, "non-routed provider not queried");
}

/// B04: cancellation between providers yields Cancelled with partial results
/// (stale-query guard).
#[test]
fn b04_cancellation_guard() {
    let mut c = coord(1); // one worker: strictly sequential, cancel is visible
    for i in 0..8 {
        c.register(Box::new(MockProvider {
            id: format!("files-{i}"),
            count: 1,
            delay_ms: 30,
            panic_on_query: false,
            err_on_query: false,
            live: Arc::new(AtomicUsize::new(0)),
        }));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel2 = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        cancel2.store(true, Ordering::SeqCst);
    });
    let r = c.search("query", Some(&cancel));
    assert_eq!(r.state, SearchResultState::Cancelled);
    assert!(
        r.commands.len() < 8,
        "cancelled run returns partial results"
    );
}
