//! Streamable HTTP transport (MVP4.4 P0-B, review 48-design §4/§8/§13):
//! MCP 2026-07-28 stateless HTTP wire compatibility for the existing
//! `McpTransport` abstraction.
//!
//! Boundary:
//! - HTTP client + TLS live ONLY here (launcher-mcp); no other crate ever
//!   depends on an HTTP library (§47).
//! - Stateless: no initialize, no session id, no sticky routing (§12);
//!   `initialize()` is a local no-op.
//! - Every request self-describes via `MCP-Protocol-Version` /
//!   `Mcp-Method` / `Mcp-Name` headers and `params._meta` (§8/§11);
//!   headers and body are derived from ONE source so they can never
//!   diverge (§9). Custom headers may never override reserved ones (§25).
//! - Bounded I/O at the allocation boundary: response body is read
//!   through a capped take (§22); the request body is size-checked before
//!   send (§23).
//! - Redirects are disabled (§19/§42) — a 3xx is a protocol violation.

use std::io::Read;
use std::time::Duration;

use crate::auth::AuthProviderT;
use crate::compat::McpProtocolProfile;
use crate::error::McpError;
use crate::protocol::{
    method, InitializeResult, JsonRpcRequest, JsonRpcResponse, ToolCallResult, ToolsListResult,
};
use crate::transport::http_policy::{
    status_error, validate_content_type, validate_url, UrlPolicy,
};
use crate::transport::McpTransport;

/// Bounded response body (review P0-B §22): enforced at the allocation
/// boundary via a capped read — a hostile server can never make the
/// transport buffer an unbounded body.
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
/// Bounded total request body (§23): arguments ≤256KB (executor-side) plus
/// the JSON envelope/meta must both fit.
pub const MAX_REQUEST_BYTES: usize = 256 * 1024;

pub struct StreamableHttpTransport {
    endpoint: String,
    url: String,
    profile: McpProtocolProfile,
    next_id: u64,
    /// Optional auth provider (P0-C). Only the transport ever sees tokens;
    /// executors/proposals/workflow/catalog never do (§22).
    auth: Option<Box<dyn AuthProviderT>>,
    /// Shared agent: TLS certificate validation ON, redirects DISABLED
    /// (§29/§19), bounded timeouts (§17). TCP reuse is the HTTP library's
    /// concern; MCP protocol sessions do not exist (§12/§18).
    agent: ureq::Agent,
}

impl StreamableHttpTransport {
    /// Create a transport for one configured endpoint URL. The URL is
    /// validated against the network policy HERE (§6/§20) — before any
    /// socket is opened.
    pub fn new(url: &str, profile: McpProtocolProfile, policy: &UrlPolicy) -> Result<Self, McpError> {
        if profile.needs_handshake() {
            // Streamable HTTP is a 2026-profile transport only
            return Err(McpError::InvalidInput(
                "streamable http requires the 2026-07-28 profile".into(),
            ));
        }
        let (_scheme, host) = validate_url(url, policy)?;
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build();
        Ok(Self {
            endpoint: host,
            url: url.to_string(),
            profile,
            next_id: 0,
            auth: None,
            agent,
        })
    }

    /// Attach an auth provider (P0-C). Only one per endpoint; tokens are
    /// injected here and never leave the transport boundary.
    pub fn set_auth_provider(&mut self, provider: Box<dyn AuthProviderT>) {
        self.auth = Some(provider);
    }

    fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Build the full 2026 request envelope from ONE source of truth:
    /// headers and body are generated together and self-checked for
    /// consistency before send (review P0-B §8/§9).
    fn build_request(
        &self,
        id: u64,
        rpc_method: &str,
        tool_name: Option<&str>,
        mut params: serde_json::Value,
    ) -> Result<(http::HeaderLine, String), McpError> {
        if !params.is_object() {
            params = serde_json::json!({});
        }
        // _meta: transport metadata only — never enters proposals/steps/effects
        params["_meta"] = serde_json::json!({
            "io.modelcontextprotocol/clientInfo": {"name": "native-launcher", "version": "4.4.0"},
            "protocolVersion": self.profile.as_str(),
        });
        let body_req = JsonRpcRequest::new(id, rpc_method, params);
        let mut body = body_req.to_line();
        body.pop(); // runtime-style trailing newline is not wanted in HTTP
        if body.len() > MAX_REQUEST_BYTES {
            return Err(McpError::InvalidInput(format!(
                "request body exceeds {MAX_REQUEST_BYTES} bytes"
            )));
        }
        // header/body consistency is enforced by construction + validated
        // before send (§9): the (method, name) pair is written once, twice.
        let header = http::HeaderLine {
            version: self.profile.as_str().to_string(),
            method: rpc_method.to_string(),
            name: tool_name.map(str::to_string),
        };
        header.check_against_body(&body)?;
        Ok((header, body))
    }

    /// POST one JSON-RPC request and return the parsed response. Bounded
    /// read; content-type validated; HTTP status mapped (§41).
    fn post(&mut self, header: &http::HeaderLine, body: &str) -> Result<JsonRpcResponse, McpError> {
        // Authorization header is OWNED by the auth provider (§23): derived
        // exclusively from the credential store, never from tool metadata,
        // tool arguments, AI proposals or workflow input.
        let auth_header = match self.auth.as_mut() {
            Some(a) => a.current_header().map_err(|e| McpError::AuthenticationRequired(e.to_string()))?,
            None => None,
        };
        let response = self
            .send_once(header, body, auth_header.as_deref())?;
        if response.status == 401 {
            // §33: refresh exactly once, retry the original request ONCE;
            // a second 401 is the AuthenticationRequired boundary
            let retry_header = match self.auth.as_mut() {
                Some(a) => a.on_unauthorized().map_err(|e| McpError::AuthenticationRequired(e.to_string()))?,
                None => None,
            };
            let Some(h) = retry_header else {
                return Err(McpError::AuthenticationRequired(format!(
                    "http 401 from {} (auth required)", self.endpoint
                )));
            };
            let retried = self.send_once(header, body, Some(&h))?;
            if retried.status == 401 {
                return Err(McpError::AuthenticationRequired(format!(
                    "http 401 from {} after refresh", self.endpoint
                )));
            }
            return Self::parse_response(retried);
        }
        Self::parse_response(response)
    }

    fn parse_response(response: http::HttpResponse) -> Result<JsonRpcResponse, McpError> {
        validate_content_type(Some(&response.content_type))?;
        // bounded body read at the allocation boundary (SS22): the take
        // caps bytes pulled from the socket, never read-then-check
        let mut bytes = Vec::with_capacity(8192);
        let mut limited = response.reader.take((MAX_RESPONSE_BYTES + 1) as u64);
        limited
            .read_to_end(&mut bytes)
            .map_err(|_| McpError::ProtocolViolation("response read failed".into()))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(McpError::ProtocolViolation(format!(
                "response exceeds {MAX_RESPONSE_BYTES}-byte cap"
            )));
        }
        serde_json::from_str::<JsonRpcResponse>(&String::from_utf8_lossy(&bytes)).map_err(|e| {
            // an HTML error page or any non-JSON body is a protocol
            // violation, never a business error (SS15)
            McpError::ProtocolViolation(format!("malformed http response body: {e}"))
        })
    }

    /// One HTTP POST attempt. 403 is NEVER retried here (§34): permission
    /// is not an expiry condition.
    fn send_once(
        &self,
        header: &http::HeaderLine,
        body: &str,
        auth_header: Option<&str>,
    ) -> Result<http::HttpResponse, McpError> {
        let mut request = self
            .agent
            .post(&self.url)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json")
            .set("MCP-Protocol-Version", &header.version)
            .set("Mcp-Method", &header.method);
        if let Some(name) = &header.name {
            request = request.set("Mcp-Name", name);
        }
        if let Some(token) = auth_header {
            request = request.set("Authorization", token);
        }
        let response = request
            .send_string(body)
            .map_err(|e| match e {
                ureq::Error::Status(code, _resp) => {
                    status_error(code, &self.endpoint).unwrap_or_else(|| {
                        McpError::ServerUnavailable(format!("http {code} from {}", self.endpoint))
                    })
                }
                ureq::Error::Transport(t) => {
                    let msg = t.to_string();
                    if msg.contains("Redirect") {
                        // redirects are disabled: any redirect attempt is a
                        // policy violation, never silently followed (§42)
                        McpError::ProtocolViolation(format!("redirect blocked: {msg}"))
                    } else {
                        McpError::ServerUnavailable(format!("http transport: {msg}"))
                    }
                }
            })?;
        Ok(http::HttpResponse {
            status: response.status(),
            content_type: response.content_type().to_string(),
            reader: Box::new(response.into_reader()),
        })
    }

    fn rpc(
        &mut self,
        rpc_method: &str,
        tool_name: Option<&str>,
        params: serde_json::Value,
        required_any: &[&str],
        error_for: &dyn Fn(&crate::protocol::JsonRpcError) -> Option<McpError>,
    ) -> Result<serde_json::Value, McpError> {
        let id = self.next_id();
        let (header, body) = self.build_request(id, rpc_method, tool_name, params)?;
        let resp = self.post(&header, &body)?;
        if let Some(err) = &resp.error {
            if let Some(mapped) = error_for(err) {
                return Err(mapped);
            }
            return Err(McpError::from_jsonrpc_error(err.code, &err.message, tool_name.unwrap_or("")));
        }
        if resp.id != Some(id) {
            return Err(McpError::ProtocolViolation(format!(
                "response id mismatch: got {:?}, want {}",
                resp.id, id
            )));
        }
        let v = resp.result.unwrap_or(serde_json::Value::Null);
        let ok = v
            .as_object()
            .map(|o| required_any.iter().any(|k| o.contains_key(*k)))
            .unwrap_or(false);
        if !ok {
            return Err(McpError::ProtocolViolation(format!(
                "{rpc_method}: missing result discriminator (want any of {required_any:?})"
            )));
        }
        Ok(v)
    }
}

impl McpTransport for StreamableHttpTransport {
    /// Stateless profile: no handshake exists — nothing is sent (§12).
    fn initialize(&mut self) -> Result<InitializeResult, McpError> {
        Ok(InitializeResult {
            protocol_version: self.profile.as_str().into(),
            capabilities: serde_json::json!({}),
            server_info: None,
        })
    }

    fn list_tools(&mut self) -> Result<ToolsListResult, McpError> {
        let v = self.rpc(method::TOOLS_LIST, None, serde_json::json!({}), &["tools"], &|_| None)?;
        serde_json::from_value(v).map_err(|e| McpError::ProtocolViolation(e.to_string()))
    }

    fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
        _execution_id: &str,
    ) -> Result<ToolCallResult, McpError> {
        // -32601 → UnknownTool keeps its CommandNotFound class over HTTP too
        let v = self.rpc(
            method::TOOLS_CALL,
            Some(tool_name),
            serde_json::json!({"name": tool_name, "arguments": arguments}),
            &["content", "isError", "structuredContent"],
            &|err| (err.code == -32601).then(|| McpError::UnknownTool(tool_name.to_string())),
        )?;
        serde_json::from_value(v).map_err(|e| McpError::ProtocolViolation(e.to_string()))
    }

    /// `server/discover` (§31): capability discovery metadata — never a
    /// tools/list replacement (§32).
    fn discover_server(&mut self) -> Result<serde_json::Value, McpError> {
        self.rpc(
            "server/discover",
            None,
            serde_json::json!({}),
            &["serverInfo", "tools", "capabilities"],
            &|_| None,
        )
    }

    fn shutdown(&mut self) {
        // stateless: nothing to tear down (no session exists)
    }
}

/// Internal HTTP plumbing (§41: status and body semantics are separate
/// layers); header/body consistency rule (§9) with direct test surface.
mod http {
    use super::McpError;

    pub struct HttpResponse {
        pub status: u16,
        pub content_type: String,
        pub reader: Box<dyn std::io::Read>,
    }

    #[derive(Debug)]
    pub struct HeaderLine {
        pub version: String,
        pub method: String,
        pub name: Option<String>,
    }

    impl HeaderLine {
        /// The frozen consistency rule (§9): header method/name must equal
        /// the body's method/params.name. The transport derives both from
        /// one source; this check is the belt to those suspenders and is
        /// exercised directly by HTTP-COMPAT-004..007.
        pub fn check_against_body(&self, body: &str) -> Result<(), McpError> {
            let v: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| McpError::ProtocolViolation(format!("body not json: {e}")))?;
            let body_method = v["method"].as_str().unwrap_or_default();
            if body_method != self.method {
                return Err(McpError::ProtocolViolation(format!(
                    "header/body mismatch: Mcp-Method={} body={body_method}",
                    self.method
                )));
            }
            let body_name = v["params"]["name"].as_str();
            match (self.name.as_deref(), body_name) {
                (Some(h), Some(b)) if h != b => Err(McpError::ProtocolViolation(format!(
                    "header/body mismatch: Mcp-Name={h} body={b}"
                ))),
                (Some(_), None) | (None, Some(_)) => Err(McpError::ProtocolViolation(
                    "header/body mismatch: Mcp-Name presence differs".into(),
                )),
                _ => Ok(()),
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::http::HeaderLine;
    use super::*;

    fn transport(url: &str) -> StreamableHttpTransport {
        StreamableHttpTransport::new(url, McpProtocolProfile::V2026_07_28, &UrlPolicy::default())
            .unwrap()
    }

    /// HTTP-COMPAT-001/002/003/008 (§8/§11): envelope generation — headers
    /// and body are derived from one source; _meta is transport metadata.
    #[test]
    fn http_compat_envelope_generation() {
        let t = transport("http://127.0.0.1:9/mcp");
        let (header, body) = t
            .build_request(1, "tools/call", Some("evaluate"),
                serde_json::json!({"name": "evaluate", "arguments": {"q": "x"}}))
            .unwrap();
        assert_eq!(header.method, "tools/call");
        assert_eq!(header.name.as_deref(), Some("evaluate"));
        assert_eq!(header.version, "2026-07-28");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["method"], "tools/call");
        assert_eq!(v["params"]["name"], "evaluate");
        assert!(v["params"]["_meta"]["io.modelcontextprotocol/clientInfo"].is_object());
        // and the consistency check passes on our own output
        assert!(header.check_against_body(&body).is_ok());
    }

    /// HTTP-COMPAT-004..007 (§9): header/body mismatches are hard protocol
    /// violations — the frozen consistency rule, driven directly.
    #[test]
    fn http_compat_header_body_consistency() {
        let header = HeaderLine {
            version: "2026-07-28".into(),
            method: "tools/call".into(),
            name: Some("search".into()),
        };
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "search", "arguments": {}}
        })
        .to_string();
        assert!(header.check_against_body(&body).is_ok());

        // method mismatch (§9 example)
        let bad = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}
        })
        .to_string();
        assert!(matches!(
            header.check_against_body(&bad),
            Err(McpError::ProtocolViolation(_))
        ));
        // name mismatch (§9 example: header search, body delete)
        let bad_name = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "delete", "arguments": {}}
        })
        .to_string();
        assert!(matches!(
            header.check_against_body(&bad_name),
            Err(McpError::ProtocolViolation(_))
        ));
        // version mismatch surfaces in the header source, not the body:
        // the transport never lets a foreign version through
        let foreign = HeaderLine {
            version: "2025-06-18".into(),
            method: "tools/call".into(),
            name: None,
        };
        let body_no_name = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {}
        })
        .to_string();
        assert!(foreign.check_against_body(&body_no_name).is_ok()); // self-consistent...
        // ...but a foreign profile can never construct this transport
        assert!(StreamableHttpTransport::new(
            "http://127.0.0.1:9/mcp",
            McpProtocolProfile::V2025_06_18,
            &UrlPolicy::default(),
        )
        .is_err());
    }

    /// HTTP-SEC-008 (§23): oversized request bodies are rejected BEFORE
    /// any socket is opened.
    #[test]
    fn http_sec_oversized_request_rejected() {
        let t = transport("http://127.0.0.1:9/mcp");
        let huge = "x".repeat(MAX_REQUEST_BYTES + 1);
        let err = t
            .build_request(1, "tools/call", Some("t"),
                serde_json::json!({"arguments": {"blob": huge}}))
            .unwrap_err();
        assert!(matches!(err, McpError::InvalidInput(_)), "got {err:?}");
    }
}
