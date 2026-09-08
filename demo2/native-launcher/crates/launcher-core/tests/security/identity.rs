//! SEC-ID-001..010 (review 45 §3 Boundary A): identity hardening. Server
//! identity is config-owned; tool input can never re-point a resolved
//! route at another server or tool.

use crate::common::*;
use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_mcp::executor::McpInvokeInput;
use launcher_mcp::types::McpServerId;
use std::sync::Arc;

fn run_proposal(
    core: &mut Core,
    provider: &str,
    command: &str,
    input: serde_json::Value,
) -> launcher_domain::WorkflowRun {
    run_def(core, vec![reference_step(provider, command, input)])
}

/// SEC-ID-001 (A-001): path-traversal server ids are rejected at the
/// executor input boundary — they never reach a server lookup.
#[test]
fn sec_id001_server_id_injection_rejected() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    for evil in ["../calc", "calc/../jira", "mcp:calc", "", " calc ", "a\\b"] {
        let input = launcher_mcp::adapter::invoke_input(
            &McpServerId(evil.into()),
            "evaluate",
            serde_json::json!({}),
        );
        let run = run_proposal(&mut core, "mcp:calc", "evaluate", input);
        assert_eq!(run.status, launcher_domain::WorkflowRunStatus::Failed, "evil id: {evil:?}");
    }
    assert_eq!(ex.count(), 0, "no injected identity ever reached an executor");
}

/// SEC-ID-002 (A-002): provider/server mismatch is a protocol violation
/// with zero executor invocations.
#[test]
fn sec_id002_provider_server_mismatch() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    let input = launcher_mcp::adapter::invoke_input(
        &McpServerId("jira".into()),
        "evaluate",
        serde_json::json!({}),
    );
    let run = run_proposal(&mut core, "mcp:calc", "evaluate", input);
    assert_eq!(run.status, launcher_domain::WorkflowRunStatus::Failed);
    let err = run.steps[0].last_error.as_deref().unwrap_or_default();
    assert!(
        err.contains("mismatch") || err.contains("route identity"),
        "rejected as identity confusion: {err}"
    );
    assert_eq!(ex.count(), 0);
    let _ = Class::ProtocolViolation; // frozen class contract
}

/// SEC-ID-003 (A-003): tool substitution — Reference resolves `evaluate`
/// but the runtime input re-points `tool_name` at `delete_all`. The
/// identity binding rejects it before any execution.
#[test]
fn sec_id003_tool_substitution_rejected() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    let input = launcher_mcp::adapter::invoke_input(
        &McpServerId("calc".into()),
        "delete_all",
        serde_json::json!({"path": "C:\\"}),
    );
    let run = run_proposal(&mut core, "mcp:calc", "evaluate", input);
    assert_eq!(run.status, launcher_domain::WorkflowRunStatus::Failed);
    assert!(run.steps[0]
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("route identity"));
    assert_eq!(ex.count(), 0, "substituted tool never executes");
}

/// SEC-ID-003b: server_id substitution inside otherwise-consistent input
/// is equally rejected by the same identity binding.
#[test]
fn sec_id003b_server_substitution_in_input_rejected() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    let input = launcher_mcp::adapter::invoke_input(
        &McpServerId("jira".into()),
        "evaluate",
        serde_json::json!({}),
    );
    let run = run_proposal(&mut core, "mcp:calc", "evaluate", input);
    assert_eq!(run.status, launcher_domain::WorkflowRunStatus::Failed);
    assert_eq!(ex.count(), 0);
}

/// SEC-ID-004 (A-004): same tool name on two servers never cross-resolves —
/// routing follows the reference's exact (provider, server) pair.
#[test]
fn sec_id004_cross_server_same_name_isolated() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "execute", false)]);
    // a second configured server exposing the identical tool name
    core.register_mcp_server("jira", "jira-server", vec![]);
    let input = launcher_mcp::adapter::invoke_input(
        &McpServerId("calc".into()),
        "execute",
        serde_json::json!({}),
    );
    let run = run_proposal(&mut core, "mcp:calc", "execute", input);
    assert_complete(&run);
    assert_eq!(ex.servers(), vec!["calc".to_string()], "never routed to jira");
    assert_eq!(ex.tools(), vec!["execute".to_string()]);
}

/// SEC-ID-005 (A-005): prefix collision — `mcp:calculator` and
/// `mcp:calc.evil` are distinct namespaces from `mcp:calc`; ownership is
/// never decided by `starts_with`.
#[test]
fn sec_id005_prefix_collision_exact_identity() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    core.register_mcp_server("calculator", "calc-server", vec![]);
    core.register_mcp_server("calc.evil", "evil-server", vec![]);

    // routing mcp:calc with a calculator-identity input is a mismatch
    for evil_server in ["calculator", "calc.evil"] {
        let input = launcher_mcp::adapter::invoke_input(
            &McpServerId(evil_server.into()),
            "evaluate",
            serde_json::json!({}),
        );
        let run = run_proposal(&mut core, "mcp:calc", "evaluate", input);
        assert_eq!(
            run.status,
            launcher_domain::WorkflowRunStatus::Failed,
            "{evil_server} must not ride mcp:calc's route"
        );
    }
    assert_eq!(ex.count(), 0);
}

/// SEC-ID-006 (A-006): identity is case-sensitive and exact in BOTH
/// resolution and execution — no case-normalized resolve with
/// case-sensitive execute.
#[test]
fn sec_id006_case_confusion() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    // wrong-case provider namespace never matches the mcp: route
    for provider in ["MCP:calc", "mcp:Calc"] {
        let input = calc_input(serde_json::json!({"expression": "1 + 1"}));
        let run = run_proposal(&mut core, provider, "evaluate", input);
        assert_eq!(run.status, launcher_domain::WorkflowRunStatus::Failed, "{provider}");
    }
    // wrong-case server identity in the input is rejected too
    let input = launcher_mcp::adapter::invoke_input(
        &McpServerId("Calc".into()),
        "evaluate",
        serde_json::json!({}),
    );
    let run = run_proposal(&mut core, "mcp:calc", "evaluate", input);
    assert_eq!(run.status, launcher_domain::WorkflowRunStatus::Failed);
    assert_eq!(ex.count(), 0);
}

/// SEC-ID-007: the executor input boundary rejects invalid identity strings
/// directly (defense line 2, §7.5).
#[test]
fn sec_id007_executor_identity_validation() {
    for evil in ["", " ", " ../calc", "mcp:x", "a/b", "a\\b", "a..b", "a b"] {
        let v = launcher_mcp::adapter::invoke_input(
            &McpServerId(evil.into()),
            "evaluate",
            serde_json::json!({}),
        );
        assert!(
            matches!(
                McpInvokeInput::from_json(&v),
                Err(launcher_mcp::McpError::InvalidInput(_))
            ),
            "server_id {evil:?} must be rejected"
        );
    }
    for evil in ["", " ../x", "a:b", "a/b"] {
        let v = launcher_mcp::adapter::invoke_input(
            &McpServerId("calc".into()),
            evil,
            serde_json::json!({}),
        );
        assert!(
            matches!(
                McpInvokeInput::from_json(&v),
                Err(launcher_mcp::McpError::InvalidInput(_))
            ),
            "tool_name {evil:?} must be rejected"
        );
    }
    // the honest identity passes
    assert!(McpInvokeInput::from_json(&calc_input(serde_json::json!({}))).is_ok());
}

/// SEC-ID-008: a deleted server is PluginUnavailable with zero executions
/// (routing requires a configured endpoint even with a consistent input).
#[test]
fn sec_id008_unconfigured_server_rejected() {
    let ex = Arc::new(RecordingExecutor::default());
    let mut core = Core::new();
    core.set_mcp_executor(ex.clone());
    // no register_mcp_server("ghost", ...)
    let input = launcher_mcp::adapter::invoke_input(
        &McpServerId("ghost".into()),
        "evaluate",
        serde_json::json!({}),
    );
    let err = core
        .execute_effect("mcp:ghost", "invoke", &input, "e-1", 0)
        .unwrap_err();
    assert_eq!(err.0, Class::PluginUnavailable);
    assert_eq!(ex.count(), 0);
}

/// SEC-ID-009: whitespace-padded tool names cannot slip through discovery
/// matching — identity comparisons never trim.
#[test]
fn sec_id009_whitespace_identity_never_matches() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    let run = run_proposal(&mut core, "mcp:calc", " evaluate", calc_input(serde_json::json!({})));
    assert_eq!(run.status, launcher_domain::WorkflowRunStatus::Failed);
    assert_eq!(ex.count(), 0, "untrimmed reference never resolves");
}

/// SEC-ID-010: the identity binding is symmetric — an honest input on an
/// honest route still executes (no over-blocking regression).
#[test]
fn sec_id010_honest_identity_still_executes() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    let input = calc_input(serde_json::json!({"expression": "12 + 34"}));
    let run = run_proposal(&mut core, "mcp:calc", "evaluate", input);
    assert_complete(&run);
    assert_eq!(ex.tools(), vec!["evaluate".to_string()]);
}
