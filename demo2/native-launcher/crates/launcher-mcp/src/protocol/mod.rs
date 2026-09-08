//! MCP protocol DTOs (Phase 1, Spec section 2): JSON-RPC 2.0 envelopes plus
//! the three methods MVP4.3 uses — `initialize`, `tools/list`, `tools/call`.
//! Protocol types stay in this crate and never leak into launcher-domain
//! (Spec section 30).

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod method {
    pub const INITIALIZE: &str = "initialize";
    pub const INITIALIZED_NOTIFICATION: &str = "notifications/initialized";
    pub const TOOLS_LIST: &str = "tools/list";
    pub const TOOLS_CALL: &str = "tools/call";
}

/// JSON-RPC 2.0 request (launcher → server).
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: &str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        }
    }

    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("request serializes");
        s.push('\n');
        s
    }
}

/// JSON-RPC 2.0 response (server → launcher), one per line on stdout.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    pub id: Option<u64>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
    /// Notifications (no id) are tolerated and skipped by the transport.
    #[serde(default)]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// `initialize` request params (Spec section 2).
#[derive(Debug, Clone, Serialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: &'static str,
    pub capabilities: Value,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub name: &'static str,
    pub version: &'static str,
}

/// `initialize` result.
#[derive(Debug, Clone, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion", default)]
    pub protocol_version: String,
    #[serde(default)]
    pub capabilities: Value,
    #[serde(default)]
    pub server_info: Option<Value>,
}

/// `tools/list` result (Spec section 10). Profile-neutral: 2026 cache
/// hints (`ttlMs`) are optional discovery metadata and never leak past the
/// catalog.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsListResult {
    #[serde(default)]
    pub tools: Vec<ToolDto>,
    #[serde(default)]
    pub next_cursor: Option<String>,
    /// 2026 profile: server-advertised cache lifetime (`ttlMs`).
    #[serde(default, rename = "ttlMs")]
    pub ttl_ms: Option<u64>,
}

/// MCP tool definition as returned by the server. Untrusted input: only the
/// fields projected by the adapter survive into the catalog.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolDto {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
    #[serde(default)]
    pub annotations: Value,
}

/// `tools/call` params (launcher → server).
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// `tools/call` result.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallResult {
    #[serde(default)]
    pub content: Vec<ContentItem>,
    #[serde(default, rename = "isError")]
    pub is_error: bool,
    /// Optional structured output (MCP spec `structuredContent`); the
    /// legacy compatibility profile servers may omit it.
    #[serde(default, rename = "structuredContent")]
    pub structured_content: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Protocol DTOs roundtrip against the wire shapes the fixture server
    /// and real MCP servers emit.
    #[test]
    fn dto_roundtrip() {
        let req = JsonRpcRequest::new(1, method::INITIALIZE, serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "native-launcher", "version": "0.1"}
        }));
        let line = req.to_line();
        assert!(line.ends_with('\n'));
        assert!(line.contains("\"jsonrpc\":\"2.0\""));

        let resp: JsonRpcResponse =
            serde_json::from_str(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "calc", "version": "1.0"}
                }
            }).to_string())
            .unwrap();
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_some());

        // notification (no id) parses without error
        let n: JsonRpcResponse = serde_json::from_str(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
        .unwrap();
        assert!(n.id.is_none());

        let tools: ToolsListResult = serde_json::from_value(serde_json::json!({
            "tools": [
                {"name": "evaluate", "description": "eval",
                 "inputSchema": {"type": "object"}}
            ]
        }))
        .unwrap();
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "evaluate");
    }
}

pub mod session;
pub mod session_manager;
pub use session::{
    ProtocolSession, ProtocolSessionId, ProtocolSessionKey, ProtocolSessionState,
    SessionSnapshot,
};
pub use session_manager::{ProtocolSessionManager, SessionError};
