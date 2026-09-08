//! MVP3.0 acceptance (review 15 §24 / review 16 §15/16/20): the full
//! Command → ActionDescriptor → Action Resolution → Action chain over a real
//! PluginHost, using the calculator-plus reference plugin.

use std::path::PathBuf;

use launcher_domain::{ActionKind, PluginManifest};

fn base_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_plugin-calculator-plus"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn manifest_with(capabilities: &[&str]) -> (PluginManifest, PathBuf) {
    let m: PluginManifest = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "id": "calc.plus",
        "name": "Calculator Plus",
        "api_version": "0.1",
        "executable": "plugin-calculator-plus.exe",
        "capabilities": capabilities,
        "timeout_ms": 2000
    }))
    .unwrap();
    (m, base_dir())
}

#[test]
fn acceptance_primary_secondary_and_fault_containment() {
    // manifest declares clipboard.write -> Copy/Insert Ready; the unknown
    // `plugin.*` secondary must be dropped without killing the command.
    let (m, base) = manifest_with(&["clipboard.write"]);
    let mut h = launcher_plugin_host::PluginHandle::spawn(m, &base).unwrap();
    let cmds = h.query("12+34*2").unwrap();
    assert_eq!(cmds.len(), 1);
    let cmd = &cmds[0];

    // fault containment: 4 descriptors in, 3 valid actions out (magic hidden)
    assert_eq!(cmd.actions.len(), 3);
    // primary = first Ready action (order preserved): Copy
    assert_eq!(cmd.actions[0].kind, ActionKind::Copy);
    assert_eq!(cmd.title, "= 80");
    // MVP3.2-A: shortcut travels descriptor -> resolved action
    assert_eq!(cmd.actions[0].shortcut.as_deref(), Some("Ctrl+Shift+C"));
    // secondaries follow in declaration order
    assert_eq!(cmd.actions[1].kind, ActionKind::Open); // Open History
                                                       // MVP3.2-B: paste injects a keystroke into another window -> the
                                                       // confirmation execution policy is attached by the descriptor
    assert_eq!(cmd.actions[2].kind, ActionKind::Paste);
    assert!(cmd.actions[2].confirmation_required);

    // provider identity is host-authoritative regardless of plugin output
    assert_eq!(cmd.provider_id, "plugin:calc.plus");

    // non-math query: informational command, zero actions, still listed
    let cmds = h.query("hello").unwrap();
    assert_eq!(cmds.len(), 1);
    assert!(cmds[0].actions.is_empty());
}

#[test]
fn acceptance_capability_denied_presents_disabled() {
    // same binary, but the manifest does NOT declare clipboard.write:
    // Copy is denied (INV-027 monotonicity, no host auto-grant) and becomes
    // Disabled: presentable but non-executable (MVP3.1), never lost.
    let (m, base) = manifest_with(&[]);
    let mut h = launcher_plugin_host::PluginHandle::spawn(m, &base).unwrap();
    let cmds = h.query("5+5").unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].actions.len(), 3);
    assert!(cmds[0].actions[0]
        .disabled_reason
        .as_deref()
        .unwrap()
        .contains("clipboard.write"));
    assert!(cmds[0].actions[1].disabled_reason.is_none());
    assert!(cmds[0].actions[2].disabled_reason.is_none());

    // primary = first READY action (order preserved): History
    let primary = cmds[0].primary_action().unwrap();
    assert_eq!(primary.kind, ActionKind::Open);
    assert!(primary.disabled_reason.is_none());
    assert_eq!(cmds[0].title, "= 10");
}
