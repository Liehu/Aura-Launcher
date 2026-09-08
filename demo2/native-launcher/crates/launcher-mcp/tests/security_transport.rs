//! SEC-PROTO-001..012 (MVP4.3 Phase 10, review 45 §7 Boundary E): the
//! stdio protocol channel under adversarial server behavior. Every
//! misbehavior must map into the frozen failure taxonomy without hanging,
//! corrupting parser state, or growing memory unboundedly.

use std::time::Duration;

use launcher_mcp::error::McpError;
use launcher_mcp::transport::stdio::StdioTransport;
use launcher_mcp::transport::McpTransport;

fn evil(mode: &str) -> StdioTransport {
    StdioTransport::spawn(
        env!("CARGO_BIN_EXE_mcp-evil-server"),
        &[mode.to_string()],
        Duration::from_millis(700),
    )
    .expect("evil server spawns")
}

fn evil_long_timeout(mode: &str) -> StdioTransport {
    StdioTransport::spawn(
        env!("CARGO_BIN_EXE_mcp-evil-server"),
        &[mode.to_string()],
        Duration::from_secs(5),
    )
    .expect("evil server spawns")
}

fn call(t: &mut StdioTransport) -> Result<launcher_mcp::protocol::ToolCallResult, McpError> {
    t.call_tool("evaluate", serde_json::json!({"expression": "12 + 34"}), "e-sec")
}

/// SEC-PROTO-001 (E-001): unparseable JSON on the protocol channel is a
/// ProtocolViolation — never a BusinessError, never ignored.
#[test]
fn sec_proto001_malformed_json() {
    let mut t = evil("malformed");
    t.initialize().unwrap();
    let err = call(&mut t).unwrap_err();
    assert!(matches!(err, McpError::ProtocolViolation(_)), "got {err:?}");
    t.shutdown();
}

/// SEC-PROTO-002 (E-002): a response with the wrong JSON-RPC id is a hard
/// protocol violation.
#[test]
fn sec_proto002_wrong_response_id() {
    let mut t = evil("wrong-id");
    t.initialize().unwrap();
    let err = call(&mut t).unwrap_err();
    assert!(matches!(err, McpError::ProtocolViolation(_)), "got {err:?}");
    t.shutdown();
}

/// SEC-PROTO-003 (E-003): a tools/list envelope answering tools/call does
/// not deserialize into a tool result → ProtocolViolation.
#[test]
fn sec_proto003_wrong_method_envelope() {
    let mut t = evil("wrong-method");
    t.initialize().unwrap();
    let err = call(&mut t).unwrap_err();
    assert!(matches!(err, McpError::ProtocolViolation(_)), "got {err:?}");
    t.shutdown();
}

/// SEC-PROTO-004 (E-004): notification injection cannot break response
/// correlation — the real answer is still matched by id.
#[test]
fn sec_proto004_notification_injection_is_skipped() {
    let mut t = evil_long_timeout("flood-notifications");
    t.initialize().unwrap();
    let r = call(&mut t).expect("notifications are skipped, not consumed");
    assert_eq!(r.content[0].text.as_deref(), Some("46"));
    t.shutdown();
}

/// SEC-PROTO-005 (E-005): stdout is a protocol channel — diagnostics on
/// it are a protocol violation, never tolerated noise.
#[test]
fn sec_proto005_stdout_garbage_is_violation() {
    let mut t = evil("garbage");
    t.initialize().unwrap();
    let err = call(&mut t).unwrap_err();
    assert!(matches!(err, McpError::ProtocolViolation(_)), "got {err:?}");
    t.shutdown();
}

/// SEC-PROTO-006 (E-006): a huge stderr flood cannot corrupt the JSON
/// parser or the execution — stderr is never read as protocol.
#[test]
fn sec_proto006_stderr_flood_is_ignored() {
    let mut t = evil_long_timeout("stderr-flood");
    t.initialize().unwrap();
    let r = call(&mut t).expect("stderr is not a protocol channel");
    assert_eq!(r.content[0].text.as_deref(), Some("46"));
    t.shutdown();
}

/// SEC-PROTO-007 (E-007): a 10k notification flood still completes within
/// the timeout — correlation is by id and memory stays bounded.
#[test]
fn sec_proto007_notification_flood_bounded() {
    let mut t = evil_long_timeout("flood-notifications");
    t.initialize().unwrap();
    let started = std::time::Instant::now();
    let r = call(&mut t).expect("flood is skipped within the deadline");
    assert_eq!(r.content[0].text.as_deref(), Some("46"));
    assert!(started.elapsed() < Duration::from_secs(4), "flood handled, not timed out");
    t.shutdown();
}

/// SEC-PROTO-008 (E-008): an oversized frame is a protocol violation and
/// is never buffered whole — the bounded reader discards it by cap.
#[test]
fn sec_proto008_oversized_frame_rejected() {
    let mut t = evil_long_timeout("oversized");
    t.initialize().unwrap();
    let err = call(&mut t).unwrap_err();
    assert!(matches!(err, McpError::ProtocolViolation(_)), "got {err:?}");
    // the cap constant is part of the frozen contract surface
    assert_eq!(launcher_mcp::transport::stdio::MAX_FRAME_BYTES, 256 * 1024);
    t.shutdown();
}

/// SEC-PROTO-009 (E-009): a truncated frame followed by exit is classified
/// (ProtocolViolation / ProcessCrashed / ServerUnavailable) — never a hang.
#[test]
fn sec_proto009_truncated_response_classified() {
    let mut t = evil("truncated");
    t.initialize().unwrap();
    let started = std::time::Instant::now();
    let err = call(&mut t).unwrap_err();
    assert!(
        matches!(
            err,
            McpError::ProtocolViolation(_) | McpError::ProcessCrashed(_) | McpError::ServerUnavailable(_)
        ),
        "got {err:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(3), "no hang on truncation");
    t.shutdown();
}

/// SEC-PROTO-010 (E-010): a server that never answers times out, and the
/// session is cleaned up so the next execution can spawn a fresh server.
#[test]
fn sec_proto010_hang_times_out_and_cleans_up() {
    let mut t = evil("hang");
    t.initialize().unwrap();
    let started = std::time::Instant::now();
    let err = call(&mut t).unwrap_err();
    assert!(matches!(err, McpError::Timeout(_)), "got {err:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
    t.shutdown(); // kill + wait: process reaped
    // a fresh session of the same (evil) server behaves normally
    let mut fresh = evil("ok");
    fresh.initialize().unwrap();
    let r = call(&mut fresh).unwrap();
    assert_eq!(r.content[0].text.as_deref(), Some("46"));
    fresh.shutdown();
}

/// SEC-PROTO-011 (E-011): a server crashing at tools/call is classified by
/// exit status — ProcessCrashed → ProtocolViolation class, never Timeout.
#[test]
fn sec_proto011_crash_classified_by_exit_status() {
    let mut t = evil("crash");
    t.initialize().unwrap();
    let err = call(&mut t).unwrap_err();
    assert!(matches!(err, McpError::ProcessCrashed(_)), "got {err:?}");
    assert_eq!(
        err.failure_class(),
        launcher_domain::workflow::WorkflowFailureClass::ProtocolViolation
    );
    t.shutdown();
}

/// SEC-PROTO-012 (E-012): a response written in two chunks is parsed only
/// once complete — framing never fires early on a partial message.
#[test]
fn sec_proto012_partial_write_is_not_parsed_early() {
    let mut t = evil_long_timeout("slow-write");
    t.initialize().unwrap();
    let r = call(&mut t).expect("dribbled response parses when complete");
    assert_eq!(r.content[0].text.as_deref(), Some("46"));
    t.shutdown();
}
