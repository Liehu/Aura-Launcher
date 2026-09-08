//! Minimal MCP Streamable HTTP fixture server (P0-B E2E): std::net only —
//! no HTTP library. Speaks the 2026 stateless rules the transport must
//! satisfy: header/body consistency, `_meta`, method discriminators.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

/// A started fixture: its base URL. Dropping it does NOT stop the thread;
/// tests bind per-test on port 0 and keep the listener alive in scope.
pub struct HttpFixture {
    pub url: String,
}

impl HttpFixture {
    /// Fixture variant that REQUIRES `Authorization: Bearer launcher-test`.
    pub fn start_auth() -> Self {
        let listener = Arc::new(TcpListener::bind("127.0.0.1:0").expect("bind loopback"));
        let port = listener.local_addr().unwrap().port();
        let listener2 = Arc::clone(&listener);
        static AUTH_FAIL: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
        std::thread::spawn(move || {
            for stream in listener2.incoming().flatten() {
                handle_auth(stream, &AUTH_FAIL);
            }
        });
        HttpFixture { url: format!("http://127.0.0.1:{port}/mcp") }
    }
}

/// AUTH-mode handler: every request without `Authorization: Bearer
/// launcher-test` receives 401 + WWW-Authenticate (review 54 Scenario A).
fn handle_auth(stream: TcpStream, fail_mode: &AtomicU16) {
    let mut stream = stream;
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut authorization: Option<String> = None;
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        if let Some((k, raw_value)) = line.split_once(':') {
            let value = raw_value.trim().to_string();
            if k.eq_ignore_ascii_case("authorization") {
                authorization = Some(value);
            } else if k.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap_or(0);
            }
        }
    }
    if authorization.as_deref() != Some("Bearer launcher-test-token") {
        let body = serde_json::json!({
            "error": "invalid_token",
            "error_description": "valid credential required"
        })
        .to_string();
        let resp = format!(
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nWWW-Authenticate: Bearer realm=\"mcp\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }
    let mut body = String::new();
    if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        if reader.read(&mut buf).is_err() {
            return;
        }
        body = String::from_utf8_lossy(&buf).into_owned();
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else { return };
    let id = v["id"].as_u64().unwrap_or(0);
    let text = if v["method"] == "tools/call" {
        let expr = v["params"]["arguments"]["expression"].as_str().unwrap_or("0 + 0");
        let parts: Vec<&str> = expr.split_whitespace().collect();
        let (a, op, b) = (parts[0], parts[1], parts[2]);
        let (a, b): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
        match op {
            "+" => format!("{}", (a + b) as i64),
            "-" => format!("{}", (a - b) as i64),
            "*" => format!("{}", (a * b) as i64),
            _ => "0".into(),
        }
    } else {
        "0".into()
    };
    let result = serde_json::json!({
        "jsonrpc": "2.0", "id": id,
        "result": {"content": [{"type": "text", "text": text}], "isError": false}
    });
    let resp_body = result.to_string();
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
        resp_body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = fail_mode.load(Ordering::SeqCst);
}

impl HttpFixture {
    pub fn start() -> Self {
        let listener = Arc::new(TcpListener::bind("127.0.0.1:0").expect("bind loopback"));
        let port = listener.local_addr().unwrap().port();
        static FAIL_MODE: AtomicU16 = AtomicU16::new(0);
        let listener2 = Arc::clone(&listener);
        std::thread::spawn(move || {
            for stream in listener2.incoming().flatten() {
                handle(stream, &FAIL_MODE);
            }
        });
        drop(listener); // stop-accepting is fine: tests are short-lived
        HttpFixture {
            url: format!("http://127.0.0.1:{port}/mcp"),
        }
    }
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

fn handle(stream: TcpStream, fail_mode: &AtomicU16) {
    let mut stream = stream;
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    // headers
    let mut headers = Vec::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim();
            if k.eq_ignore_ascii_case("content-length") {
                content_length = v.parse().unwrap_or(0);
            }
            headers.push((k.trim().to_ascii_lowercase(), v.to_string()));
        }
    }
    let mut body = String::new();
    if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        if reader.read(&mut buf).is_err() {
            return;
        }
        body = String::from_utf8_lossy(&buf).into_owned();
    }
    let header = |name: &str| -> Option<String> {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };

    // header/body consistency (review P0-B §9): the fixture enforces the
    // exact rule the transport must satisfy
    let body_v: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            let mut s = stream;
            respond(&mut s, "400 Bad Request", "text/html", "<html>error</html>");
            return;
        }
    };
    if header("mcp-method").as_deref() != body_v["method"].as_str() {
        let mut s = stream;
        respond(&mut s, "400 Bad Request", "application/json", "{\"error\":\"Mcp-Method mismatch\"}");
        return;
    }
    if let (Some(hn), Some(bn)) = (
        header("mcp-name").as_deref(),
        body_v["params"]["name"].as_str(),
    ) {
        if hn != bn {
            let mut s = stream;
            respond(&mut s, "400 Bad Request", "application/json", "{\"error\":\"Mcp-Name mismatch\"}");
            return;
        }
    }
    if header("mcp-protocol-version").as_deref() != Some("2026-07-28") {
        let mut s = stream;
        respond(&mut s, "400 Bad Request", "application/json", "{\"error\":\"version\"}");
        return;
    }

    let id = body_v["id"].as_u64().unwrap_or(0);
    let respond_json = |stream: &mut TcpStream, result: serde_json::Value| {
        respond(
            stream,
            "200 OK",
            "application/json",
            &serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
        );
    };
    let respond_err = |stream: &mut TcpStream, code: i64, msg: &str| {
        respond(
            stream,
            "200 OK",
            "application/json",
            &serde_json::json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": code, "message": msg}
            })
            .to_string(),
        );
    };

    match body_v["method"].as_str().unwrap_or("") {
        "tools/list" => {
            // ttlMs cache hint + unknown extension field (must be ignored)
            respond_json(
                &mut stream,
                serde_json::json!({
                    "tools": [{
                        "name": "evaluate",
                        "title": "Evaluate arithmetic",
                        "description": "Evaluates 'a op b' arithmetic expressions",
                        "inputSchema": {"type": "object",
                            "properties": {"expression": {"type": "string"}}}
                    }],
                    "ttlMs": 30_000,
                    "io.modelcontextprotocol/unsupportedExt": {"x": 1}
                }),
            );
        }
        "tools/call" => {
            // unknown tool: -32601 must keep its CommandNotFound class over HTTP
            if body_v["params"]["name"].as_str() != Some("evaluate") {
                respond_err(&mut stream, -32601, "tool not found");
                return;
            }
            let expr = body_v["params"]["arguments"]["expression"].as_str().unwrap_or("");
            let a: f64 = expr.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(f64::NAN);
            let op = expr.split_whitespace().nth(1).unwrap_or("").to_string();
            let b: f64 = expr.split_whitespace().nth(2).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN);
            let value = match op.as_str() {
                "+" => Some(a + b),
                "-" => Some(a - b),
                "*" => Some(a * b),
                "/" if b != 0.0 => Some(a / b),
                "%" if b != 0.0 => Some(a % b),
                _ => None,
            };
            match value {
                Some(v) => {
                    let text = if v == v.trunc() {
                        format!("{}", v as i64)
                    } else {
                        format!("{v}")
                    };
                    // §13: structuredContent present alongside content
                    respond_json(
                        &mut stream,
                        serde_json::json!({
                            "content": [{"type": "text", "text": text}],
                            "structuredContent": {"value": v},
                            "isError": false
                        }),
                    );
                }
                None => respond_err(&mut stream, -32000, "cannot evaluate expression"),
            }
        }
        "server/discover" => respond_json(
            &mut stream,
            serde_json::json!({
                "serverInfo": {"name": "calc-http", "version": "1.0.0"},
                "capabilities": {"tools": {}}
            }),
        ),
        // unknown tool: -32601 must keep its CommandNotFound class over HTTP
        _ if body_v["params"]["name"].is_string() => {
            respond_err(&mut stream, -32601, "tool not found");
        }
        _ => respond_err(&mut stream, -32601, "unknown method"),
    }
    let _ = fail_mode.load(Ordering::SeqCst);
}
