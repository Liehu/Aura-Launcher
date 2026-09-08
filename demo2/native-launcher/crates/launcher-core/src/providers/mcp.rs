//! McpProvider (MVP4.3 Phase 4 + Phase 5 / Spec sections 12-13): serves MCP
//! tool commands from the local catalog. Discovery (tools/list) happens once
//! per refresh in the adapter; the provider only queries the cached catalog
//! — and answers the frozen empty-query discovery contract (WF-006 option ②,
//! closing the MCP half of DISCOVERY-TODO-001).
//!
//! Phase 5 (review 41 §5.5): the provider is a Discovery/Proposal producer
//! ONLY. It no longer judges execution availability — projected invoke
//! actions are resolved against the host capability grant (`mcp.invoke`),
//! and execution flows exclusively through the ActionEngine's effect
//! routing (Core → McpExecutor).

use launcher_domain::{Capability, Command, QueryContext};
use launcher_mcp::compat::McpProtocolProfile;
use launcher_mcp::adapter;
use launcher_mcp::transport::http_policy::UrlPolicy;
use launcher_mcp::transport::streamable_http::StreamableHttpTransport;
use launcher_mcp::catalog::McpCatalog;
use launcher_mcp::types::McpServerId;
use launcher_mcp::transport::McpTransport;

use crate::Provider;

/// One configured MCP server's provider. Lazy: the server process is
/// spawned on first discovery, the catalog is cached, and subsequent
/// queries (including ReferenceResolver fresh queries) hit the cache.
pub enum ProviderTransport {
    Stdio { program: String, args: Vec<String> },
    Http { url: String, allow_private_network: bool, allow_plain_http: bool },
}

pub struct McpProvider {
    server_id: String,
    transport: ProviderTransport,
    timeout: std::time::Duration,
    commands: Vec<Command>,
    last_error: Option<String>,
    discovered: bool,
    /// Host policy: whether this server's tools may execute (`mcp.invoke`
    /// granted). Config/policy-owned; never derived from tool metadata
    /// (INV-068). Default: granted.
    invoke_granted: bool,
    /// Wire profile (Phase 11): confined to launcher-mcp.
    profile: McpProtocolProfile,
}

impl McpProvider {
    pub fn new(server_id: String, program: String, args: Vec<String>) -> Self {
        Self {
            server_id,
            transport: ProviderTransport::Stdio { program, args },
            timeout: std::time::Duration::from_millis(5000),
            commands: Vec::new(),
            last_error: None,
            discovered: false,
            invoke_granted: true,
            profile: McpProtocolProfile::default(),
        }
    }

    /// Streamable HTTP endpoint constructor (P0-B): 2026 stateless profile.
    pub fn new_http(
        server_id: String,
        url: String,
        allow_private_network: bool,
        allow_plain_http: bool,
    ) -> Self {
        Self {
            server_id,
            transport: ProviderTransport::Http { url, allow_private_network, allow_plain_http },
            timeout: std::time::Duration::from_millis(5000),
            commands: Vec::new(),
            last_error: None,
            discovered: false,
            invoke_granted: true,
            profile: McpProtocolProfile::V2026_07_28,
        }
    }

    /// Wire profile for this server's transport sessions (Phase 11).
    pub fn set_profile(&mut self, profile: McpProtocolProfile) {
        if self.profile != profile {
            self.profile = profile;
            if self.discovered {
                let _ = self.reproject();
            }
        }
    }

    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Host policy toggle: deny → projected invoke actions are Disabled.
    pub fn set_invoke_granted(&mut self, granted: bool) {
        if self.invoke_granted != granted {
            self.invoke_granted = granted;
            // re-project under the new policy if already discovered
            if self.discovered {
                let _ = self.reproject();
            }
        }
    }

    fn grants(&self) -> Vec<Capability> {
        if self.invoke_granted {
            vec![Capability::McpInvoke]
        } else {
            Vec::new()
        }
    }

    fn reproject(&mut self) -> Result<(), launcher_mcp::McpError> {
        let mut transport: Box<dyn launcher_mcp::transport::McpTransport> =
            match &self.transport {
                ProviderTransport::Stdio { program, args } => {
                    Box::new(launcher_mcp::transport::stdio::StdioTransport::spawn_with_profile(
                        program,
                        args,
                        self.timeout,
                        self.profile,
                    )?)
                }
                ProviderTransport::Http { url, allow_private_network, allow_plain_http } => {
                    let policy = UrlPolicy {
                        allow_private_network: *allow_private_network,
                        allow_plain_http: *allow_plain_http,
                    };
                    Box::new(StreamableHttpTransport::new(url, self.profile, &policy)?)
                }
            };
        let mut catalog = McpCatalog::new(
            McpServerId(self.server_id.clone()),
            self.server_id.clone(),
        );
        let result = catalog.refresh_from(&mut transport);
        transport.shutdown();
        result?;
        let grants = self.grants();
        self.commands = catalog
            .snapshot()
            .servers
            .into_iter()
            .flat_map(|s| adapter::server_to_commands_with_grants(&s, &grants))
            .collect();
        Ok(())
    }

    /// Refresh the catalog over a live stdio session (Spec section 27).
    fn discover(&mut self) -> Result<(), launcher_mcp::McpError> {
        self.reproject()
    }
}

impl Provider for McpProvider {
    fn id(&self) -> &str {
        "mcp"
    }

    /// MCP server identity within the `mcp:` namespace (INV-MCP-006).
    fn plugin_identity(&self) -> Option<&str> {
        Some(&self.server_id)
    }

    /// Empty query = command discovery (WF-006 option ② / Spec section 13):
    /// returns every catalog tool as a Command. Non-empty queries filter by
    /// title/name substring. Execution availability is the resolver's
    /// decision (Phase 5): the provider only carries the host grant.
    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if !self.discovered {
            if let Err(e) = self.discover() {
                self.last_error = Some(e.to_string());
                return Vec::new();
            }
            self.discovered = true;
        }
        let text = q.normalized.to_lowercase();
        self.commands
            .iter()
            .filter(|c| {
                text.is_empty()
                    || c.title.to_lowercase().contains(&text)
                    || c.id.to_lowercase().contains(&text)
            })
            .cloned()
            .collect()
    }

    fn take_last_error(&mut self) -> Option<String> {
        self.last_error.take()
    }
}

/// Semantic catalog type for MCP tool invocation in the planner-facing
/// ActionCatalog (Phase 9, review 44 §7): pure description data.
pub const MCP_CATALOG_ACTION_TYPE: &str = "plugin.mcp.invoke";
