use launcher_ipc::{
    method, InitializeParams, InitializeResult, QueryParams, Request, Response, PROTOCOL_VERSION,
};
use std::io::{BufRead, Write};

// contract "process-tree cleanup": on query spawns a long-lived grandchild
// (ping.exe, ~30s) besides itself -> killing the plugin (Job Object,
// ADR-0005) must reap the whole tree, leaving no orphan ping.exe.
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
                let _: QueryParams = serde_json::from_value(req.params).unwrap();
                let _child = std::process::Command::new("cmd")
                    .args(["/C", "ping -n 30 127.0.0.1 >NUL"])
                    .spawn();
                // the test manifest is legacy profile (no schema_version),
                // so a bare array is the accepted shape here
                let resp = Response::ok(req.id, serde_json::json!([{ "title": "child spawned" }]));
                out.write_all(resp.to_line().as_bytes()).unwrap();
                out.flush().unwrap();
            }
        }
    }
}
