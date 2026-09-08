#!/usr/bin/env python3
"""P2.4-E transform: wire plugin provider failure paths to the diagnostics log (run once)."""
from pathlib import Path

# ---- 1. global log accessor in plugin_diagnostics.rs
p = Path("crates/launcher-core/src/providers/plugin_diagnostics.rs")
s = p.read_text(encoding="utf8")
old = """/// Bounded diagnostic ring. Oldest entries fall off; capacity is fixed at
/// construction (INV: no unbounded collections)."""
new = """/// P2.4-E01/E03: process-wide diagnostic log shared by all plugin providers.
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
        .unwrap_or_else(|_| "{\"entries\":[]}".into())
}

/// Bounded diagnostic ring. Oldest entries fall off; capacity is fixed at
/// construction (INV: no unbounded collections)."""
assert old in s, "diag"
s = s.replace(old, new, 1)
p.write_text(s, encoding="utf8", newline="\n")
print("diagnostics ok")

# ---- 2. plugin provider: record failures/success into the global log
p2 = Path("crates/launcher-core/src/providers/plugin.rs")
s2 = p2.read_text(encoding="utf8")

# imports
old = """use launcher_plugin_host::{PluginError, PluginHandle};"""
if old not in s2:
    # find actual import line
    import re
    m = re.search(r"use launcher_plugin_host::\{[^}]+\};", s2)
    print("import line:", m.group(0) if m else "NONE")
    old = m.group(0)
new = old + """
use super::plugin_diagnostics::{global_record, DiagnosticClass, DiagnosticEntry};"""
s2 = s2.replace(old, new, 1)

# helper fn + record calls inside query()
old = """    fn query(&mut self, q: &QueryContext) -> Vec<Command> {"""
new = """    /// P2.4-E01: structured diagnostic for one interaction (observation
    /// only — never an execution path, spec P2.4-E).
    fn record_diag(&self, class: DiagnosticClass, elapsed_ms: u64, results: Option<usize>) {
        global_record(DiagnosticEntry {
            plugin_id: self.manifest.id.clone(),
            class,
            query_id: None,
            runtime_id: Some(self.runtime_id.clone()),
            protocol_session_id: None,
            elapsed_ms,
            frame_size: None,
            result_count: results,
        });
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {"""
assert old in s2, "query sig"
s2 = s2.replace(old, new, 1)
p2.write_text(s2, encoding="utf8", newline="\n")
print("provider probe ok")
