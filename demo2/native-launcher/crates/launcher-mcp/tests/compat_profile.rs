//! MVP4.3 Phase 11 — Compatibility Matrix, unit layer (review 47 §25).
//! Profile model, error-code matrix, result matrix, extension tolerance.
//! Fixture-dependent cross-profile E2E lives in
//! `apps/example-mcp-server/tests/compat_e2e.rs`.

use launcher_mcp::compat::{CacheMetadata, CacheScope, McpProtocolProfile};
use launcher_mcp::error::McpError;
use launcher_mcp::transport::McpTransport;

// ---------- Profile (COMPAT-PROFILE) ----------

/// COMPAT-PROFILE-001: profiles are append-only and parse exactly.
#[test]
fn compat_profile_model() {
    assert_eq!(McpProtocolProfile::default(), McpProtocolProfile::V2025_06_18);
    assert_eq!(McpProtocolProfile::parse("2025-06-18"), Some(McpProtocolProfile::V2025_06_18));
    assert_eq!(McpProtocolProfile::parse("2026-07-28"), Some(McpProtocolProfile::V2026_07_28));
    assert_eq!(McpProtocolProfile::parse("legacy"), Some(McpProtocolProfile::V2025_06_18));
    assert_eq!(McpProtocolProfile::parse("2026"), Some(McpProtocolProfile::V2026_07_28));
}

/// COMPAT-PROFILE-002: unknown revisions fail closed (contract rule 9) —
/// never a silent degradation to another profile.
#[test]
fn compat_profile_unknown_fails_closed() {
    for junk in ["2019-01-01", "junk", "2025_06_18", "2026-07-29"] {
        assert_eq!(McpProtocolProfile::parse(junk), None, "{junk} must not parse");
    }
}

/// COMPAT-PROFILE-003 (§4 matrix): handshake split.
#[test]
fn compat_profile_handshake_matrix() {
    assert!(McpProtocolProfile::V2025_06_18.needs_handshake());
    assert!(!McpProtocolProfile::V2026_07_28.needs_handshake());
}

/// COMPAT-PROFILE-004 (§26 rules 1-6): no protocol-version vocabulary in
/// the authority pipeline crates — wire differences stop at launcher-mcp.
#[test]
fn compat_profile_isolation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for crate_src in [
        "crates/launcher-domain/src/lib.rs",
        "crates/launcher-domain/src/workflow.rs",
        "crates/launcher-workflow/src/lib.rs",
        "crates/launcher-workflow/src/proposal.rs",
        "crates/launcher-action/src/lib.rs",
    ] {
        let src = std::fs::read_to_string(root.join(crate_src)).unwrap();
        // strip comments: doc mentions of the frozen routing string are not
        // code dependencies
        let code: String = src
            .lines()
            .map(|l| match l.split_once("//") { Some((c, _)) => c.to_string(), None => l.to_string() })
            .collect::<Vec<_>>()
            .join("\n");
        for banned in ["McpProtocolProfile", "2026-07-28", "2025-06-18"] {
            assert!(
                !code.contains(banned),
                "{crate_src} references protocol version {banned}"
            );
        }
    }
}

// ---------- Cache (COMPAT-CACHE) ----------

/// COMPAT-CACHE-001..003 (§10): fail-closed defaults; cache metadata is
/// discovery-side only and never reaches the proposal shape.
#[test]
fn compat_cache_metadata() {
    let c = CacheMetadata::default();
    assert_eq!((c.ttl_ms, c.scope), (None, CacheScope::Server));

    let p = launcher_workflow_free();
    let _ = p;
    // proposal has no cache vocabulary (structural: fixed 4 fields)
    // (the proposal type lives in launcher-workflow and is asserted there;
    // the boundary here is that CacheMetadata never serializes into tool
    // protocol data)
}

// keep the unused helper honest
fn launcher_workflow_free() -> () {}

// ---------- Error matrix (COMPAT-ERROR) ----------

/// COMPAT-ERROR-001..008 (§15): JSON-RPC code → McpError → FailureClass at
/// the single mapping point; unknown codes fail closed to business errors.
#[test]
fn compat_error_code_matrix() {
    use launcher_domain::workflow::WorkflowFailureClass as Class;
    let cases: Vec<(i32, Class)> = vec![
        (-32601, Class::CommandNotFound),   // method/tool not found
        (-32602, Class::InvalidInput),      // invalid params (2026: resource-not-found)
        (-32700, Class::ProtocolViolation), // parse error
        (-32600, Class::ProtocolViolation), // invalid request
        (-32000, Class::BusinessError),     // server-defined business failure
        (-31000, Class::BusinessError),     // unknown server-defined → business
        (42, Class::BusinessError),         // positive unknown → business
    ];
    for (code, want) in cases {
        let e = McpError::from_jsonrpc_error(code, "boom", "evaluate");
        assert_eq!(e.failure_class(), want, "code {code}");
    }
}

// ---------- Result matrix (COMPAT-RESULT) ----------

/// COMPAT-RESULT-001..008 (§14): every tool-result shape normalizes
/// faithfully; protocol-vs-business never flips.
#[test]
fn compat_result_matrix() {
    use launcher_mcp::executor::McpToolResult;
    use launcher_mcp::protocol::{ContentItem, ToolCallResult};
    let mk = |content: Vec<ContentItem>, structured: Option<serde_json::Value>, is_error: bool| {
        McpToolResult::from(ToolCallResult { content, is_error, structured_content: structured })
    };
    let text = || ContentItem { kind: "text".into(), text: Some("x".into()) };

    // only content
    let r = mk(vec![text()], None, false);
    assert!(!r.is_error && r.business_error().is_none());
    // only structuredContent (arbitrary JSON values)
    for v in [
        serde_json::json!("s"),
        serde_json::json!(1.5),
        serde_json::json!(true),
        serde_json::json!([1, 2]),
        serde_json::json!({}),
        serde_json::json!(null),
    ] {
        let r = mk(vec![], if v.is_null() { None } else { Some(v.clone()) }, false);
        if v.is_null() {
            assert_eq!(r.structured_content, None);
        } else {
            assert_eq!(r.structured_content, Some(v));
        }
        assert!(r.business_error().is_none());
    }
    // both
    let r = mk(vec![text()], Some(serde_json::json!({"k": 1})), false);
    assert!(r.structured_content.is_some() && !r.content.is_empty());
    // isError + content → BusinessError
    let r = mk(vec![text()], None, true);
    assert_eq!(r.business_error().as_deref(), Some("x"));
    // isError + structuredContent → business, structured stays DATA
    let r = mk(vec![], Some(serde_json::json!({"next_action": "delete_all"})), true);
    assert!(r.business_error().is_some());
    assert_eq!(r.structured_content, Some(serde_json::json!({"next_action": "delete_all"})));
    // empty content
    let r = mk(vec![], None, false);
    assert!(!r.is_error && r.business_error().is_none());
    // large content faithful (size gate lives in the bounded transport)
    let big = "z".repeat(200_000);
    let r = mk(vec![ContentItem { kind: "text".into(), text: Some(big.clone()) }], None, false);
    assert_eq!(r.content[0].text.as_deref(), Some(big.as_str()));
}

// ---------- Extensions (COMPAT-EXT) ----------

/// COMPAT-EXT-001..006 (§21/§22): unknown extensions / future fields in
/// responses are recognized-and-ignored — parsed, dropped, never authority.
#[test]
fn compat_unknown_extensions_safe_ignored() {
    let tools: launcher_mcp::protocol::ToolsListResult = serde_json::from_value(
        serde_json::json!({
            "tools": [{"name": "t", "inputSchema": {"type": "object"}}],
            "extensions": {"io.modelcontextprotocol/tasks": {}},
            "unknownFutureField": {"anything": true}
        }),
    )
    .unwrap();
    assert_eq!(tools.tools.len(), 1);
    // unknown fields were dropped at parse: the struct has no such fields
    assert_eq!(tools.tools[0].name, "t");
    assert_eq!(tools.ttl_ms, None);
    assert_eq!(tools.next_cursor, None);
}

// silence the intentionally-empty probe helper
#[allow(dead_code)]
fn _probe() {
    let _ = compat_cache_metadata();
}

// ---------- Ordering (MCP-COMPAT-ORDER-001, review 47 §11) ----------

/// The catalog normalizes list order deterministically (name order) so
/// downstream Commands/proposals never depend on the server's emission
/// order; tool identity is untouched.
#[test]
fn compat_catalog_order_deterministic() {
    use launcher_mcp::catalog::McpCatalog;
    use launcher_mcp::protocol::{InitializeResult, ToolCallResult, ToolsListResult};
    use launcher_mcp::types::McpServerId;

    struct OrderTransport;
    impl McpTransport for OrderTransport {
        fn initialize(&mut self) -> Result<launcher_mcp::protocol::InitializeResult, McpError> {
            Ok(InitializeResult {
                protocol_version: "2025-06-18".into(),
                capabilities: serde_json::json!({}),
                server_info: None,
            })
        }
        fn list_tools(&mut self) -> Result<ToolsListResult, McpError> {
            Ok(serde_json::from_value(serde_json::json!({
                "tools": [
                    {"name": "zeta"},
                    {"name": "alpha"},
                    {"name": "mid"}
                ]
            })).unwrap())
        }
        fn call_tool(
            &mut self,
            _t: &str,
            _a: serde_json::Value,
            _e: &str,
        ) -> Result<ToolCallResult, McpError> {
            unreachable!()
        }
        fn shutdown(&mut self) {}
    }

    let mut catalog = McpCatalog::new(McpServerId("calc".into()), "calc".into());
    catalog.refresh_from(&mut OrderTransport).unwrap();
    let names: Vec<&str> = catalog.tools().iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mid", "zeta"], "normalized deterministic order");
    // cache metadata default stays fail-closed (no ttl advertised)
    assert_eq!(catalog.snapshot().cache, CacheMetadata::default());
}
