//! Workflow diagnostics snapshot (P2.6-F01, spec `P2.6 开发设计规范`
//! §30-§31): a bounded, host-queryable snapshot of workflow run activity —
//! run status, per-run step progress, and failure records. Observation
//! only: diagnostics never mutate runs and never execute effects.

use crate::durable::{RunCheckpoint, RunStatus, RunStore};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RunDiagnostic {
    pub run_id: String,
    pub graph_id: String,
    pub graph_version: u64,
    pub status: RunStatus,
    pub finished_count: usize,
    pub skipped_count: usize,
    pub updated_at_ms: i64,
}

/// Aggregate diagnostics for all runs tracked by a store, optionally
/// filtered by status. Output is deterministic (sorted by run_id).
pub fn snapshot(store: &RunStore, status: Option<RunStatus>) -> Result<Vec<RunDiagnostic>, rusqlite::Error> {
    let runs: Vec<RunCheckpoint> = match status {
        Some(s) => store.list_by_status(s)?,
        None => {
            let mut all = Vec::new();
            for s in [
                RunStatus::Running,
                RunStatus::Paused,
                RunStatus::AwaitingApproval,
                RunStatus::Finished,
                RunStatus::Failed,
            ] {
                all.extend(store.list_by_status(s)?);
            }
            all.sort_by(|a, b| a.run_id.cmp(&b.run_id));
            all
        }
    };
    Ok(runs
        .into_iter()
        .map(|cp| RunDiagnostic {
            run_id: cp.run_id,
            graph_id: cp.graph_id,
            graph_version: cp.graph_version,
            status: cp.status,
            finished_count: cp.finished.len(),
            skipped_count: cp.skipped.len(),
            updated_at_ms: cp.updated_at_ms,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable::RunCheckpoint;

    fn store() -> RunStore {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_diag_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        RunStore::open(&dir.join("runs.db")).unwrap()
    }

    fn cp(run_id: &str, status: RunStatus) -> RunCheckpoint {
        RunCheckpoint {
            run_id: run_id.into(),
            graph_id: "wf.d".into(),
            graph_version: 1,
            status,
            finished: vec!["a".into()],
            skipped: vec![],
            variables: serde_json::json!({}),
            updated_at_ms: 100,
        }
    }

    /// F01: snapshot aggregates runs by status deterministically.
    #[test]
    fn snapshot_aggregates_by_status() {
        let s = store();
        s.checkpoint(&cp("r-finished", RunStatus::Finished)).unwrap();
        s.checkpoint(&cp("r-running", RunStatus::Running)).unwrap();

        let diag = snapshot(&s, None).unwrap();
        assert_eq!(diag.len(), 2, "all runs visible");
        // sorted by run_id
        assert!(diag[0].run_id <= diag[1].run_id);
        let running = snapshot(&s, Some(RunStatus::Running)).unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].status, RunStatus::Running);
    }
}
