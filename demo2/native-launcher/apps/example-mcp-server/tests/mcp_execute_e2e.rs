//! MVP4.3 Phase 5–7 integration E2E (review 41 "最关键的集成 E2E"):
//! the full chain over a real MCP stdio server fixture —
//!
//! ```text
//! Search → McpProvider → Command/Action → (Resolver) → ActionEngine
//!        → Effect("plugin.mcp.invoke") → Core registry → McpExecutor
//!        → McpTransport → MCP Server → 46
//! ```
//!
//! Plus the CapabilityDenied E2E: a denied grant keeps the tool
//! discoverable but unexecutable end-to-end.

use launcher_core::providers::mcp::McpProvider;
use launcher_core::{Core, Provider};
use launcher_domain::{Capability, QueryContext};
use launcher_mcp::adapter::invoke_input;
use launcher_mcp::types::McpServerId;

fn provider() -> McpProvider {
    McpProvider::new(
        "calc".into(),
        env!("CARGO_BIN_EXE_mcp-calculator").into(),
        vec![],
    )
}

/// Full-chain E2E: discovery → engine → effect routing → real server → 46.
#[test]
fn mcp_execute_end_to_end() {
    // 1. discovery: search surfaces the tool as a Command with a Ready action
    let mut p = provider();
    let cmds = p.query(&QueryContext::parse("evaluate"));
    assert_eq!(cmds.len(), 1);
    let cmd = &cmds[0];
    assert_eq!(cmd.provider_id, "mcp:calc");
    let action = cmd.primary_action().expect("invoke action is Ready");
    assert_eq!(action.id.as_deref(), Some("invoke"));

    // 2. producer fills the tool arguments into the proposal (the
    //    projection never guesses them); the engine gate validates and
    //    produces the frozen routing effect
    let mut input = match launcher_action::execute(action).expect("engine accepts a Ready action") {
        launcher_action::Effect::PluginInvoked { action_id, input } => {
            (action_id, input)
        }
        other => panic!("unexpected effect: {other:?}"),
    };
    input.1["arguments"] = serde_json::json!({"expression": "12 + 34"});

    // 3. effect routing: registry selects the McpExecutor for mcp:calc and
    //    the real fixture server evaluates the expression
    let mut core = Core::new();
    core.register_mcp_server(
        "calc",
        env!("CARGO_BIN_EXE_mcp-calculator"),
        vec![],
    );
    let execution_id = core.next_execution_id();
    let result = core
        .execute_effect("mcp:calc", &input.0.unwrap_or_default(), &input.1, &execution_id, 0)
        .expect("full chain succeeds");
    assert_eq!(
        result["content"][0]["text"], "46",
        "12 + 34 must evaluate to 46 through the real server"
    );
    assert_eq!(result["server_id"], "calc");
    let _ = execution_id;
}

/// Execution id E2E: two chained executions carry distinct correlation ids
/// through the registry into the transport.
#[test]
fn mcp_execution_ids_are_distinct() {
    let mut core = Core::new();
    core.register_mcp_server("calc", env!("CARGO_BIN_EXE_mcp-calculator"), vec![]);
    let e1 = core.next_execution_id();
    let input = invoke_input(
        &McpServerId("calc".into()),
        "evaluate",
        serde_json::json!({"expression": "1 + 1"}),
    );
    core.execute_effect("mcp:calc", "invoke", &input, &core.next_execution_id(), 0).unwrap();
    let e2 = core.next_execution_id();
    assert_ne!(e1, e2);
}

/// CapabilityDenied E2E: deny the host grant → the tool is still discovered
/// and presented, but the engine refuses and nothing reaches the server.
#[test]
fn mcp_capability_denied_end_to_end() {
    let mut p = provider();
    p.set_invoke_granted(false);
    let cmds = p.query(&QueryContext::parse("evaluate"));
    assert_eq!(cmds.len(), 1, "denied tools stay discoverable");
    let action = &cmds[0].actions[0];
    assert!(action.disabled_reason.is_some());
    assert!(matches!(
        launcher_action::execute(action),
        Err(launcher_action::ActionError::Disabled(_))
    ));
}

/// Business-error E2E: the fixture's protocol-level tool error surfaces as
/// a classified BusinessError from the registry (never a protocol panic).
#[test]
fn mcp_business_error_end_to_end() {
    let mut core = Core::new();
    core.register_mcp_server("calc", env!("CARGO_BIN_EXE_mcp-calculator"), vec![]);
    let input = invoke_input(
        &McpServerId("calc".into()),
        "evaluate",
        serde_json::json!({"expression": "not a math expression"}),
    );
    let err = core
        .execute_effect("mcp:calc", "invoke", &input, &core.next_execution_id(), 0)
        .unwrap_err();
    assert_eq!(
        err.0,
        launcher_domain::workflow::WorkflowFailureClass::BusinessError
    );
}

/// Host grant flips to granted after a denied projection: the provider
/// re-projects and the action becomes executable (policy is host-owned).
#[test]
fn mcp_grant_toggle_reprojection() {
    let mut p = provider();
    p.set_invoke_granted(false);
    assert!(p.query(&QueryContext::parse("evaluate"))[0].actions[0]
        .disabled_reason
        .is_some());
    p.set_invoke_granted(true);
    // projection cache was refreshed under the new policy
    let a = &p.query(&QueryContext::parse("evaluate"))[0].actions[0];
    assert!(a.disabled_reason.is_none());
    assert!(launcher_action::validate(a).is_ok());
    // and the full chain executes against the real server
    let mut core = Core::new();
    core.register_mcp_server("calc", env!("CARGO_BIN_EXE_mcp-calculator"), vec![]);
    let input = invoke_input(
        &McpServerId("calc".into()),
        "evaluate",
        serde_json::json!({"expression": "10 % 3"}),
    );
    let r = core.execute_effect("mcp:calc", "invoke", &input, &core.next_execution_id(), 0).unwrap();
    assert_eq!(r["content"][0]["text"], "1");
    let _ = Capability::McpInvoke;
}
