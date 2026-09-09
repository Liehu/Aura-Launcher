//! DISCOVERY-TODO-001 closure (external-plugin half, WF-006 option 2):
//! with an EMPTY popup session, an ActionReference to calculator-plus
//! resolves via fresh empty-text discovery against the REAL plugin binary
//! and executes through the engine → PluginBroker `execute_action` RPC.
//!
//! Mirrors MCP-WF-002 (the MCP half of the same TODO, closed in MVP4.3).

use std::path::PathBuf;

use launcher_core::providers::plugin::PluginProvider;
use launcher_core::workflow_backend::CoreWorkflowBackend;
use launcher_core::Core;
use launcher_domain::{
    ActionReference, StepRunStatus, WorkflowAction, WorkflowDefinition, WorkflowRunStatus,
    WorkflowStep,
};
use launcher_workflow::WorkflowRunner;

fn provider_from_manifest(tag: &str, capabilities: &[&str]) -> PluginProvider {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_plugin-calculator-plus"));
    let base = exe.parent().map(PathBuf::from).unwrap_or_default();
    let manifest = serde_json::json!({
        "schema_version": 1,
        "id": "com.example.calculator.plus",
        "name": "Calculator Plus",
        "api_version": "0.1",
        "executable": "plugin-calculator-plus.exe",
        "capabilities": capabilities,
        "timeout_ms": 3000
    });
    let dir = std::env::temp_dir().join(format!("calc-plus-disc-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugin.json"), serde_json::to_string(&manifest).unwrap()).unwrap();
    // the packaged binary lives next to the test binaries, so point the
    // manifest at it by copying it into the temp plugin dir (INV-013: the
    // executable must live inside the plugin directory)
    // a previous test's plugin process may still hold its copy; use a
    // per-test dir and retry briefly
    let dst = dir.join("plugin-calculator-plus.exe");
    let mut copied = false;
    for _ in 0..20 {
        match std::fs::copy(&exe, &dst) {
            Ok(_) => {
                copied = true;
                break;
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
    assert!(copied, "could not copy plugin exe");
    PluginProvider::from_manifest_file(&dir.join("plugin.json")).unwrap()
}

fn definition(command_id: &str) -> WorkflowDefinition {
    WorkflowDefinition {
        id: "wf-plugin-discovery".into(),
        version: 1,
        name: "wf-plugin-discovery".into(),
        steps: vec![WorkflowStep {
            step_id: "evaluate".into(),
            action: WorkflowAction::Reference(ActionReference {
                provider_id: "plugin:com.example.calculator.plus".into(),
                command_id: command_id.into(),
                action_id: "invoke".into(),
            }),
            input: serde_json::json!({ "expression": "2*3" }),
            condition: None,
            output: None,
            on_success: None,
            on_failure: None,
            on_condition_false: None,
            failure_policy: Default::default(),
        }],
        failure_policy: Default::default(),
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    }
}

#[test]
fn probe_provider_discovery() {
    let provider = provider_from_manifest("probe", &["clipboard.write", "plugin.invoke"]);
    let mut core = Core::new();
    core.register(Box::new(provider));
    let mut found = Vec::new();
    for p in core.providers_mut() {
        if p.id() == "plugin" {
            let cmds = p.query(&launcher_domain::QueryContext::parse(""));
            eprintln!("discovery returned {} commands", cmds.len());
            for c in &cmds {
                eprintln!("  {} :: {} (score {})", c.provider_id, c.id, c.score);
                for a in &c.actions {
                    eprintln!("    action id={:?} kind={:?} confirm={}", a.id, a.kind, a.confirmation_required);
                }
            }
            found = cmds;
        }
    }
    assert_eq!(found.len(), 1);
}

/// The closed loop: empty session → fresh empty-text discovery → the plugin
/// catalog supplies the command → engine → PluginBroker `execute_action` RPC.
#[test]
fn wf001_external_plugin_resolves_via_fresh_discovery() {
    let provider = provider_from_manifest("wf001", &["clipboard.write", "plugin.invoke"]);
    let mut core = Core::new();
    core.register(Box::new(provider));

    let def = definition("com.example.calculator.plus:evaluate");
    let mut runner = WorkflowRunner::new(CoreWorkflowBackend {
        core: &mut core,
        session_results: &[],
        discovery_limit: 50,
    });
    let run = runner
        .run(&def, "wr-discovery".into(), 1)
        .expect("definition valid");
    assert_eq!(
        run.status,
        WorkflowRunStatus::Succeeded,
        "steps: {:?}",
        run.steps
    );
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
    // the plugin evaluated the expression and returned its payload; the
    // broker wraps it with the host-assigned provenance
    let output = run.steps[0].output.clone().expect("step output captured");
    assert_eq!(output["provider"], "plugin:com.example.calculator.plus");
    assert_eq!(output["result"]["value"], serde_json::json!("6"));
}

/// A reference to a command the catalog does not carry must surface the
/// frozen CommandNotFound class — never a silent pass.
#[test]
fn wf002_unknown_command_fails_as_command_not_found() {
    let provider = provider_from_manifest("wf002", &[]);
    let mut core = Core::new();
    core.register(Box::new(provider));

    let def = definition("com.example.calculator.plus:does-not-exist");
    let mut runner = WorkflowRunner::new(CoreWorkflowBackend {
        core: &mut core,
        session_results: &[],
        discovery_limit: 50,
    });
    let run = runner
        .run(&def, "wr-discovery-miss".into(), 1)
        .expect("definition valid");
    assert_ne!(run.status, WorkflowRunStatus::Succeeded);
    let err = run.steps[0].last_error.clone().unwrap_or_default();
    assert!(
        err.contains("does-not-exist"),
        "expected CommandNotFound reason, got {err}"
    );
}
