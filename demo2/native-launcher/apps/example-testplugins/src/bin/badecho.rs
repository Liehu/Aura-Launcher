use launcher_ipc::{
    method, InitializeParams, InitializeResult, Request, Response, PROTOCOL_VERSION,
};
use std::io::{BufRead, Write};

// contract "query_id echo": handshake ok, but the query result echoes the
// wrong query_id -> host must treat the response as a protocol violation.
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
                let resp = Response::ok(
                    req.id,
                    serde_json::json!({ "query_id": "not-the-id-you-sent", "commands": [{ "title": "stale" }] }),
                );
                out.write_all(resp.to_line().as_bytes()).unwrap();
                out.flush().unwrap();
            }
        }
    }
}
