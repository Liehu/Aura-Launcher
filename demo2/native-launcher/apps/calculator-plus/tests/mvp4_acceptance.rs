//! MVP4.0 acceptance (review 21): plugin-owned actions execute through
//! ActionEngine -> PluginBroker -> execute_action RPC. Identity binding,
//! capability gating, and execution_id echo all verified over a real host.

use std::path::PathBuf;

use launcher_domain::{ActionDescriptor, ActionKind, Capability, PluginManifest};

fn base_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_plugin-calculator-plus"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn spawn() -> launcher_plugin_host::PluginHandle {
    let json =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugin.json"))
            .unwrap();
    let m = PluginManifest::parse(&json).unwrap();
    launcher_plugin_host::PluginHandle::spawn(m, &base_dir()).unwrap()
}

/// The plugin's query result now carries a plugin-owned action that resolved
/// to PluginInvoke (own-plugin routing + plugin.invoke capability).
#[test]
fn plugin_owned_action_resolves_to_plugin_invoke() {
    let mut h = spawn();
    let cmds = h.query("6*7").unwrap();
    let echo = cmds[0]
        .actions
        .iter()
        .find(|a| a.kind == ActionKind::PluginInvoke)
        .expect("plugin-owned action must resolve to PluginInvoke");
    assert_eq!(echo.id.as_deref(), Some("echo"));
    assert!(echo.disabled_reason.is_none());
}

/// The full effect path: engine-validated action executed via the
/// `execute_action` RPC with execution_id echo and context generation.
#[test]
fn execute_action_rpc_roundtrip() {
    let mut h = spawn();
    let input = serde_json::json!({ "source": "6*7", "marker": "mvp4" });
    let result = h.execute_action("echo", &input, "e-1", 1).unwrap();
    assert_eq!(result["by"], "calculator-plus");
    assert_eq!(result["echoed"], input);
}

/// Unknown actions on a plugin that supports execute_action fail
/// deterministically (and the host survives).
#[test]
fn execute_action_unknown_action_fails_cleanly() {
    let mut h = spawn();
    let err = h
        .execute_action("nope", &serde_json::json!({}), "e-2", 1)
        .unwrap_err();
    assert!(err.to_string().contains("unknown action"));
    // host survives and can execute again
    let result = h.execute_action("echo", &serde_json::json!({}), "e-3", 1).unwrap();
    assert_eq!(result["by"], "calculator-plus");
}

/// Resolver-level guarantees (ADR-0014 §17/§22): capability monotonicity and
/// identity binding for plugin-owned descriptors.
#[test]
fn resolver_identity_and_capability_binding() {
    let d = |ty: &str| ActionDescriptor {
        id: "echo".into(),
        title: None,
        action_type: ty.into(),
        input: serde_json::json!({}),
        requires: vec![],
        shortcut: None,
        confirmation: None,
    };
    // cross-plugin denied even with plugin.invoke granted
    assert!(launcher_domain::resolve_descriptor_for(
        &d("plugin.other.t"),
        &[Capability::PluginInvoke],
        Some("calc.plus")
    )
    .is_err());
    // own plugin without the capability denied
    assert!(launcher_domain::resolve_descriptor_for(
        &d("plugin.calc.plus.t"),
        &[],
        Some("calc.plus")
    )
    .is_err());
    // own plugin with capability -> ready
    assert!(launcher_domain::resolve_descriptor_for(
        &d("plugin.calc.plus.t"),
        &[Capability::PluginInvoke],
        Some("calc.plus")
    )
    .is_ok());
}
