//! Stable route-identity helpers (Spec section 5/17, INV-MCP-006).
//!
//! MCP's raw identity (`server_id` + `tool_name`) MUST NOT directly become
//! Launcher authority identity. The host owns the projection:
//!
//! ```text
//! provider_id = "mcp:<server_id>"     (host-assigned namespace)
//! command_id  = <tool.name>           (stable within one server)
//! action_id   = "invoke"              (frozen: a tool IS callable)
//! ```

use crate::types::McpServerId;

/// Host-assigned provider namespace for an MCP server.
pub fn mcp_provider_id(server_id: &McpServerId) -> String {
    format!("mcp:{}", server_id.as_str())
}

/// Command id projection: the verbatim MCP tool name.
pub fn mcp_command_id(tool_name: &str) -> String {
    tool_name.to_string()
}

/// Frozen action id for every MCP tool invocation.
pub const MCP_ACTION_ID: &str = "invoke";

/// True when `provider_id` addresses the MCP namespace (`mcp:<server_id>`).
pub fn is_mcp_provider(provider_id: &str) -> bool {
    provider_id.starts_with("mcp:")
}

/// Extract the `server_id` from an MCP provider id (`mcp:<server_id>`).
pub fn server_id_from_provider(provider_id: &str) -> Option<&str> {
    provider_id.strip_prefix("mcp:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::McpTool;

    fn tool(server: &str, name: &str) -> McpTool {
        McpTool {
            server_id: McpServerId(server.into()),
            name: name.into(),
            title: None,
            description: None,
            input_schema: serde_json::json!({"type": "object"}),
            annotations: serde_json::json!({}),
        }
    }

    /// INV-MCP-006: route identity is stable and namespace-scoped.
    #[test]
    fn route_identity_scoping() {
        let t = tool("github", "search_repositories");
        assert_eq!(t.route(), ("mcp:github".into(), "search_repositories".into(), "invoke".into()));

        // same tool name on a different server = different route
        let t2 = tool("jira", "search_repositories");
        assert_ne!(t.route(), t2.route());

        // identity helpers
        assert!(is_mcp_provider("mcp:github"));
        assert!(!is_mcp_provider("plugin:calc"));
        assert_eq!(server_id_from_provider("mcp:github"), Some("github"));
    }

    /// MCP-029/030/031: injection and collision resistance come from
    /// namespace scoping, not name sanitization.
    #[test]
    fn cross_server_confusion_is_impossible() {
        // a tool cannot claim another server's namespace via its name
        let evil = tool("evil", "com.example.calc");
        let (provider, ..) = evil.route();
        assert_eq!(provider, "mcp:evil");
        // the server id is config-owned; a malicious name never changes it
    }
}
