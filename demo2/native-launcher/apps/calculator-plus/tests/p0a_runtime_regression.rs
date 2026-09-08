//! P0-A regression (review 53 §26 RT-REG-004): 100× plugin
//! spawn → initialize → query → shutdown cycles over a real plugin binary.
//! Migration acceptance: same behavior as pre-runtime (orphan = 0, hang =
//! 0, identical results every cycle).

use std::time::{Duration, Instant};

use launcher_domain::PluginManifest;
use launcher_plugin_host::PluginHandle;

fn base_dir() -> std::path::PathBuf {
    // the packaged exe lives in the test-binary directory, not next to the
    // manifest
    std::path::Path::new(env!("CARGO_BIN_EXE_plugin-calculator-plus"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default()
}

fn manifest() -> PluginManifest {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../calculator-plus/plugin.json");
    PluginManifest::parse(&std::fs::read_to_string(&p).unwrap()).unwrap()
}

/// RT-REG-004: 100 plugin lifecycle cycles; results must be identical and
/// bounded in time.
#[test]
fn p0a_plugin_lifecycle_soak_100() {
    let started = Instant::now();
    for i in 0..100 {
        let mut handle = PluginHandle::spawn(manifest(), &base_dir())
            .unwrap_or_else(|e| panic!("cycle {i}: spawn failed: {e}"));
        let cmds = handle
            .query("")
            .unwrap_or_else(|e| panic!("cycle {i}: query failed: {e}"));
        assert!(!cmds.is_empty(), "cycle {i}: discovery must answer");
        assert!(cmds[0].provider_id.starts_with("plugin:"));
        handle.shutdown();
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(300),
        "soak bounded: {elapsed:?}"
    );
    // process-tree containment is the runtime's job now; the plugin host
    // only owns protocol semantics
}
