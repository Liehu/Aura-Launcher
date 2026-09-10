//! P3.1-B0 MGT-3/MGT-4: plugin enable/disable is authoritative PER QUERY
//! once a registry is attached — a management-page disable applies
//! immediately (no restart), re-enabling restores candidates, and the
//! state persists across registry reopen. Uses a compliant Python plugin
//! (skipped when LAUNCHER_PYTHON is unset, like plugin_crash_soak).

use launcher_core::{providers::plugin_registry::PluginRegistry, PluginProvider, Provider};
use launcher_domain::QueryContext;
use std::path::PathBuf;
use std::sync::Arc;

const TOGGLER_PY: &str = r#"
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    req = json.loads(line)
    method = req.get("method")
    rid = req.get("id")
    params = req.get("params") or {}
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": rid,
              "result": {"protocol_version": params.get("protocol_version")}})
    elif method == "query":
        qid = params.get("query_id")
        send({"jsonrpc": "2.0", "id": rid, "result": {"query_id": qid, "commands": [
            {"id": "toggle", "title": "toggle: " + params.get("text", ""), "type": "entry",
             "input": {}, "requires": [], "actions": []}
        ]}})
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": rid, "result": {"bye": True}})
        break
"#;

const MANIFEST_JSON: &str = r#"{
  "schema_version": 1,
  "id": "com.test.toggle",
  "name": "Toggler",
  "version": "1.0.0",
  "api_version": "0.1",
  "runtime": { "type": "python", "executable": "toggler.py" },
  "capabilities": [],
  "timeout_ms": 3000,
  "idle_timeout_ms": 10000
}"#;

fn interpreter() -> Option<PathBuf> {
    std::env::var("LAUNCHER_PYTHON")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn setup(tag: &str) -> (PathBuf, Arc<PluginRegistry>, PluginProvider) {
    let dir = std::env::temp_dir().join(format!("nl_p31_mgt_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("toggler.py"), TOGGLER_PY).unwrap();
    std::fs::write(dir.join("plugin.json"), MANIFEST_JSON).unwrap();
    let reg = Arc::new(PluginRegistry::open(&dir.join("registry.db")).unwrap());
    let mut p = PluginProvider::from_manifest_file(&dir.join("plugin.json")).unwrap();
    p.set_interpreter_with_source(interpreter().unwrap(), "test");
    p.set_base_dir(dir.clone());
    p.set_registry(reg.clone());
    (dir, reg, p)
}

/// MGT-3: disable via the registry (what the management page does) makes
/// the very next query empty; re-enable restores candidates. No restart.
#[test]
fn mgt3_disable_is_immediate_and_reversible() {
    let Some(_py) = interpreter() else {
        return; // LAUNCHER_PYTHON unset: skip like plugin_crash_soak
    };
    let (dir, reg, mut p) = setup("a");
    let q = QueryContext::parse("anything");
    assert!(
        !p.query(&q).is_empty(),
        "enabled plugin must produce candidates"
    );

    reg.set_enabled("com.test.toggle", false).unwrap();
    assert!(
        p.query(&q).is_empty(),
        "disabled plugin must produce no candidates immediately"
    );

    reg.set_enabled("com.test.toggle", true).unwrap();
    assert!(
        !p.query(&q).is_empty(),
        "re-enabled plugin must produce candidates immediately"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// MGT-4: the disabled state persists across registry reopen.
#[test]
fn mgt4_disable_persists_across_registry_reopen() {
    let Some(_py) = interpreter() else {
        return;
    };
    let (dir, reg, mut p) = setup("b");
    let q = QueryContext::parse("anything");
    assert!(!p.query(&q).is_empty());

    reg.set_enabled("com.test.toggle", false).unwrap();
    drop(reg);
    drop(p);

    let reg2 = Arc::new(PluginRegistry::open(&dir.join("registry.db")).unwrap());
    let mut p2 = PluginProvider::from_manifest_file(&dir.join("plugin.json")).unwrap();
    p2.set_interpreter_with_source(interpreter().unwrap(), "test");
    p2.set_base_dir(dir.clone());
    p2.set_registry(reg2.clone());
    assert!(p2.query(&q).is_empty(), "persisted disable must hold");
    reg2.set_enabled("com.test.toggle", true).unwrap();
    assert!(!p2.query(&q).is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
