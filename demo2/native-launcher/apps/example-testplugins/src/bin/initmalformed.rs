use launcher_ipc::{method, Request};
use std::io::{BufRead, Write};

// contract "initialize malformed": replies to initialize with invalid JSON
// -> host must reject at spawn with Malformed, never crash.
fn main() {
    let mut out = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = Request::from_line(&line) else {
            continue;
        };
        if req.method == method::INITIALIZE {
            out.write_all(format!("}}}}not json {}\n", req.id).as_bytes())
                .unwrap();
            out.flush().unwrap();
        }
    }
}
