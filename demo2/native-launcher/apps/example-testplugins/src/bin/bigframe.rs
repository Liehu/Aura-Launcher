use launcher_ipc::{
    method, InitializeParams, InitializeResult, Request, Response, PROTOCOL_VERSION,
};
use std::io::{BufRead, Write};

// contract "frame limit": handshake ok, then one query result with a
// ~300KB subtitle -> single frame exceeds MAX_FRAME_BYTES -> violation.
fn main() {
    let mut out = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = Request::from_line(&line) else {
            continue;
        };
        match req.method.as_str() {
            method::INITIALIZE => {
                let _: InitializeParams = serde_json::from_value(req.params).unwrap();
                let resp = Response::ok(
                    req.id,
                    serde_json::to_value(InitializeResult {
                        protocol_version: PROTOCOL_VERSION.into(),
                    })
                    .unwrap(),
                );
                out.write_all(resp.to_line().as_bytes()).unwrap();
                out.flush().unwrap();
            }
            _ => {
                let big = "x".repeat(300 * 1024);
                let resp = Response::ok(
                    req.id,
                    serde_json::json!({ "query_id": "ignored", "commands": [{ "title": "big", "subtitle": big }] }),
                );
                out.write_all(resp.to_line().as_bytes()).unwrap();
                out.flush().unwrap();
            }
        }
    }
}
