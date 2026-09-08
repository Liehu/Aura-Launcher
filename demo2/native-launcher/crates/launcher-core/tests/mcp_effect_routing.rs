//! MVP4.3 Phase 7 effect-routing tests (review 41, MCP-053..060) plus the
//! MCP-ARCH-001 architecture test: proposal → resolve → effect → executor
//! boundary assertions.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use launcher_core::providers::mcp::McpProvider;
use launcher_core::{Core, Provider};
use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{
    resolve_descriptor, ActionDescriptor, ActionKind, Capability, QueryContext,
};
use launcher_mcp::adapter::{invoke_descriptor, invoke_input};
use launcher_mcp::executor::{McpExecutor, McpInvokeInput, McpToolResult};
use launcher_mcp::types::{McpServerId, McpTool};

/// Recording mock executor: counts invocations, captures inputs + ids.
struct MockExecutor {
    calls: Mutex<Vec<(String, String, String)>>, // (server, tool, execution_id)
    live: AtomicUsize,
}

impl MockExecutor {
    fn new() -> Self {
        Self { calls: Mutex::new(Vec::new()), live: AtomicUsize::new(0) }
    }
    fn count(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }
}

impl McpExecutor for MockExecutor {
    fn execute(
        &self,
        input: &McpInvokeInput,
        execution_id: &str,
    ) -> Result<McpToolResult, launcher_mcp::McpError> {
        self.live.fetch_add(1, Ordering::SeqCst);
        self.calls
            .lock()
            .unwrap()
            .push((input.server_id.to_string(), input.tool_name.clone(), execution_id.into()));
        Ok(McpToolResult {
            content: vec![launcher_mcp::executor::McpContent {
                kind: "text".into(),
                text: Some("46".into()),
            }],
            structured_content: None,
            is_error: false,
        })
    }
}

fn core_with_executor(ex: std::sync::Arc<dyn McpExecutor>) -> Core {
    let mut core = Core::new();
    core.register_mcp_server("calc", "mcp-calculator", vec![]);
    core.set_mcp_executor(ex);
    core
}

fn calc_input(expression: &str) -> serde_json::Value {
    invoke_input(
        &McpServerId("calc".into()),
        "evaluate",
        serde_json::json!({"expression": expression}),
    )
}

/// MCP-053: the engine emits the frozen routing classification — a resolved
/// MCP action executes to `Effect::PluginInvoked` carrying the invoke input.
#[test]
fn mcp053_engine_emits_plugin_mcp_invoke() {
    let d = invoke_descriptor(&tool("evaluate"), &McpServerId("calc".into()));
    let a = resolve_descriptor(&d, &[Capability::McpInvoke]).unwrap();
    assert_eq!(a.kind, ActionKind::PluginInvoke);
    match launcher_action::execute(&a) {
        Ok(launcher_action::Effect::PluginInvoked { input, .. }) => {
            assert_eq!(input["tool_name"], "evaluate");
            assert_eq!(input["server_id"], "calc");
        }
        other => panic!("expected PluginInvoked, got {other:?}"),
    }
}

/// MCP-054: the registry routes `mcp:calc` to the McpExecutor.
#[test]
fn mcp054_correct_executor_selected() {
    let ex = std::sync::Arc::new(MockExecutor::new());
    let mut core = core_with_executor(ex.clone());
    let result = core
        .execute_effect("mcp:calc", "invoke", &calc_input("12 + 34"), &core.next_execution_id(), 0)
        .unwrap();
    assert_eq!(ex.count(), 1, "mcp namespace reaches the mcp executor");
    assert_eq!(result["content"][0]["text"], "46");
    assert_eq!(result["tool_name"], "evaluate");
}

/// MCP-055: unknown / namespace-less provider ids are rejected at the
/// registry — they never reach any executor.
#[test]
fn mcp055_unknown_namespace_rejected() {
    let ex = std::sync::Arc::new(MockExecutor::new());
    let mut core = core_with_executor(ex.clone());
    // namespace/input mismatch is refused before any executor runs
    for provider in ["mcp:ghost", "mcp:evil", "weird", ""] {
        let err = core
            .execute_effect(provider, "invoke", &calc_input("1 + 1"), "e-x", 0)
            .unwrap_err();
        assert!(
            matches!(
                err.0,
                Class::InvalidInput | Class::PluginUnavailable | Class::ProtocolViolation
            ),
            "provider {provider} must be refused, got {:?}",
            err
        );
    }
    // a well-formed namespace routes even without an explicit executor
    // override only when the server is configured
    let mut core = Core::new();
    core.register_mcp_server("calc", "mcp-calculator", vec![]);
    let err = core
        .execute_effect("mcp:ghost", "invoke", &calc_input("1 + 1"), "e-x", 0)
        .unwrap_err();
    // A-002 (review 45): cross-server mismatch is a protocol violation
    assert_eq!(err.0, Class::ProtocolViolation);
    assert_eq!(ex.count(), 0, "no executor was ever reached");
}

/// MCP-056: a Disabled action never reaches the executor — the engine's
/// validate gate refuses first.
#[test]
fn mcp056_disabled_never_reaches_executor() {
    let ex = std::sync::Arc::new(MockExecutor::new());
    let mut core = core_with_executor(ex.clone());
    let d = invoke_descriptor(&tool("evaluate"), &McpServerId("calc".into()));
    let mut a = resolve_descriptor(&d, &[Capability::McpInvoke]).unwrap();
    a.disabled_reason = Some("capability denied: mcp.invoke".into());
    assert!(matches!(
        launcher_action::execute(&a),
        Err(launcher_action::ActionError::Disabled(_))
    ));
    assert_eq!(ex.count(), 0);
    let _ = &mut core; // registry never consulted
}

/// MCP-057: capability denial happens at the resolver — the descriptor
/// never becomes executable, the executor is never reachable.
#[test]
fn mcp057_capability_denied_never_reaches_executor() {
    let ex = std::sync::Arc::new(MockExecutor::new());
    let mut core = core_with_executor(ex.clone());
    let d = invoke_descriptor(&tool("evaluate"), &McpServerId("calc".into()));
    assert!(resolve_descriptor(&d, &[]).is_err());
    // and even routing the raw proposal through the registry without a
    // grant would still be a separate explicit act — the resolver path
    // above is the only bridge proposal → effect
    assert_eq!(ex.count(), 0);
    let _ = &mut core;
}

/// MCP-058: the registry's correlation id reaches the executor verbatim
/// and is monotonic.
#[test]
fn mcp058_execution_id_preserved() {
    let ex = std::sync::Arc::new(MockExecutor::new());
    let mut core = core_with_executor(ex.clone());
    let id1 = core.next_execution_id();
    core.execute_effect("mcp:calc", "invoke", &calc_input("1 + 1"), &id1, 0).unwrap();
    let id2 = core.next_execution_id();
    core.execute_effect("mcp:calc", "invoke", &calc_input("2 + 2"), &id2, 0).unwrap();
    let calls = ex.calls.lock().unwrap();
    assert_eq!(calls[0].2, "e-1");
    assert_eq!(calls[1].2, "e-2");
    // never injected into tool arguments at the boundary either
    assert_ne!(calls[0].1, "e-1");
}

/// MCP-059: the effect input survives engine → registry → executor.
#[test]
fn mcp059_effect_input_preserved() {
    let ex = std::sync::Arc::new(MockExecutor::new());
    let mut core = core_with_executor(ex.clone());
    core.execute_effect("mcp:calc", "invoke", &calc_input("12 + 34"), &core.next_execution_id(), 0).unwrap();
    let calls = ex.calls.lock().unwrap();
    assert_eq!(calls[0].0, "calc");
    assert_eq!(calls[0].1, "evaluate");
}

/// MCP-060: there is no direct Provider → Executor path. The provider is a
/// discovery/proposal producer only; its trait-level execute is the
/// plugin-broker hook and refuses for MCP tools.
#[test]
fn mcp060_no_provider_to_executor_path() {
    let mut p = McpProvider::new("calc".into(), "mcp-calculator".into(), vec![]);
    let cmds = p.query(&QueryContext::parse(""));
    // proposal shape: routing identity only, no resolved/authorized data
    let json = serde_json::to_value(&cmds[0]).unwrap();
    for banned in ["resolved", "authorized", "confirmed", "effect", "execution_id"] {
        assert!(!json.to_string().contains(banned), "command leaks {banned}");
    }
    // provider-side execution is refused (default trait hook)
    let err = p.execute_action("invoke", &calc_input("1 + 1"), "e-test", 0).unwrap_err();
    assert_eq!(err.0, Class::InvalidInput);
}

fn tool(name: &str) -> McpTool {
    McpTool {
        server_id: McpServerId("calc".into()),
        name: name.into(),
        title: Some("Evaluate".into()),
        description: None,
        input_schema: serde_json::json!({"type": "object"}),
        annotations: serde_json::json!({"destructiveHint": false}),
    }
}

/// MCP-ARCH-001 (review 41 §最关键的集成): the five structural assertions
/// of MVP4.3's trust boundaries.
#[test]
fn mcp_arch_001_boundary_assertions() {
    let ex = std::sync::Arc::new(MockExecutor::new());
    let mut core = core_with_executor(ex.clone());
    let d = invoke_descriptor(&tool("evaluate"), &McpServerId("calc".into()));

    // 1. Provider cannot execute: discovery-only (see mcp060) — here we
    //    additionally prove the provider produces no effect on query.
    let mut p = McpProvider::new("calc".into(), "mcp-calculator".into(), vec![]);
    let _ = p.query(&QueryContext::parse(""));
    assert_eq!(ex.count(), 0, "provider query must not execute");

    // 2. Resolver cannot execute: it only classifies; without a grant it
    //    denies, with a grant it returns data — never an effect.
    assert!(resolve_descriptor(&d, &[]).is_err());
    let _ = resolve_descriptor(&d, &[Capability::McpInvoke]).unwrap();
    assert_eq!(ex.count(), 0, "resolution must not execute");
    let _ = &mut core;

    // 3. Workflow cannot bypass the engine: routing requires the engine
    //    output shape; a raw proposal blob is not an effect input.
    let raw_proposal = serde_json::json!({
        "provider_id": "mcp:calc", "command_id": "evaluate", "action_id": "invoke",
    });
    let err = core.execute_effect("mcp:calc", "invoke", &raw_proposal, "e-1", 0).unwrap_err();
    assert_eq!(err.0, Class::InvalidInput, "raw proposal is not an invoke input");

    // 4. McpExecutor cannot accept a raw ActionDescriptor: the input
    //    contract rejects descriptor-shaped data.
    let descriptor_blob = serde_json::to_value(&d).unwrap();
    assert!(matches!(
        McpInvokeInput::from_json(&descriptor_blob),
        Err(launcher_mcp::McpError::InvalidInput(_))
    ));

    // 5. MCP tool metadata cannot grant capability: `requires` is frozen
    //    by the host projection and the grant comes only from the caller.
    let mut annotated = tool("evaluate");
    annotated.annotations = serde_json::json!({"destructive": false, "allowed": true});
    let d2 = invoke_descriptor(&annotated, &McpServerId("calc".into()));
    assert!(resolve_descriptor(&d2, &[]).is_err());
    assert!(matches!(
        resolve_descriptor(&d2, &[]),
        Err(launcher_domain::DescriptorError::CapabilityDenied(m))
            if m == vec![Capability::McpInvoke]
    ));
    assert_eq!(ex.count(), 0);
}

/// Registry sanity: an ActionDescriptor whose type collides with a
/// hypothetical per-tool semantic type (`plugin.mcp.readFile`) is NOT a
/// known routing identity — the host projects every tool to the same
/// `plugin.mcp.invoke` (review 41 §5.1).
#[test]
fn no_per_tool_semantic_types() {
    let d = ActionDescriptor {
        id: "invoke".into(),
        title: None,
        action_type: "plugin.mcp.readFile".into(),
        input: serde_json::json!({"server_id": "fs", "tool_name": "read_file", "arguments": {}}),
        requires: vec![],
        shortcut: None,
        confirmation: None,
    };
    // generic plugin.* routing requires an own-plugin binding the MCP
    // namespace does not have → unknown/invalid, never silently routed
    assert!(resolve_descriptor(&d, &[Capability::McpInvoke]).is_err());
}
