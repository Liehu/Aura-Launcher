//! Minimal deterministic MCP stdio server (MVP4.3 E2E fixture).
//!
//! Implements the frozen subset: initialize → notifications/initialized →
//! tools/list → tools/call, newline-delimited JSON-RPC 2.0 over stdio.
//! Exposes one `evaluate` tool (arithmetic) so the full chain is observable.

use std::io::{BufRead, Write};

fn eval(expr: &str) -> Option<f64> {
    // restricted single-op evaluator: "a op b" with + - * / %
    let mut it = expr.split_whitespace();
    let a: f64 = it.next()?.parse().ok()?;
    let op = it.next()?;
    let b: f64 = it.next()?.parse().ok()?;
    match op {
        "+" => Some(a + b),
        "-" => Some(a - b),
        "*" => Some(a * b),
        "/" if b != 0.0 => Some(a / b),
        "%" if b != 0.0 => Some(a % b),
        _ => None,
    }
}

fn respond(out: &mut impl Write, id: u64, result: serde_json::Value) {
    let line = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn respond_err(out: &mut impl Write, id: u64, code: i64, msg: &str) {
    let line = serde_json::json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg}});
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn main() {
    // Optional profile mode: `mcp-calculator 2026` serves the stateless
    // 2026-07-28 profile (no initialize/initialized handshake; every
    // request must carry `_meta.protocolVersion`; `server/discover`
    // available). Default = legacy 2025-06-18 session profile.
    let profile_2026 = std::env::args().nth(1).as_deref() == Some("2026");
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) else {
            respond_err(&mut out, 0, -32700, "parse error");
            continue;
        };
        let id = req["id"].as_u64().unwrap_or(0);
        let method = req["method"].as_str().unwrap_or("");
        if profile_2026 {
            let meta_version = req["params"]["_meta"]["protocolVersion"].as_str().unwrap_or("");
            if method != "notifications/initialized" && meta_version != "2026-07-28" {
                // stateless requests are self-describing: a request without
                // the profile discriminator is a protocol violation
                respond_err(&mut out, id, -32600, "missing _meta.protocolVersion");
                continue;
            }
            match method {
                "tools/list" => {
                    respond(&mut out, id, tools_list_result_2026());
                }
                "server/discover" => respond(
                    &mut out,
                    id,
                    serde_json::json!({
                        "serverInfo": {"name": "calc-mcp", "version": "1.0.0"},
                        "capabilities": {"tools": {}},
                        "extensions": {"io.modelcontextprotocol/tasks": "unsupported"}
                    }),
                ),
                "tools/call" => tools_call(&mut out, id, &req["params"]["arguments"]),
                _ => {}
            }
            continue;
        }
        match method {
            "initialize" => respond(
                &mut out,
                id,
                serde_json::json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "calc-mcp", "version": "1.0.0"}
                }),
            ),
            "notifications/initialized" => {}
            "tools/list" => respond(&mut out, id, tools_list_result_2026()),
            "tools/call" => tools_call(&mut out, id, &req["params"]["arguments"]),
            other => respond_err(&mut out, id, -32601, &format!("unknown method: {other}")),
        }
    }
}

fn tools_list_result_2026() -> serde_json::Value {
    serde_json::json!({
        "tools": [{
            "name": "evaluate",
            "title": "Evaluate arithmetic",
            "description": "Evaluates 'a op b' arithmetic expressions",
            "inputSchema": {
                "type": "object",
                "properties": {"expression": {"type": "string"}},
                "required": ["expression"]
            }
        }],
        "ttlMs": 60000
    })
}

fn tools_call(out: &mut impl Write, id: u64, args: &serde_json::Value) {
    let expr = args["expression"].as_str().unwrap_or("");
    match eval(expr) {
        Some(v) => {
            let text = if v == v.trunc() {
                format!("{}", v as i64)
            } else {
                format!("{v}")
            };
            respond(
                out,
                id,
                serde_json::json!({
                    "content": [{"type": "text", "text": text}],
                    "structuredContent": {"value": v},
                    "isError": false
                }),
            );
        }
        None => respond_err(out, id, -32000, "cannot evaluate expression"),
    }
}
