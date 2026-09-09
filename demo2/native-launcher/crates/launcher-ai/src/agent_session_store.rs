//! Agent Session persistence (P27-C07, spec `P2.7 开发设计规范` §26):
//! SQLite persistence for agent sessions so an in-flight agent can survive
//! process restart. Stores session_id, state, turn count, and the last
//! goal — enough for the host to surface a "resume agent?" prompt.
//! Corruption policy mirrors other stores: rebuild empty.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSessionRecord {
    pub session_id: String,
    pub goal: String,
    pub turn: u64,
    pub status: String,
    pub updated_at_ms: i64,
}

pub struct AgentSessionStore {
    conn: Mutex<Connection>,
}

impl AgentSessionStore {
    pub fn open(db_path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if db_path.exists() {
            let header = std::fs::read(db_path)
                .map(|b| b[..16.min(b.len())].to_vec())
                .unwrap_or_default();
            if !header.starts_with(b"SQLite format 3\0") && !header.is_empty() {
                tracing::warn!(path = %db_path.display(), "agent session db corrupt — rebuilding");
                let _ = std::fs::remove_file(db_path);
            }
        }
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS agent_sessions (
                session_id    TEXT PRIMARY KEY,
                goal          TEXT NOT NULL,
                turn          INTEGER NOT NULL DEFAULT 0,
                status        TEXT NOT NULL DEFAULT 'running',
                updated_at_ms INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Upsert a session record (single statement, atomic).
    pub fn save(&self, record: &AgentSessionRecord) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("session store lock");
        conn.execute(
            "INSERT INTO agent_sessions
             (session_id, goal, turn, status, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(session_id) DO UPDATE SET
               goal = ?2, turn = ?3, status = ?4, updated_at_ms = ?5",
            params![
                record.session_id,
                record.goal,
                record.turn as i64,
                record.status,
                record.updated_at_ms,
            ],
        )?;
        Ok(())
    }

    /// Load one session.
    pub fn load(&self, session_id: &str) -> Result<Option<AgentSessionRecord>, rusqlite::Error> {
        let conn = self.conn.lock().expect("session store lock");
        let row = conn.query_row(
            "SELECT session_id, goal, turn, status, updated_at_ms
             FROM agent_sessions WHERE session_id = ?1",
            params![session_id],
            |r| {
                Ok(AgentSessionRecord {
                    session_id: r.get(0)?,
                    goal: r.get(1)?,
                    turn: r.get::<_, i64>(2)? as u64,
                    status: r.get(3)?,
                    updated_at_ms: r.get(4)?,
                })
            },
        );
        match row {
            Ok(cp) => Ok(Some(cp)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// All active sessions (Running/Paused) — recovery scan.
    pub fn list_active(&self) -> Result<Vec<AgentSessionRecord>, rusqlite::Error> {
        let conn = self.conn.lock().expect("session store lock");
        let mut stmt = conn.prepare(
            "SELECT session_id, goal, turn, status, updated_at_ms
             FROM agent_sessions WHERE status IN ('running', 'paused')
             ORDER BY updated_at_ms",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AgentSessionRecord {
                session_id: r.get(0)?,
                goal: r.get(1)?,
                turn: r.get::<_, i64>(2)? as u64,
                status: r.get(3)?,
                updated_at_ms: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Mark a session as terminal (completed/failed/cancelled) and remove.
    pub fn finish(&self, session_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("session store lock");
        conn.execute(
            "DELETE FROM agent_sessions WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> AgentSessionStore {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_asess_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        AgentSessionStore::open(&dir.join("sessions.db")).unwrap()
    }

    fn record(sid: &str, turn: u64, status: &str) -> AgentSessionRecord {
        AgentSessionRecord {
            session_id: sid.into(),
            goal: "do the thing".into(),
            turn,
            status: status.into(),
            updated_at_ms: 1000,
        }
    }

    /// C07: session persists across reopen (restart-survivable).
    #[test]
    fn session_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("nl_asess_reopen_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("sessions.db");
        {
            let s = AgentSessionStore::open(&db).unwrap();
            s.save(&record("sess-1", 3, "running")).unwrap();
        }
        {
            let s = AgentSessionStore::open(&db).unwrap();
            let r = s.load("sess-1").unwrap().unwrap();
            assert_eq!(r.turn, 3);
            assert_eq!(r.status, "running");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// C07: active sessions discoverable for recovery scan.
    #[test]
    fn active_sessions_discoverable() {
        let s = store();
        s.save(&record("sess-a", 2, "running")).unwrap();
        s.save(&record("sess-b", 1, "paused")).unwrap();
        let active = s.list_active().unwrap();
        assert_eq!(active.len(), 2);
    }

    /// Corrupt store rebuilds empty.
    #[test]
    fn corrupt_store_rebuilds() {
        let dir = std::env::temp_dir().join(format!("nl_asess_c_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("sessions.db");
        std::fs::write(&db, b"NOT SQLITE").unwrap();
        let s = AgentSessionStore::open(&db).unwrap();
        assert!(s.list_active().unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
