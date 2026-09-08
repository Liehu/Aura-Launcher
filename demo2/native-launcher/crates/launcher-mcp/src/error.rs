//! Normalized MCP errors (Spec section 18): every raw MCP failure maps to
//! one of the frozen Workflow failure classes — the Workflow layer never
//! guesses from raw error strings.

use launcher_domain::workflow::WorkflowFailureClass;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum McpError {
    #[error("invalid arguments: {0}")]
    InvalidInput(String),
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("server unavailable: {0}")]
    ServerUnavailable(String),
    #[error("connection timeout after {0:?}")]
    Timeout(std::time::Duration),
    #[error("tool execution error: {0}")]
    ToolExecutionError(String),
    #[error("malformed protocol response: {0}")]
    ProtocolViolation(String),
    #[error("server process crashed: exit code {0:?}")]
    ProcessCrashed(Option<i32>),
    /// HTTP 401 — transport-level auth boundary (P0-B). FailureClass
    /// decision is deferred to P0-C; for now it degrades to
    /// PluginUnavailable so Workflow behavior is unchanged.
    #[error("authentication required: {0}")]
    AuthenticationRequired(String),
    /// HTTP 403 — transport-level permission boundary (P0-B).
    #[error("permission denied: {0}")]
    PermissionDenied(String),
}

impl McpError {
    /// FailureClass projection (Spec section 18 table). Single mapping point:
    /// Workflow/AI/UI never classify MCP raw errors themselves.
    pub fn failure_class(&self) -> WorkflowFailureClass {
        use WorkflowFailureClass as C;
        match self {
            McpError::InvalidInput(_) => C::InvalidInput,
            McpError::UnknownTool(_) => C::CommandNotFound,
            McpError::ServerUnavailable(_) => C::PluginUnavailable,
            McpError::Timeout(_) => C::Timeout,
            McpError::ToolExecutionError(_) => C::BusinessError,
            McpError::ProtocolViolation(_) | McpError::ProcessCrashed(_) => C::ProtocolViolation,
            // P0-C decides the final class; degrade conservatively for now
            McpError::AuthenticationRequired(_) | McpError::PermissionDenied(_) => {
                C::PluginUnavailable
            }
        }
    }

    /// JSON-RPC error-code matrix (review 47 §15): the single place a wire
    /// error code becomes an McpError. Unknown codes fail closed to
    /// ToolExecutionError (business failure, server stays alive) — never a
    /// success, never a panic. Workflow never matches raw codes.
    pub fn from_jsonrpc_error(code: i32, message: &str, tool: &str) -> Self {
        match code {
            // method/tool not found
            -32601 => McpError::UnknownTool(tool.to_string()),
            // invalid params (2026-07-28: resource-not-found moved here too)
            -32602 => McpError::InvalidInput(message.to_string()),
            // parse error / invalid request on the protocol channel
            -32700 | -32600 => {
                McpError::ProtocolViolation(format!("jsonrpc error [{code}]: {message}"))
            }
            // server-defined / everything else: business failure
            _ => McpError::ToolExecutionError(format!("[{code}] {message}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MCP-023..028: failure mapping is centralized (Spec section 18).
    #[test]
    fn failure_mapping_table() {
        let cases: Vec<(McpError, WorkflowFailureClass)> = vec![
            (McpError::InvalidInput("bad".into()), WorkflowFailureClass::InvalidInput),
            (McpError::UnknownTool("t".into()), WorkflowFailureClass::CommandNotFound),
            (McpError::ServerUnavailable("down".into()), WorkflowFailureClass::PluginUnavailable),
            (McpError::Timeout(std::time::Duration::from_secs(1)), WorkflowFailureClass::Timeout),
            (McpError::ToolExecutionError("biz".into()), WorkflowFailureClass::BusinessError),
            (McpError::ProtocolViolation("bad frame".into()), WorkflowFailureClass::ProtocolViolation),
            (McpError::ProcessCrashed(Some(1)), WorkflowFailureClass::ProtocolViolation),
        ];
        for (err, class) in cases {
            assert_eq!(err.failure_class(), class, "mapping for {err:?}");
        }
    }
}
