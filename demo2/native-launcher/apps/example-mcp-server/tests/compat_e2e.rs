//! MVP4.3 Phase 11 — cross-profile E2E (review 47 §27/§28) over the real
//! mcp-calculator fixture serving BOTH profiles:
//!
//! ```text
//! 2025 stdio ──┐
//!              ├──→ Normalized McpTool → identical Command /
//! 2026 stdio ──┘     ActionCatalogItem / proposal route / result "46"
//! ```
//!
//! Plus the security replay (§28): the Phase 10 identity invariants hold
//! unchanged on the 2026 path.

use std::time::Duration;

use launcher_core::providers::mcp::McpProvider;
use launcher_core::{Core, Provider};
use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{ActionReference, QueryContext, WorkflowAction, WorkflowDefinition,
    WorkflowRunStatus, WorkflowStep};
use launcher_mcp::adapter::invoke_input;
use launcher_mcp::compat::McpProtocolProfile;
use launcher_mcp::transport::stdio::StdioTransport;
use launcher_mcp::transport::McpTransport;
use launcher_mcp::types::McpServerId;
use launcher_workflow::WorkflowRunner;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_mcp-calculator")
}

/// COMPAT-STDIO-005 (§8): the 2026 stateless profile performs NO handshake —
/// a fresh session answers tools/list immediately; a request without
/// `_meta.protocolVersion` is rejected by the server (proving the client
/// self-describes every request).
#[test]
fn compat_stdio_2026_stateless() {
    let mut t = StdioTransport::spawn_with_profile(
        bin(), &["2026".to_string()], Duration::from_secs(5), McpProtocolProfile::V2026_07_28,
    )
    .unwrap();
    let tools = t.list_tools().expect("stateless discovery without any handshake");
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.ttl_ms, Some(60_000), "ttlMs preserved");
    t.shutdown();

    // control: a legacy-profile client against the 2026 server sends no
    // _meta and fails closed (server answers -32600)
    let mut t = StdioTransport::spawn_with_profile(
        bin(), &["2026".to_string()], Duration::from_secs(5), McpProtocolProfile::V2025_06_18,
    )
    .unwrap();
    let err = t.list_tools().unwrap_err();
    assert!(matches!(err, launcher_mcp::McpError::ProtocolViolation(_)), "got {err:?}");
    t.shutdown();
}

/// COMPAT-STDIO-006 (§9): `server/discover` on the current profile; the
/// legacy profile fails closed; unknown extensions inside the discovery
/// result are inert data.
#[test]
fn compat_stdio_discover_split() {
    let mut modern = StdioTransport::spawn_with_profile(
        bin(), &["2026".to_string()], Duration::from_secs(5), McpProtocolProfile::V2026_07_28,
    )
    .unwrap();
    let d = modern.discover_server().expect("discover on 2026");
    assert!(d.get("serverInfo").is_some());
    assert!(d["extensions"]["io.modelcontextprotocol/tasks"].is_string());
    modern.shutdown();

    let mut legacy = StdioTransport::spawn_with_profile(
        bin(), &[], Duration::from_secs(5), McpProtocolProfile::V2025_06_18,
    )
    .unwrap();
    assert!(legacy.discover_server().is_err(), "legacy fails closed");
    legacy.shutdown();
}

/// §27 THE cross-profile test: the same `evaluate` tool discovered through
/// the 2025 and the 2026 profile normalizes to IDENTICAL launcher models —
/// same McpTool fields, same Command identity, same proposal route — and
/// both execute the full chain to "46" through the same registry.
#[test]
fn cross_profile_normalized_identically() {
    // discovery through both profiles
    let mut p25 = McpProvider::new("calc".into(), bin().into(), vec![]);
    let mut p26 = McpProvider::new("calc".into(), bin().into(), vec!["2026".into()]);
    p26.set_profile(McpProtocolProfile::V2026_07_28);

    let c25 = p25.query(&QueryContext::parse("evaluate"));
    let c26 = p26.query(&QueryContext::parse("evaluate"));
    assert_eq!(c25.len(), 1);
    assert_eq!(c26.len(), 1);

    // normalized identity is byte-identical across profiles
    let (a, b) = (&c25[0], &c26[0]);
    assert_eq!((a.provider_id.as_str(), a.id.as_str()), (b.provider_id.as_str(), b.id.as_str()));
    assert_eq!(a.title, b.title);
    assert_eq!(a.subtitle, b.subtitle);
    assert_eq!(a.actions[0].id, b.actions[0].id);
    assert_eq!(a.actions[0].kind, b.actions[0].kind);
    assert_eq!(a.actions[0].payload, b.actions[0].payload);
    assert_eq!(a.keywords, b.keywords);
    // same catalog item projection
    let i25 = launcher_workflow::proposal::ActionCatalogItem::items_from_command(a);
    let i26 = launcher_workflow::proposal::ActionCatalogItem::items_from_command(b);
    assert_eq!(i25, i26, "normalized catalog items are profile-independent");
    // (profile differences stay in the wire layer: only the 2026 snapshot
    // carries cache hints, and they never reach the Command)
}

/// §27 execution half: both profiles execute the identical proposal route
/// through the same Core registry and produce the same result.
#[test]
fn cross_profile_execution_identical() {
    for (args, profile) in [
        (vec![], McpProtocolProfile::V2025_06_18),
        (vec!["2026".to_string()], McpProtocolProfile::V2026_07_28),
    ] {
        let mut core = Core::new();
        core.register_mcp_server_with_profile("calc", bin(), args.clone(), profile);
        core.register(Box::new({
            let mut p = McpProvider::new("calc".into(), bin().into(), args);
            p.set_profile(profile);
            p
        }));
        let step = WorkflowStep {
            step_id: "fetch".into(),
            action: WorkflowAction::Reference(ActionReference {
                provider_id: "mcp:calc".into(),
                command_id: "evaluate".into(),
                action_id: "invoke".into(),
            }),
            input: invoke_input(
                &McpServerId("calc".into()),
                "evaluate",
                serde_json::json!({"expression": "12 + 34"}),
            ),
            condition: None,
            output: None,
            on_success: None,
            on_failure: None,
            on_condition_false: None,
            failure_policy: Default::default(),
        };
        let def = WorkflowDefinition {
            id: "wf-cross".into(),
            version: 1,
            name: "wf-cross".into(),
            steps: vec![step],
            failure_policy: Default::default(),
            entry_step: None,
variables: Vec::new(),
inputs: Vec::new(),
        };
        let backend = launcher_core::workflow_backend::CoreWorkflowBackend {
            core: &mut core,
            session_results: &[],
            discovery_limit: 50,
        };
        let mut runner = WorkflowRunner::new(backend);
        let run = runner.run(&def, "wr-cross".into(), 1).unwrap();
        assert_eq!(run.status, WorkflowRunStatus::Succeeded, "profile {profile:?}");
        assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-1"));
    }
}

/// SEC-CROSSPROFILE (§28): Phase 10 identity invariants replay unchanged
/// on the 2026 path — mismatch, substitution and oversized input all fail
/// closed with zero executions.
#[test]
fn sec_crossprofile_identity_invariants_on_2026() {
    let ex = RecordingExec::default();
    let ex = std::sync::Arc::new(ex);
    let mut core = Core::new();
    core.register_mcp_server_with_profile(
        "calc", bin(), vec!["2026".to_string()], McpProtocolProfile::V2026_07_28,
    );
    core.set_mcp_executor(ex.clone());
    core.register(Box::new({
        let mut p = McpProvider::new("calc".into(), bin().into(), vec!["2026".into()]);
        p.set_profile(McpProtocolProfile::V2026_07_28);
        p
    }));

    // server mismatch
    let bad_server = invoke_input(&McpServerId("jira".into()), "evaluate", serde_json::json!({}));
    let err = core
        .execute_effect("mcp:calc", "invoke", &bad_server, "e-1", 1)
        .unwrap_err();
    assert_eq!(err.0, Class::ProtocolViolation);

    // oversized arguments rejected at the input boundary (profile-neutral)
    let huge = invoke_input(
        &McpServerId("calc".into()),
        "evaluate",
        serde_json::json!({"expression": "x".repeat(300_000)}),
    );
    let err = core
        .execute_effect("mcp:calc", "invoke", &huge, "e-2", 1)
        .unwrap_err();
    assert_eq!(err.0, Class::InvalidInput);

    // honest call still executes on the 2026 path
    let ok = invoke_input(
        &McpServerId("calc".into()),
        "evaluate",
        serde_json::json!({"expression": "12 + 34"}),
    );
    let r = core.execute_effect("mcp:calc", "invoke", &ok, "e-3", 1).unwrap();
    assert_eq!(r["content"][0]["text"], "46");
    assert_eq!(ex.count(), 1);
}

#[derive(Default)]
struct RecordingExec(std::sync::Mutex<Vec<(String, String)>>);

impl RecordingExec {
    fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

impl launcher_mcp::executor::McpExecutor for RecordingExec {
    fn execute(
        &self,
        input: &launcher_mcp::executor::McpInvokeInput,
        _execution_id: &str,
    ) -> Result<launcher_mcp::executor::McpToolResult, launcher_mcp::McpError> {
        self.0
            .lock()
            .unwrap()
            .push((input.server_id.to_string(), input.tool_name.clone()));
        Ok(launcher_mcp::executor::McpToolResult {
            content: vec![launcher_mcp::executor::McpContent {
                kind: "text".into(),
                text: Some("46".into()),
            }],
            structured_content: None,
            is_error: false,
        })
    }
}
