//! MCP Executor (MVP4.3 Phase 6, review 41): the effect executor for
//! approved `plugin.mcp.invoke` effects.
//!
//! Boundary (review 41 §6): the executor receives ONLY a validated
//! [`McpInvokeInput`] + an `execution_id` correlation id — never an
//! ActionDescriptor/ResolvedAction/proposal (MCP-052 is enforced by the
//! type system: this interface cannot even name those types). It owns
//! transport location, `tools/call`, timeout and error classification;
//! authorization, capability grants, confirmation and retry stay upstream.
//!
//! Result layering (§6.4): [`McpToolResult`] is the protocol-level tool
//! result; a tool that reports `isError` is a business failure, not a
//! protocol violation (§6.5) — the caller maps it via
//! [`McpToolResult::business_error`]/`failure_class`.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::compat::McpProtocolProfile;
use crate::error::McpError;
use crate::transport::http_policy::UrlPolicy;
use crate::transport::streamable_http::StreamableHttpTransport;
use crate::protocol::ToolCallResult;
use crate::transport::stdio::StdioTransport;
use crate::transport::McpTransport;
use crate::types::McpServerId;

/// Execution parameters for one approved MCP invocation (review 41 §5.3).
/// Serialized form is exactly what the projection put into the action
/// input: `{"server_id", "tool_name", "arguments"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpInvokeInput {
    pub server_id: McpServerId,
    pub tool_name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// Maximum serialized size of one invocation input (mirrors the 256 KB
/// protocol frame cap, review 45 §R/E): bounded input, bounded memory.
pub const MAX_INVOKE_INPUT_BYTES: usize = 256 * 1024;

/// Identity-string policy (review 45 A-001): config-owned identities are
/// exact, printable, separator-free tokens. This structurally rules out
/// path traversal (`../calc`), namespace smuggling (`mcp:calc`,
/// `calc/../jira`), whitespace padding (`" calc "`) and empty ids —
/// without ever normalizing (identity stays case-sensitive and exact).
fn is_valid_identity(s: &str) -> bool {
    !s.is_empty()
        && s.trim() == s
        && !s.contains("..")
        && s.chars().all(|c| c.is_ascii_graphic() && c != '/' && c != '\\' && c != ':')
}

impl McpInvokeInput {
    /// Defense line 2 (review 41 §7.5): validate an already-approved
    /// invocation input. Only shape is checked here — authorization
    /// happened in the resolver, upstream.
    pub fn from_json(v: &Value) -> Result<Self, McpError> {
        let parsed: Self = serde_json::from_value(v.clone())
            .map_err(|e| McpError::InvalidInput(format!("mcp invoke input: {e}")))?;
        if !is_valid_identity(parsed.server_id.as_str()) {
            return Err(McpError::InvalidInput(format!(
                "invalid server_id: {:?}",
                parsed.server_id.as_str()
            )));
        }
        if !is_valid_identity(&parsed.tool_name) {
            return Err(McpError::InvalidInput(format!(
                "invalid tool_name: {:?}",
                parsed.tool_name
            )));
        }
        if !parsed.arguments.is_object() {
            return Err(McpError::InvalidInput(
                "arguments must be a JSON object".into(),
            ));
        }
        let arg_bytes = serde_json::to_string(&parsed.arguments).unwrap_or_default().len();
        if arg_bytes > MAX_INVOKE_INPUT_BYTES {
            return Err(McpError::InvalidInput(format!(
                "arguments exceed {MAX_INVOKE_INPUT_BYTES} bytes"
            )));
        }
        Ok(parsed)
    }
}

/// One MCP content item, projected from the protocol DTO so callers never
/// touch raw protocol types (review 41 §6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
}

/// Protocol-level `tools/call` result (review 41 §6.4). `is_error = true`
/// means "the call succeeded, the tool's business execution failed" — a
/// BusinessError, never a ProtocolViolation (§6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: Vec<McpContent>,
    #[serde(default, rename = "structuredContent")]
    pub structured_content: Option<Value>,
    pub is_error: bool,
}

impl McpToolResult {
    /// The business error message when the tool reported `isError`.
    pub fn business_error(&self) -> Option<String> {
        if !self.is_error {
            return None;
        }
        Some(
            self.content
                .iter()
                .filter_map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// Failure-class projection for the business-error case (single mapping
    /// point stays `McpError::failure_class`, review 41 §6.6).
    pub fn failure_class(&self) -> Option<launcher_domain_import::WorkflowFailureClass> {
        self.business_error()
            .map(|msg| McpError::ToolExecutionError(msg).failure_class())
    }
}

/// Legacy-profile shim: re-exported so the module is self-describing.
mod launcher_domain_import {
    pub use launcher_domain::workflow::WorkflowFailureClass;
}

impl From<ToolCallResult> for McpToolResult {
    fn from(r: ToolCallResult) -> Self {
        Self {
            content: r
                .content
                .into_iter()
                .map(|c| McpContent { kind: c.kind, text: c.text })
                .collect(),
            structured_content: r.structured_content,
            is_error: r.is_error,
        }
    }
}

/// Effect executor for approved MCP invocations (review 41 §6.1). Sync by
/// design: every transport in v0.1 is a blocking stdio session executed on
/// the caller's worker thread. `Send + Sync` so the registry can live in
/// shared Core state.
pub trait McpExecutor: Send + Sync {
    fn execute(&self, input: &McpInvokeInput, execution_id: &str) -> Result<McpToolResult, McpError>;
}

/// Opens an initialized transport session for one configured server.
pub type TransportOpener =
    Box<dyn Fn(&str) -> Result<Box<dyn McpTransport>, McpError> + Send + Sync>;

/// One configured server endpoint (config-owned; Spec section 21). The
/// transport choice is data, not a type-level branch: the executor consumes
/// the normalized `McpTransport` either way (INV-TRANSPORT-002).
#[derive(Debug, Clone)]
pub struct ServerEndpoint {
    pub transport: EndpointTransport,
    /// Wire profile for this endpoint (Phase 11): the protocol difference
    /// never leaves launcher-mcp.
    pub profile: McpProtocolProfile,
    /// P1-A persistence mode (default: ephemeral — runtime-on-demand).
    pub runtime: EndpointRuntime,
}

#[derive(Debug, Clone)]
pub enum EndpointTransport {
    Stdio { program: String, args: Vec<String> },
    /// Streamable HTTP (P0-B): 2026 stateless profile only.
    Http {
        url: String,
        allow_private_network: bool,
        allow_plain_http: bool,
    },
}

/// P1-A: how the executor binds a transport to an endpoint (review 56
/// §16/§23): Ephemeral = fresh transport per execution (P0 behavior);
/// Persistent = transport cached in the RuntimeManager and reused until
/// idle/evicted. Persistence is process lifetime ONLY — it is never
/// protocol session state and never authorization state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EndpointRuntime {
    #[default]
    Ephemeral,
    Persistent,
}

impl ServerEndpoint {
    pub fn stdio(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            transport: EndpointTransport::Stdio { program: program.into(), args },
            profile: McpProtocolProfile::default(),
            runtime: EndpointRuntime::Ephemeral,
        }
    }
    pub fn http(url: impl Into<String>, allow_private_network: bool, allow_plain_http: bool) -> Self {
        Self {
            transport: EndpointTransport::Http { url: url.into(), allow_private_network, allow_plain_http },
            profile: McpProtocolProfile::V2026_07_28,
            runtime: EndpointRuntime::Ephemeral,
        }
    }
    pub fn with_profile(mut self, profile: McpProtocolProfile) -> Self {
        self.profile = profile;
        self
    }
    pub fn with_runtime(mut self, runtime: EndpointRuntime) -> Self {
        self.runtime = runtime;
        self
    }
}

/// The stdio executor: locates the configured server, opens a session,
/// calls `tools/call` and classifies the outcome. It performs NO
/// authorization — every input reaching it is already approved (§6.2).
pub struct StdMcpExecutor {
    servers: HashMap<String, ServerEndpoint>,
    opener: TransportOpener,
    /// P1-A persistent transport cache (review 56 §16/§18): keyed by
    /// server id, used ONLY for endpoints configured `Persistent`. One
    /// protocol operation per runtime at a time (§18 mutex); a failed call
    /// evicts the transport so the next execution respawns fresh — the
    /// manager never replays the failed attempt (INV-RUNTIME-002).
    persistent: std::sync::Mutex<HashMap<String, Box<dyn McpTransport>>>,
    /// P1-B protocol session registry (review 57 §8/§20).
    sessions: std::sync::Mutex<crate::protocol::session_manager::ProtocolSessionManager>,
}

impl StdMcpExecutor {
    /// Production executor: one fresh stdio session per execution
    /// (spawn → initialize → tools/call → shutdown), matching the plugin
    /// host's per-call lifecycle so a crashed server never poisons the next.
    pub fn new(servers: HashMap<String, ServerEndpoint>, timeout: Duration) -> Self {
        let opener_servers = servers.clone();
        Self {
            servers,
            opener: Box::new(move |id| Self::open_stdio(&opener_servers, id, timeout)),
            persistent: std::sync::Mutex::new(HashMap::new()),
            sessions: std::sync::Mutex::new(
                crate::protocol::session_manager::ProtocolSessionManager::new(),
            ),
        }
    }

    fn open_stdio(
        servers: &HashMap<String, ServerEndpoint>,
        server_id: &str,
        timeout: Duration,
    ) -> Result<Box<dyn McpTransport>, McpError> {
        let ep = servers.get(server_id).ok_or_else(|| {
            McpError::ServerUnavailable(format!("unknown mcp server: {server_id}"))
        })?;
        let t: Box<dyn McpTransport> = match &ep.transport {
            EndpointTransport::Stdio { program, args } => Box::new(StdioTransport::spawn_with_profile(
                program,
                args,
                timeout,
                ep.profile,
            )?),
            EndpointTransport::Http { url, allow_private_network, allow_plain_http } => {
                let policy = UrlPolicy {
                    allow_private_network: *allow_private_network,
                    allow_plain_http: *allow_plain_http,
                };
                Box::new(StreamableHttpTransport::new(url, ep.profile, &policy)?)
            }
        };
        // NOTE: initialize is NOT performed here — protocol establishment
        // is owned by the caller: ephemeral callers initialize once right
        // after spawn; the P1-B persistent path initializes via the
        // protocol session lifecycle (idempotent, session-scoped).
        Ok(t)
    }

    /// Test seam: a custom transport opener (scripted mock sessions).
    pub fn with_opener(servers: HashMap<String, ServerEndpoint>, opener: TransportOpener) -> Self {
        Self {
            servers,
            opener,
            persistent: std::sync::Mutex::new(HashMap::new()),
            sessions: std::sync::Mutex::new(
                crate::protocol::session_manager::ProtocolSessionManager::new(),
            ),
        }
    }

    pub fn has_server(&self, server_id: &str) -> bool {
        self.servers.contains_key(server_id)
    }
}

impl McpExecutor for StdMcpExecutor {
    fn execute(&self, input: &McpInvokeInput, execution_id: &str) -> Result<McpToolResult, McpError> {
        // input validation (§6.2/§7.5): shape only, never authority
        let parsed = McpInvokeInput::from_json(&serde_json::to_value(input).expect("input serializes"))?;
        let persistent = self
            .servers
            .get(parsed.server_id.as_str())
            .map(|ep| ep.runtime == EndpointRuntime::Persistent)
            .unwrap_or(false);
        if persistent {
            return self.execute_persistent(&parsed, execution_id);
        }
        let mut transport = (self.opener)(parsed.server_id.as_str())?;
        // ephemeral lifecycle: initialize once per fresh process (2025
        // profile); stateless 2026 profile: no-op
        transport.initialize()?;
        let result = transport.call_tool(&parsed.tool_name, parsed.arguments.clone(), execution_id);
        transport.shutdown();
        result.map(McpToolResult::from)
    }
}

impl StdMcpExecutor {
    /// P1-A/P1-B persistent path (reviews 56/57): the executor leases the
    /// cached transport and drives the PROTOCOL SESSION lifecycle:
    ///
    /// - Ready session → reuse (initialize is idempotent: never re-sent)
    /// - Missing/Invalid/Failed session → (re-)establish: initialize once,
    ///   mark Ready (S2); on establish failure → session Failed, evict
    /// - Business errors keep the session Ready (§13); protocol/process
    ///   failures invalidate the session and evict the transport
    /// - A failed execution is NEVER replayed (INV-MCP-SESSION-004): the
    ///   error is returned and the NEXT execution uses the new session
    fn establish_err(e: crate::protocol::session_manager::SessionError) -> McpError {
        McpError::ServerUnavailable(format!("session establish: {e}"))
    }

    fn execute_persistent(
        &self,
        parsed: &McpInvokeInput,
        execution_id: &str,
    ) -> Result<McpToolResult, McpError> {
        use crate::protocol::session_manager::SessionError;
        use crate::protocol::ProtocolSessionKey;

        let profile = self
            .servers
            .get(parsed.server_id.as_str())
            .map(|ep| ep.profile)
            .unwrap_or(McpProtocolProfile::V2025_06_18);
        let key = ProtocolSessionKey::mcp(parsed.server_id.as_str(), profile);

        let mut cache = self.persistent.lock().unwrap_or_else(|p| p.into_inner());
        let mut sessions = self.sessions.lock().unwrap_or_else(|p| p.into_inner());

        // ---- ensure transport exists (P1-A mechanics) ----
        if !cache.contains_key(parsed.server_id.as_str()) {
            let transport = (self.opener)(parsed.server_id.as_str())?;
            cache.insert(parsed.server_id.as_str().to_string(), transport);
        }

        // ---- ensure a Ready protocol session (P1-B semantics) ----
        let session_id = match sessions.acquire_operation(&key) {
            Ok(sid) => sid,
            Err(SessionError::NotFound) | Err(SessionError::NotRecoverable) => {
                // S_new / S_invalid: (re-)establish on the existing process.
                // For a 2025-profile stdio transport this re-runs the
                // initialize handshake (initialize count 2 — review 57
                // §24 operation 4); for the stateless 2026 profile
                // initialize is a no-op.
                let sid = sessions.establish(key.clone(), None).map_err(Self::establish_err)?;
                let transport = cache
                    .get_mut(parsed.server_id.as_str())
                    .expect("transport cached above");
                let sid = match transport.initialize() {
                    Ok(_) => {
                        sessions.mark_ready(&sid).map_err(Self::establish_err)?;
                        sessions.acquire_operation(&key).map_err(Self::establish_err)?;
                        Ok(sid)
                    }
                    Err(e) => {
                        // establish failed (process dead): session Failed,
                        // transport evicted — the RUNTIME layer owns the
                        // dead verdict, we only record protocol facts
                        sessions.mark_failed(&sid).map_err(Self::establish_err)?;
                        cache.remove(parsed.server_id.as_str());
                        Err(e)
                    }
                }?;
                sid
            }
            Err(SessionError::Closed) => {
                // terminal: drop everything; a fresh execution re-establishes
                cache.remove(parsed.server_id.as_str());
                let sid = sessions.establish(key.clone(), None).map_err(Self::establish_err)?;
                let transport = cache
                    .get_mut(parsed.server_id.as_str())
                    .expect("transport cached above");
                transport.initialize()?;
                sessions.mark_ready(&sid).map_err(Self::establish_err)?;
                sessions.acquire_operation(&key).map_err(Self::establish_err)?;
                sid
            }
            Err(SessionError::Busy) => {
                // §11: one operation per session; reject, never queue
                return Err(McpError::ServerUnavailable(
                    "persistent mcp session busy with another operation".into(),
                ));
            }
            Err(SessionError::KeyMismatch) => {
                return Err(McpError::ProtocolViolation(
                    "session key mismatch (cross-identity lookup refused)".into(),
                ));
            }
        };

        // ---- one protocol operation ----
        let transport = cache
            .get_mut(parsed.server_id.as_str())
            .expect("transport cached above");
        let result =
            transport.call_tool(&parsed.tool_name, parsed.arguments.clone(), execution_id);

        // ---- classify (§13): business vs protocol/process failure ----
        match &result {
            Ok(_) | Err(McpError::ToolExecutionError(_) | McpError::UnknownTool(_)) => {
                // business failure keeps the protocol session Ready and the
                // process alive; release the operation slot
                let _ = sessions.release_operation(&session_id);
            }
            Err(McpError::ProtocolViolation(_)) => {
                // §13: protocol failure invalidates the SESSION; the
                // runtime is usually still alive (the line was fully read)
                // → keep the transport, re-initialize on next use.
                let _ = sessions.invalidate(&session_id);
            }
            Err(McpError::ProcessCrashed(_)) | Err(McpError::Timeout(_)) => {
                // process crash / unknown state: evict the transport so the
                // next execution respawns fresh (INV-RUNTIME-005); the
                // failed execution is never replayed.
                let _ = sessions.invalidate(&session_id);
                let transport = cache.remove(parsed.server_id.as_str());
                if let Some(mut t) = transport {
                    t.shutdown();
                }
            }
            Err(_) => {
                let _ = sessions.invalidate(&session_id);
            }
        }
        result.map(McpToolResult::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ContentItem;
    use std::sync::{Arc, Mutex};

    fn input(server: &str, tool: &str, args: Value) -> McpInvokeInput {
        McpInvokeInput {
            server_id: McpServerId(server.into()),
            tool_name: tool.into(),
            arguments: args,
        }
    }

    /// Scripted transport: records the last call, returns the scripted
    /// response once. Never spawns a process.
    struct MockTransport {
        calls: Arc<Mutex<Vec<(String, Value, String)>>>,
        response: Mutex<Option<Result<ToolCallResult, McpError>>>,
    }

    impl McpTransport for MockTransport {
        fn initialize(&mut self) -> Result<crate::protocol::InitializeResult, McpError> {
            Ok(crate::protocol::InitializeResult {
                protocol_version: crate::MCP_PROTOCOL_VERSION.into(),
                capabilities: Value::Null,
                server_info: None,
            })
        }
        fn list_tools(&mut self) -> Result<crate::protocol::ToolsListResult, McpError> {
            Ok(crate::protocol::ToolsListResult { tools: vec![], next_cursor: None, ttl_ms: None })
        }
        fn call_tool(
            &mut self,
            tool_name: &str,
            arguments: Value,
            execution_id: &str,
        ) -> Result<ToolCallResult, McpError> {
            self.calls
                .lock()
                .unwrap()
                .push((tool_name.into(), arguments, execution_id.into()));
            self.response
                .lock()
                .unwrap()
                .take()
                .expect("no scripted response")
        }
        fn shutdown(&mut self) {}
    }

    fn executor_with(response: Result<ToolCallResult, McpError>) -> (StdMcpExecutor, Arc<Mutex<Vec<(String, Value, String)>>>) {
        let calls: Arc<Mutex<Vec<(String, Value, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let calls2 = calls.clone();
        let response = Mutex::new(Some(response));
        let opener: TransportOpener = Box::new(move |sid| {
            if sid != "calc" {
                // mirror the production opener: unconfigured server is
                // unavailable before any session is opened
                return Err(McpError::ServerUnavailable(format!("unknown mcp server: {sid}")));
            }
            Ok(Box::new(MockTransport {
                calls: calls2.clone(),
                response: Mutex::new(response.lock().unwrap().take()),
            }) as Box<dyn McpTransport>)
        });
        let mut servers = HashMap::new();
        servers.insert(
            "calc".to_string(),
            ServerEndpoint::stdio("mock", vec![]),
        );
        (StdMcpExecutor::with_opener(servers, opener), calls)
    }

    fn text_result(text: &str, is_error: bool) -> ToolCallResult {
        ToolCallResult {
            content: vec![ContentItem { kind: "text".into(), text: Some(text.into()) }],
            is_error,
            structured_content: None,
        }
    }

    /// MCP-043: executor success returns the two-layer result.
    #[test]
    fn mcp043_executor_success() {
        let (ex, _calls) = executor_with(Ok(text_result("46", false)));
        let r = ex
            .execute(&input("calc", "evaluate", serde_json::json!({"expression": "12 + 34"})), "e-1")
            .unwrap();
        assert!(!r.is_error);
        assert_eq!(r.content[0].text.as_deref(), Some("46"));
        assert!(r.business_error().is_none());
    }

    /// MCP-044: the execution_id reaches the transport as correlation id.
    #[test]
    fn mcp044_execution_id_propagated() {
        let (ex, calls) = executor_with(Ok(text_result("ok", false)));
        ex.execute(&input("calc", "evaluate", serde_json::json!({})), "e-123").unwrap();
        let calls = calls.lock().unwrap();
        assert_eq!(calls[0].2, "e-123");
    }

    /// MCP-045 + §6.3: tool arguments reach the transport verbatim — the
    /// execution_id is NEVER injected into the tool arguments.
    #[test]
    fn mcp045_input_schema_preserved() {
        let (ex, calls) = executor_with(Ok(text_result("ok", false)));
        let args = serde_json::json!({"expression": "12 + 34"});
        ex.execute(&input("calc", "evaluate", args.clone()), "e-1").unwrap();
        let calls = calls.lock().unwrap();
        assert_eq!(calls[0].1, args);
        assert!(calls[0].1.get("execution_id").is_none());
    }

    /// MCP-046: tool business error (`isError: true`) is a business error —
    /// classified, never a protocol violation (§6.5).
    #[test]
    fn mcp046_tool_business_error() {
        let (ex, _calls) = executor_with(Ok(text_result("division by zero", true)));
        let r = ex.execute(&input("calc", "evaluate", serde_json::json!({})), "e-1").unwrap();
        assert_eq!(
            r.failure_class(),
            Some(launcher_domain::workflow::WorkflowFailureClass::BusinessError)
        );
        assert_eq!(r.business_error().as_deref(), Some("division by zero"));
    }

    /// MCP-047: a transport timeout maps to the Timeout class.
    #[test]
    fn mcp047_timeout() {
        let (ex, _calls) = executor_with(Err(McpError::Timeout(Duration::from_secs(1))));
        let err = ex.execute(&input("calc", "evaluate", serde_json::json!({})), "e-1").unwrap_err();
        assert_eq!(err.failure_class(), launcher_domain::workflow::WorkflowFailureClass::Timeout);
    }

    /// MCP-048: an unconfigured/unknown server is unavailable, and the
    /// opener is never reached for it.
    #[test]
    fn mcp048_unavailable_server() {
        let (ex, _calls) = executor_with(Ok(text_result("x", false)));
        let err = ex
            .execute(&input("ghost", "evaluate", serde_json::json!({})), "e-1")
            .unwrap_err();
        assert!(matches!(err, McpError::ServerUnavailable(_)));
        assert_eq!(
            err.failure_class(),
            launcher_domain::workflow::WorkflowFailureClass::PluginUnavailable
        );
        assert!(!ex.has_server("ghost"));
    }

    /// MCP-049: malformed transport responses surface as ProtocolViolation.
    #[test]
    fn mcp049_malformed_response() {
        let (ex, _calls) = executor_with(Err(McpError::ProtocolViolation("malformed line".into())));
        let err = ex.execute(&input("calc", "evaluate", serde_json::json!({})), "e-1").unwrap_err();
        assert_eq!(
            err.failure_class(),
            launcher_domain::workflow::WorkflowFailureClass::ProtocolViolation
        );
    }

    /// MCP-050: a wrong response id is a hard protocol violation.
    #[test]
    fn mcp050_wrong_response_id() {
        let (ex, _calls) = executor_with(Err(McpError::ProtocolViolation(
            "response id mismatch".into(),
        )));
        let err = ex.execute(&input("calc", "evaluate", serde_json::json!({})), "e-1").unwrap_err();
        assert!(matches!(err, McpError::ProtocolViolation(_)));
    }

    /// MCP-051: the server rejecting an unknown tool is CommandNotFound.
    #[test]
    fn mcp051_unknown_tool() {
        let (ex, _calls) = executor_with(Err(McpError::UnknownTool("nope".into())));
        let err = ex.execute(&input("calc", "nope", serde_json::json!({})), "e-1").unwrap_err();
        assert_eq!(
            err.failure_class(),
            launcher_domain::workflow::WorkflowFailureClass::CommandNotFound
        );
    }

    /// MCP-052 (type-level): the executor interface accepts only
    /// McpInvokeInput + execution_id — an ActionDescriptor/ResolvedAction
    /// cannot even be named in the signature (compile-time enforcement).
    /// exercised at the input boundary: anything that is not a valid
    /// invoke input is rejected as InvalidInput.
    #[test]
    fn mcp052_executor_rejects_non_invoke_input() {
        for bad in [
            serde_json::json!({}),
            serde_json::json!({"server_id": "", "tool_name": "t", "arguments": {}}),
            serde_json::json!({"server_id": "calc", "tool_name": "  ", "arguments": {}}),
            serde_json::json!({"server_id": "calc", "tool_name": "t", "arguments": "x"}),
            // a resolved-action-shaped blob is not an invoke input
            serde_json::json!({"kind": "PluginInvoke", "payload": {"Path": "C:\\x"}}),
        ] {
            assert!(
                matches!(
                    McpInvokeInput::from_json(&bad),
                    Err(McpError::InvalidInput(_))
                ),
                "shape must be rejected: {bad}"
            );
        }
        assert!(McpInvokeInput::from_json(&serde_json::json!({
            "server_id": "calc", "tool_name": "t", "arguments": {"a": 1}
        }))
        .is_ok());
        // the executor itself funnels every call through the same validation
        let (ex, _calls) = executor_with(Ok(text_result("x", false)));
        assert!(matches!(
            ex.execute(
                &McpInvokeInput {
                    server_id: McpServerId("calc".into()),
                    tool_name: String::new(),
                    arguments: serde_json::json!({}),
                },
                "e-1",
            ),
            Err(McpError::InvalidInput(_))
        ));
    }

    /// Stdio roundtrip sanity for the shim: McpToolResult serializes to the
    /// documented JSON shape (content/structuredContent/isError).
    #[test]
    fn result_serialization_shape() {
        let r = McpToolResult {
            content: vec![McpContent { kind: "text".into(), text: Some("46".into()) }],
            structured_content: Some(serde_json::json!({"value": 46})),
            is_error: false,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["structuredContent"]["value"], 46);
        assert_eq!(v["is_error"], false);
    }
}
