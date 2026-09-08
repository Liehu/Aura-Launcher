//! Domain Contract Test Kit (review 17 §16): CAT-001..CAT-011.
//!
//! Runs the Command/Action Contract v0.1 (ADR-0011) semantics against the
//! calculator-plus reference plugin (Domain Conformance Reference) through a
//! real PluginHost, plus resolver-level tests for paths the demo plugin does
//! not emit. SDKs must run this kit in addition to the Plugin Contract Test
//! Kit.

use std::path::PathBuf;

use launcher_domain::{
    resolve_descriptor, ActionDescriptor, ActionKind, Capability, PluginManifest,
};

fn base_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_plugin-calculator-plus"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn spawn_with(capabilities: &[&str]) -> launcher_plugin_host::PluginHandle {
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
    launcher_plugin_host::PluginHandle::spawn(m, &base_dir()).unwrap()
}

/// CAT-001 command identity: ids are stable across queries within a session
/// and never encode query-instance identity (INV-028).
#[test]
fn cat001_command_identity_stable() {
    let mut h = spawn_with(&["clipboard.write"]);
    let a = h.query("5+5").unwrap();
    let b = h.query("5+5").unwrap();
    assert_eq!(a[0].id, b[0].id);
    assert_eq!(a[0].id, "calc.plus:= 10");
    assert!(!a[0].id.contains("q-"), "id must not contain query ids");
}

/// CAT-002 provider authority: provider_id is host-generated from the
/// manifest, plugin output can never supply it (INV-029).
#[test]
fn cat002_provider_authority() {
    let mut h = spawn_with(&[]);
    let cmds = h.query("1+1").unwrap();
    assert_eq!(cmds[0].provider_id, "plugin:calc.plus");
}

/// CAT-003 action ordering: resolver preserves plugin declaration order.
#[test]
fn cat003_action_order_preserved() {
    let mut h = spawn_with(&["clipboard.write"]);
    let cmds = h.query("6*7").unwrap();
    let kinds: Vec<_> = cmds[0].actions.iter().map(|a| a.kind).collect();
    assert_eq!(
        kinds,
        vec![ActionKind::Copy, ActionKind::Open, ActionKind::Paste]
    );
}

/// CAT-004 primary resolution: first Ready action is primary (Enter target).
#[test]
fn cat004_primary_is_first_ready() {
    let mut h = spawn_with(&["clipboard.write"]);
    let cmds = h.query("6*7").unwrap();
    assert_eq!(cmds[0].actions[0].kind, ActionKind::Copy);
}

/// CAT-005 capability monotonicity: requires ⊆ manifest; no auto-grant.
/// Combined scenario (review 17 §9): Copy denied + unknown hidden +
/// History Ready → History becomes primary.
#[test]
fn cat005_capability_monotonicity_and_combined_fallback() {
    // resolver level
    let d = ActionDescriptor {
        id: "copy".into(),
        title: None,
        action_type: "system.copy_to_clipboard".into(),
        input: serde_json::json!({ "text": "1" }),
        requires: vec![Capability::ClipboardWrite],
        shortcut: None,
        confirmation: None,
    };
    assert!(matches!(
        resolve_descriptor(&d, &[]),
        Err(launcher_domain::DescriptorError::CapabilityDenied(_))
    ));
    // host level combined scenario (review 17 section 9): Copy -> Disabled,
    // unknown -> Hidden, History -> Ready and becomes primary via fallback
    let mut h = spawn_with(&[]);
    let cmds = h.query("5+5").unwrap();
    assert_eq!(cmds[0].actions.len(), 3); // Copy(dis) History Paste
    assert!(cmds[0].actions[0].disabled_reason.is_some());
    assert_eq!(cmds[0].primary_action().unwrap().kind, ActionKind::Open);
    assert_eq!(cmds[0].title, "= 10");
}

/// CAT-006 context eligibility: requires_context judgment. DEFERRED —
/// context supply to plugins is ⏸ (contract reserves the field; INV-032
/// semantics frozen, implementation waits for MVP3.1+).
#[test]
fn cat006_context_eligibility_deferred() {
    eprintln!("CAT-006 deferred: requires_context implementation waits for context supply");
}

/// CAT-007 unknown secondary isolation: a bad action never kills its
/// command (INV-031 fault containment).
#[test]
fn cat007_unknown_secondary_isolated() {
    let mut h = spawn_with(&["clipboard.write"]);
    let cmds = h.query("2+2").unwrap(); // plugin emits 4 actions, 1 unknown
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].actions.len(), 3, "unknown secondary dropped only");
}

/// CAT-008 unknown primary behavior: unknown/unsupported types are never
/// executed (INV-030); at resolver level they are deterministically rejected.
#[test]
fn cat008_unknown_type_never_executes() {
    let d = ActionDescriptor {
        id: "magic".into(),
        title: None,
        action_type: "plugin.calc.magic".into(), // reserved namespace, v0.1
        input: serde_json::json!({}),
        requires: vec![],
        shortcut: None,
        confirmation: None,
    };
    assert!(matches!(
        resolve_descriptor(&d, &[Capability::ClipboardWrite]),
        Err(launcher_domain::DescriptorError::UnknownActionType(_))
    ));
}

/// CAT-009 invalid input: missing/type-mismatched input never executes.
#[test]
fn cat009_invalid_input() {
    let d = ActionDescriptor {
        id: "copy".into(),
        title: None,
        action_type: "system.copy_to_clipboard".into(),
        input: serde_json::json!({}),
        requires: vec![],
        shortcut: None,
        confirmation: None,
    };
    assert!(matches!(
        resolve_descriptor(&d, &[]),
        Err(launcher_domain::DescriptorError::InvalidInput(_))
    ));
}

/// CAT-010 empty actions semantics: informational command stays listed and
/// is simply non-executable.
#[test]
fn cat010_empty_actions_informational() {
    let mut h = spawn_with(&[]);
    let cmds = h.query("hello").unwrap();
    assert_eq!(cmds.len(), 1);
    assert!(cmds[0].actions.is_empty());
}

/// CAT-011 resolved-action boundary: what the host emits are plain domain
/// Actions — Ready ones are engine-valid; Disabled ones are refused by the
/// engine gate (INV-033 + MVP3.1 presentation semantics).
#[test]
fn cat011_resolved_action_boundary() {
    let mut h = spawn_with(&["clipboard.write"]);
    let cmds = h.query("7-3").unwrap();
    for a in &cmds[0].actions {
        match (a.disabled_reason.as_deref(), a.confirmation_required) {
            (None, false) => {
                launcher_action::validate(a).expect("ready action must be engine-valid");
            }
            (Some(_), _) => assert!(matches!(
                launcher_action::validate(a),
                Err(launcher_action::ActionError::Disabled(_))
            )),
            // confirmation policy gate (MVP3.2-B): presentable, gated in engine
            (None, true) => assert!(matches!(
                launcher_action::validate(a),
                Err(launcher_action::ActionError::ConfirmationRequired)
            )),
        }
    }
    assert_eq!(cmds[0].actions[0].kind, ActionKind::Copy);
}

/// CAT-012 (MVP3.2-A): shortcut dispatch resolves to the stable action_id of
/// the selected command; first declared match wins (collision rule).
#[test]
fn cat012_shortcut_resolves_to_stable_action_id() {
    let mut h = spawn_with(&["clipboard.write"]);
    let cmds = h.query("7-3").unwrap();
    let copy = &cmds[0].actions[0];
    assert_eq!(copy.shortcut.as_deref(), Some("Ctrl+Shift+C"));
    assert_eq!(copy.id.as_deref(), Some("copy"));
    // dispatch key -> action id lookup is deterministic: first declared match
    let hit: Option<String> = cmds[0]
        .actions
        .iter()
        .find(|a| a.shortcut.as_deref() == Some("Ctrl+Shift+C"))
        .and_then(|a| a.id.clone());
    assert_eq!(hit.as_deref(), Some("copy"));
}
