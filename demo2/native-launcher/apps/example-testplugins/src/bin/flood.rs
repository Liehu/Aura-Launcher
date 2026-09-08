use launcher_ipc::{
    method, InitializeParams, InitializeResult, Request, Response, PROTOCOL_VERSION,
};
use std::io::{BufRead, Write};

// contract "flood": handshake ok, then returns 150 results as a legacy
// bare array -> host must truncate to MAX_PLUGIN_RESULTS (100). The frame
// stays under MAX_FRAME_BYTES; oversized frames are covered by bigframe.
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
                let items: Vec<serde_json::Value> = (0..150)
                    .map(|i| serde_json::json!({ "title": format!("flood-{i}") }))
                    .collect();
                let resp = Response::ok(req.id, serde_json::Value::Array(items));
                out.write_all(resp.to_line().as_bytes()).unwrap();
                out.flush().unwrap();
            }
        }
    }
}
