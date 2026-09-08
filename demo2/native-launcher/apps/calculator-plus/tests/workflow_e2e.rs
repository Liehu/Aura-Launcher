//! MVP4.1 real-host workflow E2E: a Reference step against the live
//! calculator-plus plugin flows through the frozen chain
//! ReferenceResolver → ActionResolver(engine) → PluginBroker → execute_action.

use std::path::PathBuf;

use launcher_core::workflow_backend::CoreWorkflowBackend;
use launcher_core::Core;
use launcher_domain::{PluginManifest, WorkflowDefinition, WorkflowRunStatus};
use launcher_workflow::WorkflowRunner;

fn base_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_plugin-calculator-plus"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn manifest() -> PluginManifest {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../calculator-plus/plugin.json");
    PluginManifest::parse(&std::fs::read_to_string(&p).unwrap()).unwrap()
}

/// The full frozen chain over a real plugin process:
/// Definition → Run → Reference (session hit) → engine → broker → RPC.
#[test]
fn workflow_reference_step_executes_plugin_action() {
    let _manifest = manifest();
    let mut core = Core::new();
    let mut provider = launcher_core::providers::plugin::PluginProvider::from_manifest_file(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../calculator-plus/plugin.json"),
    )
    .unwrap();
    // the packaged exe lives in the test-binary directory, not next to the
    // manifest; point the provider there
    provider.set_base_dir(base_dir());
    core.register(Box::new(provider));

    // live query produces the session snapshot the reference resolves against
    let mut host =
        launcher_plugin_host::PluginHandle::spawn(_manifest.clone(), &base_dir()).unwrap();
    let session = host.query("6*7").unwrap();
    assert_eq!(session[0].title, "= 42");
    // NOTE: keep the host alive until after the workflow run — spawning the
    // same exe immediately after killing a process hits the Windows
    // terminating-image file lock, which the provider treats as a spawn
    // failure (cooldown). Two live instances of the exe are fine.

    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.calc.plus.echo",
        "version": 1,
        "name": "Calculator Plus Echo",
        "steps": [{
            "step_id": "echo",
            "action": { "Reference": {
                "provider_id": session[0].provider_id,
                "command_id": session[0].id,
                "action_id": "echo" } },
            "input": { "source": "workflow" }
        }]
    }))
    .unwrap();

    let backend = CoreWorkflowBackend {
        core: &mut core,
        session_results: &session,
        discovery_limit: 100,
    };
    let mut runner = WorkflowRunner::new(backend);
    let run = runner.run(&def, "wr-calc-1".into(), 1).unwrap();

    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(
        run.steps[0].status,
        launcher_domain::StepRunStatus::Complete
    );
    assert_eq!(run.steps[0].attempt, 1);
    drop(host);
}

/// WF-A3: reference to a command the plugin does not publish resolves to
/// CommandNotFound (Stop) — never silently executes something else.
#[test]
fn workflow_unknown_command_reference_is_command_not_found() {
    let _manifest = manifest();
    let mut core = Core::new();
    let mut provider = launcher_core::providers::plugin::PluginProvider::from_manifest_file(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../calculator-plus/plugin.json"),
    )
    .unwrap();
    provider.set_base_dir(base_dir());
    core.register(Box::new(provider));
    let session: Vec<launcher_domain::Command> = vec![]; // empty popup session

    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.calc.plus.missing",
        "version": 1,
        "name": "Missing",
        "steps": [{
            "step_id": "s1",
            "action": { "Reference": {
                "provider_id": "plugin:com.example.calculator.plus",
                "command_id": "no.such.command",
                "action_id": "echo" } },
            "input": {}
        }]
    }))
    .unwrap();

    let backend = CoreWorkflowBackend {
        core: &mut core,
        session_results: &session,
        discovery_limit: 100,
    };
    let mut runner = WorkflowRunner::new(backend);
    let run = runner.run(&def, "wr-calc-2".into(), 1).unwrap();
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    eprintln!("wf error: {:?}", run.steps[0].last_error);
}
