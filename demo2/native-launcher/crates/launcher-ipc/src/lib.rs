//! JSON-RPC-style message model shared by the plugin host and plugins.
//!
//! Transport-agnostic: messages are framed as newline-delimited JSON on any
//! byte stream (stdio, named pipe). Contract tests live in `tests/`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const PROTOCOL_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("malformed JSON: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error("empty message")]
    Empty,
}

impl Request {
    pub fn new(id: u64, method: &str, params: Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }

    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("request serializes");
        s.push('\n');
        s
    }

    pub fn from_line(line: &str) -> Result<Self, IpcError> {
        if line.trim().is_empty() {
            return Err(IpcError::Empty);
        }
        Ok(serde_json::from_str(line)?)
    }
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: u64, code: i32, message: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }

    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("response serializes");
        s.push('\n');
        s
    }

    pub fn from_line(line: &str) -> Result<Self, IpcError> {
        if line.trim().is_empty() {
            return Err(IpcError::Empty);
        }
        Ok(serde_json::from_str(line)?)
    }
}

/// Known methods of the plugin protocol (PLUGIN-CONTRACT-v0.1 §15).
pub mod method {
    pub const INITIALIZE: &str = "initialize";
    pub const QUERY: &str = "query";
    pub const SHUTDOWN: &str = "shutdown";
    /// MVP4.0 (ADR-0014): separate effect channel for `plugin.*` actions;
    /// uses `execution_id`, fully disjoint from the `query_id` space.
    pub const EXECUTE_ACTION: &str = "execute_action";
}

/// Params of the `execute_action` method (MVP4.0). `execution_id` is
/// host-generated from its own sequence and MUST be echoed, exactly like
/// `query_id` for queries — the two id spaces never mix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecuteActionParams {
    pub execution_id: String,
    pub action_id: String,
    #[serde(default)]
    pub input: serde_json::Value,
    /// ContextSnapshot generation the resolution was bound to (INV-044).
    #[serde(default)]
    pub context_generation: u64,
}

/// Expected result shape of `execute_action`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteActionResult {
    pub execution_id: String,
    #[serde(default)]
    pub result: serde_json::Value,
}

/// JSON-RPC error codes (contract §11): standard codes plus the private
/// Host/plugin range -32001..-32007.
pub mod error_code {
    // standard JSON-RPC 2.0
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    // private plugin-protocol range
    pub const CAPABILITY_DENIED: i32 = -32001;
    pub const TIMEOUT: i32 = -32002;
    pub const PLUGIN_CRASHED: i32 = -32003;
    pub const RESULT_TOO_LARGE: i32 = -32004;
    pub const RATE_LIMITED: i32 = -32005;
    pub const PLUGIN_UNAVAILABLE: i32 = -32006;
    pub const VERSION_MISMATCH: i32 = -32007;
}

/// Params of the `initialize` handshake (Host -> Plugin, first request).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub plugin_id: String,
}

/// Expected result of the `initialize` handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: String,
}

/// Params of the `query` method (contract §7): every query carries a
/// Host-generated `query_id` that the plugin MUST echo back in its result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryParams {
    pub query_id: String,
    pub text: String,
    #[serde(default = "default_query_limit")]
    pub limit: usize,
}

fn default_query_limit() -> usize {
    MAX_PLUGIN_RESULTS
}

/// Expected shape of a `query` result: the echoed `query_id` plus commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub query_id: String,
    #[serde(default)]
    pub commands: Vec<Value>,
}

/// Result items returned by a plugin are validated against the Native UI
/// Schema (design spec 10): a list view with title/subtitle strings.
///
/// `actions` entries are either legacy strings (pre-freeze plugins; the
/// first becomes the Execute target) or ActionDescriptor objects
/// (ACTION-CONTRACT-v0.1) resolved by the host. Malformed entries are
/// dropped individually — action-level fault containment — and never
/// invalidate the containing item (INV-031).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResultItem {
    pub title: String,
    /// Stable logical id (INV-028): when present, the host assigns the
    /// command id `<manifest.id>:<id>` instead of deriving it from the title.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub actions: Vec<serde_json::Value>,
    /// Bounded ranking hint in [0, 1] (COMMAND-CONTRACT §3/§6). Enables
    /// non-lexical results (e.g. calculators) to surface; clamped on parse.
    #[serde(default)]
    pub score: f32,
}

/// Maximum results accepted from a plugin; anything above is truncated.
pub const MAX_PLUGIN_RESULTS: usize = 100;

/// Validate and truncate a raw plugin result array. Malformed items are
/// rejected (whole response treated as malformed) per plugin contract tests.
pub fn sanitize_results(value: Value) -> Result<Vec<PluginResultItem>, String> {
    let items: Vec<PluginResultItem> =
        serde_json::from_value(value).map_err(|e| format!("invalid result schema: {e}"))?;
    // normalize the hint here so hosts can trust [0,1] (missing/invalid = 0)
    let mut items = items;
    for it in items.iter_mut() {
        if !it.score.is_finite() || it.score < 0.0 {
            it.score = 0.0;
        }
        it.score = it.score.min(1.0);
    }
    if items.len() > MAX_PLUGIN_RESULTS {
        Ok(items.into_iter().take(MAX_PLUGIN_RESULTS).collect())
    } else {
        Ok(items)
    }
}

/// Hard cap on a single NDJSON protocol frame (contract §14 resource
/// limits). Anything larger is a protocol violation, not a truncation case.
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

/// Validate a `query` response body against the expected `query_id`.
///
/// v0.1 response profile (contract §6): `{"query_id": "...", "commands":
/// [...]}` with a mandatory echo. A bare array is only accepted for plugins
/// on the Legacy Manifest Profile (`schema_version` absent); for v1
/// manifests it is a protocol violation. A mismatched echo is always a
/// violation.
pub fn parse_query_result(
    value: Value,
    expected_query_id: &str,
    allow_legacy_array: bool,
) -> Result<Vec<PluginResultItem>, String> {
    let commands = match &value {
        Value::Array(_) if allow_legacy_array => value, // legacy shape: no echo available
        Value::Array(_) => {
            return Err(
                "legacy bare-array response is not allowed for schema_version 1 manifests"
                    .to_string(),
            )
        }
        Value::Object(_) => {
            let parsed: QueryResult =
                serde_json::from_value(value).map_err(|e| format!("invalid result schema: {e}"))?;
            if parsed.query_id != expected_query_id {
                return Err(format!(
                    "query_id echo mismatch: expected {expected_query_id}, got {}",
                    parsed.query_id
                ));
            }
            Value::Array(parsed.commands)
        }
        other => return Err(format!("invalid result schema: {other}")),
    };
    sanitize_results(commands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip() {
        let r = Request::new(7, method::QUERY, serde_json::json!({"query":"abc"}));
        let line = r.to_line();
        assert!(line.ends_with('\n'));
        assert_eq!(Request::from_line(&line).unwrap(), r);
    }

    #[test]
    fn malformed_json_rejected() {
        assert!(Request::from_line("{not json").is_err());
        assert!(Response::from_line("hello").is_err());
    }

    #[test]
    fn empty_line_rejected() {
        assert!(matches!(Request::from_line("  \n"), Err(IpcError::Empty)));
    }

    #[test]
    fn sanitize_truncates_flood() {
        let items: Vec<Value> = (0..100_000)
            .map(|i| serde_json::json!({"title": format!("t{i}")}))
            .collect();
        let out = sanitize_results(Value::Array(items)).unwrap();
        assert_eq!(out.len(), MAX_PLUGIN_RESULTS);
    }

    #[test]
    fn sanitize_rejects_wrong_schema() {
        assert!(sanitize_results(serde_json::json!([{"no_title": 1}])).is_err());
        assert!(sanitize_results(serde_json::json!("not a list")).is_err());
    }

    #[test]
    fn error_response_roundtrip() {
        let r = Response::err(3, -32601, "unknown method");
        let back = Response::from_line(&r.to_line()).unwrap();
        assert!(back.result.is_none());
        assert_eq!(back.error.unwrap().code, -32601);
    }

    #[test]
    fn query_result_echo_is_validated() {
        let items = serde_json::json!({"query_id": "q-1", "commands": [{"title": "a"}]});
        let out = parse_query_result(items, "q-1", false).unwrap();
        assert_eq!(out.len(), 1);

        let mismatch = serde_json::json!({"query_id": "q-2", "commands": []});
        assert!(parse_query_result(mismatch, "q-1", false).is_err());
    }

    #[test]
    fn query_result_legacy_array_only_for_legacy_profile() {
        // legacy manifest profile: bare array tolerated
        let out = parse_query_result(serde_json::json!([{"title": "a"}]), "q-9", true).unwrap();
        assert_eq!(out.len(), 1);
        // v1 manifest profile: bare array is a protocol violation
        assert!(parse_query_result(serde_json::json!([{"title": "a"}]), "q-9", false).is_err());
        assert!(parse_query_result(serde_json::json!("junk"), "q-9", true).is_err());
    }

    #[test]
    fn execute_action_params_roundtrip() {
        let p = ExecuteActionParams {
            execution_id: "e-1".into(),
            action_id: "export".into(),
            input: serde_json::json!({"format": "pdf"}),
            context_generation: 7,
        };
        let back: ExecuteActionParams =
            serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn query_params_roundtrip() {
        let p = QueryParams {
            query_id: "q-1".into(),
            text: "term".into(),
            limit: 10,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["query_id"], "q-1");
        assert_eq!(v["text"], "term");
        let back: QueryParams = serde_json::from_value(v).unwrap();
        assert_eq!(back, p);
    }
}
