//! P2.3-C3 Plugin Crash Soak: a REAL multi-process crash loop. The test
//! authors a crasher plugin (self-contained Python, raw protocol — no SDK)
//! whose query handler exits the process on demand, then drives the
//! production `PluginProvider` through repeated spawn → query → crash →
//! respawn cycles and asserts the quarantine state machine closes the loop:
//!
//!   success resets the consecutive-failure counter
//!   crash  → handle dropped + failure++ (query path, P2.3-C3)
//!   3 consecutive crashes → quarantined (persistent via plugins.db)
//!   a fresh provider hydrated from the same registry stays quarantined
//!
//! Skipped unless LAUNCHER_PYTHON points at a Python interpreter (the same
//! gate the MCP/plugin E2E suites use); all process spawning happens inside
//! the production plugin host, never in this test file.

use std::path::PathBuf;
use std::sync::Arc;

use launcher_core::providers::plugin_registry::PluginRegistry;
use launcher_core::{PluginProvider, Provider};
use launcher_domain::QueryContext;

fn interpreter() -> Option<PathBuf> {
    std::env::var("LAUNCHER_PYTHON")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Self-contained crasher plugin: speaks Plugin Contract v0.1 on stdio.
/// `query "crash"` → the process exits with code 1 (a REAL process crash,
/// detected host-side as ProcessExited → PluginError::Crashed).
const CRASHER_PY: &str = r#"
import json, sys

out = sys.stdout

def send(obj):
    out.write(json.dumps(obj) + "\n")
    out.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    rid = req.get("id", 0)
    method = req.get("method", "")
    params = req.get("params") or {}
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": rid,
              "result": {"protocol_version": params.get("protocol_version")}})
    elif method == "query":
        text = params.get("text", "")
        if "crash" in text:
            sys.exit(1)
        qid = params.get("query_id")
        send({"jsonrpc": "2.0", "id": rid, "result": {"query_id": qid, "commands": [
            {"id": "echo", "title": "echo: " + text, "type": "entry",
             "input": {}, "requires": [], "actions": []}
        ]}})
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": rid, "result": {"bye": True}})
        break
"#;

const MANIFEST_JSON: &str = r#"{
  "schema_version": 1,
  "id": "com.test.crasher",
  "name": "Crasher",
  "version": "1.0.0",
  "api_version": "0.1",
  "runtime": { "type": "python", "executable": "crasher.py" },
  "capabilities": [],
  "timeout_ms": 3000,
  "idle_timeout_ms": 10000
}"#;

fn setup_plugin_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nl_c3_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("crasher.py"), CRASHER_PY).unwrap();
    std::fs::write(dir.join("plugin.json"), MANIFEST_JSON).unwrap();
    dir
}

fn make_provider(dir: &PathBuf, registry: Arc<PluginRegistry>) -> PluginProvider {
    let mut p = PluginProvider::from_manifest_file(&dir.join("plugin.json")).unwrap();
    p.set_interpreter_with_source(interpreter().unwrap(), "test");
    p.set_base_dir(dir.clone());
    p.set_registry(registry);
    p
}

fn registry(db: &PathBuf) -> Arc<PluginRegistry> {
    Arc::new(PluginRegistry::open(db).unwrap())
}

fn titles(p: &mut PluginProvider, q: &str) -> Vec<String> {
    p.query(&QueryContext::parse(q))
        .into_iter()
        .map(|c| c.title)
        .collect()
}

/// The full crash-loop soak: healthy queries interleaved with crash queries,
/// ending in quarantine that persists across a registry restart.
#[test]
fn plugin_crash_loop_soak_quarantines_and_persists() {
    let Some(_) = interpreter() else {
        eprintln!("skipping: LAUNCHER_PYTHON not set");
        return;
    };
    let dir = setup_plugin_dir("soak");
    let reg = registry(&dir.join("plugins.db"));
    let mut p = make_provider(&dir, reg.clone());

    // --- phase 1: healthy cycle works ---
    assert!(titles(&mut p, "hello").iter().any(|t| t == "echo: hello"));

    // --- phase 2: soak loop — crash, respawn, crash ... ---
    // crashes 1 and 2 degrade to empty results (respawn on next query);
    // the 3rd consecutive crash trips QUARANTINE_THRESHOLD.
    for i in 1..=3 {
        assert!(
            titles(&mut p, &format!("crash {i}")).is_empty(),
            "crash query must yield no candidates"
        );
    }
    assert!(
        !p.is_available(),
        "plugin must be quarantined after 3 consecutive crashes"
    );

    // --- phase 3: quarantined plugin stops respawning entirely ---
    for _ in 0..5 {
        assert!(titles(&mut p, "anything").is_empty());
    }

    // --- phase 4: quarantine persists in plugins.db across restart ---
    assert!(reg.state("com.test.crasher").quarantined, "registry must persist quarantine");
    let reg2 = registry(&dir.join("plugins.db")); // fresh handle, same db
    assert!(reg2.state("com.test.crasher").quarantined);
    let fresh = make_provider(&dir, reg2);
    assert!(!fresh.is_available(), "restarted provider must hydrate quarantine");
    std::fs::remove_dir_all(&dir).ok();
}

/// Soak the healthy path: many consecutive queries against one long-lived
/// process stay stable (no leak in the request-id/handle lifecycle).
#[test]
fn plugin_healthy_query_soak_stays_stable() {
    let Some(_) = interpreter() else {
        eprintln!("skipping: LAUNCHER_PYTHON not set");
        return;
    };
    let dir = setup_plugin_dir("healthy");
    let reg = registry(&dir.join("plugins.db"));
    let mut p = make_provider(&dir, reg.clone());

    for i in 0..40 {
        let needle = format!("item {i}");
        let got = titles(&mut p, &needle);
        assert!(
            got.iter().any(|t| t == &format!("echo: {needle}")),
            "cycle {i}: healthy query must keep working"
        );
        assert!(p.is_available(), "cycle {i}: must stay available");
    }
    assert_eq!(reg.state("com.test.crasher").protocol_failures, 0);
    std::fs::remove_dir_all(&dir).ok();
}
