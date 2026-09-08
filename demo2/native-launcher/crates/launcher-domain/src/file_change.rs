//! File-change model (P2.1-B, review 76 §9/§28/§30): pure cross-layer types.
//!
//! Frozen semantics:
//! - A watcher `FileChange` is only a HINT ("something may have happened at
//!   this path") — never database truth (INV-INDEX-004). The Coordinator
//!   re-stats the filesystem and produces `VerifiedChange`s, which are the
//!   only input the writer accepts.
//! - `VerifiedChange` collapses rename semantics into Delete(old)+Upsert(new)
//!   so the writer knows nothing about Windows rename events.
//! - `IndexHealth`/`IndexStatus` are the ONLY view Search/UI get of index
//!   maintenance; watcher/queue/coalescer stay invisible (review 76 §22/§27).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// One watcher event (hint). Paths are raw; identity comparison happens via
/// [`crate::normalize_path_identity`] downstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub kind: FileChangeKind,
    pub path: String,
    pub old_path: Option<String>,
}

/// Result of re-statting the filesystem for one path (review 76 §28):
/// metadata only — no hash, no content, no mime (bounded work guarantee).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileObservation {
    /// Original (display) path as observed on disk.
    pub path: String,
    /// Lexical canonical identity (`normalize_path_identity`).
    pub normalized_path: String,
    pub is_dir: bool,
    pub size: i64,
    pub modified_ms: i64,
}

/// The only mutation vocabulary the incremental writer understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedChange {
    Upsert(FileObservation),
    Delete {
        normalized_path: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexHealth {
    /// No index content yet (first rebuild never committed).
    Empty,
    Ready,
    Updating,
    Rebuilding,
    Degraded,
    Failed,
    /// P2.4-B01: one or more watch roots are missing (deleted/dismounted).
    /// The committed index remains fully queryable; maintenance polls for
    /// reappearance at a bounded rate and rescans + re-watches on return.
    Unavailable,
}

/// Read-only maintenance view for Search/diagnostics (review 76 §22;
/// P2.4-B05 health model per spec `01-P2.4-DESIGN-SPEC.md` §5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStatus {
    pub health: IndexHealth,
    pub generation: u64,
    pub pending_events: usize,
    pub dirty_roots: usize,
    pub indexed_entries: u64,
    pub last_error: Option<String>,
    /// Live watcher threads (one per watched root).
    pub watcher_count: usize,
    /// Roots currently missing from the filesystem.
    pub unavailable_roots: usize,
    /// Unix ms of the last successful batch commit / recovery.
    pub last_success_ms: Option<i64>,
    /// Unix ms of the last maintenance/commit failure.
    pub last_failure_ms: Option<i64>,
    /// Bounded recoveries performed (dirty-root rescans + root reappearances).
    pub recovery_count: u64,
}

impl Default for IndexStatus {
    fn default() -> Self {
        Self {
            health: IndexHealth::Empty,
            generation: 0,
            pending_events: 0,
            dirty_roots: 0,
            indexed_entries: 0,
            last_error: None,
            watcher_count: 0,
            unavailable_roots: 0,
            last_success_ms: None,
            last_failure_ms: None,
            recovery_count: 0,
        }
    }
}
