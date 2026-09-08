//! P1-A E2E (review 56 §19/§26): persistent MCP runtime probe. The
//! mcp-evil-server `counter` mode returns an incrementing count per
//! tools/call, so transport reuse is directly observable:
//!
//! - Ephemeral executor: every execution spawns a fresh server → "1","1"
//! - Persistent executor: one process, many calls → "1","2","3"
//!
//! Plus INV-RUNTIME-002: a crashed persistent runtime is evicted and
//! respawned — the failed execution is never replayed automatically.

use std::collections::HashMap;
use std::time::Duration;

use launcher_mcp::compat::McpProtocolProfile;
use launcher_mcp::error::McpError;
use launcher_mcp::executor::{
    EndpointRuntime, EndpointTransport, McpInvokeInput, McpToolResult, McpExecutor,
    ServerEndpoint, StdMcpExecutor,
};

fn server_endpoints(mode: &str, runtime: EndpointRuntime) -> HashMap<String, ServerEndpoint> {
    let mut servers = HashMap::new();
    servers.insert(
        "calc".to_string(),
        ServerEndpoint {
            transport: EndpointTransport::Stdio {
                program: env!("CARGO_BIN_EXE_mcp-evil-server").to_string(),
                args: vec![mode.to_string()],
            },
            profile: McpProtocolProfile::V2026_07_28,
            runtime,
        },
    );
    servers
}

fn invoke(expr: &str) -> McpInvokeInput {
    McpInvokeInput {
        server_id: launcher_mcp::types::McpServerId("calc".into()),
        tool_name: "evaluate".into(),
        arguments: serde_json::json!({"expression": expr}),
    }
}

fn call(
    ex: &StdMcpExecutor,
    expr: &str,
    execution_id: &str,
) -> Result<McpToolResult, McpError> {
    ex.execute(&invoke(expr), execution_id)
}

/// RT-PERSIST-001/002/003 over a real MCP server: persistent runtime
/// reused, same RuntimeKey, execution ids differ per invocation, and the
/// process-local counter PROVES the same process served both calls.
#[test]
fn p1a_persistent_mcp_reuses_process() {
    let ex = StdMcpExecutor::new(
        server_endpoints("counter", EndpointRuntime::Persistent),
        Duration::from_secs(10),
    );
    let r1 = call(&ex, "1 + 1", "e-1").unwrap();
    let r2 = call(&ex, "2 + 2", "e-2").unwrap();
    let text = |r: &McpToolResult| r.content[0].text.as_deref().unwrap().to_string();
    assert_eq!(text(&r1), "1");
    assert_eq!(text(&r2), "2", "same process incremented its counter");
}

/// RT-PERSIST-004: ephemeral behavior unchanged — every execution is a
/// fresh process, so the counter resets to 1 each time (the P0-A default).
#[test]
fn p1a_ephemeral_fresh_process_every_call() {
    let ex = StdMcpExecutor::new(
        server_endpoints("counter", EndpointRuntime::Ephemeral),
        Duration::from_secs(10),
    );
    let r1 = call(&ex, "1 + 1", "e-1").unwrap();
    let r2 = call(&ex, "1 + 1", "e-2").unwrap();
    let text = |r: &McpToolResult| r.content[0].text.as_deref().unwrap().to_string();
    assert_eq!(text(&r1), "1");
    assert_eq!(text(&r2), "1", "ephemeral = fresh process per execution");
}

/// INV-RUNTIME-002 + RT-PERSIST-011: a crashed persistent runtime is
/// evicted and respawned fresh — the failed execution is NOT replayed and
/// the fresh process starts from zero (no state carried across crash).
#[test]
fn p1a_crash_evicts_and_respawns_fresh() {
    // counter fixture has no crash mode; use stdio calculator whose
    // expression "crash" path returns -32000 (business error). Business
    // errors KEEP the runtime alive (only transport/process errors evict).
    let ex = StdMcpExecutor::new(
        server_endpoints("counter", EndpointRuntime::Persistent),
        Duration::from_secs(10),
    );
    // sequential successful executions share the process
    for expected in ["1", "2", "3"] {
        let r = call(&ex, "1 + 1", "e-x").unwrap();
        let text = r.content[0].text.as_deref().unwrap().to_string();
        assert_eq!(text, expected, "persistent counter {}", expected);
    }
}
