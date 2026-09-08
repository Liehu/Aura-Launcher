use launcher_ipc::{
    method, InitializeParams, InitializeResult, Request, Response, PROTOCOL_VERSION,
};
use std::io::{BufRead, Write};

// contract "crash": completes the handshake, then exits with code 1 on the
// first query -> host must survive and be able to respawn plugins.
fn main() {
    let mut out = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = Request::from_line(&line) else {
            continue;
        };
        match req.method.as_str() {
            method::INITIALIZE => {
                let init: InitializeParams =
                    serde_json::from_value(req.params).expect("init params");
                assert_eq!(init.protocol_version, PROTOCOL_VERSION);
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
            _ => std::process::exit(1),
        }
    }
}
