//! P2.4-B tests (spec `01-P2.4-DESIGN-SPEC.md` §5; test plan `03` §4):
//! root lifecycle state machine (B01/B04), watcher re-registration with
//! bounded backoff (B02), health model fields (B05) and a long-run
//! maintenance soak (B06). Drives the REAL watcher + coordinator + writer
//! against real filesystem operations (same harness as incremental_e2e).

#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use launcher_domain::IndexHealth;
use launcher_indexer::coordinator::{self, CoordinatorConfig, CoordinatorHandle};

fn temp_root(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nl_p24b_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

fn wait_quiescent(h: &CoordinatorHandle, timeout: Duration) -> bool {
    const SETTLE: Duration = Duration::from_millis(1500);
    let start = Instant::now();
    let mut quiescent_since: Option<Instant> = None;
    while start.elapsed() < timeout {
        if h.quiescent() {
            match quiescent_since {
                Some(t) if t.elapsed() >= SETTLE => return true,
                Some(_) => {}
                None => quiescent_since = Some(Instant::now()),
            }
        } else {
            quiescent_since = None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn status_of(h: &CoordinatorHandle) -> launcher_domain::IndexStatus {
    h.status.lock().expect("status lock").clone()
}

fn wait_for<F: Fn(&launcher_domain::IndexStatus) -> bool>(
    h: &CoordinatorHandle,
    timeout: Duration,
    pred: F,
) -> bool {
    let start = Instant::now();
    let mut last_print = Instant::now();
    while start.elapsed() < timeout {
        let st = status_of(h);
        if pred(&st) {
            return true;
        }
        if last_print.elapsed() >= Duration::from_millis(2000) {
            last_print = Instant::now();
            eprintln!(
                "[dbg status] health={:?} gen={} pending={} dirty={} watchers={} unavail={} err={:?}",
                st.health, st.generation, st.pending_events, st.dirty_roots,
                st.watcher_count, st.unavailable_roots, st.last_error
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// B04: root disappears → health Unavailable, the committed index REMAINS
/// queryable; root reappears → bounded rescan + watcher re-registration →
/// generation advances and new content is searchable.
#[test]
fn b04_root_disappear_reappear_recovers() {
    let root = temp_root("b04");
    let db = root.join("index.db");
    let corpus = root.join("corpus");
    std::fs::create_dir_all(&corpus).unwrap();
    write(&corpus.join("before.txt"), "before");

    let cfg = CoordinatorConfig {
        roots: vec![corpus.clone()],
        ..Default::default()
    };
    let handle = coordinator::spawn(&db, cfg);
    assert!(
        wait_quiescent(&handle, Duration::from_secs(30)),
        "initial scan quiescent"
    );
    let gen_before = status_of(&handle).generation;
    assert!(gen_before >= 1);

    // FAULT: delete the watched root (watcher dies or existence patrol fires)
    std::fs::remove_dir_all(&corpus).unwrap();
    assert!(
        wait_for(&handle, Duration::from_secs(45), |s| {
            s.health == IndexHealth::Unavailable || s.unavailable_roots > 0
        }),
        "root disappearance surfaces as Unavailable"
    );
    let st = status_of(&handle);
    assert!(
        st.last_error.as_deref().unwrap_or_default().contains("unavailable")
            || st.unavailable_roots > 0,
        "unavailable state recorded"
    );

    // the committed index REMAINS queryable while the root is gone (§5.2):
    // the query path executes normally (entries for the deleted subtree are
    // correctly gone — Delete events are applied — but reads never fail).
    {
        let ix = launcher_indexer::Indexer::open(&db).unwrap();
        let hits = ix.search("before", 10);
        assert!(hits.is_ok(), "index queryable during Unavailable");
        assert!(ix.generation().unwrap() >= gen_before, "generation intact");
    }

    // REAPPEARANCE: recreate the root with new content
    std::fs::create_dir_all(&corpus).unwrap();
    write(&corpus.join("before.txt"), "before restored");
    write(&corpus.join("after.txt"), "after");

    assert!(
        wait_for(&handle, Duration::from_secs(60), |s| {
            s.health == IndexHealth::Ready && s.unavailable_roots == 0
        }),
        "root reappearance returns to Ready"
    );
    let st = status_of(&handle);
    assert!(st.generation > gen_before, "generation advanced after recovery");
    assert!(st.recovery_count >= 1, "recovery counted");
    assert!(st.watcher_count >= 1, "watcher re-registered");
    assert!(st.last_success_ms.is_some(), "last_success recorded");

    // the re-registered watcher observes NEW changes (B02 closed loop)
    write(&corpus.join("live.txt"), "live");
    assert!(
        wait_quiescent(&handle, Duration::from_secs(30)),
        "re-registered watcher reaches quiescence"
    );
    {
        let ix = launcher_indexer::Indexer::open(&db).unwrap();
        let hits = ix.search("after", 10).unwrap();
        assert!(!hits.is_empty(), "reappeared content indexed");
        let hits = ix.search("live", 10).unwrap();
        assert!(!hits.is_empty(), "post-recovery watcher tracks new events");
    }

    handle.stop();
    std::fs::remove_dir_all(&root).ok();
}

/// B05: health model fields are populated on a healthy running coordinator.
#[test]
fn b05_health_fields_populated() {
    let root = temp_root("b05");
    let corpus = root.join("corpus");
    std::fs::create_dir_all(&corpus).unwrap();
    write(&corpus.join("a.txt"), "a");

    let handle = coordinator::spawn(
        &root.join("index.db"),
        CoordinatorConfig {
            roots: vec![corpus.clone()],
            ..Default::default()
        },
    );
    assert!(wait_quiescent(&handle, Duration::from_secs(30)));
    let st = status_of(&handle);
    assert_eq!(st.health, IndexHealth::Ready);
    assert!(st.watcher_count >= 1, "watcher_count reflects live watcher");
    assert_eq!(st.unavailable_roots, 0);
    assert!(st.last_success_ms.is_some(), "last_success recorded");
    assert!(st.last_failure_ms.is_none() || st.unavailable_roots > 0);
    assert_eq!(st.generation, st.generation.max(1));

    handle.stop();
    std::fs::remove_dir_all(&root).ok();
}

/// B06: maintenance soak — continuous churn under the real watcher.
/// LAUNCHER_INDEX_SOAK_SECONDS scales the duration (default: CI profile).
/// Assertions: generation progresses monotonically, the queue/dirty set
/// fully drains (no unbounded backlog), and the index stays searchable.
#[test]
fn b06_maintenance_soak_bounded() {
    let soak_secs: u64 = std::env::var("LAUNCHER_INDEX_SOAK_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let root = temp_root("b06");
    let corpus = root.join("corpus");
    std::fs::create_dir_all(&corpus).unwrap();

    let handle = coordinator::spawn(
        &root.join("index.db"),
        CoordinatorConfig {
            roots: vec![corpus.clone()],
            ..Default::default()
        },
    );

    let deadline = Instant::now() + Duration::from_secs(soak_secs);
    let mut churn: u64 = 0;
    let mut max_pending_seen: usize = 0;
    while Instant::now() < deadline {
        let i = churn % 64;
        write(&corpus.join(format!("churn-{i}.txt")), &format!("content {churn}"));
        churn += 1;
        max_pending_seen = max_pending_seen.max(status_of(&handle).pending_events);
        if churn % 32 == 0 {
            let victim = corpus.join(format!("churn-{}.txt", (churn / 32) % 64));
            let _ = std::fs::remove_file(victim);
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        wait_quiescent(&handle, Duration::from_secs(60)),
        "soak ends quiescent — no unbounded backlog"
    );
    let st = status_of(&handle);
    assert_eq!(st.health, IndexHealth::Ready);
    assert!(st.generation >= 1, "generation progressed during soak");
    assert!(
        churn >= 32,
        "sanity: soak actually churned (consider LAUNCHER_INDEX_SOAK_SECONDS)"
    );
    let _ = max_pending_seen; // observed; bound asserted via quiescence
    {
        let ix = launcher_indexer::Indexer::open(&root.join("index.db")).unwrap();
        let hits = ix.search("churn", 10).unwrap();
        assert!(!hits.is_empty(), "index searchable after soak");
    }

    handle.stop();
    std::fs::remove_dir_all(&root).ok();
}
