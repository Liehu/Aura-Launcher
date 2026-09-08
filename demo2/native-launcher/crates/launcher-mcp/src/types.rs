//! MCP types (Spec sections 5/7/10): catalog, server, and tool identity.
//!
//! Identity rules (INV-MCP-006): `server_id` is config-owned, stable and
//! unique; route identity is `(provider_id, command_id, action_id)` with
//! `provider_id = "mcp:<server_id>"`, `command_id = <tool.name>`,
//! `action_id = "invoke"`.

use serde::{Deserialize, Serialize};

/// Config-owned, stable, unique MCP server identity (Spec section 28).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct McpServerId(pub String);

impl McpServerId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for McpServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One MCP tool as discovered from `tools/list`, projected into Launcher
/// identity. Names are kept verbatim for display but never used as
/// authorization identity (INV-MCP-005).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpTool {
    pub server_id: McpServerId,
    /// MCP `tool.name` — stable within one server.
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// JSON Schema (2020-12) for the tool input, kept verbatim. Schema is
    /// shape/type validation only — never authorization (Spec section 9).
    /// Wire name matches the MCP `tools/list` field.
    #[serde(default, rename = "inputSchema")]
    pub input_schema: serde_json::Value,
    /// Untrusted metadata hints (`readOnlyHint`, `destructiveHint`, ...).
    /// Policy input only, never authority (INV-MCP-004).
    #[serde(default)]
    pub annotations: serde_json::Value,
}

impl McpTool {
    /// Route identity triple (INV-MCP-006).
    pub fn route(&self) -> (String, String, String) {
        (
            mcp_provider_id(&self.server_id),
            self.name.clone(),
            "invoke".to_string(),
        )
    }
}

/// One configured + discovered MCP server.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServer {
    pub id: McpServerId,
    pub name: String,
    pub tools: Vec<McpTool>,
}

/// Immutable point-in-time snapshot of all discovered servers/tools
/// (Spec section 11: Discovery Cache → MCP Catalog → Command projection).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct McpCatalogSnapshot {
    pub servers: Vec<McpServer>,
    /// Discovery cache bookkeeping (review 47 §10). Discovery-side only:
    /// MUST NOT enter ActionProposal / WorkflowStep / Effect.
    pub cache: crate::compat::CacheMetadata,
}

impl McpCatalogSnapshot {
    pub fn server(&self, id: &McpServerId) -> Option<&McpServer> {
        self.servers.iter().find(|s| &s.id == id)
    }

    /// All tools across servers.
    pub fn tools(&self) -> impl Iterator<Item = &McpTool> {
        self.servers.iter().flat_map(|s| s.tools.iter())
    }
}

/// `provider_id` for an MCP server: `mcp:<server_id>` (Spec section 5).
pub fn mcp_provider_id(server_id: &McpServerId) -> String {
    format!("mcp:{server_id}")
}

/// `command_id` for a tool: the verbatim MCP `tool.name`.
pub fn mcp_command_id(tool_name: &str) -> String {
    tool_name.to_string()
}

/// `action_id` is frozen to `invoke` — an MCP tool is already a callable
/// capability (Spec section 6).
pub const MCP_ACTION_ID: &str = "invoke";
