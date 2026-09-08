//! P2.3-C4 MCP Runtime/Session Recovery Matrix (supplement to P1-B):
//! the two recovery state machines — RUNTIME (process) and SESSION
//! (initialize handshake) — must recover INDEPENDENTLY and compose under
//! mixed failure sequences. Drives the real persistent mcp-evil-server
//! `counter` fixture, whose per-process counters make every respawn and
//! re-handshake directly observable.

use std::collections::HashMap;
use std::time::Duration;

use launcher_mcp::compat::McpProtocolProfile;
use launcher_mcp::McpError;
use launcher_mcp::executor::{
    EndpointRuntime, EndpointTransport, McpInvokeInput, McpExecutor, ServerEndpoint,
    StdMcpExecutor,
};
use launcher_mcp::types::McpServerId;

fn persistent_executor(profile: McpProtocolProfile) -> StdMcpExecutor {
    let mut servers = HashMap::new();
    servers.insert(
        "calc".to_string(),
        ServerEndpoint {
            transport: EndpointTransport::Stdio {
                program: env!("CARGO_BIN_EXE_mcp-evil-server").to_string(),
                args: vec!["counter".to_string()],
            },
            profile,
            runtime: EndpointRuntime::Persistent,
        },
    );
    StdMcpExecutor::new(servers, Duration::from_secs(10))
}

fn invoke(expression: &str) -> McpInvokeInput {
    McpInvokeInput {
        server_id: McpServerId("calc".into()),
        tool_name: "evaluate".into(),
        arguments: serde_json::json!({ "expression": expression }),
    }
}

/// (tool_call, initialize, process_start) — process-scoped counters, so a
/// respawn resets all three to fresh values.
fn counts(r: &launcher_mcp::executor::McpToolResult) -> (u64, u64, u64) {
    let sc = r.structured_content.as_ref().expect("structuredContent present");
    (
        sc["tool_call"].as_u64().unwrap(),
        sc["initialize"].as_u64().unwrap(),
        sc["process_start"].as_u64().unwrap(),
    )
}

/// Matrix row [2025-06-18 × crash]: on the profile WITH the initialize
/// handshake, a runtime crash must yield a fresh process that re-runs the
/// full handshake — tool_call=1, initialize=1, process_start=1 again.
#[test]
fn c4_crash_on_session_profile_respawns_with_fresh_handshake() {
    let ex = persistent_executor(McpProtocolProfile::V2025_06_18);
    let r1 = ex.execute(&invoke("1 + 1"), "e-1").unwrap();
    assert_eq!(counts(&r1), (1, 1, 1));

    let err = ex.execute(&invoke("die"), "e-2").unwrap_err();
    assert!(matches!(err, McpError::ProcessCrashed(_)), "got {err:?}");

    let r3 = ex.execute(&invoke("2 + 2"), "e-3").unwrap();
    assert_eq!(counts(&r3), (1, 1, 1), "fresh process + fresh handshake");
}

/// Matrix row [mixed]: crash and protocol-breach in one sequence. The two
/// state machines recover independently — after crash-then-breach the next
/// execution re-establishes the session on a second process and NEVER
/// replays the failed executions (INV-MCP-SESSION-004).
#[test]
fn c4_mixed_crash_and_breach_recover_independently() {
    let ex = persistent_executor(McpProtocolProfile::V2025_06_18);
    // R1: establish
    assert_eq!(counts(&ex.execute(&invoke("1 + 1"), "e-1").unwrap()), (1, 1, 1));
    // crash R1
    assert!(matches!(
        ex.execute(&invoke("die"), "e-2"),
        Err(McpError::ProcessCrashed(_))
    ));
    // R2 spawns fresh; breach invalidates its SESSION (process stays up)
    assert!(matches!(
        ex.execute(&invoke("protocol-breach"), "e-3"),
        Err(McpError::ProtocolViolation(_))
    ));
    // session re-established on R2 (init=2 on this process); the breach
    // never produced a tool_call (fixture rejects before counting) so the
    // failed execution was not replayed
    let r = ex.execute(&invoke("3 + 3"), "e-4").unwrap();
    assert_eq!(counts(&r), (1, 2, 1), "same process, re-handshaked session");
    // success resets everything: further ops just increment tool_call
    let r = ex.execute(&invoke("4 + 4"), "e-5").unwrap();
    assert_eq!(counts(&r), (2, 2, 1));
}

/// Soak: repeated crash → respawn cycles stay bounded and healthy. Each
/// cycle gets a fresh process (counters restart at 1); the executor never
/// poisons, never replays a failed execution, and keeps serving.
#[test]
fn c4_crash_loop_soak_stays_bounded() {
    let ex = persistent_executor(McpProtocolProfile::V2026_07_28);
    const CYCLES: usize = 8;
    for i in 0..CYCLES {
        assert!(
            matches!(
                ex.execute(&invoke("die"), &format!("e-crash-{i}")),
                Err(McpError::ProcessCrashed(_))
            ),
            "cycle {i}: crash must surface as ProcessCrashed"
        );
        let r = ex
            .execute(&invoke("1 + 1"), &format!("e-ok-{i}"))
            .unwrap_or_else(|e| panic!("cycle {i}: recovery failed: {e:?}"));
        assert_eq!(counts(&r).0, 1, "cycle {i}: fresh process, counters reset");
    }
}
