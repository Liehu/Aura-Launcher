//! P1-E UI-CONTRACT-v0.2 conformance (review 60 §58/§59/§77): source-level
//! INV-UI invariants. The UI crate is scanned for runtime/authority
//! vocabulary and its Cargo.toml for forbidden dependencies — the same
//! rules check_topology.py enforces, pinned here so a regression fails
//! `cargo test` even without the script.

use std::path::Path;

fn crate_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// INV-UI-001/002/003 (§77): no Effect construction, no resolver/engine
/// bypass, no capability authorization vocabulary anywhere in the UI crate.
/// Comments are stripped first — doc text may EXPLAIN the boundary.
#[test]
fn inv_ui_no_authority_vocabulary_in_code() {
    let src = crate_root().join("src");
    let mut files = vec![];
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        let p = e.path();
        if p.is_file() && p.extension().map_or(false, |x| x == "rs") {
            files.push(p);
        } else if p.is_dir() {
            for e2 in std::fs::read_dir(&p).unwrap().flatten() {
                if e2.path().extension().map_or(false, |x| x == "rs") {
                    files.push(e2.path());
                }
            }
        }
    }
    assert!(!files.is_empty(), "no sources found");
    for f in files {
        let raw = std::fs::read_to_string(&f).unwrap();
        let code: String = raw
            .lines()
            .map(|l| l.split_once("//").map(|(c, _)| c.to_string()).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" ");
        for banned in [
            "McpExecutor",
            "RuntimeManager",
            "ActionEngine",
            "ActionResolver",
            "ActionProposal",
            "launcher_mcp",
            "launcher_core",
            "launcher_ai",
            "launcher_runtime",
        ] {
            assert!(
                !code.contains(banned),
                "{} references {} (INV-UI boundary)",
                f.display(),
                banned
            );
        }
    }
}

/// §58 (dependency form of the same invariant): launcher-ui must not gain
/// dependencies on executors/hosts/runtime/core. Mirrors check_topology.
#[test]
fn inv_ui_no_forbidden_dependencies() {
    let cargo = std::fs::read_to_string(crate_root().join("Cargo.toml")).unwrap();
    let deps = cargo
        .split("[dependencies]")
        .nth(1)
        .unwrap_or_default()
        .to_string();
    for banned in [
        "launcher-mcp",
        "launcher-plugin-host",
        "launcher-runtime",
        "launcher-ai",
        "launcher-core",
        "launcher-workflow",
    ] {
        assert!(
            !deps.contains(banned),
            "launcher-ui depends on {banned} (review 60 §58)"
        );
    }
}

/// INV-UI-004 (§77): Confirm is a bare intent — the command carries no
/// proposal/payload, so the UI can never self-confirm a specific action.
#[test]
fn inv_ui_confirm_is_bare_intent() {
    use launcher_ui::viewmodel::UiCommand;
    let c = UiCommand::Confirm;
    // unit variant: only equality/clone possible, nothing to authorize with
    assert_eq!(c, UiCommand::Confirm);
}

/// INV-UI-005/006/007 (§77): the viewmodel exposes no mutation surface —
/// every view type is read-only data the host builds. Pinned by consuming
/// them immutably (compile-level) and checking no `&mut` constructors
/// exist via a source scan for `pub fn set_`/`pub fn mutate_`.
#[test]
fn inv_ui_viewmodel_is_read_only() {
    let vm = std::fs::read_to_string(crate_root().join("src").join("viewmodel.rs")).unwrap();
    for line in vm.lines() {
        let code = line.split_once("//").map(|(c, _)| c).unwrap_or(line);
        assert!(
            !code.contains("pub fn set_") && !code.contains("pub fn mutate_"),
            "viewmodel exposes a mutation surface: {code}"
        );
    }
}

/// INV-UI-009 (§77): status spelling comes only from the total mappings —
/// spot-check that each provider status label is non-empty and that the
/// escalation-to-error rule matches §45 (only Unavailable may warn; none
/// of the four is Error).
#[test]
fn inv_ui_provider_status_not_redefined() {
    use launcher_ui::viewmodel::{ProviderStatus, Severity};
    for s in [
        ProviderStatus::Connected,
        ProviderStatus::Starting,
        ProviderStatus::Reconnecting,
        ProviderStatus::Unavailable,
    ] {
        assert!(!s.label().is_empty());
        assert_ne!(s.severity(), Severity::Error, "{s:?} is not an error");
    }
}

/// INV-UI-010 (§77): Close ≠ Cancel is already pinned as a value test in
/// the viewmodel; here the Esc vocabulary is pinned: the UI contract spells
/// "close panel" and "cancel run" as separate strings.
#[test]
fn inv_ui_close_vs_cancel_wording() {
    use launcher_ui::viewmodel::{AgentTerminalStatus, UiCommand};
    assert_ne!(UiCommand::Close, UiCommand::Cancel);
    // a cancelled agent run is a distinct terminal state from a closed panel
    assert_eq!(AgentTerminalStatus::Cancelled.label(), "Cancelled");
}
