//! Durable Run Store (P2.6-B01, spec `P2.6 开发设计规范` §15-§18/§35):
//! SQLite persistence for workflow runs so a paused/failed/in-flight run
//! survives process restart. Checkpoints carry the FULL resumable state
//! (graph id/version, status, finished/skipped node sets, variables) —
//! recovery replays from the checkpoint, never from memory.
//!
//! Corruption policy mirrors catalog.db/plugins.db: a corrupt store is
//! rebuilt empty (runs are reconstructable state; the graph definition
//! itself lives elsewhere). Generation semantics are not used here —
//! run progress is the checkpoint, not a generation counter.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// Run lifecycle (§35 state machine, persisted slice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Paused,
    AwaitingApproval,
    Finished,
    Failed,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Paused => "paused",
            RunStatus::AwaitingApproval => "awaiting_approval",
            RunStatus::Finished => "finished",
            RunStatus::Failed => "failed",
        }
    }

    fn parse(s: &str) -> Option<RunStatus> {
        match s {
            "running" => Some(RunStatus::Running),
            "paused" => Some(RunStatus::Paused),
            "awaiting_approval" => Some(RunStatus::AwaitingApproval),
            "finished" => Some(RunStatus::Finished),
            "failed" => Some(RunStatus::Failed),
            _ => None,
        }
    }
}

/// One durable checkpoint: everything needed to resume the run in a fresh
/// process (§17 Checkpoint — graph id/version + state, never raw effects).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunCheckpoint {
    pub run_id: String,
    pub graph_id: String,
    pub graph_version: u64,
    pub status: RunStatus,
    /// Finished node ids (A03 join inputs).
    pub finished: Vec<String>,
    /// Skipped node ids (condition false upstream).
    pub skipped: Vec<String>,
    /// Run-scoped variables (A05).
    pub variables: serde_json::Value,
    pub updated_at_ms: i64,
}

pub struct RunStore {
    conn: Mutex<Connection>,
}

impl RunStore {
    /// Open with corruption recovery (rebuild empty, like catalog.db).
    pub fn open(db_path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if db_path.exists() {
            let header = std::fs::read(db_path)
                .map(|b| b[..16.min(b.len())].to_vec())
                .unwrap_or_default();
            if !header.starts_with(b"SQLite format 3\0") && !header.is_empty() {
                tracing::warn!(path = %db_path.display(), "workflow runs db corrupt — rebuilding");
                let _ = std::fs::remove_file(db_path);
            }
        }
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_runs (
                run_id        TEXT PRIMARY KEY,
                graph_id      TEXT NOT NULL,
                graph_version INTEGER NOT NULL,
                status        TEXT NOT NULL,
                finished      TEXT NOT NULL DEFAULT '[]',
                skipped       TEXT NOT NULL DEFAULT '[]',
                variables     TEXT NOT NULL DEFAULT '{}',
                updated_at_ms INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Upsert a checkpoint (single statement, atomic per run).
    pub fn checkpoint(&self, cp: &RunCheckpoint) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("run store lock");
        conn.execute(
            "INSERT INTO workflow_runs
             (run_id, graph_id, graph_version, status, finished, skipped, variables, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(run_id) DO UPDATE SET
               graph_id = ?2, graph_version = ?3, status = ?4, finished = ?5,
               skipped = ?6, variables = ?7, updated_at_ms = ?8",
            params![
                cp.run_id,
                cp.graph_id,
                cp.graph_version as i64,
                cp.status.as_str(),
                serde_json::to_string(&cp.finished).unwrap_or_default(),
                serde_json::to_string(&cp.skipped).unwrap_or_default(),
                cp.variables.to_string(),
                cp.updated_at_ms,
            ],
        )?;
        Ok(())
    }

    /// Load one run's latest checkpoint.
    pub fn load(&self, run_id: &str) -> Result<Option<RunCheckpoint>, rusqlite::Error> {
        let conn = self.conn.lock().expect("run store lock");
        let row = conn.query_row(
            "SELECT run_id, graph_id, graph_version, status, finished, skipped, variables, updated_at_ms
             FROM workflow_runs WHERE run_id = ?1",
            params![run_id],
            |r| {
                let status: String = r.get(3)?;
                Ok(RunCheckpoint {
                    run_id: r.get(0)?,
                    graph_id: r.get(1)?,
                    graph_version: r.get::<_, i64>(2)? as u64,
                    status: RunStatus::parse(&status).unwrap_or(RunStatus::Failed),
                    finished: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
                    skipped: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
                    variables: serde_json::from_str(&r.get::<_, String>(6)?)
                        .unwrap_or(serde_json::Value::Null),
                    updated_at_ms: r.get(7)?,
                })
            },
        );
        match row {
            Ok(cp) => Ok(Some(cp)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// All runs in a given status (recovery scan: e.g. resume Running/Paused).
    pub fn list_by_status(&self, status: RunStatus) -> Result<Vec<RunCheckpoint>, rusqlite::Error> {
        let conn = self.conn.lock().expect("run store lock");
        let mut stmt = conn.prepare(
            "SELECT run_id, graph_id, graph_version, status, finished, skipped, variables, updated_at_ms
             FROM workflow_runs WHERE status = ?1 ORDER BY updated_at_ms",
        )?;
        let rows = stmt.query_map(params![status.as_str()], |r| {
            let st: String = r.get(3)?;
            Ok(RunCheckpoint {
                run_id: r.get(0)?,
                graph_id: r.get(1)?,
                graph_version: r.get::<_, i64>(2)? as u64,
                status: RunStatus::parse(&st).unwrap_or(RunStatus::Failed),
                finished: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
                skipped: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
                variables: serde_json::from_str(&r.get::<_, String>(6)?)
                    .unwrap_or(serde_json::Value::Null),
                updated_at_ms: r.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> RunStore {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_runs_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        RunStore::open(&dir.join("runs.db")).unwrap()
    }

    fn checkpoint(run_id: &str) -> RunCheckpoint {
        RunCheckpoint {
            run_id: run_id.into(),
            graph_id: "wf.demo".into(),
            graph_version: 1,
            status: RunStatus::Running,
            finished: vec!["a".into(), "b".into()],
            skipped: vec![],
            variables: serde_json::json!({"status": "ok", "count": 3}),
            updated_at_ms: 1_790_000_000_000,
        }
    }

    /// B01 core: checkpoint persists across reopen with full state.
    #[test]
    fn checkpoint_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("nl_runs_reopen_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("runs.db");
        {
            let s = RunStore::open(&db).unwrap();
            s.checkpoint(&checkpoint("run-1")).unwrap();
        }
        {
            let s = RunStore::open(&db).unwrap();
            let cp = s.load("run-1").unwrap().expect("checkpoint survives");
            assert_eq!(cp.graph_id, "wf.demo");
            assert_eq!(cp.finished, vec!["a".to_string(), "b".to_string()]);
            assert_eq!(cp.status, RunStatus::Running);
            assert_eq!(cp.variables["count"], 3);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Recovery scan: interrupted runs are findable by status (§18).
    #[test]
    fn recovery_scan_finds_interrupted_runs() {
        let s = store();
        let mut cp = checkpoint("run-a");
        cp.status = RunStatus::Running;
        s.checkpoint(&cp).unwrap();
        let mut cp = checkpoint("run-b");
        cp.status = RunStatus::Paused;
        s.checkpoint(&cp).unwrap();
        let running = s.list_by_status(RunStatus::Running).unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].run_id, "run-a");
    }

    /// Corrupt store rebuilds empty (runs are reconstructable state).
    #[test]
    fn corrupt_store_rebuilds() {
        let dir = std::env::temp_dir().join(format!("nl_runs_corrupt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("runs.db");
        std::fs::write(&db, b"NOT SQLITE").unwrap();
        let s = RunStore::open(&db).unwrap();
        assert!(s.load("any").unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
