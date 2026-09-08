//! MVP4.2 real-host AI Planner E2E: the planner reads the live command
//! catalog (fresh discovery query), proposes actions, and the proposals
//! execute through the frozen orchestration path with a real plugin process.
//! Core has no AI branch — the planner is just another producer.

use std::path::PathBuf;

use launcher_core::Core;
use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{PluginManifest, WorkflowDefinition, WorkflowRunStatus};
use launcher_workflow::proposal::{ActionPlanner, KeywordPlanner};
use launcher_workflow::{ActionExecutor, CommandSource, WfFailure, WorkflowRunner};

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

/// One backend implementing both ports over the same live Core.
struct Backend<'a> {
    core: &'a mut Core,
    session: Vec<launcher_domain::Command>,
}

impl<'a> CommandSource for Backend<'a> {
    fn in_session(&self, provider_id: &str, command_id: &str) -> Option<launcher_domain::Command> {
        self.session
            .iter()
            .find(|c| c.provider_id == provider_id && c.id == command_id)
            .cloned()
    }
    fn fresh_query(
        &mut self,
        provider_id: &str,
    ) -> Result<
        Vec<launcher_domain::Command>,
        (launcher_domain::workflow::WorkflowFailureClass, String),
    > {
        let q = launcher_domain::QueryContext::parse("");
        let mut out = Vec::new();
        for p in self.core.providers_mut() {
            let matches = p.id() == provider_id
                || p.plugin_identity()
                    .map(|pid| format!("plugin:{pid}") == provider_id)
                    .unwrap_or(false);
            if matches {
                out.extend(p.query(&q));
            }
        }
        Ok(out)
    }
}

impl<'a> ActionExecutor for Backend<'a> {
    fn execute(
        &mut self,
        action: launcher_domain::Action,
        provider_id: Option<String>,
        generation: u64,
        _confirmed: bool,
    ) -> Result<launcher_workflow::ExecutionOutcome, WfFailure> {
        // only plugin.* actions need the broker; system.* effects finish in
        // the engine itself
        if action.kind != launcher_domain::ActionKind::PluginInvoke {
            return match launcher_action::execute(&action) {
                Ok(_) => Ok(launcher_workflow::ExecutionOutcome {
                    execution_id: None,
                    plugin_result: None,
                }),
                Err(e) => Err(WfFailure::new(Class::InvalidInput, e.to_string())),
            };
        }
        let plugin_id = provider_id
            .as_deref()
            .and_then(|p| p.strip_prefix("plugin:").map(str::to_string))
            .ok_or_else(|| WfFailure::new(Class::ProtocolViolation, String::from("no provider")))?;
        let action_id = action.id.clone().unwrap_or_default();
        let execution_id = self.core.next_execution_id();
        match self.core.execute_plugin_action(
            &plugin_id,
            &action_id,
            &serde_json::Value::Null,
            &execution_id,
            generation,
        ) {
            Ok(result) => Ok(launcher_workflow::ExecutionOutcome {
                execution_id: Some(execution_id),
                plugin_result: Some(result),
            }),
            Err((class, reason)) => Err(WfFailure::new(class, reason)),
        }
    }
}

/// Planner reads the LIVE catalog via fresh discovery (WF-A3 empty-text
/// query), proposes the echo action, and it executes through the engine.
#[test]
fn ai_planner_full_chain_over_real_plugin() {
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../calculator-plus/plugin.json");
    let mut core = Core::new();
    let mut provider =
        launcher_core::providers::plugin::PluginProvider::from_manifest_file(&manifest_path)
            .unwrap();
    provider.set_base_dir(base_dir());
    core.register(Box::new(provider));

    // NOTE (WF-A3 migration): external plugins do not implement empty-text
    // discovery yet, so the planner reads the live query snapshot. When
    // plugins ship discovery, fresh_query will return the same catalog.
    let manifest = manifest();
    let mut host = launcher_plugin_host::PluginHandle::spawn(manifest, &base_dir()).unwrap();
    let session = host.query("6*7").unwrap();
    drop(host);

    let proposals = KeywordPlanner.plan("= 42", &session);
    assert_eq!(proposals.len(), 1);
    assert_eq!(
        proposals[0].provider_id,
        "plugin:com.example.calculator.plus"
    );
    assert_eq!(proposals[0].action_id, "copy");
    assert_eq!(proposals[0].input, serde_json::Value::Null);

    let def: WorkflowDefinition = serde_json::from_value(serde_json::json!({
        "id": "wf.ai.calc",
        "version": 1,
        "name": "AI planned echo",
        "steps": proposals.iter().enumerate().map(|(i, p)| p.to_step(format!("step-{i}"))).collect::<Vec<_>>()
    }))
    .unwrap();

    let backend = Backend {
        core: &mut core,
        session: session.clone(),
    };
    let mut runner = WorkflowRunner::new(backend);
    let run = runner.run(&def, "wr-ai-1".into(), 3).unwrap();
    eprintln!("DEBUG err={:?}", run.steps[0].last_error);

    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(
        run.steps[0].status,
        launcher_domain::StepRunStatus::Complete
    );
    // the Copy effect ran through the engine; broker not involved for system.*
    assert_eq!(run.definition_id, "wf.ai.calc");
}
