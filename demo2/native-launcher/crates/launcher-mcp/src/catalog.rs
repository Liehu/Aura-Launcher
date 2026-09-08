//! Tool Catalog (Spec sections 10/11): discovered MCP servers/tools with
//! stable identity, populated from a transport's `tools/list`, projected by
//! the adapter. Catalog refresh (Spec section 27) = re-list + replace.

use crate::compat::{CacheMetadata, CacheScope};
use crate::types::{McpCatalogSnapshot, McpServer, McpServerId, McpTool};
use crate::transport::McpTransport;

/// Catalog over one live transport session. Discovery (tools/list) happens
/// once per refresh; Provider queries hit the cached snapshot, never the
/// server (Spec section 12: the provider only queries the local catalog).
pub struct McpCatalog {
    server_id: McpServerId,
    server_name: String,
    tools: Vec<McpTool>,
    cache: CacheMetadata,
}

impl McpCatalog {
    pub fn new(server_id: McpServerId, server_name: String) -> Self {
        Self { server_id, server_name, tools: Vec::new(), cache: CacheMetadata::default() }
    }

    /// Refresh from a transport session: initialize + tools/list, then
    /// replace the cached snapshot atomically (Spec section 27).
    ///
    /// Deterministic ordering (review 47 §11): the cached list is
    /// normalized to name order so downstream Commands/identity never
    /// depend on the server's emission order. Tool identity is untouched.
    pub fn refresh_from(&mut self, transport: &mut dyn McpTransport) -> Result<(), crate::McpError> {
        transport.initialize()?;
        let listed = transport.list_tools()?;
        let mut tools: Vec<McpTool> = listed
            .tools
            .into_iter()
            .map(|t| McpTool {
                server_id: self.server_id.clone(),
                name: t.name,
                title: t.title,
                description: t.description,
                input_schema: t.input_schema,
                annotations: t.annotations,
            })
            .collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        self.tools = tools;
        // 2026 cache hints are advisory discovery metadata; unknown servers
        // simply leave the fail-closed default (server-scoped, no ttl).
        self.cache = CacheMetadata { ttl_ms: listed.ttl_ms, scope: CacheScope::Server };
        Ok(())
    }

    pub fn snapshot(&self) -> McpCatalogSnapshot {
        McpCatalogSnapshot {
            servers: vec![McpServer {
                id: self.server_id.clone(),
                name: self.server_name.clone(),
                tools: self.tools.clone(),
            }],
            cache: self.cache,
        }
    }

    pub fn server_id(&self) -> &McpServerId {
        &self.server_id
    }

    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::McpError;
    use crate::protocol::{InitializeResult, ToolCallResult, ToolsListResult};

    /// Scripted fake transport: deterministic catalog tests (MCP-001/004/006).
    struct FakeTransport {
        tools: Vec<crate::protocol::ToolDto>,
        fail_initialize: bool,
        initialized: bool,
    }

    impl FakeTransport {
        fn new(tools: Vec<(&str, &str)>) -> Self {
            Self {
                tools: tools
                    .into_iter()
                    .map(|(n, d)| crate::protocol::ToolDto {
                        name: n.into(),
                        title: None,
                        description: Some(d.into()),
                        input_schema: serde_json::json!({"type": "object"}),
                        annotations: serde_json::json!({}),
                    })
                    .collect(),
                fail_initialize: false,
                initialized: false,
            }
        }
    }

    impl McpTransport for FakeTransport {
        fn initialize(&mut self) -> Result<InitializeResult, McpError> {
            if self.fail_initialize {
                return Err(McpError::ServerUnavailable("down".into()));
            }
            self.initialized = true;
            Ok(InitializeResult {
                protocol_version: crate::MCP_PROTOCOL_VERSION.into(),
                capabilities: serde_json::json!({}),
                server_info: None,
            })
        }
        fn list_tools(&mut self) -> Result<ToolsListResult, McpError> {
            assert!(self.initialized, "list before initialize");
            Ok(ToolsListResult { tools: self.tools.clone(), next_cursor: None, ttl_ms: None })
        }
        fn call_tool(
            &mut self,
            _: &str,
            _: serde_json::Value,
            _: &str,
        ) -> Result<ToolCallResult, McpError> {
            unimplemented!("catalog tests don't call tools")
        }
        fn shutdown(&mut self) {}
    }

    /// MCP-001: tool discovery populates the catalog.
    #[test]
    fn mcp001_tool_discovery() {
        let mut t = FakeTransport::new(vec![
            ("evaluate", "Evaluate an expression"),
            ("echo", "Echo input"),
        ]);
        let mut catalog = McpCatalog::new(McpServerId("calc".into()), "Calculator".into());
        catalog.refresh_from(&mut t).unwrap();
        assert_eq!(catalog.tools().len(), 2);
        assert_eq!(
            catalog.snapshot().server(&McpServerId("calc".into())).unwrap().tools.len(),
            2
        );
    }

    /// MCP-002: two servers keep separate catalog entries.
    #[test]
    fn mcp002_multiple_servers() {
        let mut a = McpCatalog::new(McpServerId("calc".into()), "Calc".into());
        let mut b = McpCatalog::new(McpServerId("jira".into()), "Jira".into());
        let mut ta = FakeTransport::new(vec![("evaluate", "eval")]);
        let mut tb = FakeTransport::new(vec![("create_issue", "create")]);
        a.refresh_from(&mut ta).unwrap();
        b.refresh_from(&mut tb).unwrap();
        let snap = a.snapshot();
        let mut all: Vec<_> = snap.servers.iter().flat_map(|s| s.tools.iter().map(|t| t.route())).collect();
        let snap_b = b.snapshot();
        all.extend(snap_b.servers.iter().flat_map(|s| s.tools.iter().map(|t| t.route())));
        // same tool name on different servers = different routes (INV-MCP-006)
        assert!(all.contains(&("mcp:calc".into(), "evaluate".into(), "invoke".into())));
        assert!(all.contains(&("mcp:jira".into(), "create_issue".into(), "invoke".into())));
    }

    /// MCP-004: server identity comes from config (stable across refresh).
    #[test]
    fn mcp004_stable_server_identity() {
        let id = McpServerId("filesystem".into());
        let mut catalog = McpCatalog::new(id.clone(), "FS".into());
        let mut t = FakeTransport::new(vec![]);
        catalog.refresh_from(&mut t).unwrap();
        assert_eq!(catalog.server_id(), &id);
    }

    /// MCP-006: refresh replaces the cached tool set.
    #[test]
    fn mcp006_catalog_refresh() {
        let mut catalog = McpCatalog::new(McpServerId("calc".into()), "Calc".into());
        let mut t = FakeTransport::new(vec![("evaluate", "eval")]);
        catalog.refresh_from(&mut t).unwrap();
        assert_eq!(catalog.tools().len(), 1);
        // server adds a tool between refreshes
        t = FakeTransport::new(vec![("evaluate", "eval"), ("echo", "echo")]);
        catalog.refresh_from(&mut t).unwrap();
        assert_eq!(catalog.tools().len(), 2);
    }
}
