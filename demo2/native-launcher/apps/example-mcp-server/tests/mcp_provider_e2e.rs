//! MVP4.3 Phase 1-4 E2E (MCP-001/005/009/010/011/026): McpProvider over a
//! real MCP stdio server fixture (mcp-calculator).

use std::path::PathBuf;

use launcher_core::providers::mcp::McpProvider;
use launcher_core::Provider;
use launcher_domain::QueryContext;

fn provider() -> McpProvider {
    McpProvider::new(
        "calc".into(),
        env!("CARGO_BIN_EXE_mcp-calculator").into(),
        vec![],
    )
}

/// MCP-001/005: empty-query discovery returns published tools as Commands.
#[test]
fn mcp005_empty_query_discovery() {
    let mut p = provider();
    let cmds = p.query(&QueryContext::parse(""));
    assert!(!cmds.is_empty());
    assert!(cmds.iter().any(|c| c.id == "evaluate"));
}

/// MCP-009/010/011: host-projected route identity.
#[test]
fn mcp009_route_identity() {
    let mut p = provider();
    let cmds = p.query(&QueryContext::parse(""));
    let c = &cmds[0];
    assert_eq!(c.provider_id, "mcp:calc");
    assert_eq!(c.actions[0].id.as_deref(), Some("invoke"));
}

/// Since Phase 5, projected invoke actions are Ready under the default
/// host grant: the provider no longer disables execution itself (review 41
/// §5.5) — availability is the resolver's decision via `mcp.invoke`.
#[test]
fn invoke_action_ready_with_host_grant() {
    let mut p = provider();
    let cmds = p.query(&QueryContext::parse(""));
    let a = &cmds[0].actions[0];
    assert_eq!(a.kind, launcher_domain::ActionKind::PluginInvoke);
    assert!(a.disabled_reason.is_none());
    assert!(launcher_action::validate(a).is_ok());
}

/// Non-empty queries filter the catalog by title/name substring.
#[test]
fn mcp_catalog_text_filtering() {
    let mut p = provider();
    let hits = p.query(&QueryContext::parse("evaluate"));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "evaluate");
    let misses = p.query(&QueryContext::parse("zzz-nothing"));
    assert!(misses.is_empty());
}

/// MCP-026: a non-existent server surfaces PluginUnavailable via
/// take_last_error instead of silently returning empty results
/// (UI-CONTRACT section 3.1 Main.Error).
#[test]
fn mcp026_unavailable_server_surfaces_error() {
    let mut p = McpProvider::new(
        "ghost".into(),
        "definitely-not-a-real-mcp-server.exe".into(),
        vec![],
    );
    let cmds = p.query(&QueryContext::parse(""));
    assert!(cmds.is_empty(), "unavailable server yields no commands");
    let err = p.take_last_error().expect("error must be surfaced");
    assert!(!err.is_empty());
    assert!(p.take_last_error().is_none(), "error cleared after take");
}

/// Fixture binary exists sanity (keeps CARGO_BIN_EXE wiring honest).
#[test]
fn fixture_server_binary_exists() {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_mcp-calculator"));
    assert!(exe.exists());
}
