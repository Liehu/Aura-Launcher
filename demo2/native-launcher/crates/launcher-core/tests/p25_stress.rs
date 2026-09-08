//! P2.5-E stress/race suite (spec `demo2/files2/101-p2.5-0.1.md` §E03/E04):
//! concurrent coordinator searches, cancellation races and supersede-style
//! churn must hold the same contracts as single-threaded runs.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use launcher_core::search_coordinator::SearchCoordinator;
use launcher_core::Provider;
use launcher_domain::{Category, Command, QueryContext};
use launcher_search::intelligence::SearchResultState;

struct StressProvider {
    id: String,
    delay_us: u64,
}

impl Provider for StressProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
        if self.delay_us > 0 {
            std::thread::sleep(std::time::Duration::from_micros(self.delay_us));
        }
        vec![Command {
            id: format!("{}:r", self.id),
            title: format!("{} result", self.id),
            subtitle: None,
            icon: None,
            provider_id: self.id.clone(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![],
            target: None,
        }]
    }
}

fn coordinator(providers: usize, delay_us: u64) -> SearchCoordinator {
    let mut c = SearchCoordinator::new(4);
    for i in 0..providers {
        c.register(Box::new(StressProvider {
            id: format!("files-{i}"),
            delay_us,
        }));
    }
    c
}

/// E03: concurrent searches through ONE coordinator — every caller gets the
/// full result set, state Complete, in-flight stays bounded.
#[test]
fn e03_concurrent_search_stress() {
    let c = Arc::new(coordinator(40, 200));
    let failures = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let c = c.clone();
        let failures = failures.clone();
        handles.push(std::thread::spawn(move || {
            for _ in 0..5 {
                let r = c.search("query", None);
                // max_in_flight is the coordinator-global peak; with 8
                // concurrent searches the honest bound is 4 per search
                if r.commands.len() != 40 || r.state != SearchResultState::Complete {
                    failures.fetch_add(1, Ordering::SeqCst);
                }
            }
        }));
    }
    for h in handles {
        h.join().expect("worker thread");
    }
    assert_eq!(failures.load(Ordering::SeqCst), 0, "40 searches, zero violations");
    assert!(
        c.search("query", None).max_in_flight <= 4 * 8,
        "global in-flight bounded by parallelism x concurrent callers"
    );
}

/// E04: supersede/cancellation race — a cancel flag flipping WHILE searches
/// run must produce either Complete (too late) or Cancelled (in time), never
/// a hang, a panic or a bogus state.
#[test]
fn e04_supersede_cancellation_race() {
    let c = Arc::new(coordinator(24, 300));
    for round in 0..6 {
        let cancel = Arc::new(AtomicBool::new(false));
        let c = c.clone();
        let cancel2 = cancel.clone();
        let flipper = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_micros(round * 100 + 50));
            cancel2.store(true, Ordering::SeqCst);
        });
        let r = c.search("query", Some(&cancel));
        flipper.join().expect("flipper");
        match r.state {
            SearchResultState::Complete | SearchResultState::Cancelled => {}
            other => panic!("round {round}: bogus state {other:?}"),
        }
        assert!(r.commands.len() <= 24);
    }
}
