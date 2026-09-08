//! Minimal helper for writing external process plugins (Tier 2+).
//!
//! A plugin is any executable that speaks newline-delimited JSON-RPC on
//! stdin/stdout using `launcher-ipc` messages. `serve()` implements the
//! contract handshake (PLUGIN-CONTRACT-v0.1 §12) so plugin authors only
//! write the query handler.

use std::io::{BufRead, Write};

use launcher_ipc::{
    error_code, method, ExecuteActionParams, InitializeParams, InitializeResult, QueryParams,
    QueryResult, Request, Response, PROTOCOL_VERSION,
};

/// Serve requests from stdin until EOF or `shutdown`. `handler` maps the
/// query text to result items; the query_id echo and protocol envelope are
/// handled here. Unknown methods get METHOD_NOT_FOUND.
pub fn serve<F>(handler: F) -> Result<(), std::io::Error>
where
    F: FnMut(&str) -> serde_json::Value,
{
    serve_with_actions(handler, |_, _, _| {
        Err("execute_action not supported by this plugin".to_string())
    })
}

/// Serve with an `execute_action` handler (MVP4.0 / ADR-0014): `action_fn`
/// receives (action_id, input, context_generation) and returns the plugin's
/// result payload. The execution_id echo envelope is handled here, so plugin
/// authors still never touch protocol mechanics.
pub fn serve_with_actions<F, A>(mut handler: F, mut action_fn: A) -> Result<(), std::io::Error>
where
    F: FnMut(&str) -> serde_json::Value,
    A: FnMut(&str, &serde_json::Value, u64) -> Result<serde_json::Value, String>,
{
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let request = match Request::from_line(&line) {
            Ok(r) => r,
            Err(_) => {
                // malformed request: respond with parse error on id 0
                let resp = Response::err(0, error_code::PARSE_ERROR, "parse error");
                stdout.write_all(resp.to_line().as_bytes())?;
                stdout.flush()?;
                continue;
            }
        };
        let response = match request.method.as_str() {
            method::INITIALIZE => {
                match serde_json::from_value::<InitializeParams>(request.params) {
                    Ok(init) if init.protocol_version == PROTOCOL_VERSION => Response::ok(
                        request.id,
                        serde_json::to_value(InitializeResult {
                            protocol_version: PROTOCOL_VERSION.to_string(),
                        })
                        .expect("initialize result serializes"),
                    ),
                    Ok(init) => Response::err(
                        request.id,
                        error_code::VERSION_MISMATCH,
                        format!(
                            "unsupported protocol_version: {} (want {PROTOCOL_VERSION})",
                            init.protocol_version
                        ),
                    ),
                    Err(e) => Response::err(request.id, error_code::INVALID_PARAMS, e.to_string()),
                }
            }
            method::QUERY => {
                let parsed = serde_json::from_value::<QueryParams>(request.params);
                let response = match parsed {
                    Ok(q) => {
                        let items = handler(&q.text);
                        // wrap into the contract shape and echo the query_id
                        Response::ok(
                            request.id,
                            serde_json::to_value(QueryResult {
                                query_id: q.query_id,
                                commands: match items {
                                    serde_json::Value::Array(items) => items,
                                    other => vec![other],
                                },
                            })
                            .expect("query result serializes"),
                        )
                    }
                    Err(e) => Response::err(request.id, error_code::INVALID_PARAMS, e.to_string()),
                };
                stdout.write_all(response.to_line().as_bytes())?;
                stdout.flush()?;
                continue;
            }
            method::EXECUTE_ACTION => {
                let response = match serde_json::from_value::<ExecuteActionParams>(request.params) {
                    Ok(p) => {
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                action_fn(&p.action_id, &p.input, p.context_generation)
                            }));
                        match outcome {
                            Ok(Ok(result)) => Response::ok(
                                request.id,
                                serde_json::json!({
                                    "execution_id": p.execution_id,
                                    "result": result,
                                }),
                            ),
                            Ok(Err(msg)) => Response::err(
                                request.id,
                                error_code::INTERNAL_ERROR,
                                format!("action failed: {msg}"),
                            ),
                            Err(_) => Response::err(
                                request.id,
                                error_code::INTERNAL_ERROR,
                                "action handler panicked",
                            ),
                        }
                    }
                    Err(e) => Response::err(request.id, error_code::INVALID_PARAMS, e.to_string()),
                };
                stdout.write_all(response.to_line().as_bytes())?;
                stdout.flush()?;
                continue;
            }
            method::SHUTDOWN => {
                let resp = Response::ok(request.id, serde_json::json!({"bye": true}));
                stdout.write_all(resp.to_line().as_bytes())?;
                stdout.flush()?;
                break;
            }
            m => Response::err(
                request.id,
                error_code::METHOD_NOT_FOUND,
                format!("unknown method: {m}"),
            ),
        };
        stdout.write_all(response.to_line().as_bytes())?;
        stdout.flush()?;
    }
    Ok(())
}

pub const PARSE_ERROR: i32 = error_code::PARSE_ERROR;
pub const METHOD_NOT_FOUND: i32 = error_code::METHOD_NOT_FOUND;
pub const INTERNAL_ERROR: i32 = error_code::INTERNAL_ERROR;
