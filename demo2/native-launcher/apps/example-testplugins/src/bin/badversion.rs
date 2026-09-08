use launcher_ipc::{method, InitializeResult, Request, Response};
use std::io::{BufRead, Write};

// contract "version-negotiation": offers an unsupported protocol version ->
// host must reject the plugin at spawn with VersionMismatch.
fn main() {
    let mut out = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = Request::from_line(&line) else {
            continue;
        };
        if req.method == method::INITIALIZE {
            let resp = Response::ok(
                req.id,
                serde_json::to_value(InitializeResult {
                    protocol_version: "9.9".into(),
                })
                .unwrap(),
            );
            out.write_all(resp.to_line().as_bytes()).unwrap();
            out.flush().unwrap();
        }
    }
}
