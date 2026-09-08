//! stdio transport (Spec section 2, REQUIRED for MCP): speaks
//! newline-delimited JSON-RPC 2.0 over the child's stdin/stdout.
//!
//! P0-A migration (reviews 52/53): process lifecycle (spawn, bounded IO,
//! timeout, kill/reap, Job Object) lives in `launcher-runtime`; this type
//! owns ONLY MCP semantics — the JSON-RPC envelope, per-method result
//! discriminators, profile handshake/self-description and the code→error
//! matrix.

use std::time::{Duration, Instant};

use launcher_runtime::{LaunchSpec, ProcessSession, RuntimeError as RtError, RuntimeLimits};

use crate::error::McpError;
use crate::protocol::{
    method, ClientInfo, InitializeParams, InitializeResult, JsonRpcRequest, JsonRpcResponse,
    ToolCallParams, ToolCallResult, ToolsListResult,
};
use crate::compat::McpProtocolProfile;
use crate::MCP_PROTOCOL_VERSION;

/// Protocol frame cap (review 45 E-008): enforced by the runtime's bounded
/// reader at the allocation boundary; re-exported here as the MCP profile
/// of the runtime limit.
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

pub struct StdioTransport {
    session: ProcessSession,
    next_id: u64,
    timeout: Duration,
    /// Wire profile: decides handshake behavior and request self-description.
    profile: McpProtocolProfile,
}

impl StdioTransport {
    /// Spawn the MCP server process (Spec section 21 config: program + args)
    /// under the legacy compatibility profile.
    pub fn spawn(program: &str, args: &[String], timeout: Duration) -> Result<Self, McpError> {
        Self::spawn_with_profile(program, args, timeout, McpProtocolProfile::V2025_06_18)
    }

    /// Spawn under an explicit profile. Profile-specific behavior is
    /// confined to handshake + request self-description (review 47 §6):
    /// framing, envelope validation and error mapping are identical.
    pub fn spawn_with_profile(
        program: &str,
        args: &[String],
        timeout: Duration,
        profile: McpProtocolProfile,
    ) -> Result<Self, McpError> {
        let spec = LaunchSpec::new(program).args(args.to_vec());
        let limits =
            RuntimeLimits::default().with_io_timeout(timeout).with_max_stdout_line_bytes(MAX_FRAME_BYTES);
        let session = ProcessSession::spawn(&spec, &limits)
            .map_err(|e| map_runtime_error(e, "server spawn"))?;
        Ok(Self { session, next_id: 0, timeout, profile })
    }

    /// Request `_meta` for the stateless profile: protocol version +
    /// client info travel with EVERY request (review 47 §8).
    fn stateless_meta(&self) -> serde_json::Value {
        serde_json::json!({
            "_meta": {
                "protocolVersion": self.profile.as_str(),
                "clientInfo": {"name": "native-launcher", "version": "0.1"}
            }
        })
    }

    /// Per-method envelope semantics (review 46 §3): a JSON-valid response
    /// is NOT automatically protocol-valid — each RPC method must present
    /// its own result discriminator, otherwise serde would silently
    /// deserialize foreign envelopes into default values. `required_any`
    /// lists the discriminators, of which the result must contain at least
    /// one. Returns the full result object on success.
    fn envelope(
        resp: JsonRpcResponse,
        ctx: &str,
        required_any: &[&str],
    ) -> Result<serde_json::Value, McpError> {
        if let Some(err) = &resp.error {
            return Err(McpError::ProtocolViolation(format!(
                "{ctx} failed: {}:{}",
                err.code, err.message
            )));
        }
        let v = resp.result.unwrap_or(serde_json::Value::Null);
        let ok = v
            .as_object()
            .map(|o| required_any.iter().any(|k| o.contains_key(*k)))
            .unwrap_or(false);
        if !ok {
            return Err(McpError::ProtocolViolation(format!(
                "{ctx}: missing result discriminator (want any of {required_any:?})"
            )));
        }
        Ok(v)
    }

    fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Send one request and wait for the response with the matching id.
    /// Notifications (no id) are skipped. One request in flight at a time.
    /// Runtime errors map to MCP errors here (review 53 §7): the runtime
    /// reports process/IO facts; this layer assigns protocol semantics.
    fn request(&mut self, req: &JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        let deadline = Instant::now() + self.timeout;
        self.session
            .write_line(req.to_line().trim_end())
            .map_err(|e| map_runtime_error(e, "server write"))?;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(McpError::Timeout(self.timeout));
            }
            let line = match self.session.read_line(remaining) {
                Ok(line) => line,
                Err(e) => return Err(map_runtime_error(e, "server response")),
            };
            let resp: JsonRpcResponse = serde_json::from_str(&line)
                .map_err(|e| McpError::ProtocolViolation(format!("malformed line: {e}")))?;
            // notifications (no id) are skipped; keep waiting
            if resp.id.is_none() && resp.method.is_some() {
                continue;
            }
            if resp.id != Some(req.id) {
                // one request in flight: a mismatched id is a hard
                // protocol violation, not a stale response
                return Err(McpError::ProtocolViolation(format!(
                    "response id mismatch: got {:?}, want {}",
                    resp.id, req.id
                )));
            }
            return Ok(resp);
        }
    }
}

/// Runtime → MCP error mapping (single point in this transport): runtime
/// facts become protocol semantics; FailureClass stays downstream.
fn map_runtime_error(e: RtError, ctx: &str) -> McpError {
    match e {
        RtError::SpawnFailed(m) => McpError::ServerUnavailable(format!("{ctx}: {m}")),
        RtError::IoTimeout(d) | RtError::StartupTimeout(d) => McpError::Timeout(d),
        RtError::ProcessExited { code } => McpError::ProcessCrashed(code),
        RtError::TransportBroken => McpError::ServerUnavailable("server closed output".into()),
        RtError::OutputLimitExceeded { stream, limit } => McpError::ProtocolViolation(format!(
            "{stream} frame exceeds {limit}-byte cap"
        )),
        RtError::ShutdownTimeout => McpError::ServerUnavailable("server shutdown timed out".into()),
        RtError::InvalidSpec(m) => McpError::ServerUnavailable(format!("{ctx}: {m}")),
    }
}

impl crate::transport::McpTransport for StdioTransport {
    /// Legacy Compatibility Profile handshake (2025-06-18): stateful
    /// initialize → initialized session. A future 2026-07-28 stateless
    /// transport replaces this method behind the same `McpTransport` trait.
    fn initialize(&mut self) -> Result<InitializeResult, McpError> {
        // Stateless profile: no handshake exists — nothing is written, and
        // the caller gets a synthesized local result (review 47 §8/§20).
        if !self.profile.needs_handshake() {
            return Ok(InitializeResult {
                protocol_version: self.profile.as_str().into(),
                capabilities: serde_json::json!({}),
                server_info: None,
            });
        }
        let req = JsonRpcRequest::new(
            self.next_id(),
            method::INITIALIZE,
            serde_json::json!(InitializeParams {
                protocol_version: MCP_PROTOCOL_VERSION,
                capabilities: serde_json::json!({}),
                client_info: ClientInfo { name: "native-launcher", version: "0.1" },
            }),
        );
        let resp = self.request(&req)?;
        // envelope semantics: an initialize answer must identify itself
        let v = Self::envelope(resp, "initialize", &["protocolVersion"])?;
        let result: InitializeResult =
            serde_json::from_value(v).map_err(|e| McpError::ProtocolViolation(e.to_string()))?;
        // version negotiation: mismatch is a hard protocol violation
        if result.protocol_version != MCP_PROTOCOL_VERSION {
            return Err(McpError::ProtocolViolation(format!(
                "unsupported MCP protocol version: {} (want {MCP_PROTOCOL_VERSION})",
                result.protocol_version
            )));
        }
        // lifecycle: notifications/initialized completes the handshake
        let note = JsonRpcRequest::new(
            self.next_id(),
            method::INITIALIZED_NOTIFICATION,
            serde_json::Value::Null,
        );
        self.session
            .write_line(note.to_line().trim_end())
            .map_err(|e| map_runtime_error(e, "initialized notification"))?;
        Ok(result)
    }

    fn list_tools(&mut self) -> Result<ToolsListResult, McpError> {
        let mut params = serde_json::json!({});
        if !self.profile.needs_handshake() {
            params["_meta"] = self.stateless_meta()["_meta"].clone();
        }
        let req = JsonRpcRequest::new(self.next_id(), method::TOOLS_LIST, params);
        let resp = self.request(&req)?;
        let v = Self::envelope(resp, "tools/list", &["tools"])?;
        serde_json::from_value(v).map_err(|e| McpError::ProtocolViolation(e.to_string()))
    }

    fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
        _execution_id: &str,
    ) -> Result<ToolCallResult, McpError> {
        let call = ToolCallParams {
            name: tool_name.to_string(),
            arguments,
        };
        let mut params = serde_json::to_value(&call).expect("tool call params serialize");
        if !self.profile.needs_handshake() {
            params["_meta"] = self.stateless_meta()["_meta"].clone();
        }
        let req = JsonRpcRequest::new(self.next_id(), method::TOOLS_CALL, params);
        let resp = self.request(&req)?;
        if let Some(err) = &resp.error {
            // single code→error mapping point (review 47 §15)
            return Err(McpError::from_jsonrpc_error(err.code, &err.message, tool_name));
        }
        // E-003 hardening (Phase 10 / review 46 §3): a tools/call answer
        // must be a tool-result envelope. Any other shape (e.g. a tools/list
        // `{"tools": ...}`) silently deserializing into an EMPTY result
        // would mask the violation — reject it instead.
        let v = Self::envelope(
            resp,
            "tools/call",
            &["content", "isError", "structuredContent"],
        )?;
        serde_json::from_value(v).map_err(|e| McpError::ProtocolViolation(e.to_string()))
    }

    /// `server/discover` — current profile only; legacy fails closed via
    /// the trait default semantics (here: explicit profile check).
    fn discover_server(&mut self) -> Result<serde_json::Value, McpError> {
        if self.profile.needs_handshake() {
            return Err(McpError::ProtocolViolation(
                "server/discover is not supported by this profile".into(),
            ));
        }
        let mut params = serde_json::json!({});
        params["_meta"] = self.stateless_meta()["_meta"].clone();
        let req = JsonRpcRequest::new(self.next_id(), "server/discover", params);
        let resp = self.request(&req)?;
        Self::envelope(resp, "server/discover", &["serverInfo", "tools", "capabilities"])
    }

    fn runtime_id(&self) -> Option<launcher_runtime::RuntimeId> {
        Some(self.session.id)
    }

    fn shutdown(&mut self) {
        // kill the whole job tree and reap (P0-A: lifecycle lives in the
        // runtime; MCP never owned the process)
        self.session.kill();
    }
}
// Drop fail-safety lives in the runtime's ProcessSession; StdioTransport
// needs no Drop of its own.
