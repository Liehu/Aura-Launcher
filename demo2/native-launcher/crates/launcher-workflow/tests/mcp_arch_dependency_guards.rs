//! MVP4.3 Phase 8 architecture tests (review 42 §23):
//!
//! - MCP-ARCH-004: WorkflowRunner has no dependency on McpExecutor.
//! - MCP-ARCH-005: ReferenceResolver has no dependency on MCP transport.
//!
//! MCP-ARCH-006 (same-path execution) is proven behaviorally in
//! `launcher-core/tests/mcp_workflow_integration.rs`, since it cannot be
//! expressed in the type system alone.
//!
//! These are source-level dependency guards complementing
//! `scripts/check_topology.py`: launcher-workflow's Cargo manifest has no
//! launcher-mcp dependency (compile-time fact), and its sources contain no
//! MCP type references (the zero-MCP-branch success criterion, §24).

use std::path::Path;

/// Strip `//` line comments so doc mentions of MCP semantics (e.g. the
/// frozen `plugin.mcp.invoke` routing string) are not mistaken for code
/// dependencies; only real source tokens count.
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| l.split_once("//").map(|(code, _)| code.to_string()).unwrap_or_else(|| l.to_string()))
        .collect::<Vec<_>>()
        .join("
")
}

fn workflow_sources() -> Vec<std::path::PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).expect("workflow src exists") {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&src, &mut files);
    assert!(!files.is_empty(), "no sources found");
    files
}

/// MCP-ARCH-004: the runner never names McpExecutor (nor any MCP type).
#[test]
fn mcp_arch004_workflow_runner_no_mcp_executor_dependency() {
    for f in workflow_sources() {
        let src = strip_line_comments(&std::fs::read_to_string(&f).unwrap());
        for banned in ["McpExecutor", "launcher_mcp", "launcher-mcp", "mcp.invoke"] {
            assert!(
                !src.contains(banned),
                "{} references {banned}: WorkflowRunner must not know MCP",
                f.display()
            );
        }
    }
}

/// MCP-ARCH-005: the ReferenceResolver never touches MCP transport,
/// catalog, or executor — it only calls the Provider abstraction.
#[test]
fn mcp_arch005_reference_resolver_no_mcp_transport_dependency() {
    for f in workflow_sources() {
        let src = strip_line_comments(&std::fs::read_to_string(&f).unwrap());
        for banned in ["McpTransport", "StdioTransport", "McpCatalog", "tools/call"] {
            assert!(
                !src.contains(banned),
                "{} references {banned}: ReferenceResolver must stay on the Provider abstraction",
                f.display()
            );
        }
    }
}

// ---- MVP4.3 Phase 9 (review 44 §20): AI-MCP-ARCH guards ----

fn proposal_source() -> String {
    strip_line_comments(&std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/proposal.rs"),
    ).expect("proposal.rs exists"))
}

/// AI-MCP-ARCH-001/002: the planner module has no dependency on
/// McpExecutor or McpTransport (covered crate-wide above; pinned here for
/// the planner file specifically, review 44 §21: the planner does not
/// know the executor exists).
#[test]
fn ai_mcp_arch001_002_planner_no_mcp_dependency() {
    let src = proposal_source();
    for banned in ["McpExecutor", "McpTransport", "launcher_mcp", "launcher-mcp"] {
        assert!(!src.contains(banned), "proposal.rs references {banned}");
    }
}

/// AI-MCP-ARCH-003: the planner cannot emit an Effect — `plan()` returns
/// `Vec<ActionProposal>` and the module never imports the engine's Effect
/// type; the only execution-adjacent names are step/proposal builders.
#[test]
fn ai_mcp_arch003_planner_cannot_emit_effect() {
    let src = proposal_source();
    for banned in ["launcher_action::Effect", "Effect::", "ResolvedAction {", "execute("] {
        assert!(!src.contains(banned), "proposal.rs contains {banned}");
    }
}
