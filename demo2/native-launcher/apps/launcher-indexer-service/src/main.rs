//! launcher-indexer-service: standalone indexer process (design spec 3.1).
//!
//! Serves newline-delimited JSON-RPC over stdin/stdout so Core can talk to it
//! out-of-process in a later slice; today it can also be driven manually:
//!
//!   launcher-indexer-service <db-path> <root-dir>...
//!
//! Methods: status, rebuild, search(query), shutdown.

use std::io::{BufRead, Write};

use launcher_indexer::Indexer;
use launcher_ipc::{method, Request, Response};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: launcher-indexer-service <db-path> <root-dir>...");
        std::process::exit(2);
    }
    let db = std::path::PathBuf::from(&args[0]);
    let roots: Vec<std::path::PathBuf> = args[1..].iter().map(std::path::PathBuf::from).collect();

    tracing::info!("indexer.started");
    let mut indexer = Indexer::open(&db)?;
    indexer.rebuild(&roots)?;
    let (n,) = indexer.status()?;
    tracing::info!(files = n, "indexer.updated");

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let req = match Request::from_line(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::err(0, -32700, e.to_string());
                stdout.write_all(resp.to_line().as_bytes())?;
                stdout.flush()?;
                continue;
            }
        };
        let resp = match req.method.as_str() {
            "status" => {
                let (n,) = indexer.status().unwrap_or((0,));
                Response::ok(req.id, serde_json::json!({ "files": n }))
            }
            "rebuild" => match indexer.rebuild(&roots) {
                Ok(n) => Response::ok(req.id, serde_json::json!({ "indexed": n })),
                Err(e) => Response::err(req.id, -32603, e.to_string()),
            },
            "search" => {
                let q = req
                    .params
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                match indexer.search(q, 50) {
                    Ok(files) => {
                        let v = serde_json::to_value(&files).unwrap_or_default();
                        Response::ok(req.id, v)
                    }
                    Err(e) => Response::err(req.id, -32603, e.to_string()),
                }
            }
            m if m == method::SHUTDOWN => {
                stdout.write_all(
                    Response::ok(req.id, serde_json::json!("bye"))
                        .to_line()
                        .as_bytes(),
                )?;
                stdout.flush()?;
                break;
            }
            m => Response::err(req.id, -32601, format!("unknown method: {m}")),
        };
        stdout.write_all(resp.to_line().as_bytes())?;
        stdout.flush()?;
    }
    tracing::info!("indexer stopped");
    Ok(())
}
