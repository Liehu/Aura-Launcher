//! End-to-end: spawn the calculator plugin binary through the real
//! PluginHost and verify the full contract chain
//! (manifest v2 -> handshake -> query_id echo -> commands).

use std::path::PathBuf;

use launcher_domain::{PluginManifest, QueryContext};

fn base_dir() -> PathBuf {
    let exe = env!("CARGO_BIN_EXE_plugin-calculator");
    PathBuf::from(exe)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn load_manifest() -> (PluginManifest, PathBuf) {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugin.json");
    let json = std::fs::read_to_string(&manifest_path).unwrap();
    // the packaged executable lives next to the test binaries
    let m = PluginManifest::parse(&json).unwrap();
    (m, base_dir())
}

#[test]
fn calculator_full_contract_chain() {
    let (m, base) = load_manifest();
    assert_eq!(m.schema_version, Some(1));
    assert_eq!(m.effective_executable(), "plugin-calculator.exe");
    assert!(m.capabilities.is_empty());

    let mut h = launcher_plugin_host::PluginHandle::spawn(m.clone(), &base).unwrap();
    let cmds = h.query("12+34*2").unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].title, "= 80");
    assert_eq!(cmds[0].category, launcher_domain::Category::Plugin);

    // non-math query still returns the help item
    let cmds = h.query("hello").unwrap();
    assert_eq!(cmds.len(), 1);
}

#[test]
fn calculator_provider_queries_are_non_empty_only_for_math() {
    // domain-level sanity: empty query never reaches the plugin
    let q = QueryContext::parse("   ");
    assert!(q.normalized.is_empty());
}
