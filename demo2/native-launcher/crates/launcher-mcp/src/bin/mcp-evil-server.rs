//! Adversarial MCP stdio server fixture (MVP4.3 Phase 10, review 45 §7).
//! Behavior selected by argv[1]; speaks the legacy handshake normally, then
//! misbehaves at `tools/call` exactly one documented way. Used ONLY by the
//! SEC-PROTO adversarial suite.

use std::io::{BufRead, Write};

fn respond(out: &mut impl Write, id: u64, result: serde_json::Value) {
    let line = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn initialize_result() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "evil", "version": "6.6.6"}
    })
}

fn tools_list_result() -> serde_json::Value {
    serde_json::json!({"tools": [{
        "name": "evaluate",
        "description": "malicious by construction",
        "inputSchema": {"type": "object"},
        "annotations": {"readOnlyHint": true, "destructiveHint": false}
    }]})
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    // P1-A persistent-runtime probe: tools/call returns an incrementing
    // counter, so callers can prove they are talking to the SAME process.
    let counter_mode = mode == "counter";
    let tool_calls = std::sync::atomic::AtomicU64::new(0);
    let init_count = std::sync::atomic::AtomicU64::new(0);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let id = req["id"].as_u64().unwrap_or(0);
        match req["method"].as_str().unwrap_or("") {
            "initialize" => {
                init_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                respond(&mut out, id, initialize_result())
            }
            "notifications/initialized" => {}
            "tools/list" => respond(&mut out, id, tools_list_result()),
            "tools/call" if counter_mode
                && req["params"]["arguments"]["expression"].as_str()
                    == Some("protocol-breach") =>
            {
                // E-003 probe: foreign envelope → client-side
                // ProtocolViolation → session invalidation
                respond(&mut out, id, serde_json::json!({"tools": []}));
            }
            "tools/call" if counter_mode
                && req["params"]["arguments"]["expression"].as_str() == Some("die") =>
            {
                std::process::exit(1);
            }
            "tools/call" if counter_mode => {
                // P1-B counters (review 57 SS24): tool_call / initialize /
                // process_start are observable per process. The process
                // itself is the counter holder — a respawn resets them.
                tool_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let tc = tool_calls.load(std::sync::atomic::Ordering::SeqCst);
                let ic = init_count.load(std::sync::atomic::Ordering::SeqCst);
                respond(&mut out, id, serde_json::json!({
                    "content": [{"type": "text", "text": tc.to_string()}],
                    "structuredContent": {
                        "tool_call": tc,
                        "initialize": ic,
                        "process_start": 1
                    },
                    "isError": false
                }));
            }
            "tools/call" => match mode.as_str() {
                // E-001: unparseable JSON on the protocol channel
                "malformed" => {
                    let _ = writeln!(out, "{{invalid json");
                    let _ = out.flush();
                }
                // E-002: response id does not match the request id
                "wrong-id" => respond(&mut out, id + 1, serde_json::json!({"content": [], "isError": false})),
                // E-003: a tools/list envelope answers tools/call
                "wrong-method" => respond(&mut out, id, tools_list_result()),
                // E-004/E-007: notification storm, then the real answer
                "flood-notifications" => {
                    for i in 0..10_000 {
                        let _ = writeln!(
                            out,
                            "{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/noise\",\"params\":{{\"n\":{i}}}}}"
                        );
                    }
                    let _ = out.flush();
                    respond(&mut out, id, serde_json::json!({"content": [{"type": "text", "text": "46"}], "isError": false}));
                }
                // E-005: diagnostics on the protocol channel, then the answer
                "garbage" => {
                    let _ = writeln!(out, "DEBUG HELLO from evil server");
                    let _ = out.flush();
                    respond(&mut out, id, serde_json::json!({"content": [{"type": "text", "text": "46"}], "isError": false}));
                }
                // E-006: unbounded stderr (stdout stays clean)
                "stderr-flood" => {
                    for _ in 0..2_000 {
                        eprintln!("{}", "x".repeat(8192));
                    }
                    respond(&mut out, id, serde_json::json!({"content": [{"type": "text", "text": "46"}], "isError": false}));
                }
                // E-008: one oversized frame, then the real answer
                "oversized" => {
                    let _ = writeln!(out, "{}", "x".repeat(300_000));
                    let _ = out.flush();
                    respond(&mut out, id, serde_json::json!({"content": [{"type": "text", "text": "46"}], "isError": false}));
                }
                // E-010: never answer
                "hang" => loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                },
                // E-011: die at the call
                "crash" => std::process::exit(1),
                // E-009: truncated frame, then die
                "truncated" => {
                    let _ = write!(out, "{{\"jsonrpc\":\"2.0\",\"id\":");
                    let _ = out.flush();
                    std::process::exit(1);
                }
                // E-012: dribble one response in two writes
                "slow-write" => {
                    let line = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"content": [{"type": "text", "text": "46"}], "isError": false}}).to_string();
                    let (a, b) = line.split_at(line.len() / 2);
                    let _ = write!(out, "{a}");
                    let _ = out.flush();
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let _ = writeln!(out, "{b}");
                    let _ = out.flush();
                }
                // control: behave
                _ => respond(&mut out, id, serde_json::json!({"content": [{"type": "text", "text": "46"}], "isError": false})),
            },
            _ => {}
        }
    }
}
