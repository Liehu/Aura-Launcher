//! IndexCoordinator (P2.1-B, review 76 §20/§23/§43): the policy/orchestration
//! layer of the incremental index engine. It receives watcher hints, keeps a
//! bounded queue, coalesces, re-stats the filesystem (verification), applies
//! verified batches through the single writer, manages dirty roots and
//! publishes generation + health. It never executes SQL outside the writer
//! and never blocks interactive search (WAL + dedicated writer connection).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
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
pub fn spawn(db_path: &Path, mut cfg: CoordinatorConfig) -> CoordinatorHandle {
    let status = Arc::new(Mutex::new(IndexStatus {
        health: IndexHealth::Rebuilding,
        generation: 0,
        pending_events: 0,
        dirty_roots: 0,
        indexed_entries: 0,
        last_error: None,
    }));
    let (tx, rx) = std::sync::mpsc::channel::<CoordinatorMsg>();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));

    let stop_events = StopHandle::new().expect("CreateEventW");
    cfg.roots.retain(|r| r.exists());
    if !cfg.roots.is_empty() {
        WindowsWatcher::spawn(&cfg.roots, tx.clone(), &stop_events);
    }

    let db = db_path.to_path_buf();
    let status2 = status.clone();
    let done2 = done.clone();
    let stop_flag2 = stop_flag.clone();
    std::thread::Builder::new()
        .name("index-coordinator".into())
        .spawn(move || {
            run_loop(&db, cfg, rx, status2, stop_events, stop_rx, stop_flag2);
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
    }
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
fn run_loop(
    db_path: &Path,
    cfg: CoordinatorConfig,
    rx: Receiver<CoordinatorMsg>,
    status: Arc<Mutex<IndexStatus>>,
    stop_events: StopHandle,
    _stop_rx: Receiver<()>,
    stop_flag: Arc<AtomicBool>,
) {
    let Ok(mut writer) = Indexer::open(db_path) else {
        set_status(
            &status,
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
    for root in &cfg.roots {
        dirty.insert(&root.to_string_lossy());
    }
    recovery_pass(&mut writer, &mut dirty, &status);

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
            recovery_pass(&mut writer, &mut dirty, &status);
        }

        // bridge: raw queue → coalescer (identity-normalized)
        for e in queue.drain() {
            coalescer.push(e);
        }

        // ---- batch verification + commit (generation advances per commit) -
        let due = coalescer.len() >= cfg.batch_size || last_tick.elapsed() >= TICK;
        if due && !coalescer.is_empty() {
            let identities = coalescer.drain();
            set_status(&status, IndexHealth::Updating, identities.len() + queue.len(), dirty.len(), gen(&writer), entries(&writer), None);
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
                    set_status(&status, IndexHealth::Ready, 0, dirty.len(), stats.generation, entries(&writer), None);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "index batch failed");
                    set_status(&status, IndexHealth::Degraded, identities.len() + queue.len(), dirty.len(), gen(&writer), entries(&writer), Some(e.to_string()));
                }
            }
            last_tick = Instant::now();
        } else {
            // pending = coalescer + the raw queue (events not yet drained)
            set_status(
                &status,
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
fn recovery_pass(writer: &mut Indexer, dirty: &mut DirtyRootSet, status: &Mutex<IndexStatus>) {
    for root in dirty.drain() {
        if let Ok(Some(original)) = find_original_root(writer, &root) {
            match writer.rescan_root(std::path::Path::new(&original)) {
                Ok(n) => {
                    tracing::info!(root = %root, entries = n, "index.recovery_committed");
                    set_status(&status, IndexHealth::Ready, 0, dirty.len(), gen(writer), entries(writer), None);
                }
                Err(e) => {
                    tracing::warn!(root = %root, error = %e, "recovery rescan failed");
                    set_status(&status, IndexHealth::Degraded, 0, dirty.len(), gen(writer), entries(writer), Some(e.to_string()));
                }
            }
        } else {
            // root not in index (first run) or vanished: plain scan
            let p = PathBuf::from(&root);
            if p.exists() {
                match writer.rescan_root(&p) {
                    Ok(n) => {
                        tracing::info!(root = %root, entries = n, "index.scan_committed");
                        set_status(&status, IndexHealth::Ready, 0, dirty.len(), gen(writer), entries(writer), None);
                    }
                    Err(e) => {
                        set_status(&status, IndexHealth::Degraded, 0, dirty.len(), gen(writer), entries(writer), Some(e.to_string()));
                    }
                }
            } else {
                set_status(&status, IndexHealth::Degraded, 0, dirty.len(), gen(writer), entries(writer), Some(format!("root unavailable: {root}")));
            }
        }
    }
}

/// Find any indexed original path whose normalized form starts with `root`.
fn find_original_root(writer: &Indexer, root: &str) -> Result<Option<String>, crate::IndexError> {
    writer.first_path_under(root)
}
