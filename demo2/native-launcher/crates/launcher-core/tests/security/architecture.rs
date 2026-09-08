//! SEC-ARCH-001..006 = MCP-ARCH-007..012 (review 45 §13): negative
//! architecture guards. Each is either a source-level dependency ban or a
//! type-level impossibility demonstrated behaviorally.

use std::path::Path;

/// Source reader that strips `//` line comments: doc mentions of MCP
/// semantics must not count as dependencies — only real code tokens do.
fn src_of(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let raw = std::fs::read_to_string(root.join(rel)).expect("source exists");
    raw.lines()
        .map(|l| l.split_once("//").map(|(code, _)| code.to_string()).unwrap_or_else(|| l.to_string()))
        .collect::<Vec<_>>()
        .join("
")
}

/// MCP-ARCH-007: McpProvider cannot access McpExecutor — the provider
/// module never names the executor type.
#[test]
fn sec_arch007_provider_cannot_access_executor() {
    let src = src_of("crates/launcher-mcp/src/adapter.rs");
    let prov = src_of("crates/launcher-core/src/providers/mcp.rs");
    for banned in ["McpExecutor", "ExecutorRegistry", "execute_effect"] {
        assert!(!src.contains(banned), "adapter references {banned}");
        assert!(!prov.contains(banned), "provider references {banned}");
    }
}

/// MCP-ARCH-008: ActionPlanner cannot access the ExecutorRegistry — the
/// planner module has no registry/effect vocabulary at all.
#[test]
fn sec_arch008_planner_cannot_access_registry() {
    let src = src_of("crates/launcher-workflow/src/proposal.rs");
    for banned in ["execute_effect", "ExecutorRegistry", "McpExecutor", "Core"] {
        assert!(!src.contains(banned), "planner references {banned}");
    }
}

/// MCP-ARCH-009: McpTool metadata cannot access capability policy — the
/// MCP types have no Capability/policy vocabulary, and the projection has
/// no metadata→grant channel (proven behaviorally in SEC-META-*).
#[test]
fn sec_arch009_metadata_cannot_access_policy() {
    let types = src_of("crates/launcher-mcp/src/types.rs");
    for banned in ["Capability", "granted", "disabled_reason", "confirmation"] {
        assert!(!types.contains(banned), "McpTool types reference {banned}");
    }
}

/// MCP-ARCH-010: McpExecutor cannot mint execution_id — the correlation id
/// is a caller-supplied `&str` parameter, and the executor module contains
/// no id-minting vocabulary.
#[test]
fn sec_arch010_executor_cannot_mint_execution_id() {
    let src = src_of("crates/launcher-mcp/src/executor.rs");
    for banned in ["next_execution_id", "AtomicU64", "e-{", "format!(\"e-"] {
        assert!(!src.contains(banned), "executor module mints ids: {banned}");
    }
}

/// MCP-ARCH-011: McpExecutor cannot resolve ActionReference — the executor
/// module never references the resolver or reference types; its input is
/// the flat McpInvokeInput.
#[test]
fn sec_arch011_executor_cannot_resolve_references() {
    let src = src_of("crates/launcher-mcp/src/executor.rs");
    for banned in ["ActionReference", "ReferenceResolver", "resolve_descriptor", "ActionDescriptor"] {
        assert!(!src.contains(banned), "executor module references {banned}");
    }
}

/// MCP-ARCH-012: the UI cannot invoke the MCP executor directly — the app
/// reaches MCP only through `Core::execute_effect` (the registry); no
/// executor type or direct mcp-executor call exists in the app layer.
#[test]
fn sec_arch012_ui_cannot_invoke_executor_directly() {
    let main = src_of("apps/launcher-app/src/main.rs");
    for banned in ["McpExecutor", "execute_mcp_action", "StdioTransport", "StdMcpExecutor"] {
        assert!(!main.contains(banned), "app layer references {banned}");
    }
    assert!(main.contains("execute_effect"), "app routes through the registry");
}

// ---- P0-C (review 54): ARCH-AUTH guards ----

/// ARCH-AUTH-001: OAuth vocabulary lives ONLY in launcher-mcp — the
/// authority pipeline crates must not know OAuth/PKCE/issuer semantics.
#[test]
fn arch_auth001_oauth_vocabulary_confined() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    for rel in [
        "crates/launcher-domain/src/lib.rs",
        "crates/launcher-action/src/lib.rs",
        "crates/launcher-workflow/src/lib.rs",
        "crates/launcher-core/src/lib.rs",
        "crates/launcher-runtime/src/lib.rs",
    ] {
        let raw = std::fs::read_to_string(root.join(rel)).unwrap();
        let code: String = raw
            .lines()
            .map(|l| match l.split_once("//") { Some((c, _)) => c.to_string(), None => l.to_string() })
            .collect::<Vec<_>>()
            .join("\n");
        for banned in ["OAuth", "oauth", "PKCE", "pkce", "access_token", "Bearer"] {
            assert!(!code.contains(banned), "{rel} references {banned}");
        }
    }
}

/// ARCH-AUTH-002: config.toml can never carry credentials — the config
/// crate has no credential/token/secret fields at all.
#[test]
fn arch_auth002_config_cannot_hold_credentials() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let src = std::fs::read_to_string(root.join("crates/launcher-config/src/lib.rs")).unwrap();
    let code: String = src
        .lines()
        .map(|l| match l.split_once("//") { Some((c, _)) => c.to_string(), None => l.to_string() })
        .collect::<Vec<_>>()
        .join("\n");
    for banned in [
        "token", "client_secret", "authorization", "password", "credential",
    ] {
        assert!(
            !code.to_ascii_lowercase().contains(banned),
            "config crate carries credential-like field {banned}"
        );
    }
}
