//! Phase 12 Release Gate — MCP reliability soak (review 48 §11/§12):
//! repeated discovery + execution cycles over the real fixture. Each
//! execution spawns a FRESH server session and shuts it down — the soak
//! proves no state accumulation, no hang, no growth across N cycles.

use std::time::{Duration, Instant};

use launcher_core::providers::mcp::McpProvider;
use launcher_core::Provider;
use launcher_domain::QueryContext;
use launcher_mcp::compat::McpProtocolProfile;
use launcher_mcp::transport::stdio::StdioTransport;
use launcher_mcp::transport::McpTransport;

const CYCLES: usize = 100;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_mcp-calculator")
}

/// 100 discovery cycles: spawn → initialize → tools/list → shutdown must
/// yield an identical catalog every time (fresh session each cycle).
#[test]
fn release_soak_discovery_cycles() {
    let started = Instant::now();
    for _ in 0..CYCLES {
        let mut t = StdioTransport::spawn(bin(), &[], Duration::from_secs(5)).unwrap();
        t.initialize().unwrap();
        let tools = t.list_tools().unwrap();
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "evaluate");
        t.shutdown(); // process reaped; next cycle is a fresh session
    }
    assert!(
        started.elapsed() < Duration::from_secs(120),
        "discovery soak bounded: {:?}",
        started.elapsed()
    );
}

/// 100 execution cycles: spawn → initialize → tools/call → shutdown, with
/// identical results every cycle (no state accumulation between sessions).
#[test]
fn release_soak_execute_cycles() {
    let started = Instant::now();
    for i in 0..CYCLES {
        let mut t = StdioTransport::spawn(bin(), &[], Duration::from_secs(5)).unwrap();
        t.initialize().unwrap();
        let expr = if i % 2 == 0 { "12 + 34" } else { "10 % 3" };
        let r = t
            .call_tool("evaluate", serde_json::json!({"expression": expr}), &format!("e-{i}"))
            .unwrap();
        assert_eq!(r.content[0].text.as_deref(), Some(if i % 2 == 0 { "46" } else { "1" }));
        t.shutdown();
    }
    assert!(
        started.elapsed() < Duration::from_secs(120),
        "execute soak bounded: {:?}",
        started.elapsed()
    );
}

/// Provider-level discovery soak: cached catalog keeps answering with
/// stable identity across repeated queries (one live discovery).
#[test]
fn release_soak_provider_catalog_stability() {
    let mut p = McpProvider::new("calc".into(), bin().into(), vec![]);
    let first = p.query(&QueryContext::parse(""));
    assert!(!first.is_empty());
    let started = Instant::now();
    for _ in 0..CYCLES {
        let again = p.query(&QueryContext::parse(""));
        assert_eq!(again.len(), first.len());
        assert_eq!(again[0].provider_id, "mcp:calc");
    }
    assert!(started.elapsed() < Duration::from_secs(10), "cached queries stay fast");
}

/// Cross-profile soak sanity: the 2026 stateless path repeats cleanly too
/// (no handshake, per-request `_meta`) — bounded subset (20 cycles).
#[test]
fn release_soak_2026_cycles() {
    let started = Instant::now();
    for i in 0..20 {
        let mut t = StdioTransport::spawn_with_profile(
            bin(),
            &["2026".to_string()],
            Duration::from_secs(5),
            McpProtocolProfile::V2026_07_28,
        )
        .unwrap();
        let tools = t.list_tools().unwrap();
        assert_eq!(tools.tools.len(), 1);
        let r = t
            .call_tool("evaluate", serde_json::json!({"expression": "1 + 1"}), &format!("e-{i}"))
            .unwrap();
        assert_eq!(r.content[0].text.as_deref(), Some("2"));
        t.shutdown();
    }
    assert!(started.elapsed() < Duration::from_secs(60));
}
