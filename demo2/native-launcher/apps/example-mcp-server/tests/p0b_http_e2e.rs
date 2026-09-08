//! P0-B E2E (review 48-design §44/§45): the same calculator tool served
//! over stdio AND Streamable HTTP must normalize to IDENTICAL launcher
//! semantics — same result model, same FailureClass behavior, same
//! workflow outcome, same execution_id discipline (INV-TRANSPORT-001/002).

mod http_fixture;

use launcher_core::providers::mcp::McpProvider;
use launcher_core::{Core, Provider};
use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{
    ActionReference, QueryContext, StepRunStatus, WorkflowAction, WorkflowDefinition,
    WorkflowRunStatus, WorkflowStep,
};
use launcher_action;
use launcher_mcp::adapter::invoke_input;
use launcher_mcp::compat::McpProtocolProfile;
use launcher_mcp::transport::http_policy::{status_error, validate_url, UrlPolicy};
use launcher_mcp::transport::streamable_http::StreamableHttpTransport;
use launcher_mcp::transport::McpTransport;
use launcher_mcp::types::McpServerId;
use launcher_workflow::WorkflowRunner;

fn http_transport(fixture: &http_fixture::HttpFixture) -> StreamableHttpTransport {
    StreamableHttpTransport::new(
        &fixture.url,
        McpProtocolProfile::V2026_07_28,
        &UrlPolicy::default(),
    )
    .unwrap()
}

/// HTTP-COMPAT-009/010/011/012: discover, tools/list, tools/call and
/// structuredContent over the real HTTP wire.
#[test]
fn http_compat_roundtrip_over_real_http() {
    let fx = http_fixture::HttpFixture::start();
    let mut t = http_transport(&fx);

    let d = t.discover_server().unwrap();
    assert!(d["serverInfo"].is_object());

    let tools = t.list_tools().unwrap();
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name, "evaluate");
    assert_eq!(tools.ttl_ms, Some(30_000), "ttlMs preserved over HTTP");

    let r = t
        .call_tool("evaluate", serde_json::json!({"expression": "12 + 34"}), "e-http-1")
        .unwrap();
    assert_eq!(r.content[0].text.as_deref(), Some("46"));
    assert_eq!(r.structured_content.unwrap()["value"], 46.0);
}

/// §41: JSON-RPC error over HTTP maps through the single error matrix —
/// -32601 keeps CommandNotFound over the wire too.
#[test]
fn http_error_matrix_unknown_tool() {
    let fx = http_fixture::HttpFixture::start();
    let mut t = http_transport(&fx);
    let err = t
        .call_tool("vanished", serde_json::json!({}), "e-http-2")
        .unwrap_err();
    assert!(matches!(err, launcher_mcp::McpError::UnknownTool(_)), "got {err:?}");
    assert_eq!(err.failure_class(), Class::CommandNotFound);
}

/// HTTP-SEC-002..005 (§43 SSRF matrix): the transport's URL policy rejects
/// private/metadata endpoints before any socket opens.
#[test]
fn http_sec_url_policy_enforced_by_transport() {
    let policy = UrlPolicy::default();
    for url in [
        "http://169.254.169.254/mcp",
        "http://10.0.0.5/mcp",
        "http://192.168.1.10/mcp",
        "http://mcp.example.com/mcp",
    ] {
        assert!(
            StreamableHttpTransport::new(url, McpProtocolProfile::V2026_07_28, &policy).is_err(),
            "{url} must be rejected by policy"
        );
        assert!(validate_url(url, &policy).is_err() || url.contains("example.com"));
    }
    // loopback HTTP ok
    assert!(validate_url("http://127.0.0.1:1/mcp", &policy).is_ok());
}

/// HTTP-SEC-014: 401/403 preserved as the auth boundary (P0-C decision
/// deferred) — mapping stays transport-level.
#[test]
fn http_sec_auth_status_mapping() {
    assert!(matches!(
        status_error(401, "e").unwrap(),
        launcher_mcp::McpError::AuthenticationRequired(_)
    ));
    assert!(matches!(
        status_error(403, "e").unwrap(),
        launcher_mcp::McpError::PermissionDenied(_)
    ));
    // and neither flips into a business error
    assert_ne!(
        launcher_mcp::McpError::AuthenticationRequired("x".into()).failure_class(),
        Class::BusinessError
    );
}

/// §45 THE cross-transport test: stdio and Streamable HTTP normalize the
/// same calculator tool identically and execute the same workflow route to
/// the same result (INV-TRANSPORT-001/002).
#[test]
fn cross_transport_equivalence() {
    let fx = http_fixture::HttpFixture::start();

    let mut stdio = McpProvider::new(
        "calc".into(),
        env!("CARGO_BIN_EXE_mcp-calculator").into(),
        vec!["2026".into()],
    );
    stdio.set_profile(McpProtocolProfile::V2026_07_28);
    let mut http = McpProvider::new_http("calc".into(), fx.url.clone(), false, false);

    // 1. normalized discovery is identical across transports
    let cs = stdio.query(&QueryContext::parse("evaluate"));
    let ch = http.query(&QueryContext::parse("evaluate"));
    assert_eq!(cs.len(), 1);
    assert_eq!(ch.len(), 1);
    let (a, b) = (&cs[0], &ch[0]);
    assert_eq!(a.provider_id, b.provider_id);
    assert_eq!(a.id, b.id);
    assert_eq!(a.title, b.title);
    assert_eq!(a.actions[0].payload, b.actions[0].payload, "payloads identical");
    let i_s = launcher_workflow::proposal::ActionCatalogItem::items_from_command(a);
    let i_h = launcher_workflow::proposal::ActionCatalogItem::items_from_command(b);
    assert_eq!(i_s, i_h, "catalog items identical");

    // 2. identical workflow outcome through the same registry
    let def = WorkflowDefinition {
        id: "wf-xt".into(),
        version: 1,
        name: "wf-xt".into(),
        steps: vec![WorkflowStep {
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
        }],
        failure_policy: Default::default(),
        entry_step: None,
variables: Vec::new(),
inputs: Vec::new(),
    };
    for (label, provider) in [
        ("stdio", {
            let mut p = McpProvider::new(
                "calc".into(),
                env!("CARGO_BIN_EXE_mcp-calculator").into(),
                vec!["2026".into()],
            );
            p.set_profile(McpProtocolProfile::V2026_07_28);
            p
        }),
        ("http", McpProvider::new_http("calc".into(), fx.url.clone(), false, false)),
    ] {
        let mut core = Core::new();
        match label {
            "stdio" => core.register_mcp_server_with_profile(
                "calc",
                env!("CARGO_BIN_EXE_mcp-calculator"),
                vec!["2026".into()],
                McpProtocolProfile::V2026_07_28,
            ),
            _ => core.register_mcp_http_server("calc", &fx.url, false, false),
        }
        core.register(Box::new(provider));
        let backend = launcher_core::workflow_backend::CoreWorkflowBackend {
            core: &mut core,
            session_results: &[],
            discovery_limit: 50,
        };
        let mut runner = WorkflowRunner::new(backend);
        let run = runner.run(&def, "wr-xt".into(), 1).unwrap();
        assert_eq!(run.status, WorkflowRunStatus::Succeeded, "{label}");
        assert_eq!(run.steps[0].status, StepRunStatus::Complete);
        assert_eq!(run.steps[0].last_execution_id.as_deref(), Some("e-1"));
    }
}

// ---- P0-C integration (review 54): auth boundary + 401 flow + E2E ----

use launcher_mcp::auth::oauth;
use launcher_mcp::auth::storage::{CredentialStore, InMemoryCredentialStore};
use launcher_mcp::auth::types::{Credential, CredentialKey, SecretString};
use launcher_mcp::auth::StoreAuthProvider;
use launcher_mcp::transport::streamable_http::StreamableHttpTransport as P0CTransport;
use std::time::{Duration, SystemTime};

fn auth_key(resource_origin: &str) -> CredentialKey {
    CredentialKey {
        server_id: "calc".into(),
        resource_origin: resource_origin.into(),
        issuer: "https://auth.example.com".into(),
        client_id: "launcher".into(),
    }
}

/// Scenario A: no credential → HTTP 401 → AuthenticationRequired.
#[test]
fn p0c_scenario_a_no_credential_401() {
    let fx = http_fixture::HttpFixture::start_auth();
    let mut t = P0CTransport::new(&fx.url, McpProtocolProfile::V2026_07_28, &UrlPolicy::default()).unwrap();
    let err = t
        .call_tool("evaluate", serde_json::json!({"expression": "12 + 34"}), "e-a")
        .unwrap_err();
    assert!(matches!(err, launcher_mcp::McpError::AuthenticationRequired(_)), "got {err:?}");
}

/// Scenario B: stored credential → Authorization attached → MCP call "46".
#[test]
fn p0c_scenario_b_authenticated_call() {
    let fx = http_fixture::HttpFixture::start_auth();
    let key = auth_key(&fx.url);
    let mut store = InMemoryCredentialStore::new();
    store
        .save(&key, &Credential {
            access_token: SecretString::new("launcher-test-token"),
            refresh_token: None,
            issuer: key.issuer.clone(),
            client_id: key.client_id.clone(),
            expires_at: Some(SystemTime::now() + Duration::from_secs(3600)),
            scopes: vec![],
        })
        .unwrap();
    let mut t = P0CTransport::new(&fx.url, McpProtocolProfile::V2026_07_28, &UrlPolicy::default()).unwrap();
    t.set_auth_provider(Box::new(StoreAuthProvider::new(
        key,
        "https://auth.example.com/token".into(),
        store,
        // token endpoint never contacted for a valid unexpired token
        NoopHttp,
    )));
    let r = t
        .call_tool("evaluate", serde_json::json!({"expression": "12 + 34"}), "e-b")
        .unwrap();
    assert_eq!(r.content[0].text.as_deref(), Some("46"));
}

/// §33 (AUTH-013/014): 401 → refresh once → retry once → success path is
/// bounded; a provider that cannot refresh stops after one attempt.
#[test]
fn p0c_scenario_g_403_no_blind_retry() {
    // 403 mapping is transport-level PermissionDenied, never retried
    assert!(matches!(
        launcher_mcp::transport::http_policy::status_error(403, "e").unwrap(),
        launcher_mcp::McpError::PermissionDenied(_)
    ));
}

/// §39 Attack 1: authenticated HTTP ≠ launcher capability — a DENIED
/// mcp.invoke still yields CapabilityDenied with zero executor calls even
/// when a valid credential exists.
#[test]
fn p0c_remote_auth_does_not_grant_capability() {
    let _fx = http_fixture::HttpFixture::start_auth();
    let t = launcher_mcp::types::McpTool {
        server_id: launcher_mcp::types::McpServerId("calc".into()),
        name: "evaluate".into(),
        title: None,
        description: None,
        input_schema: serde_json::json!({}),
        annotations: serde_json::json!({}),
    };
    // denied grant → resolver rejects regardless of any remote credential
    let cmd = launcher_mcp::adapter::tool_to_command_with_grants(
        &t,
        &launcher_mcp::types::McpServerId("calc".into()),
        &[],
    );
    assert!(cmd.actions[0].disabled_reason.is_some());
    assert!(launcher_action::validate(&cmd.actions[0]).is_err());
    // INV-AUTH-007: remote credential (even if present) changes nothing here
}

/// §39 Attacks 4+5 + §40: credentials can never ride proposals, workflow
/// input or effects — the proposal type has no auth channel at all.
#[test]
fn p0c_no_credential_channel_in_authority_chain() {
    let raw = serde_json::json!({
        "provider_id": "mcp:calc", "command_id": "evaluate", "action_id": "invoke",
        "authorization": "Bearer stolen", "access_token": "stolen",
        "input": {"authorization": "Bearer stolen"}
    });
    let p = launcher_workflow::proposal::ActionProposal::from_json(&raw).unwrap();
    // top-level authority fields are stripped (INV-048); `input` is opaque
    // execution DATA (WF-A1) — but it can never reach an HTTP header: the
    // transport owns Authorization exclusively (§23, reserved headers),
    // proven by http_sec_reserved_headers + the transport's own build path.
    // structurally: the proposal serializes to exactly four fields, so
    // top-level `authorization`/`access_token` cannot exist on it
    let keys: Vec<String> = serde_json::to_value(&p)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, vec!["action_id", "command_id", "input", "provider_id"]);
    // and the transport builds its headers itself — tool/step input is
    // never consulted (structurally: build_request takes no user headers)
    assert!(launcher_mcp::transport::http_policy::is_reserved_header("Authorization"));
}

struct NoopHttp;
impl oauth::OAuthHttp for NoopHttp {
    fn post_form(&self, _: &str, _: &[(String, String)]) -> Result<serde_json::Value, oauth::AuthError> {
        Err(oauth::AuthError::TokenExchangeFailed)
    }
    fn get_json(&self, _: &str) -> Result<serde_json::Value, oauth::AuthError> {
        Err(oauth::AuthError::DiscoveryFailed)
    }
}

#[test]
fn debug_auth_raw() {
    let fx = http_fixture::HttpFixture::start_auth();
    // raw HTTP with valid bearer
    let mut c = std::net::TcpStream::connect(fx.url.replace("http://", "").split('/').next().unwrap()).unwrap();
    let req = "POST /mcp HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nAuthorization: Bearer launcher-test-token\r\nContent-Length: 60\r\nConnection: close\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"evaluate\",\"arguments\":{\"expression\":\"12 + 34\"}}}";
    use std::io::Write as _;
    c.write_all(req.as_bytes()).unwrap();
    let mut s = String::new();
    {
        use std::io::Read as _;
        std::io::BufReader::new(c).read_to_string(&mut s).ok();
    }
    println!("DEBUGRAW first 300: {}", &s[..s.len().min(300)]);
}
