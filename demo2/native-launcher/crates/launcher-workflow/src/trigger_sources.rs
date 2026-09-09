//! Trigger source helpers (P2.6-D03–D06, spec §22): typed constructors for
//! the four trigger source categories. Each source enqueues into the
//! durable TriggerQueue (P2.6-D02); the consumer service starts the matching
//! workflow. Sources are producers of METADATA only — never effects.

use crate::triggers::{TriggerKind, TriggerQueue};

/// D03: hotkey trigger — user pressed a registered hotkey.
pub fn enqueue_hotkey(
    queue: &TriggerQueue,
    workflow_id: &str,
    input: &serde_json::Value,
) -> Result<i64, rusqlite::Error> {
    queue.enqueue(TriggerKind::Hotkey, workflow_id, input)
}

/// D04: plugin trigger — a plugin requested a workflow run (via its
/// proposal, which still goes through the Resolver for approval).
pub fn enqueue_plugin(
    queue: &TriggerQueue,
    workflow_id: &str,
    plugin_id: &str,
    input: &serde_json::Value,
) -> Result<i64, rusqlite::Error> {
    let mut ev_input = input.clone();
    ev_input["_plugin_source"] = serde_json::json!(plugin_id);
    queue.enqueue(TriggerKind::Plugin, workflow_id, &ev_input)
}

/// D05: schedule trigger — a timer/cron fired (v1: manual schedule check).
pub fn enqueue_schedule(
    queue: &TriggerQueue,
    workflow_id: &str,
    input: &serde_json::Value,
) -> Result<i64, rusqlite::Error> {
    queue.enqueue(TriggerKind::Schedule, workflow_id, input)
}

/// D06: AI/MCP trigger — an agent proposal asked to run a workflow
/// (proposal is DATA, actual run still goes through Durable Runtime).
pub fn enqueue_ai_mcp(
    queue: &TriggerQueue,
    workflow_id: &str,
    input: &serde_json::Value,
) -> Result<i64, rusqlite::Error> {
    queue.enqueue(TriggerKind::AiMcp, workflow_id, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn queue() -> (TriggerQueue, PathBuf) {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_trig_h_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let q = TriggerQueue::open(&dir.join("triggers.db")).unwrap();
        (q, dir)
    }

    /// D03–D06: all four source helpers enqueue successfully with correct kind.
    #[test]
    fn all_four_sources_enqueue() {
        let (q, _dir) = queue();
        enqueue_hotkey(&q, "wf.a", &serde_json::json!({"key": "ctrl+alt+b"})).unwrap();
        enqueue_plugin(&q, "wf.b", "my-plugin", &serde_json::json!({})).unwrap();
        enqueue_schedule(&q, "wf.c", &serde_json::json!({"cron": "0 2 * * *"})).unwrap();
        enqueue_ai_mcp(&q, "wf.d", &serde_json::json!({"goal": "backup"})).unwrap();
        // all queued
        let e1 = q.dequeue().unwrap().unwrap();
        assert_eq!(e1.kind, TriggerKind::Hotkey);
        assert_eq!(e1.workflow_id, "wf.a");
        let e2 = q.dequeue().unwrap().unwrap();
        assert_eq!(e2.kind, TriggerKind::Plugin);
        assert_eq!(e2.workflow_id, "wf.b");
        let e3 = q.dequeue().unwrap().unwrap();
        assert_eq!(e3.kind, TriggerKind::Schedule);
        let e4 = q.dequeue().unwrap().unwrap();
        assert_eq!(e4.kind, TriggerKind::AiMcp);
        assert_eq!(e4.workflow_id, "wf.d");
        assert!(q.dequeue().unwrap().is_none(), "queue empty after drain");
    }
}
