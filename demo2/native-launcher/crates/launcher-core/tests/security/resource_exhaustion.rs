//! SEC-RESOURCE-001..005 (review 45 §10): resource-exhaustion matrix.
//! Hostile scale must degrade into bounded time and bounded memory, never
//! unbounded consumption or unbounded retries.

use crate::common::*;
use launcher_domain::WorkflowRunStatus;
use launcher_mcp::types::{McpServerId, McpTool};
use std::sync::Arc;

/// SEC-RESOURCE-001 (R-001): a huge tool catalog projects and queries in
/// bounded time (10k tools; no per-item recursion).
#[test]
fn sec_resource001_huge_catalog_bounded() {
    let tools: Vec<launcher_domain::Command> = (0..10_000)
        .map(|i| {
            let t = McpTool {
                server_id: McpServerId("calc".into()),
                name: format!("tool_{i}"),
                title: Some(format!("Tool {i}")),
                description: None,
                input_schema: serde_json::json!({"type": "object"}),
                annotations: serde_json::json!({}),
            };
            launcher_mcp::adapter::tool_to_command_with_grants(
                &t,
                &McpServerId("calc".into()),
                &[launcher_domain::Capability::McpInvoke],
            )
        })
        .collect();
    let started = std::time::Instant::now();
    let items: Vec<_> = tools
        .iter()
        .flat_map(launcher_workflow::proposal::ActionCatalogItem::items_from_command)
        .collect();
    assert_eq!(items.len(), 10_000);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "catalog projection stays linear and bounded"
    );
}

/// SEC-RESOURCE-002 (R-002): a huge description is carried as bounded DATA
/// and never interpreted (1 MB description: projection completes, proposal
/// stays four small fields).
#[test]
fn sec_resource002_huge_description_bounded() {
    let huge = "A".repeat(1024 * 1024);
    let t = McpTool {
        server_id: McpServerId("calc".into()),
        name: "evaluate".into(),
        title: None,
        description: Some(huge.clone()),
        input_schema: serde_json::json!({}),
        annotations: serde_json::json!({}),
    };
    let started = std::time::Instant::now();
    let item = launcher_core::catalog::mcp_catalog_item(&t);
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(item.description.as_deref(), Some(huge.as_str()));
    // the proposal pipeline never embeds the description
    let proposals = huge_tool_proposals(&t);
    for p in proposals {
        assert!(serde_json::to_string(&p).unwrap().len() < 4_096, "proposal stays small");
    }
}

fn huge_tool_proposals(t: &McpTool) -> Vec<launcher_workflow::proposal::ActionProposal> {
    use launcher_workflow::proposal::{ActionPlanner, KeywordPlanner};
    let cmd = launcher_mcp::adapter::tool_to_command_with_grants(
        t,
        &McpServerId("calc".into()),
        &[launcher_domain::Capability::McpInvoke],
    );
    KeywordPlanner.plan("evaluate", std::slice::from_ref(&cmd))
}

/// SEC-RESOURCE-003/004 (R-003/R-004): a huge tool result flows through
/// Core → Workflow as DATA once — it is returned, not amplified; the next
/// execution starts clean (per-execution sessions, no accumulation).
#[test]
fn sec_resource003_huge_result_flows_once() {
    struct BigExecutor;
    impl launcher_mcp::executor::McpExecutor for BigExecutor {
        fn execute(
            &self,
            _input: &launcher_mcp::executor::McpInvokeInput,
            _execution_id: &str,
        ) -> Result<launcher_mcp::executor::McpToolResult, launcher_mcp::McpError> {
            Ok(launcher_mcp::executor::McpToolResult {
                content: vec![launcher_mcp::executor::McpContent {
                    kind: "text".into(),
                    text: Some("B".repeat(1024 * 1024)),
                }],
                structured_content: Some(serde_json::json!({"blob": "C".repeat(1024 * 1024)})),
                is_error: false,
            })
        }
    }
    let mut core = Core::new();
    core.register_mcp_server("calc", "mcp-calculator", vec![]);
    core.set_mcp_executor(Arc::new(BigExecutor));
    core.register(Box::new(CatalogProvider {
        commands: vec![tool_command("calc", "evaluate", false)],
        fresh_queries: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    }));
    for _ in 0..3 {
        let started = std::time::Instant::now();
        let result = core
            .execute_effect(
                "mcp:calc",
                "invoke",
                &calc_input(serde_json::json!({})),
                "e-big",
                1,
            )
            .unwrap();
        assert!(serde_json::to_string(&result).unwrap().len() > 1_000_000);
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "no amplification");
    }
}

/// SEC-RESOURCE-005 (R-005): retry against a permanently unavailable MCP
/// server is strictly bounded by the frozen retry policy — no respawn
/// storm.
#[test]
fn sec_resource005_bounded_retry_no_respawn_storm() {
    struct DownExecutor;
    impl launcher_mcp::executor::McpExecutor for DownExecutor {
        fn execute(
            &self,
            _input: &launcher_mcp::executor::McpInvokeInput,
            _execution_id: &str,
        ) -> Result<launcher_mcp::executor::McpToolResult, launcher_mcp::McpError> {
            Err(launcher_mcp::McpError::ServerUnavailable("spawn failed".into()))
        }
    }
    let mut core = Core::new();
    core.register_mcp_server("calc", "mcp-calculator", vec![]);
    core.set_mcp_executor(Arc::new(DownExecutor));
    core.register(Box::new(CatalogProvider {
        commands: vec![tool_command("calc", "evaluate", false)],
        fresh_queries: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    }));
    let run = run_def(
        &mut core,
        vec![reference_step("mcp:calc", "evaluate", calc_input(serde_json::json!({})))],
    );
    assert_eq!(run.status, WorkflowRunStatus::Failed);
    // frozen default: 1 initial + 1 retry, then stop
    assert_eq!(run.steps[0].attempt, 2, "bounded retry is a security property");
}
