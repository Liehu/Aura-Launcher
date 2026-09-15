//! Plugin Lifetime v0.1 — PluginProvider integration (ADR-0019, ACC-LIFE
//! provider-level): ephemeral plugins respawn per invocation and are absent
//! afterwards; resident plugins reuse one process; resident headless
//! plugins are idled out at the use boundary. The provider code contains NO
//! lifetime branching — it only consumes PluginHost process primitives.

use std::path::PathBuf;
use std::time::Duration;

use launcher_core::{PluginProvider, Provider};
use launcher_domain::QueryContext;

const ECHO_PY: &str = r#"
import json, sys
for line in sys.stdin:
    req = json.loads(line)
    m = req.get("method")
    if m == "initialize":
        res = {"protocol_version": req["params"]["protocol_version"]}
    elif m == "query":
        res = [{"title": "echo:" + req["params"]["text"], "score": 0.5}]
    elif m == "shutdown":
        break
    else:
        continue
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": res}) + "\n")
    sys.stdout.flush()
"#;

fn python() -> Option<String> {
    if let Ok(p) = std::env::var("LAUNCHER_PYTHON") {
        if !p.trim().is_empty() {
            return Some(p);
        }
    }
    let ok = std::process::Command::new("python")
        .arg("-c")
        .arg("print(1)")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then(|| "python".to_string())
}

/// Build a provider around `lifetime` (None = legacy absent field).
fn provider(tag: &str, lifetime: Option<&str>, idle_ms: u64) -> Option<PluginProvider> {
    let py = python()?;
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("nl_life_prov_{}_{}_{}", tag, std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugin.py"), ECHO_PY).unwrap();
    let mut runtime = serde_json::json!({ "type": "python", "executable": "plugin.py" });
    if let Some(lt) = lifetime {
        runtime["lifetime"] = serde_json::json!(lt);
    }
    let manifest = serde_json::json!({
        "id": format!("life.{tag}"),
        "name": "Life",
        "runtime": runtime,
        "timeout_ms": 3000,
        "idle_timeout_ms": idle_ms,
    });
    std::fs::write(
        dir.join("plugin.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let mut p = PluginProvider::from_manifest_file(&dir.join("plugin.json")).ok()?;
    p.set_base_dir(dir.clone());
    p.set_interpreter_with_source(PathBuf::from(&py), "test");
    Some(p)
}

/// ACC-LIFE-002 + 003 (provider level): after an ephemeral query no plugin
/// process is reported; the next query spawns a fresh one and still works.
#[test]
fn provider_ephemeral_absent_between_invocations() {
    let Some(mut p) = provider("eph", Some("ephemeral"), 10_000) else {
        return;
    };
    let q = QueryContext::parse("one");
    let cmds = p.query(&q);
    assert!(cmds.iter().any(|c| c.title == "echo:one"));
    assert!(
        p.running_plugin().is_none(),
        "ephemeral plugin must not be reported as running after the invocation"
    );
    let q2 = QueryContext::parse("two");
    let cmds2 = p.query(&q2);
    assert!(
        cmds2.iter().any(|c| c.title == "echo:two"),
        "fresh spawn serves the next query"
    );
    assert!(p.running_plugin().is_none());
}

/// ACC-LIFE-005 (provider level): resident reuses the same pid across
/// queries before the idle timeout.
#[test]
fn provider_resident_reuses_pid() {
    let Some(mut p) = provider("res", Some("resident"), 10_000) else {
        return;
    };
    let mut last = None;
    for i in 0..3 {
        let cmds = p.query(&QueryContext::parse(&format!("n{i}")));
        assert!(cmds.iter().any(|c| c.title == format!("echo:n{i}")));
        let (_, pid, _) = p.running_plugin().expect("resident reported running");
        if let Some(prev) = last {
            assert_eq!(pid, prev, "resident must reuse the process");
        }
        last = Some(pid);
    }
}

/// ACC-LIFE-006 (provider level, lazy reclaim): after the idle timeout the
/// next use boundary reclaims the old process and spawns a new one.
#[test]
fn provider_resident_idle_reclaim_at_boundary() {
    let Some(mut p) = provider("residle", Some("resident"), 300) else {
        return;
    };
    p.query(&QueryContext::parse("warm"));
    let (_, pid1, _) = p.running_plugin().expect("running after warm query");
    std::thread::sleep(Duration::from_millis(600));
    p.query(&QueryContext::parse("after-idle"));
    let (_, pid2, _) = p.running_plugin().expect("running after boundary query");
    assert_ne!(pid1, pid2, "idle resident must be reclaimed and respawned");
}
