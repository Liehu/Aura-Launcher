//! Plugin Development Diagnostics (P2.4-C06/E, spec `01-P2.4-DESIGN-SPEC.md`
//! §8): structured, bounded, in-memory diagnostic records for plugin
//! interactions. Diagnostics are OBSERVATIONS ONLY — they never grant
//! capability, never execute an Effect and never bypass ActionResolver.

use std::collections::VecDeque;

/// Structured failure/interaction classification (Plugin Contract §failure
/// taxonomy + dev-mode semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticClass {
    Ok,
    Timeout,
    Crash,
    Malformed,
    Flood,
    SpawnFailed,
}

impl DiagnosticClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticClass::Ok => "ok",
            DiagnosticClass::Timeout => "timeout",
            DiagnosticClass::Crash => "crash",
            DiagnosticClass::Malformed => "malformed",
            DiagnosticClass::Flood => "flood",
            DiagnosticClass::SpawnFailed => "spawn_failed",
        }
    }
}

/// One bounded diagnostic record. Correlation ids are whatever the caller
/// already holds (query_id / runtime_id / protocol_session_id) — diagnostics
/// never mint authority-bearing identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticEntry {
    pub plugin_id: String,
    pub class: DiagnosticClass,
    pub query_id: Option<u64>,
    pub runtime_id: Option<String>,
    pub protocol_session_id: Option<String>,
    pub elapsed_ms: u64,
    /// Wire frame size in bytes when applicable (frame-limit diagnostics).
    pub frame_size: Option<usize>,
    /// Result count when applicable (flood diagnostics).
    pub result_count: Option<usize>,
}

/// P2.4-E01/E03: process-wide diagnostic log shared by all plugin providers.
/// Capacity-bounded (512 records ≈ a full dev session); snapshot/dump via
/// [`global_snapshot`] / [`global_dump_json`] never blocks and never executes.
pub fn global_log() -> &'static std::sync::Mutex<DiagnosticLog> {
    static LOG: std::sync::OnceLock<std::sync::Mutex<DiagnosticLog>> = std::sync::OnceLock::new();
    LOG.get_or_init(|| std::sync::Mutex::new(DiagnosticLog::new(512)))
}

pub fn global_record(entry: DiagnosticEntry) {
    if let Ok(mut log) = global_log().lock() {
        log.record(entry);
    }
}

pub fn global_snapshot() -> Vec<DiagnosticEntry> {
    global_log().lock().map(|l| l.snapshot()).unwrap_or_default()
}

/// E03: deterministic JSON snapshot for dev tooling / support bundles.
pub fn global_dump_json() -> String {
    let entries: Vec<serde_json::Value> = global_snapshot()
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "plugin_id": e.plugin_id,
                "class": e.class.as_str(),
                "query_id": e.query_id,
                "runtime_id": e.runtime_id,
                "protocol_session_id": e.protocol_session_id,
                "elapsed_ms": e.elapsed_ms,
                "frame_size": e.frame_size,
                "result_count": e.result_count,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "entries": entries }))
        .unwrap_or_default()
}

/// Bounded diagnostic ring. Oldest entries fall off; capacity is fixed at
/// construction (INV: no unbounded collections).
#[derive(Debug)]
pub struct DiagnosticLog {
    capacity: usize,
    entries: VecDeque<DiagnosticEntry>,
}

impl DiagnosticLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::new(),
        }
    }

    pub fn record(&mut self, entry: DiagnosticEntry) {
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Deterministic snapshot (oldest → newest), for dev-tooling dumps.
    pub fn snapshot(&self) -> Vec<DiagnosticEntry> {
        self.entries.iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(class: DiagnosticClass, n: usize) -> DiagnosticEntry {
        DiagnosticEntry {
            plugin_id: "dev.plugin".into(),
            class,
            query_id: Some(n as u64),
            runtime_id: Some(format!("rt-{n}")),
            protocol_session_id: Some("sess-1".into()),
            elapsed_ms: n as u64 * 10,
            frame_size: Some(256),
            result_count: Some(3),
        }
    }

    /// Spec §8: every failure carries classification + plugin id +
    /// correlation ids + elapsed + bounded payload.
    #[test]
    fn records_carry_full_correlation() {
        let mut log = DiagnosticLog::new(8);
        log.record(entry(DiagnosticClass::Timeout, 1));
        let snap = log.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].class, DiagnosticClass::Timeout);
        assert_eq!(snap[0].plugin_id, "dev.plugin");
        assert_eq!(snap[0].query_id, Some(1));
        assert!(snap[0].runtime_id.is_some());
        assert!(snap[0].protocol_session_id.is_some());
    }

    /// Bounded ring: oldest fall off, order stays oldest → newest.
    #[test]
    fn ring_is_bounded_and_ordered() {
        let mut log = DiagnosticLog::new(4);
        for n in 0..10 {
            log.record(entry(if n % 2 == 0 {
                DiagnosticClass::Ok
            } else {
                DiagnosticClass::Crash
            }, n));
        }
        assert_eq!(log.len(), 4);
        let ids: Vec<u64> = log.snapshot().iter().map(|e| e.query_id.unwrap()).collect();
        assert_eq!(ids, vec![6, 7, 8, 9], "oldest dropped, order preserved");
    }

    /// Diagnostics never execute: the log is inert data (pin the contract —
    /// the API surface has no execution path by construction).
    #[test]
    fn classification_vocabulary_is_stable() {
        assert_eq!(DiagnosticClass::Ok.as_str(), "ok");
        assert_eq!(DiagnosticClass::Timeout.as_str(), "timeout");
        assert_eq!(DiagnosticClass::Crash.as_str(), "crash");
        assert_eq!(DiagnosticClass::Malformed.as_str(), "malformed");
        assert_eq!(DiagnosticClass::Flood.as_str(), "flood");
        assert_eq!(DiagnosticClass::SpawnFailed.as_str(), "spawn_failed");
    }
}
