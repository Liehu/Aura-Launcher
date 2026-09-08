//! IndexCoordinator (P2.1-B, review 76 §20/§23/§43): the policy/orchestration
//! layer of the incremental index engine. It receives watcher hints, keeps a
//! bounded queue, coalesces, re-stats the filesystem (verification), applies
//! verified batches through the single writer, manages dirty roots and
//! publishes generation + health. It never executes SQL outside the writer
//! and never blocks interactive search (WAL + dedicated writer connection).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use launcher_domain::{
    FileChange, FileObservation, IndexHealth, IndexStatus, VerifiedChange,
};

use crate::incremental::{BoundedQueue, Coalescer, DirtyRootSet};
use crate::watcher::{CoordinatorMsg, StopHandle, WindowsWatcher};
use crate::Indexer;

pub const DEFAULT_QUEUE_CAPACITY: usize = 4096;
pub const DEFAULT_BATCH_SIZE: usize = 256;
const TICK: Duration = Duration::from_millis(300);

#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// INV-INDEX-001: hard event-queue capacity (full → dirty root).
    pub queue_capacity: usize,
    /// Events (or tick) before a batch is verified + committed.
    pub batch_size: usize,
    /// Roots to watch AND index.
    pub roots: Vec<PathBuf>,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            batch_size: DEFAULT_BATCH_SIZE,
            roots: Vec::new(),
        }
    }
}

/// Public handle: status polling + shutdown.
pub struct CoordinatorHandle {
    pub status: Arc<Mutex<IndexStatus>>,
    stop_tx: std::sync::mpsc::Sender<()>,
    done: Arc<AtomicBool>,
}

impl CoordinatorHandle {
    pub fn stop(self) {
        let _ = self.stop_tx.send(());
    }

    pub fn is_running(&self) -> bool {
        !self.done.load(Ordering::SeqCst)
    }

    /// True when the coordinator is quiescent (review 76 §47): Ready, no
    /// pending events, no dirty roots — the precondition for the eventual
    /// consistency assertion (filesystem == index).
    pub fn quiescent(&self) -> bool {
        match self.status.lock() {
            Ok(s) => {
                s.health == IndexHealth::Ready
                    && s.pending_events == 0
                    && s.dirty_roots == 0
            }
            Err(_) => false,
        }
    }
}

/// Spawn the coordinator: initial rescan of all roots (the startup rebuild,
/// through the SAME single writer), then the watch/maintenance loop. The
/// watcher is started BEFORE the first rescan and keeps collecting during it;
/// queued events are applied after the rebuild commit (review 76 §24) — no
/// rebuild gap.
pub fn spawn(db_path: &Path, cfg: CoordinatorConfig) -> CoordinatorHandle {
    let status = Arc::new(Mutex::new(IndexStatus {
        health: IndexHealth::Rebuilding,
        ..Default::default()
    }));
    let (tx, rx) = std::sync::mpsc::channel::<CoordinatorMsg>();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));

    let stop_events = StopHandle::new().expect("CreateEventW");
    // P2.4-B01: ALL configured roots are handed to the coordinator; watchers
    // are spawned for existing roots only, missing ones start Unavailable and
    // are polled for reappearance (bounded rate) by the maintenance sweep.
    let existing: Vec<PathBuf> = cfg
        .roots
        .iter()
        .filter(|r| r.exists())
        .cloned()
        .collect();
    if !existing.is_empty() {
        WindowsWatcher::spawn(&existing, tx.clone(), &stop_events);
    }

    let db = db_path.to_path_buf();
    let tx2 = tx.clone();
    let status2 = status.clone();
    let done2 = done.clone();
    let stop_flag2 = stop_flag.clone();
    std::thread::Builder::new()
        .name("index-coordinator".into())
        .spawn(move || {
            run_loop(&db, cfg, rx, tx2, status2, stop_events, stop_rx, stop_flag2);
            done2.store(true, Ordering::SeqCst);
        })
        .ok();

    CoordinatorHandle {
        status,
        stop_tx,
        done,
    }
}

fn set_status(
    status: &Mutex<IndexStatus>,
    maint: &Maintenance,
    health: IndexHealth,
    pending: usize,
    dirty: usize,
    generation: u64,
    entries: u64,
    err: Option<String>,
) {
    if let Ok(mut s) = status.lock() {
        s.health = health;
        s.pending_events = pending;
        s.dirty_roots = dirty;
        s.generation = generation;
        s.indexed_entries = entries;
        s.last_error = err;
        // P2.4-B05 health model (spec §5.3)
        s.watcher_count = maint.watcher_count;
        s.unavailable_roots = maint.unavailable.len();
        s.last_success_ms = maint.last_success_ms;
        s.last_failure_ms = maint.last_failure_ms;
        s.recovery_count = maint.recovery_count;
    }
}

/// P2.4-B01/B02: bounded re-registration + reappearance scheduling.
///
/// Backoff: 1s doubling to a 60s cap for the first REWATCH_MAX_ATTEMPTS
/// attempts; after that the root switches to a bounded-rate slow poll (60s) —
/// never an unbounded retry loop, and never abandoned (self-healing on
/// reappearance).
struct Maintenance {
    /// (root, attempts, due_at) scheduled for watcher re-registration.
    rewatch: Vec<(String, u32, Instant)>,
    unavailable: std::collections::HashSet<String>,
    /// Roots currently believed to be watched (existence is re-checked on
    /// every sweep pass — a deleted root is detected even when its watcher
    /// thread happens to survive the deletion).
    watched: Vec<String>,
    last_existence_check: Instant,
    watcher_count: usize,
    last_success_ms: Option<i64>,
    last_failure_ms: Option<i64>,
    recovery_count: u64,
}

const REWATCH_BASE_MS: u64 = 1_000;
const REWATCH_CAP_MS: u64 = 60_000;
const REWATCH_MAX_ATTEMPTS: u32 = 6;
const REWATCH_SLOW_POLL_MS: u64 = 60_000;

impl Maintenance {
    fn new() -> Self {
        Self {
            rewatch: Vec::new(),
            unavailable: std::collections::HashSet::new(),
            watched: Vec::new(),
            last_existence_check: Instant::now(),
            watcher_count: 0,
            last_success_ms: None,
            last_failure_ms: None,
            recovery_count: 0,
        }
    }

    fn mark_unavailable(&mut self, root: &str) {
        // keys are NORMALIZED identities: call sites spell roots differently
        // (original-case PathBuf vs watcher message strings) and a case
        // mismatch would leak stale Unavailable state forever.
        if self
            .unavailable
            .insert(launcher_domain::normalize_path_identity(root))
        {
            tracing::warn!(root = %root, "index.root_unavailable");
        }
        self.last_failure_ms = Some(now_ms());
    }

    fn schedule_rewatch(&mut self, root: &str) {
        self.mark_unavailable(root);
        let key = launcher_domain::normalize_path_identity(root);
        if !self.rewatch.iter().any(|(r, _, _)| r == &key) {
            self.rewatch.push((key, 0, Instant::now()));
        }
    }

    fn backoff_ms(attempts: u32) -> u64 {
        if attempts >= REWATCH_MAX_ATTEMPTS {
            REWATCH_SLOW_POLL_MS
        } else {
            REWATCH_BASE_MS
                .saturating_mul(1u64 << attempts.min(6))
                .min(REWATCH_CAP_MS)
        }
    }

    fn sweep(
        &mut self,
        writer: &mut Indexer,
        status: &Mutex<IndexStatus>,
        stop_events: &StopHandle,
        tx: &Sender<CoordinatorMsg>,
    ) {
        let now = Instant::now();

        // existence patrol (cheap stat calls, at most 1/s): a deleted watched
        // root is scheduled for recovery even if its watcher thread survives
        if self.last_existence_check.elapsed() >= std::time::Duration::from_millis(1_000) {
            self.last_existence_check = now;
            let mut i = 0;
            while i < self.watched.len() {
                let root = self.watched[i].clone();
                if PathBuf::from(&root).exists() {
                    i += 1;
                } else {
                    tracing::warn!(root = %root, "index.watch_root_missing");
                    self.watched.remove(i);
                    self.watcher_count = self.watcher_count.saturating_sub(1);
                    self.last_failure_ms = Some(now_ms());
                    self.schedule_rewatch(&root);
                }
            }
        }

        let mut i = 0;
        while i < self.rewatch.len() {
            let (root, attempts, due_at) = self.rewatch[i].clone();
            if now < due_at {
                i += 1;
                continue;
            }
            let path = PathBuf::from(&root);
            if !path.exists() {
                // still gone: bounded backoff, never abandoned
                let next = attempts + 1;
                self.rewatch[i] = (
                    root.clone(),
                    next,
                    now + std::time::Duration::from_millis(Self::backoff_ms(next)),
                );
                i += 1;
                continue;
            }
            // root is back (or a live-root watcher glitch): bounded subtree
            // rescan + watcher re-registration
            match writer.rescan_root(&path) {
                Ok(n) => {
                    WindowsWatcher::spawn_root(&path, tx.clone(), stop_events);
                    self.watcher_count += 1;
                    if !self.watched.iter().any(|w| w == &root) {
                        self.watched.push(root.clone());
                    }
                    self.recovery_count += 1;
                    self.last_success_ms = Some(now_ms());
                    self.unavailable.remove(&root);
                    self.rewatch.remove(i);
                    tracing::info!(root = %root, entries = n, "index.root_recovered_and_rewatched");
                    set_status(status, self, IndexHealth::Ready, 0, 0, gen(writer), entries(writer), None);
                }
                Err(e) => {
                    self.last_failure_ms = Some(now_ms());
                    let next = attempts + 1;
                    self.rewatch[i] = (
                        root.clone(),
                        next,
                        now + std::time::Duration::from_millis(Self::backoff_ms(next)),
                    );
                    tracing::warn!(root = %root, error = %e, "index.root_rescan_failed");
                    i += 1;
                }
            }
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Verify one changed path against the REAL filesystem (INV-INDEX-004):
/// Present → Upsert(observation); Missing → Delete. This is the only place
/// hints become database mutations.
fn verify(change: &FileChange) -> Vec<VerifiedChange> {
    let mut out = Vec::new();
    if let Some(old) = &change.old_path {
        // rename: verification collapses it into Delete(old) + Upsert(new)
        out.extend(verify_one(old));
    }
    out.extend(verify_one(&change.path));
    out
}

fn verify_one(path: &str) -> Vec<VerifiedChange> {
    let normalized = launcher_domain::normalize_path_identity(path);
    if normalized.is_empty() || launcher_domain::is_device_namespace(path) {
        return Vec::new();
    }
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            out_vec(VerifiedChange::Upsert(FileObservation {
                path: path.to_string(),
                normalized_path: normalized,
                is_dir: meta.is_dir(),
                size: meta.len() as i64,
                modified_ms,
            }))
        }
        Err(_) => out_vec(VerifiedChange::Delete { normalized_path: normalized }),
    }
}

fn out_vec(v: VerifiedChange) -> Vec<VerifiedChange> {
    vec![v]
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn run_loop(
    db_path: &Path,
    cfg: CoordinatorConfig,
    rx: Receiver<CoordinatorMsg>,
    tx: Sender<CoordinatorMsg>,
    status: Arc<Mutex<IndexStatus>>,
    stop_events: StopHandle,
    _stop_rx: Receiver<()>,
    stop_flag: Arc<AtomicBool>,
) {
    let mut maint = Maintenance::new();
    let Ok(mut writer) = Indexer::open(db_path) else {
        set_status(
            &status,
            &maint,
            IndexHealth::Failed,
            0,
            0,
            0,
            0,
            Some("cannot open index db".into()),
        );
        return;
    };
    let mut queue = BoundedQueue::new(cfg.queue_capacity);
    let mut coalescer = Coalescer::default();
    let mut dirty = DirtyRootSet::default();
    let mut last_tick = Instant::now();

    // ---- initial rebuild through the single writer (B10 baseline) ----
    // P2.4-B01: every configured root enters the state machine — existing
    // roots are watched + scanned, missing ones start Unavailable and are
    // polled for reappearance by the maintenance sweep (Configured ->
    // Unavailable -> Reappeared -> rescan + re-watch).
    let mut watching: Vec<PathBuf> = Vec::new();
    for root in &cfg.roots {
        if root.exists() {
            watching.push(root.clone());
            dirty.insert(&root.to_string_lossy());
        } else {
            maint.mark_unavailable(&root.to_string_lossy());
        }
    }
    maint.watcher_count = watching.len();
    maint.watched = watching.iter().map(|p| p.to_string_lossy().to_string()).collect();
    recovery_pass(&mut writer, &mut dirty, &status, &mut maint);

    loop {
        // drain stop / events with a tick so batches flush regularly
        let deadline = last_tick + TICK;
        let now = Instant::now();
        let timeout = deadline.saturating_duration_since(now);
        match rx.recv_timeout(timeout) {
            // a watcher's own Stop is benign (root gone); only OUR stop
            // channel ends the coordinator
            Ok(CoordinatorMsg::Stop) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                stop_flag.store(true, Ordering::SeqCst);
            }
            Ok(CoordinatorMsg::Overflow { root }) => {
                dirty.insert(&root);
                // queued events for this root are untrustworthy — drop them
                queue = BoundedQueue::new(queue.capacity());
                coalescer = Coalescer::default();
            }
            Ok(CoordinatorMsg::WatcherExited { root }) => {
                // P2.4-B02: the watcher thread died (watch error or the root
                // itself vanished). Mark dirty (close any event gap via the
                // recovery rescan) and schedule bounded re-registration.
                tracing::warn!(root = %root, "watcher.exited — scheduling re-registration");
                maint.watcher_count = maint.watcher_count.saturating_sub(1);
                maint.last_failure_ms = Some(now_ms());
                dirty.insert(&root);
                maint.schedule_rewatch(&root);
            }
            Ok(CoordinatorMsg::Change(c)) => {
                let push_failed = !queue.push(c.clone());
                if push_failed {
                    // INV-INDEX-001 full: drop queued events and mark their
                    // CONTAINING watched root dirty — a file path itself is
                    // not a rescan root; only subtree roots recover.
                    let cap = queue.capacity();
                    let mut q = std::mem::replace(&mut queue, BoundedQueue::new(cap));
                    let mut overflowed = false;
                    for e in q.drain() {
                        if let Some(r) = containing_root(&cfg.roots, &e.path) {
                            dirty.insert(&r.to_string_lossy());
                            overflowed = true;
                        }
                    }
                    if !overflowed {
                        for r in &cfg.roots {
                            dirty.insert(&r.to_string_lossy());
                        }
                    }
                    // the rejected event's root must be dirty too
                    if let Some(r) = containing_root(&cfg.roots, &c.path) {
                        dirty.insert(&r.to_string_lossy());
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        if stop_flag.load(Ordering::SeqCst) {
            stop_events.signal();
            break;
        }

        // ---- dirty recovery pass (bounded subtree rescan, INV-INDEX-002) --
        if !dirty.is_empty() {
            recovery_pass(&mut writer, &mut dirty, &status, &mut maint);
        }

        // ---- P2.4-B01/B02 maintenance sweep: watcher re-registration with
        // bounded backoff + root reappearance (Unavailable -> rescan). ----
        maint.sweep(&mut writer, &status, &stop_events, &tx);

        // bridge: raw queue → coalescer (identity-normalized)
        for e in queue.drain() {
            coalescer.push(e);
        }

        // ---- batch verification + commit (generation advances per commit) -
        let due = coalescer.len() >= cfg.batch_size || last_tick.elapsed() >= TICK;
        if due && !coalescer.is_empty() {
            let identities = coalescer.drain();
            set_status(&status, &maint, IndexHealth::Updating, identities.len() + queue.len(), dirty.len(), gen(&writer), entries(&writer), None);
            let mut changes: Vec<VerifiedChange> = Vec::new();
            for id in &identities {
                let hint = FileChange {
                    kind: launcher_domain::FileChangeKind::Modified,
                    path: id.clone(),
                    old_path: None,
                };
                changes.extend(verify(&hint));
            }
            match writer.apply_batch(&changes) {
                Ok(stats) => {
                    tracing::debug!(
                        upserts = stats.upserted,
                        deletes = stats.deleted,
                        generation = stats.generation,
                        "index.batch_committed"
                    );
                    set_status(&status, &maint, IndexHealth::Ready, 0, dirty.len(), stats.generation, entries(&writer), None);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "index batch failed");
                    set_status(&status, &maint, IndexHealth::Degraded, identities.len() + queue.len(), dirty.len(), gen(&writer), entries(&writer), Some(e.to_string()));
                }
            }
            last_tick = Instant::now();
        } else {
            // pending = coalescer + the raw queue (events not yet drained)
            set_status(
                &status,
                &maint,
                if dirty.is_empty() && queue.is_empty() && coalescer.is_empty() {
                    IndexHealth::Ready
                } else if dirty.is_empty() {
                    IndexHealth::Updating
                } else {
                    IndexHealth::Degraded
                },
                coalescer.len() + queue.len(),
                dirty.len(),
                gen(&writer),
                entries(&writer),
                None,
            );
        }
    }
}

/// The configured watch root that contains `path` (normalized prefix match).
fn containing_root(roots: &[PathBuf], path: &str) -> Option<PathBuf> {
    let norm = launcher_domain::normalize_path_identity(path);
    roots
        .iter()
        .find(|r| {
            let rid = launcher_domain::normalize_path_identity(&r.to_string_lossy());
            norm == rid || norm.starts_with(&format!("{rid}\\"))
        })
        .cloned()
}

fn gen(writer: &Indexer) -> u64 {
    writer.generation().unwrap_or(0)
}

fn entries(writer: &Indexer) -> u64 {
    writer.status().map(|(n,)| n as u64).unwrap_or(0)
}

/// Bounded subtree recovery (review 76 §16-19): delete everything under the
/// root in the index, re-scan the REAL subtree (reparse points not followed),
/// insert in bounded batches, bump generation once per root commit.
/// NOT a full-disk rebuild — only dirty roots are touched.
fn recovery_pass(
    writer: &mut Indexer,
    dirty: &mut DirtyRootSet,
    status: &Mutex<IndexStatus>,
    maint: &mut Maintenance,
) {
    for root in dirty.drain() {
        if let Ok(Some(original)) = find_original_root(writer, &root) {
            match writer.rescan_root(std::path::Path::new(&original)) {
                Ok(n) => {
                    tracing::info!(root = %root, entries = n, "index.recovery_committed");
                    maint.recovery_count += 1;
                    maint.last_success_ms = Some(now_ms());
                    maint.unavailable.remove(&root);
                    set_status(status, maint, IndexHealth::Ready, 0, dirty.len(), gen(writer), entries(writer), None);
                }
                Err(e) => {
                    tracing::warn!(root = %root, error = %e, "recovery rescan failed");
                    maint.last_failure_ms = Some(now_ms());
                    set_status(status, maint, IndexHealth::Degraded, 0, dirty.len(), gen(writer), entries(writer), Some(e.to_string()));
                }
            }
        } else {
            // root not in index (first run) or vanished: plain scan
            let p = PathBuf::from(&root);
            if p.exists() {
                match writer.rescan_root(&p) {
                    Ok(n) => {
                        tracing::info!(root = %root, entries = n, "index.scan_committed");
                        maint.recovery_count += 1;
                        maint.last_success_ms = Some(now_ms());
                        maint.unavailable.remove(&root);
                        set_status(status, maint, IndexHealth::Ready, 0, dirty.len(), gen(writer), entries(writer), None);
                    }
                    Err(e) => {
                        maint.last_failure_ms = Some(now_ms());
                        set_status(status, maint, IndexHealth::Degraded, 0, dirty.len(), gen(writer), entries(writer), Some(e.to_string()));
                    }
                }
            } else {
                // P2.4-B04: root vanished — Unavailable, index stays queryable;
                // the maintenance sweep polls for reappearance.
                maint.mark_unavailable(&root);
                set_status(status, maint, IndexHealth::Unavailable, 0, dirty.len(), gen(writer), entries(writer), Some(format!("root unavailable: {root}")));
            }
        }
    }
}

/// Find any indexed original path whose normalized form starts with `root`.
fn find_original_root(writer: &Indexer, root: &str) -> Result<Option<String>, crate::IndexError> {
    writer.first_path_under(root)
}
