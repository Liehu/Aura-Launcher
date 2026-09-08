//! P1-B E2E (review 57): protocol session lifecycle over a REAL
//! persistent MCP server. The counter fixture reports per-process
//! `tool_call` / `initialize` / `process_start` counts via
//! structuredContent, making session reuse, idempotent initialize,
//! invalidation and crash recovery directly observable.

use std::collections::HashMap;
use std::time::Duration;

use launcher_mcp::compat::McpProtocolProfile;
use launcher_mcp::McpError;
use launcher_mcp::executor::{
    EndpointRuntime, EndpointTransport, McpInvokeInput, McpExecutor, McpToolResult,
    ServerEndpoint, StdMcpExecutor,
};
use launcher_mcp::types::McpServerId;

fn servers(profile: McpProtocolProfile, runtime: EndpointRuntime) -> HashMap<String, ServerEndpoint> {
    let mut servers = HashMap::new();
    servers.insert(
        "calc".to_string(),
        ServerEndpoint {
            transport: EndpointTransport::Stdio {
                program: env!("CARGO_BIN_EXE_mcp-evil-server").to_string(),
                args: vec!["counter".to_string()],
            },
            profile,
            runtime,
        },
    );
    servers
}

fn persistent_executor(profile: McpProtocolProfile) -> StdMcpExecutor {
    StdMcpExecutor::new(servers(profile, EndpointRuntime::Persistent), Duration::from_secs(10))
}

fn invoke(expression: &str) -> McpInvokeInput {
    McpInvokeInput {
        server_id: McpServerId("calc".into()),
        tool_name: "evaluate".into(),
        arguments: serde_json::json!({"expression": expression}),
    }
}

fn counts(r: &McpToolResult) -> (u64, u64, u64) {
    let sc = r.structured_content.as_ref().expect("structuredContent present");
    (
        sc["tool_call"].as_u64().unwrap(),
        sc["initialize"].as_u64().unwrap(),
        sc["process_start"].as_u64().unwrap(),
    )
}

/// SESSION-001/002 + §24 operations 1-2: the first operation establishes
/// (initialize=1, tool_call=1); the second reuses the Ready session —
/// initialize is idempotent (still 1) while tool_call increments to 2.
/// process_start stays 1: same process throughout.
#[test]
fn session001_002_persistent_reuse_idempotent_initialize() {
    let ex = persistent_executor(McpProtocolProfile::V2025_06_18);
    let r1 = ex.execute(&invoke("1 + 1"), "e-1").unwrap();
    let (tc1, init1, ps1) = counts(&r1);
    assert_eq!((tc1, init1, ps1), (1, 1, 1));

    let r2 = ex.execute(&invoke("2 + 2"), "e-2").unwrap();
    let (tc2, init2, ps2) = counts(&r2);
    assert_eq!((tc2, init2, ps2), (2, 1, 1), "initialize idempotent, same process");
}

/// SESSION-003 + §13: a protocol violation invalidates the SESSION while
/// the runtime usually stays alive; the next execution re-establishes
/// (initialize=2) on the SAME process and the failed execution is never
/// replayed (INV-MCP-SESSION-004).
#[test]
fn session003_protocol_violation_invalidates_session_keeps_process() {
    let ex = persistent_executor(McpProtocolProfile::V2025_06_18);
    // op1: establish
    let r1 = ex.execute(&invoke("1 + 1"), "e-1").unwrap();
    assert_eq!(counts(&r1).0, 1);
    // breach probe: foreign envelope → ProtocolViolation
    let err = ex.execute(&invoke("protocol-breach"), "e-2").unwrap_err();
    assert!(matches!(err, McpError::ProtocolViolation(_)), "got {err:?}");
    // next execution: session re-established on the SAME process
    let r3 = ex.execute(&invoke("3 + 3"), "e-3").unwrap();
    let (tc, init, ps) = counts(&r3);
    assert_eq!(init, 2, "re-established: initialize count now 2");
    assert_eq!(ps, 1, "runtime stayed alive");
    assert!(tc >= 1);
}

/// INV-MCP-SESSION-002/007: a live runtime does not imply a valid session
/// and vice versa — after invalidation the executor state machine refuses
/// to use the stale session and establishes S2 (S1 ≠ S2).
#[test]
fn session_invalid_vs_runtime_alive() {
    let ex = persistent_executor(McpProtocolProfile::V2025_06_18);
    let _ = ex.execute(&invoke("1 + 1"), "e-1").unwrap();
    let _ = ex.execute(&invoke("protocol-breach"), "e-2").unwrap_err();
    // S2 (fresh initialize on R1) serves the next execution fine
    let r = ex.execute(&invoke("4 + 4"), "e-3").unwrap();
    let (_, init, _) = counts(&r);
    assert_eq!(init, 2);
}

/// RT-PERSIST-011 / INV-RUNTIME-002: a crashed persistent process is
/// evicted; the next execution respawns a FRESH process (counters reset)
/// and the failed execution is never replayed.
#[test]
fn p1a_crash_evicts_and_respawns_fresh() {
    let ex = persistent_executor(McpProtocolProfile::V2026_07_28);
    // 2026 stateless: no initialize handshake; counters purely tool_call
    let r1 = ex.execute(&invoke("1 + 1"), "e-1").unwrap();
    assert_eq!(counts(&r1).0, 1);
    // crash the process
    let err = ex.execute(&invoke("die"), "e-2").unwrap_err();
    assert!(matches!(err, McpError::ProcessCrashed(_)), "got {err:?}");
    // next execution respawns a fresh process: counters restart
    let r3 = ex.execute(&invoke("1 + 1"), "e-3").unwrap();
    assert_eq!(counts(&r3).0, 1);
    let _ = McpProtocolProfile::V2025_06_18;
}

/// RT-PERSIST-004: ephemeral behavior unchanged — every execution spawns
/// a fresh process (counters always 1).
#[test]
fn ephemeral_regression_unchanged() {
    let ex = StdMcpExecutor::new(
        servers(McpProtocolProfile::V2025_06_18, EndpointRuntime::Ephemeral),
        Duration::from_secs(10),
    );
    for _ in 0..3 {
        let r = ex.execute(&invoke("1 + 1"), "e-x").unwrap();
        assert_eq!(counts(&r).0, 1, "fresh process every time");
    }
}
