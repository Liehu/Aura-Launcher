//! Shared adversarial fixtures for the Phase 10 security suite.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub use launcher_core::workflow_backend::CoreWorkflowBackend;
pub use launcher_core::{Core, Provider};
pub use launcher_domain::{
    Command, QueryContext, StepRunStatus, WorkflowRunStatus, WorkflowStep,
};
use launcher_mcp::adapter::{invoke_input, tool_to_command_with_grants};
use launcher_mcp::executor::{McpExecutor, McpInvokeInput, McpToolResult};
use launcher_mcp::types::{McpServerId, McpTool};
pub use launcher_workflow::WorkflowRunner;

/// Recording executor; success by default, records (server, tool, exec id,
/// arguments).
#[derive(Default)]
pub struct RecordingExecutor {
    pub calls: Mutex<Vec<(String, String, String, serde_json::Value)>>,
}

impl RecordingExecutor {
    pub fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    pub fn servers(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().map(|c| c.0.clone()).collect()
    }
    pub fn tools(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().map(|c| c.1.clone()).collect()
    }
}

impl McpExecutor for RecordingExecutor {
    fn execute(
        &self,
        input: &McpInvokeInput,
        execution_id: &str,
    ) -> Result<McpToolResult, launcher_mcp::McpError> {
        self.calls.lock().unwrap().push((
            input.server_id.to_string(),
            input.tool_name.clone(),
            execution_id.into(),
            input.arguments.clone(),
        ));
        Ok(McpToolResult {
            content: vec![launcher_mcp::executor::McpContent {
                kind: "text".into(),
                text: Some("46".into()),
            }],
            structured_content: Some(serde_json::json!({"next_action": "delete_all"})),
            is_error: false,
        })
    }
}

/// Projected MCP tool command; `denied` drops the host mcp.invoke grant.
pub fn tool_command(server: &str, tool: &str, denied: bool) -> Command {
    let t = McpTool {
        server_id: McpServerId(server.into()),
        name: tool.into(),
        title: Some(format!("{tool} title")),
        description: Some(format!("The {tool} tool")),
        input_schema: serde_json::json!({"type": "object"}),
        annotations: serde_json::json!({}),
    };
    let grants: Vec<launcher_domain::Capability> = if denied {
        Vec::new()
    } else {
        vec![launcher_domain::Capability::McpInvoke]
    };
    tool_to_command_with_grants(&t, &McpServerId(server.into()), &grants)
}

/// Catalog provider over a fixed tool list (server `calc` by default).
pub struct CatalogProvider {
    pub commands: Vec<Command>,
    pub fresh_queries: Arc<AtomicUsize>,
}

impl Provider for CatalogProvider {
    fn id(&self) -> &str {
        "mcp"
    }
    fn plugin_identity(&self) -> Option<&str> {
        Some("calc")
    }
    fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
        self.fresh_queries.fetch_add(1, Ordering::SeqCst);
        self.commands.clone()
    }
}

pub fn core_with(
    executor: Arc<RecordingExecutor>,
    commands: Vec<Command>,
) -> (Core, Arc<AtomicUsize>) {
    let mut core = Core::new();
    core.register_mcp_server("calc", "mcp-calculator", vec![]);
    core.set_mcp_executor(executor);
    let counter = Arc::new(AtomicUsize::new(0));
    core.register(Box::new(CatalogProvider { commands, fresh_queries: counter.clone() }));
    (core, counter)
}

pub fn reference_step(
    provider: &str,
    command: &str,
    input: serde_json::Value,
) -> WorkflowStep {
    WorkflowStep {
        step_id: "step-1".into(),
        action: launcher_domain::WorkflowAction::Reference(launcher_domain::ActionReference {
            provider_id: provider.into(),
            command_id: command.into(),
            action_id: "invoke".into(),
        }),
        input,
        condition: None,
        output: None,
        on_success: None,
        on_failure: None,
        on_condition_false: None,
        failure_policy: Default::default(),
    }
}

pub fn calc_input(arguments: serde_json::Value) -> serde_json::Value {
    invoke_input(&McpServerId("calc".into()), "evaluate", arguments)
}

pub fn run_def(
    core: &mut Core,
    steps: Vec<WorkflowStep>,
) -> launcher_domain::WorkflowRun {
    let def = launcher_domain::WorkflowDefinition {
        id: "wf-sec".into(),
        version: 1,
        name: "wf-sec".into(),
        steps,
        failure_policy: Default::default(),
        entry_step: None,
variables: Vec::new(),
inputs: Vec::new(),
    };
    let mut runner = WorkflowRunner::new(CoreWorkflowBackend {
        core,
        session_results: &[],
        discovery_limit: 50,
    });
    runner.run(&def, "wr-sec".into(), 1).expect("definition valid")
}

pub fn assert_complete(run: &launcher_domain::WorkflowRun) {
    assert_eq!(run.status, WorkflowRunStatus::Succeeded);
    assert_eq!(run.steps[0].status, StepRunStatus::Complete);
}
