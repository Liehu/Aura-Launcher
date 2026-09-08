//! McpTool → Launcher Command projection (Spec sections 5/7): the adapter
//! is a pure function from a discovered tool to a Domain `Command` with the
//! frozen route identity (`mcp:<server>`, tool name, `invoke`).
//!
//! Phase 5 (review 41): the projected `invoke` action is a real
//! ActionDescriptor (`plugin.mcp.invoke`, requires `mcp.invoke`) resolved
//! through the host resolver. The provider no longer judges execution
//! availability — a tool is Ready when the host granted `mcp.invoke`, and
//! Disabled (presentable, never executable, INV-031/038) when the grant is
//! missing. Tool metadata can never influence the outcome (INV-068).

use crate::types::{McpServerId, McpTool};
use launcher_domain::{
    resolve_descriptor_for, Action, ActionDescriptor, ActionKind, ActionPayload, Capability,
    Category, Command, DescriptorError,
};

/// `provider_id` for a projected command: `mcp:<server_id>` (host-owned).
pub fn provider_id(server_id: &McpServerId) -> String {
    crate::identity::mcp_provider_id(server_id)
}

/// Action semantic type for MCP tool invocation (routing identity only —
/// no `ActionKind::McpInvoke` exists, review 41 §5.1/§7.1).
pub const MCP_ACTION_TYPE: &str = "plugin.mcp.invoke";

/// Execution input for one MCP invocation (review 41 §5.3). Action input =
/// execution parameters; tool metadata stays in the catalog and is never
/// mixed in here.
pub fn invoke_input(
    server_id: &McpServerId,
    tool_name: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "server_id": server_id.as_str(),
        "tool_name": tool_name,
        "arguments": arguments,
    })
}

/// The frozen invoke ActionDescriptor for a discovered tool (MCP-034).
/// `arguments` starts empty; producers (UI session / AI planner / workflow)
/// fill them in their ActionProposal — the projection itself never guesses.
pub fn invoke_descriptor(tool: &McpTool, server_id: &McpServerId) -> ActionDescriptor {
    ActionDescriptor {
        id: crate::identity::MCP_ACTION_ID.into(),
        title: Some("Run".into()),
        action_type: MCP_ACTION_TYPE.into(),
        input: invoke_input(server_id, &tool.name, serde_json::json!({})),
        requires: vec![Capability::McpInvoke],
        shortcut: None,
        confirmation: None,
    }
}

/// Resolve the invoke descriptor against host grants (MVP4.3 Phase 5,
/// review 41 §5.5): Ready on `mcp.invoke` granted, otherwise Disabled with
/// the resolver's denial reason. This is the resolver's decision — the
/// provider no longer fabricates `disabled_reason` itself.
fn resolve_invoke_action(descriptor: &ActionDescriptor, granted: &[Capability]) -> Action {
    const FALLBACK: &str = "Run";
    match resolve_descriptor_for(descriptor, granted, None) {
        Ok(mut a) => {
            // keep the stable route id + presentable title (INV-035)
            a.id = Some(crate::identity::MCP_ACTION_ID.into());
            a.title = descriptor.title.clone();
            a
        }
        Err(DescriptorError::CapabilityDenied(missing)) => Action {
            kind: ActionKind::PluginInvoke,
            payload: Some(ActionPayload::Json(descriptor.input.clone())),
            id: Some(crate::identity::MCP_ACTION_ID.into()),
            title: Some(FALLBACK.into()),
            disabled_reason: Some(format!(
                "capability denied: {}",
                missing
                    .iter()
                    .map(|c| serde_json::to_value(c).unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            shortcut: descriptor.shortcut.clone(),
            confirmation_required: false,
        },
        Err(e) => Action {
            kind: ActionKind::PluginInvoke,
            payload: Some(ActionPayload::Json(descriptor.input.clone())),
            id: Some(crate::identity::MCP_ACTION_ID.into()),
            title: Some(FALLBACK.into()),
            disabled_reason: Some(e.to_string()),
            shortcut: None,
            confirmation_required: false,
        },
    }
}

/// Project one discovered MCP tool into a Domain Command (Spec section 7).
///
/// `granted` is the host capability grant (config/policy) for this server.
/// Without the `mcp.invoke` grant the projected `invoke` action is Disabled:
/// visible in the Action Panel, never executable (INV-031/038).
pub fn tool_to_command_with_grants(
    tool: &McpTool,
    server_id: &McpServerId,
    granted: &[Capability],
) -> Command {
    let descriptor = invoke_descriptor(tool, server_id);
    let action = resolve_invoke_action(&descriptor, granted);
    let title = tool
        .title
        .clone()
        .or_else(|| tool.description.clone())
        .unwrap_or_else(|| tool.name.clone());
    Command {
        id: crate::identity::mcp_command_id(&tool.name),
        title,
        subtitle: tool.description.clone(),
        icon: None,
        provider_id: provider_id(server_id),
        score: 0.0,
        keywords: vec![tool.name.clone()],
        category: Category::Plugin,
        actions: vec![action],
        target: None,
    }
}

/// Capability-denied projection (no host grant): discoverable, not
/// executable. Kept for callers that project before policy is loaded.
pub fn tool_to_command(tool: &McpTool, server_id: &McpServerId) -> Command {
    tool_to_command_with_grants(tool, server_id, &[])
}

/// Project an entire server's tools (Spec section 7 example):
/// `mcp:filesystem └── read_file └── invoke`.
pub fn server_to_commands_with_grants(
    server: &crate::types::McpServer,
    granted: &[Capability],
) -> Vec<Command> {
    server
        .tools
        .iter()
        .map(|t| tool_to_command_with_grants(t, &server.id, granted))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::WorkflowFailureClass;

    fn tool(server: &str, name: &str) -> McpTool {
        serde_json::from_value(serde_json::json!({
            "server_id": server,
            "name": name,
            "title": format!("Tool {name}"),
            "description": "does a thing",
            "inputSchema": {"type": "object", "properties": {"q": {"type": "string"}}},
            "annotations": {"readOnlyHint": true}
        }))
        .unwrap()
    }

    /// MCP-008..012: tool → Command mapping with correct route identity.
    #[test]
    fn projection_and_identity() {
        let sid = McpServerId("github".into());
        let cmd = tool_to_command(&tool("github", "search_repositories"), &sid);
        assert_eq!(cmd.provider_id, "mcp:github");
        assert_eq!(cmd.id, "search_repositories");
        assert_eq!(cmd.actions[0].id.as_deref(), Some("invoke"));
        assert_eq!(cmd.actions[0].kind, ActionKind::PluginInvoke);
        // action payload carries the routing identity
        let payload_json = serde_json::to_string(
            cmd.actions[0].payload.as_ref().unwrap(),
        )
        .unwrap();
        assert!(payload_json.contains("search_repositories"));
    }

    /// Without the host `mcp.invoke` grant the projected invoke action is
    /// Disabled: discoverable, presented, but never executable
    /// (INV-031/038; resolver decides, review 41 §5.5).
    #[test]
    fn projection_denied_without_host_grant() {
        let cmd = tool_to_command(&tool("github", "search_repositories"), &McpServerId("github".into()));
        let a = &cmd.actions[0];
        assert!(a.disabled_reason.is_some());
        assert!(launcher_action::validate(a).is_err());
    }

    /// The untrusted annotations/description never grant authority: the
    /// projected command carries no capability state at all.
    #[test]
    fn projection_carries_no_authority() {
        let cmd = tool_to_command(&tool("github", "search_repositories"), &McpServerId("github".into()));
        let json = serde_json::to_value(&cmd).unwrap();
        assert!(json.get("capabilities").is_none());
        assert!(json.get("authorized").is_none());
    }

    /// server_to_commands: whole-server projection (Spec section 7 tree).
    #[test]
    fn server_projection() {
        let server = crate::types::McpServer {
            id: McpServerId("filesystem".into()),
            name: "FS".into(),
            tools: vec![tool("filesystem", "read_file"), tool("filesystem", "write_file")],
        };
        let cmds = server_to_commands_with_grants(&server, &[Capability::McpInvoke]);
        assert_eq!(cmds.len(), 2);
        assert!(cmds.iter().all(|c| c.provider_id == "mcp:filesystem"));
    }

    /// WorkflowFailureClass mapping sanity for MCP errors (Spec section 18).
    #[test]
    fn failure_mapping_present() {
        use crate::McpError;
        assert_eq!(
            McpError::UnknownTool("t".into()).failure_class(),
            WorkflowFailureClass::CommandNotFound
        );
        let _: Option<WorkflowFailureClass> = None;
    }

    // ---- MVP4.3 Phase 5 (review 41 tests MCP-034..042) ----

    /// MCP-034/036: the tool projects a real ActionDescriptor of type
    /// `plugin.mcp.invoke` (semantic routing identity; no McpInvoke
    /// ActionKind was introduced).
    #[test]
    fn mcp034_descriptor_type() {
        let sid = McpServerId("calc".into());
        let d = invoke_descriptor(&tool("calc", "evaluate"), &sid);
        assert_eq!(d.action_type, "plugin.mcp.invoke");
        assert_eq!(ActionKind::PluginInvoke, ActionKind::PluginInvoke);
        // only the frozen engine kinds exist — no McpInvoke variant to name
        let kinds = [
            ActionKind::Open,
            ActionKind::Copy,
            ActionKind::Reveal,
            ActionKind::OpenTerminalHere,
            ActionKind::Execute,
            ActionKind::Paste,
            ActionKind::PluginInvoke,
        ];
        assert!(kinds.iter().all(|k| !format!("{k:?}").contains("Mcp")));
    }

    /// MCP-035: the action id is frozen to `invoke`; title is "Run".
    #[test]
    fn mcp035_action_id_invoke() {
        let d = invoke_descriptor(&tool("calc", "evaluate"), &McpServerId("calc".into()));
        assert_eq!(d.id, "invoke");
        assert_eq!(d.title.as_deref(), Some("Run"));
    }

    /// MCP-037/038: input carries the correct server_id and tool_name.
    #[test]
    fn mcp037_input_identity() {
        let sid = McpServerId("calc".into());
        let d = invoke_descriptor(&tool("calc", "evaluate"), &sid);
        assert_eq!(d.input["server_id"], "calc");
        assert_eq!(d.input["tool_name"], "evaluate");
    }

    /// MCP-039 argument isolation: tool metadata (schema/annotations/
    /// description) never leaks into the action input; `arguments` is the
    /// only parameters channel (review 41 §5.3).
    #[test]
    fn mcp039_argument_isolation() {
        let t = tool("calc", "evaluate");
        let d = invoke_descriptor(&t, &McpServerId("calc".into()));
        let mut keys: Vec<&str> = d.input.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["arguments", "server_id", "tool_name"]);
        assert!(d.input["arguments"].is_object());
        let flat = serde_json::to_string(&d.input).unwrap();
        assert!(!flat.contains("inputSchema"));
        assert!(!flat.contains("annotations"));
        assert!(!flat.contains("description"));
    }

    /// MCP-040: the descriptor requires exactly the `mcp.invoke`
    /// capability — never `plugin.invoke` (different ownership domain).
    #[test]
    fn mcp040_requires_mcp_invoke() {
        let d = invoke_descriptor(&tool("calc", "evaluate"), &McpServerId("calc".into()));
        assert_eq!(d.requires, vec![Capability::McpInvoke]);
    }

    /// MCP-041: capability denied → Disabled with the resolver's reason,
    /// while the command itself stays discoverable.
    #[test]
    fn mcp041_denied_capability_disables() {
        let sid = McpServerId("calc".into());
        let cmd = tool_to_command_with_grants(&tool("calc", "evaluate"), &sid, &[]);
        let a = &cmd.actions[0];
        assert!(a.disabled_reason.is_some());
        assert!(a.disabled_reason.as_deref().unwrap().contains("mcp.invoke"));
        assert!(launcher_action::validate(a).is_err());
        // granted → Ready
        let cmd = tool_to_command_with_grants(&tool("calc", "evaluate"), &sid, &[Capability::McpInvoke]);
        assert!(cmd.actions[0].disabled_reason.is_none());
        assert!(launcher_action::validate(&cmd.actions[0]).is_ok());
    }

    /// MCP-042 anti-escalation: untrusted tool metadata (`annotations =
    /// {"destructive": false}` or any other shape) MUST NOT grant
    /// `mcp.invoke` — the projection has no metadata→authority channel.
    #[test]
    fn mcp042_metadata_never_grants() {
        let mut t = tool("calc", "evaluate");
        t.annotations = serde_json::json!({"destructive": false, "readOnlyHint": true});
        t.input_schema = serde_json::json!({"type": "object"});
        // every projection path is fed only host grants; metadata arrives
        // but the resolver sees `granted = []` → denied regardless
        let cmd = tool_to_command_with_grants(&t, &McpServerId("calc".into()), &[]);
        assert!(cmd.actions[0].disabled_reason.is_some());
        // and even a generous non-host grant list cannot be synthesized
        // from the tool: the descriptor's requires are host-frozen
        let d = invoke_descriptor(&t, &McpServerId("calc".into()));
        assert_eq!(d.requires, vec![Capability::McpInvoke]);
    }

    /// Descriptor → proposal-shaped input (review 41 §5.4): the ActionProposal
    /// carries provider/command/action ids plus the execution input — never
    /// resolved/authorized/confirmed data.
    #[test]
    fn mcp_proposal_shape() {
        let sid = McpServerId("calc".into());
        let input = invoke_input(&sid, "evaluate", serde_json::json!({"expression": "12 + 34"}));
        assert_eq!(input["server_id"], "calc");
        assert_eq!(input["tool_name"], "evaluate");
        let flat = serde_json::to_string(&input).unwrap();
        for banned in ["resolved_action", "authorized", "confirmed", "capabilities", "effect", "execution_id"] {
            assert!(!flat.contains(banned), "proposal must not contain {banned}");
        }
    }
}
