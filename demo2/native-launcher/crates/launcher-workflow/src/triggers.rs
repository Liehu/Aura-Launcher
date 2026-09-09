//! Trigger Framework + durable queue (P2.6-D01/D02, spec
//! `P2.6 开发设计规范` §22-§24): trigger events from any source
//! (hotkey/plugin/schedule/ai-mcp) land in a DURABLE FIFO queue; the
//! workflow service dequeues them to start runs. The queue carries trigger
//! METADATA only — never credentials or payload secrets.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    Hotkey,
    Plugin,
    Schedule,
    AiMcp,
}

impl TriggerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TriggerKind::Hotkey => "hotkey",
            TriggerKind::Plugin => "plugin",
            TriggerKind::Schedule => "schedule",
            TriggerKind::AiMcp => "ai_mcp",
        }
    }

    fn parse(s: &str) -> Option<TriggerKind> {
        match s {
            "hotkey" => Some(TriggerKind::Hotkey),
            "plugin" => Some(TriggerKind::Plugin),
            "schedule" => Some(TriggerKind::Schedule),
            "ai_mcp" => Some(TriggerKind::AiMcp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerEvent {
    pub id: i64,
    pub kind: TriggerKind,
    /// Workflow id this event asks to start.
    pub workflow_id: String,
    /// Trigger input (workflow variables seed) — metadata only.
    pub input: serde_json::Value,
    pub queued_at_ms: i64,
}

pub struct TriggerQueue {
    conn: Mutex<Connection>,
}

impl TriggerQueue {
    pub fn open(db_path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS trigger_queue (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                kind          TEXT NOT NULL,
                workflow_id   TEXT NOT NULL,
                input         TEXT NOT NULL DEFAULT '{}',
                queued_at_ms  INTEGER NOT NULL,
                consumed      INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Enqueue a trigger event (FIFO).
    pub fn enqueue(
        &self,
        kind: TriggerKind,
        workflow_id: &str,
        input: &serde_json::Value,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().expect("queue lock");
        conn.execute(
            "INSERT INTO trigger_queue (kind, workflow_id, input, queued_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                kind.as_str(),
                workflow_id,
                input.to_string(),
                now_ms()
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Dequeue the oldest unconsumed event (consume-once: marked immediately).
    pub fn dequeue(&self) -> Result<Option<TriggerEvent>, rusqlite::Error> {
        let conn = self.conn.lock().expect("queue lock");
        let row = conn.query_row(
            "SELECT id, kind, workflow_id, input, queued_at_ms FROM trigger_queue
             WHERE consumed = 0 ORDER BY id LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        );
        let (id, kind, workflow_id, input, queued_at_ms) = match row {
            Ok(v) => v,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };
        conn.execute("UPDATE trigger_queue SET consumed = 1 WHERE id = ?1", [id])?;
        Ok(Some(TriggerEvent {
            id,
            kind: TriggerKind::parse(&kind).unwrap_or(TriggerKind::Plugin),
            workflow_id,
            input: serde_json::from_str(&input).unwrap_or(serde_json::Value::Null),
            queued_at_ms,
        }))
    }

    /// Pending (unconsumed) depth — health/diagnostics.
    pub fn pending(&self) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().expect("queue lock");
        conn.query_row(
            "SELECT count(*) FROM trigger_queue WHERE consumed = 0",
            [],
            |r| r.get(0),
        )
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

    #[test]
    fn fifo_queue_consume_once() {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_trig_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let q = TriggerQueue::open(&dir.join("triggers.db")).unwrap();
        q.enqueue(TriggerKind::Hotkey, "wf.backup", &serde_json::json!({"src": "a"}))
            .unwrap();
        q.enqueue(TriggerKind::Schedule, "wf.nightly", &serde_json::json!({}))
            .unwrap();
        assert_eq!(q.pending().unwrap(), 2);
        let e1 = q.dequeue().unwrap().unwrap();
        assert_eq!(e1.kind, TriggerKind::Hotkey);
        assert_eq!(e1.workflow_id, "wf.backup");
        assert_eq!(q.pending().unwrap(), 1);
        let e2 = q.dequeue().unwrap().unwrap();
        assert_eq!(e2.kind, TriggerKind::Schedule);
        assert!(q.dequeue().unwrap().is_none(), "consume-once: empty at end");
        std::fs::remove_dir_all(&dir).ok();
    }
}
