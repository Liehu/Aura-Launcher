use launcher_ipc::{
    method, InitializeParams, InitializeResult, Request, Response, PROTOCOL_VERSION,
};
use std::io::{BufRead, Write};

// contract "slow": handshake is immediate; query answers after 5s -> host
// must time out the query without blocking and kill the process.
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
                std::thread::sleep(std::time::Duration::from_secs(5));
                let resp = Response::ok(
                    req.id,
                    serde_json::json!({ "query_id": "late", "commands": [{ "title": "too late" }] }),
                );
                out.write_all(resp.to_line().as_bytes()).unwrap();
                out.flush().unwrap();
            }
        }
    }
}
