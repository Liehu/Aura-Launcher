//! Human Approval (P2.6-C01/C02, spec `P2.6 开发设计规范` §19-§21):
//! durable approval requests backed by SQLite. An approval pauses the run
//! (AwaitingApproval checkpoint) until an explicit decision; only an
//! APPROVED decision lets the scheduler execute the node — a REJECTED
//! decision skips it. Decisions are consume-once per (run, node) and
//! persist across restarts.

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApprovalStatus::Pending => "pending",
            ApprovalStatus::Approved => "approved",
            ApprovalStatus::Rejected => "rejected",
        }
    }

    fn parse(s: &str) -> ApprovalStatus {
        match s {
            "approved" => ApprovalStatus::Approved,
            "rejected" => ApprovalStatus::Rejected,
            _ => ApprovalStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRecord {
    pub approval_id: String,
    pub run_id: String,
    pub node_id: String,
    pub action_ref: String,
    pub status: ApprovalStatus,
}

pub struct ApprovalStore {
    conn: Mutex<Connection>,
}

impl ApprovalStore {
    pub fn open(db_path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS approvals (
                approval_id TEXT PRIMARY KEY,
                run_id      TEXT NOT NULL,
                node_id     TEXT NOT NULL,
                action_ref  TEXT NOT NULL,
                status      TEXT NOT NULL DEFAULT 'pending',
                updated_ms  INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create (or keep) a pending request for (run, node). Idempotent: a
    /// repeated scheduler pass never resets an existing decision.
    pub fn request(
        &self,
        approval_id: &str,
        run_id: &str,
        node_id: &str,
        action_ref: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("approval lock");
        conn.execute(
            "INSERT OR IGNORE INTO approvals
             (approval_id, run_id, node_id, action_ref, status, updated_ms)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
            params![
                approval_id,
                run_id,
                node_id,
                action_ref,
                now_ms()
            ],
        )?;
        Ok(())
    }

    /// Explicit decision (§21): the ONLY transition out of pending. Returns
    /// false when the approval id is unknown.
    pub fn decide(
        &self,
        approval_id: &str,
        approved: bool,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().expect("approval lock");
        let n = conn.execute(
            "UPDATE approvals SET status = ?2, updated_ms = ?3
             WHERE approval_id = ?1 AND status = 'pending'",
            params![
                approval_id,
                if approved {
                    ApprovalStatus::Approved.as_str()
                } else {
                    ApprovalStatus::Rejected.as_str()
                },
                now_ms()
            ],
        )?;
        Ok(n > 0)
    }

    /// Scheduler query: has the node's approval been granted? Rejected or
    /// still-pending are both "not approved" (rejected → skip the node).
    pub fn decision_for(
        &self,
        run_id: &str,
        node_id: &str,
    ) -> Result<Option<ApprovalStatus>, rusqlite::Error> {
        let conn = self.conn.lock().expect("approval lock");
        let status: Option<String> = conn
            .query_row(
                "SELECT status FROM approvals WHERE run_id = ?1 AND node_id = ?2",
                params![run_id, node_id],
                |r| r.get(0),
            )
            .ok();
        Ok(status.map(|s| ApprovalStatus::parse(&s)))
    }

    /// All pending approvals (UI/audit surface, P2.6-C03 input).
    pub fn pending(&self) -> Result<Vec<ApprovalRecord>, rusqlite::Error> {
        let conn = self.conn.lock().expect("approval lock");
        let mut stmt = conn.prepare(
            "SELECT approval_id, run_id, node_id, action_ref, status
             FROM approvals WHERE status = 'pending' ORDER BY updated_ms",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ApprovalRecord {
                approval_id: r.get(0)?,
                run_id: r.get(1)?,
                node_id: r.get(2)?,
                action_ref: r.get(3)?,
                status: ApprovalStatus::Pending,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> ApprovalStore {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_appr_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        ApprovalStore::open(&dir.join("approvals.db")).unwrap()
    }

    /// C01/C02 core: pending request → explicit approve; decisions persist
    /// across reopen; unknown ids report false.
    #[test]
    fn approval_lifecycle_persists() {
        let dir = std::env::temp_dir().join(format!("nl_appr_life_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("approvals.db");
        {
            let s = ApprovalStore::open(&db).unwrap();
            s.request("ap-1", "run-1", "n2", "cmd:deploy").unwrap();
            assert_eq!(s.pending().unwrap().len(), 1);
            // idempotent re-request never duplicates nor resets
            s.request("ap-1", "run-1", "n2", "cmd:deploy").unwrap();
            assert_eq!(s.pending().unwrap().len(), 1);
            assert!(s.decide("ap-1", true).unwrap());
        }
        {
            let s = ApprovalStore::open(&db).unwrap();
            assert!(s.pending().unwrap().is_empty(), "decided leaves pending");
            let d = s.decision_for("run-1", "n2").unwrap().unwrap();
            assert_eq!(d, ApprovalStatus::Approved);
            assert!(!s.decide("unknown", true).unwrap());
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Rejection is a first-class decision (node gets skipped, not run).
    #[test]
    fn rejection_recorded() {
        let s = store();
        s.request("ap-2", "run-2", "n9", "cmd:x").unwrap();
        assert!(s.decide("ap-2", false).unwrap());
        assert_eq!(
            s.decision_for("run-2", "n9").unwrap(),
            Some(ApprovalStatus::Rejected)
        );
    }
}
