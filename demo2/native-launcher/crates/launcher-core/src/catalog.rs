//! Unified ActionCatalog projection (MVP4.3 Phase 9, review 44 §4/§26):
//! the AI Planner sees capability DESCRIPTIONS from every provider —
//! built-in, plugin and MCP — through one provider-agnostic pipeline.
//!
//! ```text
//! Provider Registry → Commands → Catalog Projection → ActionCatalog → Planner
//! ```
//!
//! The projection never carries authority (review 44 §6/§24): items are
//! pure data ([`launcher_workflow::proposal::ActionCatalogItem`]); MCP tool
//! metadata (description/inputSchema/annotations) is untrusted DATA for
//! planning only and can never grant `mcp.invoke` (INV-068).

use launcher_mcp::types::McpTool;
use launcher_workflow::proposal::ActionCatalogItem;

/// Project one discovered MCP tool into a planner catalog item, preserving
/// the tool's published input schema (review 44 §8: the planner MAY use
/// name/title/description/inputSchema/annotations to build arguments —
/// schema understanding is not execution-input authority, §10).
pub fn mcp_catalog_item(tool: &McpTool) -> ActionCatalogItem {
    ActionCatalogItem {
        provider_id: launcher_mcp::identity::mcp_provider_id(&tool.server_id),
        command_id: launcher_mcp::identity::mcp_command_id(&tool.name),
        title: tool.title.clone().unwrap_or_else(|| tool.name.clone()),
        description: tool.description.clone(),
        action_id: launcher_mcp::identity::MCP_ACTION_ID.into(),
        action_title: Some("Run".into()),
        action_type: crate::providers::mcp::MCP_CATALOG_ACTION_TYPE.into(),
        input_schema: (!tool.input_schema.is_null()).then(|| tool.input_schema.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> McpTool {
        serde_json::from_value(serde_json::json!({
            "server_id": "github",
            "name": "search_repositories",
            "title": "Search repositories",
            "description": "Search GitHub repositories",
            "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}},
            "annotations": {"readOnlyHint": true}
        }))
        .unwrap()
    }

    /// AI-MCP-001/002/003: MCP tools project into the unified catalog with
    /// identity and input schema preserved.
    #[test]
    fn mcp_item_projects_with_schema() {
        let it = mcp_catalog_item(&tool());
        assert_eq!(it.provider_id, "mcp:github");
        assert_eq!(it.command_id, "search_repositories");
        assert_eq!(it.action_id, "invoke");
        assert_eq!(it.action_type, "plugin.mcp.invoke");
        assert_eq!(it.title, "Search repositories");
        let schema = it.input_schema.expect("schema preserved");
        assert_eq!(schema["properties"]["query"]["type"], "string");
    }

    /// AI-MCP-004: no authority fields, ever.
    #[test]
    fn mcp_item_has_no_authority() {
        let flat = serde_json::to_string(&mcp_catalog_item(&tool())).unwrap();
        for banned in ["authorized", "confirmed", "granted", "effect", "execution_id", "enabled"] {
            assert!(!flat.contains(banned), "mcp catalog item leaks {banned}");
        }
    }

    /// §8: schema is present but never authority — the item type has no
    /// capability/confirmation fields to poison (structural guarantee).
    #[test]
    fn schema_is_not_authority() {
        let it = mcp_catalog_item(&tool());
        let json = serde_json::to_value(&it).unwrap();
        assert!(json.get("requires").is_none());
        assert!(json.get("confirmation").is_none());
        assert!(json.get("capabilities").is_none());
    }
}
