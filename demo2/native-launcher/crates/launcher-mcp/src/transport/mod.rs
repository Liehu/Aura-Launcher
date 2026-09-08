//! MCP transport abstraction (Spec section 2). stdio is REQUIRED for
//! MVP4.3; HTTP / remote auth stay architecture-reserved so the Core API
//! never freezes around a specific transport.

pub mod http_policy;
pub mod stdio;
pub mod streamable_http;

use crate::error::McpError;
use crate::protocol::{InitializeResult, ToolCallResult, ToolsListResult};

/// Transport-agnostic MCP client. One transport instance = one server
/// session under the legacy profile; under the stateless profile each
/// request is self-describing and no session exists (review 47 §20:
/// application code never holds a protocol session object).
pub trait McpTransport: Send {
    /// Legacy profile: `initialize` handshake + `notifications/initialized`.
    /// Stateless profile: no-op — returns a synthesized local result and
    /// writes nothing (there is no handshake to perform).
    fn initialize(&mut self) -> Result<InitializeResult, McpError>;
    /// `tools/list` discovery (Spec section 27).
    fn list_tools(&mut self) -> Result<ToolsListResult, McpError>;
    /// `tools/call` with a fresh execution correlation id (INV-MCP-008).
    fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
        execution_id: &str,
    ) -> Result<ToolCallResult, McpError>;
    /// `server/discover` (current profile only). Profiles without it fail
    /// closed with ProtocolViolation — never silently emulate.
    fn discover_server(&mut self) -> Result<serde_json::Value, McpError> {
        Err(McpError::ProtocolViolation(
            "server/discover is not supported by this profile".into(),
        ))
    }
    /// Runtime association (P1-B): the protocol session records which
    /// runtime it is bound to. Transports without a process return None.
    fn runtime_id(&self) -> Option<launcher_runtime::RuntimeId> {
        None
    }

    /// Terminate the server process tree.
    fn shutdown(&mut self);
}

/// Object-safety shim: callers holding a boxed transport (e.g. providers
/// serving multiple endpoint kinds) delegate transparently.
impl McpTransport for Box<dyn McpTransport> {
    fn initialize(&mut self) -> Result<InitializeResult, McpError> {
        (**self).initialize()
    }
    fn list_tools(&mut self) -> Result<ToolsListResult, McpError> {
        (**self).list_tools()
    }
    fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
        execution_id: &str,
    ) -> Result<ToolCallResult, McpError> {
        (**self).call_tool(tool_name, arguments, execution_id)
    }
    fn discover_server(&mut self) -> Result<serde_json::Value, McpError> {
        (**self).discover_server()
    }
    fn shutdown(&mut self) {
        (**self).shutdown()
    }
}
