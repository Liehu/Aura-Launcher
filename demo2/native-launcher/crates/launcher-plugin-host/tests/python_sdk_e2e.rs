//! Python SDK cross-language validation (ADR-0009): the real PluginHost
//! spawns `plugins/python/example-calculator` through the configured
//! interpreter and runs the contract chain (handshake -> query_id echo ->
//! shutdown).
//!
//! Interpreter resolution under test: `LAUNCHER_PYTHON` env > `python` on
//! PATH; skipped when no interpreter is available. A user config
//! `python_path` takes precedence in the app layer (launcher-app).

use std::path::PathBuf;

use launcher_domain::PluginManifest;
use launcher_plugin_host::PluginHandle;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/python")
        .canonicalize()
        .expect("plugins/python exists")
}

fn find_interpreter() -> Option<String> {
    let candidates = [
        std::env::var("LAUNCHER_PYTHON")
            .ok()
            .filter(|v| !v.is_empty()),
        Some("python".to_string()),
        Some("py".to_string()),
    ];
    for c in candidates.into_iter().flatten() {
        // expand env vars the same way the host does
        let expanded = launcher_plugin_host::expand_env(&c);
        let ok = std::process::Command::new(&expanded)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(expanded);
        }
    }
    None
}

#[test]
fn python_sdk_full_contract_chain() {
    let Some(interpreter) = find_interpreter() else {
        eprintln!(
            "skipping python_sdk_full_contract_chain: no interpreter \
             (set LAUNCHER_PYTHON to run this test)"
        );
        return;
    };

    let base = workspace_root().join("example-calculator");
    let manifest_json = std::fs::read_to_string(base.join("plugin.json")).unwrap();
    let mut manifest = PluginManifest::parse(&manifest_json).unwrap();
    assert!(manifest.is_python());
    manifest.interpreter = Some(PathBuf::from(&interpreter));

    let mut h = PluginHandle::spawn(manifest, &base).unwrap();

    // handshake happened inside spawn; query goes through query_id echo
    let cmds = h.query("12+34*2").unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].title, "= 80");
    assert_eq!(cmds[0].category, launcher_domain::Category::Plugin);

    // non-math query returns the help item (SDK Command serialization works)
    let cmds = h.query("hello").unwrap();
    assert!(cmds[0].title.contains("Calculator"));

    // graceful shutdown: the SDK answers `shutdown` and exits cleanly
    h.shutdown();
    assert_eq!(h.wait_exit(), Some(0));
}
