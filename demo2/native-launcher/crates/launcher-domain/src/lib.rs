//! Pure domain model: Command / Action / Provider / Plugin manifest.
//!
//! This crate must stay free of OS and IO dependencies.

pub mod expr;
pub mod pinyin;
pub mod failure;
pub mod file_change;
pub mod icon;
pub mod workflow;
pub use expr::{parse_expr, evaluate, Expr, Value, ExprError};
pub use icon::{IconKey, IconSource, IconVariant};
pub use failure::{FailureClass, RecoveryPolicy};
pub use file_change::{FileChange, FileChangeKind, FileObservation, IndexHealth, IndexStatus, VerifiedChange};
pub use workflow::{
    Condition, OutputBinding, VariableDeclaration, WorkflowInputDeclaration,
    WorkflowLimits, SKIP_BRANCH_NOT_SELECTED, SKIP_CONDITION_FALSE,
    ActionReference, FailureAction, RetryPolicy, StepFailurePolicy, StepRun, StepRunStatus,
    WorkflowAction, WorkflowDefinition, WorkflowFailureClass, WorkflowFailurePolicy, WorkflowRun,
    WorkflowRunStatus, WorkflowStep,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The set of effects the Action Engine can perform (design spec 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    Open,
    Copy,
    Reveal,
    OpenTerminalHere,
    Execute,
    /// Paste the current clipboard into the previously focused window
    /// (MVP3.2 `system.paste`; a normal Host-owned effect via the engine).
    Paste,
    /// `plugin.<id>.*` (MVP4.0 / ADR-0014): validated by the resolver; the
    /// Effect is produced by the PluginBroker through the plugin's
    /// `execute_action` RPC. The broker is an effect executor under the
    /// engine, never an independent gateway (INV-047).
    PluginInvoke,
    /// `system.run_as_admin` (P2-C): ShellExecute "runas" on an executable
    /// target. Host-owned effect via the engine, like Open/Execute.
    RunAsAdmin,
}

/// A single payload an action operates on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPayload {
    Path(String),
    Text(String),
    CommandLine(String),
    /// Target window for `system.paste` (the pre-popup foreground the host
    /// restores before sending the keystroke).
    Hwnd(i64),
    /// Free-form JSON input carried by `PluginInvoke` actions.
    Json(serde_json::Value),
    None,
}

/// One executable action attached to a command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub kind: ActionKind,
    #[serde(default)]
    pub payload: Option<ActionPayload>,
    /// Stable within the containing Command (ACTION-CONTRACT §4); plugin
    /// descriptors keep their id here so the UI can select by id (INV-035).
    #[serde(default)]
    pub id: Option<String>,
    /// Presentable label for the Action Panel.
    #[serde(default)]
    pub title: Option<String>,
    /// `Some(reason)` = Disabled: presentable but non-executable
    /// (e.g. capability denied at resolution time). `None` = Ready.
    /// Hidden actions never reach a Command at all (INV-031/038).
    #[serde(default)]
    pub disabled_reason: Option<String>,
    /// Accelerator label + dispatch key, e.g. "Ctrl+Shift+C" (MVP3.2-A).
    #[serde(default)]
    pub shortcut: Option<String>,
    /// Execution policy (MVP3.2-B, INV-041): the engine refuses to execute
    /// until the host confirms (clears this flag on user confirmation).
    #[serde(default)]
    pub confirmation_required: bool,
}

/// Plugin-provided action declaration (ACTION-CONTRACT-v0.1 §1).
///
/// Untrusted input (INV-026): always pass through [`resolve_descriptor`]
/// before the Action Engine sees it. Wire format shared with
/// `launcher-ipc::PluginResultItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDescriptor {
    /// Stable within the containing Command (INV-028 层次); not globally unique.
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    /// `system.*` supported in v0.1; `plugin.<id>.*` reserved/unsupported.
    #[serde(rename = "type")]
    pub action_type: String,
    /// Type-specific input object (ACTION-CONTRACT §3 table).
    #[serde(default)]
    pub input: serde_json::Value,
    /// MUST be a subset of the manifest's declared capabilities (INV-027).
    #[serde(default)]
    pub requires: Vec<Capability>,
    /// Optional accelerator, e.g. "Ctrl+Shift+C". Dispatch resolves to the
    /// stable action_id; it never binds an effect directly (INV-039).
    #[serde(default)]
    pub shortcut: Option<String>,
    /// `"none"` (default) or `"confirm"`. Confirmation is an EXECUTION
    /// POLICY state (INV-041); the host may also require confirmation by
    /// capability policy (shell.execute / process.spawn).
    #[serde(default)]
    pub confirmation: Option<String>,
}

/// Why a descriptor could not become a [`ResolvedAction`](Action).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DescriptorError {
    #[error("unknown or unsupported action type: {0}")]
    UnknownActionType(String),
    #[error("invalid input for {0}")]
    InvalidInput(String),
    #[error("capability denied: {0:?}")]
    CapabilityDenied(Vec<Capability>),
}

/// Resolve an untrusted descriptor into an executable domain Action
/// (COMMAND/ACTION-CONTRACT v0.1, ADR-0011). Pure: no IO, no UI — the
/// resolver is UI-independent and reusable by AI/Workflow callers.
///
/// Capability monotonicity (INV-027): `requires` outside `granted` (the
/// manifest declaration) is denied — an action can never increase the
/// plugin's authority.
pub fn resolve_descriptor(
    d: &ActionDescriptor,
    granted: &[Capability],
) -> Result<Action, DescriptorError> {
    resolve_descriptor_for(d, granted, None)
}

/// Resolver with plugin routing (MVP4.0 / ADR-0014). `own_plugin` is the
/// host-verified manifest id of the plugin that produced the descriptor:
/// `plugin.<id>.*` types resolve ONLY when `<id>` equals it (cross-plugin
/// invocation is denied), and implicitly require the `plugin.invoke`
/// capability on top of the declared `requires` (INV-027 still applies).
/// `None` keeps the pre-MVP4 behavior (`plugin.*` = unknown type).
pub fn resolve_descriptor_for(
    d: &ActionDescriptor,
    granted: &[Capability],
    own_plugin: Option<&str>,
) -> Result<Action, DescriptorError> {
    let mut effective_requires: Vec<Capability> = d.requires.clone();
    // MVP4.3 Phase 5 (review 41 §5.1/§5.2): `plugin.mcp.invoke` is MCP
    // semantic routing identity, not a plugin namespace — it requires the
    // host-granted `mcp.invoke` capability and never `plugin.invoke`, and
    // knows no own-plugin binding.
    if d.action_type == "plugin.mcp.invoke" {
        if !effective_requires.contains(&Capability::McpInvoke) {
            effective_requires.push(Capability::McpInvoke);
        }
        return resolve_system(d, granted, &effective_requires, own_plugin);
    }
    if let Some(rest) = d.action_type.strip_prefix("plugin.") {
        match own_plugin {
            None => return Err(DescriptorError::UnknownActionType(d.action_type.clone())),
            Some(own) => {
                if !owns_plugin_namespace(own, rest) {
                    return Err(DescriptorError::InvalidInput(format!(
                        "cross-plugin action denied: {} (producer is {own})",
                        d.action_type
                    )));
                }
            }
        }
        if !effective_requires.contains(&Capability::PluginInvoke) {
            effective_requires.push(Capability::PluginInvoke);
        }
    }
    resolve_system(d, granted, &effective_requires, own_plugin)
}

/// Identity fields of a plugin-class action payload whose values are bound
/// to the host-resolved route (MVP4.3 Phase 10, review 45 A-003): runtime
/// input may provide execution parameters, but MUST NOT re-point these at
/// another server/tool than the one the Reference resolved to. This closes
/// the tool-substitution confused-deputy: `Reference = evaluate` +
/// `input.tool_name = delete_all` is rejected instead of executed.
pub const ACTION_IDENTITY_FIELDS: &[&str] = &["server_id", "tool_name"];

/// Compare a host-projected payload against a runtime-provided input and
/// return the identity fields whose values conflict. Fields absent from
/// the runtime input are fine (the projection remains authoritative);
/// present-and-different is a substitution attempt. Pure function so the
/// runner, registry and tests share one definition.
pub fn identity_mismatch(projected: &serde_json::Value, runtime: &serde_json::Value) -> Vec<&'static str> {
    let (Some(proj), Some(rt)) = (projected.as_object(), runtime.as_object()) else {
        return Vec::new();
    };
    ACTION_IDENTITY_FIELDS
        .iter()
        .copied()
        .filter(|f| match (proj.get(*f), rt.get(*f)) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        })
        .collect()
}

/// Namespace ownership for `plugin.<id>.*` action types (ADR-0014 §3):/// `target` is owned by `owner` when it equals `owner` exactly, or extends
/// it at a segment boundary (`owner + "." + ...`). A shared prefix without
/// the dot separator (`com.example.calc` vs `com.example.calculator`) is
/// NOT ownership. Comparison is case-sensitive; empty either side is false.
/// Security boundary (INV-047): kept as a pure function with an exhaustive
/// test matrix.
pub fn owns_plugin_namespace(owner: &str, target: &str) -> bool {
    if owner.is_empty() || target.is_empty() {
        return false;
    }
    target == owner
        || target
            .strip_prefix(owner)
            .is_some_and(|rest| rest.starts_with('.'))
}

fn resolve_system(
    d: &ActionDescriptor,
    granted: &[Capability],
    effective_requires: &[Capability],
    own_plugin: Option<&str>,
) -> Result<Action, DescriptorError> {
    let missing: Vec<Capability> = effective_requires
        .iter()
        .filter(|c| !granted.contains(c))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(DescriptorError::CapabilityDenied(missing));
    }
    let input = &d.input;
    let get_str = |key: &str| -> Option<String> {
        input.get(key).and_then(|v| v.as_str()).map(str::to_string)
    };
    let base = |kind: ActionKind, payload: Option<ActionPayload>| Action {
        kind,
        payload,
        id: None,
        title: None,
        disabled_reason: None,
        shortcut: d.shortcut.clone(),
        confirmation_required: d.confirmation.as_deref() == Some("confirm")
            || d.requires
                .iter()
                .any(|c| matches!(c, Capability::ShellExecute | Capability::ProcessSpawn)),
    };
    let invalid = || DescriptorError::InvalidInput(d.action_type.clone());
    match d.action_type.as_str() {
        "system.copy_to_clipboard" => get_str("text")
            .map(|text| base(ActionKind::Copy, Some(ActionPayload::Text(text))))
            .ok_or_else(invalid),
        "system.open" => get_str("target")
            .map(|t| base(ActionKind::Open, Some(ActionPayload::Path(t))))
            .ok_or_else(invalid),
        "system.reveal" => get_str("path")
            .map(|p| base(ActionKind::Reveal, Some(ActionPayload::Path(p))))
            .ok_or_else(invalid),
        "system.open_terminal_here" => get_str("path")
            .map(|p| base(ActionKind::OpenTerminalHere, Some(ActionPayload::Path(p))))
            .ok_or_else(invalid),
        "system.execute" => get_str("command_line")
            .map(|c| base(ActionKind::Execute, Some(ActionPayload::CommandLine(c))))
            .ok_or_else(invalid),
        // MVP3.2: pastes the current clipboard into the previously focused
        // window; a normal Host-owned effect via the Action Engine (INV-045).
        "system.paste" => Ok(base(ActionKind::Paste, None)),
        // MVP4.0 (ADR-0014): plugin-owned actions reach here only when the
        // routing in `resolve_descriptor_for` accepted the target; the
        // Effect itself is produced by the PluginBroker under the engine.
        ty if ty.starts_with("plugin.") => {
            if !d.input.is_object() {
                return Err(invalid());
            }
            let _ = own_plugin;
            Ok(base(
                ActionKind::PluginInvoke,
                Some(ActionPayload::Json(d.input.clone())),
            ))
        }
        other => Err(DescriptorError::UnknownActionType(other.to_string())),
    }
}

/// Stable identifier for a command within a provider.
pub type CommandId = String;

/// The unified discoverable/executable object (design spec 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub id: CommandId,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub provider_id: String,
    #[serde(default)]
    pub score: f32,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub category: Category,
    #[serde(default)]
    pub actions: Vec<Action>,
    /// Path / command line used by Open/Execute actions.
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    #[default]
    Application,
    File,
    Folder,
    Command,
    Plugin,
}

impl Command {
    /// The primary action: the FIRST READY action in declaration order
    /// (primary = first Ready, COMMAND-CONTRACT §5). Disabled actions are
    /// skipped, never executed by Enter.
    pub fn primary_action(&self) -> Option<&Action> {
        self.actions.iter().find(|a| a.disabled_reason.is_none())
    }
}

/// Query sent to providers.
#[derive(Debug, Clone, Default)]
pub struct QueryContext {
    pub raw: String,
    pub normalized: String,
    pub tokens: Vec<String>,
}

impl QueryContext {
    pub fn parse(raw: &str) -> Self {
        let normalized = raw
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let tokens: Vec<String> = normalized.split_whitespace().map(str::to_string).collect();
        Self {
            raw: raw.to_string(),
            normalized,
            tokens,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ContextMeta {
    pub timestamp_ms: u64,
    pub confidence: f32,
}

/// A snapshot of desktop context (design spec 6).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSnapshot {
    #[serde(default)]
    pub foreground_app: Option<String>,
    #[serde(default)]
    pub current_folder: Option<String>,
    #[serde(default)]
    pub selected_items: Vec<String>,
    #[serde(default)]
    pub clipboard_type: Option<String>,
    #[serde(default)]
    pub meta: Option<ContextMeta>,
}

/// Plugin capabilities (PLUGIN-CONTRACT-v0.1 §10).
///
/// Wire format uses the contract's dotted names (`network.connect`,
/// `context.location.read`, ...); the Rust variants stay idiomatic.
/// The old snake_case wire names were an unpublished implementation detail
/// and are NOT accepted (protocol change documented in ADR-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    #[serde(rename = "filesystem.read")]
    FilesystemRead,
    #[serde(rename = "filesystem.write")]
    FilesystemWrite,
    #[serde(rename = "clipboard.read")]
    ClipboardRead,
    #[serde(rename = "clipboard.write")]
    ClipboardWrite,
    #[serde(rename = "network.connect")]
    Network,
    #[serde(rename = "shell.execute")]
    ShellExecute,
    #[serde(rename = "process.launch")]
    ProcessSpawn,
    #[serde(rename = "notification.send")]
    Notifications,
    #[serde(rename = "ui.render")]
    UiRender,
    #[serde(rename = "context.process.read")]
    ContextProcessRead,
    #[serde(rename = "context.window.read")]
    ContextWindowRead,
    #[serde(rename = "context.location.read")]
    ContextLocationRead,
    #[serde(rename = "context.selection.read")]
    ContextSelectionRead,
    #[serde(rename = "store.read")]
    StoreRead,
    #[serde(rename = "store.write")]
    StoreWrite,
    /// Host -> Plugin effect channel for `plugin.<id>.*` actions (MVP4.0).
    /// Target binding (own-plugin only) is enforced separately by the host.
    #[serde(rename = "plugin.invoke")]
    PluginInvoke,
    /// Host -> MCP tool execution capability (MVP4.3 Phase 5, review 41 §5.2).
    /// Same Capability Model, different ownership domain from
    /// `plugin.invoke`: granted ONLY by host configuration/policy, never by
    /// MCP tool metadata (INV-068).
    #[serde(rename = "mcp.invoke")]
    McpInvoke,
}

/// How the plugin process is launched (PLUGIN-CONTRACT-v0.1 §5, ADR-0009).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSpec {
    /// `"process"` (native executable) or `"python"` (script run by the
    /// configured interpreter; `executable` is the script path).
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    /// Plugin's own release version (contract §4); optional in v0.1 manifests.
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default = "default_plugin_version")]
    pub api_version: String,
    /// Manifest data-format version; when present it MUST be 1.
    #[serde(default)]
    pub schema_version: Option<u64>,
    /// Legacy flat form; optional so pure v2 manifests (runtime.executable)
    /// also parse. Precedence: `runtime.executable` wins when declared.
    #[serde(default)]
    pub executable: String,
    #[serde(default)]
    pub runtime: Option<RuntimeSpec>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_idle_timeout_ms")]
    pub idle_timeout_ms: u64,
    /// Interpreter for `runtime.type = "python"`, resolved by the host app
    /// from user config / `LAUNCHER_PYTHON` (ADR-0009). Not part of the
    /// manifest wire format.
    #[serde(skip)]
    pub interpreter: Option<std::path::PathBuf>,
    /// Where the interpreter came from ("config" / "env:LAUNCHER_PYTHON" /
    /// "path"); diagnostics only, travels with `interpreter` into
    /// `LaunchPlan.resolved_from` (ADR-0010 §2).
    #[serde(skip)]
    pub interpreter_source: Option<String>,
}

fn default_plugin_version() -> String {
    "0.1".into()
}
fn default_timeout_ms() -> u64 {
    2000
}
fn default_idle_timeout_ms() -> u64 {
    10_000
}

#[derive(Debug, Clone, PartialEq, Error)]
#[error("manifest error: {0}")]
pub struct ManifestError(pub String);

impl PluginManifest {
    pub fn parse(json: &str) -> Result<Self, ManifestError> {
        let m: PluginManifest =
            serde_json::from_str(json).map_err(|e| ManifestError(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.id.trim().is_empty() {
            return Err(ManifestError("id is required".into()));
        }
        if self.name.trim().is_empty() {
            return Err(ManifestError("name is required".into()));
        }
        if self.api_version != "0.1" {
            return Err(ManifestError(format!(
                "unsupported api_version: {}",
                self.api_version
            )));
        }
        if let Some(v) = self.schema_version {
            if v != 1 {
                return Err(ManifestError(format!("unsupported schema_version: {v}")));
            }
        }
        if let Some(rt) = &self.runtime {
            if rt.kind != "process" && rt.kind != "python" {
                return Err(ManifestError(format!(
                    "unsupported runtime.type: {} (only \"process\" and \"python\")",
                    rt.kind
                )));
            }
        }
        if self.effective_executable().trim().is_empty() {
            return Err(ManifestError("executable is required".into()));
        }
        if self.timeout_ms == 0 || self.timeout_ms > 60_000 {
            return Err(ManifestError("timeout_ms out of range (1..=60000)".into()));
        }
        Ok(())
    }

    /// Host-side allow/deny check (MVP permission model).
    pub fn requests(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Executable to spawn: `runtime.executable` when declared, else the
    /// legacy top-level `executable` (contract §5; both stay relative to the
    /// plugin directory).
    pub fn effective_executable(&self) -> &str {
        self.runtime
            .as_ref()
            .and_then(|rt| rt.executable.as_deref())
            .unwrap_or(&self.executable)
    }

    /// Extra argv for the plugin process (contract §5 `runtime.args`).
    pub fn spawn_args(&self) -> &[String] {
        self.runtime
            .as_ref()
            .map(|rt| rt.args.as_slice())
            .unwrap_or(&[])
    }

    /// True when the manifest declares the Python script runtime.
    pub fn is_python(&self) -> bool {
        self.runtime
            .as_ref()
            .map(|rt| rt.kind == "python")
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_serialization_roundtrip() {
        let c = Command {
            id: "1".into(),
            title: "VS Code".into(),
            subtitle: Some("Editor".into()),
            icon: None,
            provider_id: "apps".into(),
            score: 1.0,
            keywords: vec!["code".into()],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: Some("code".into()),
        };
        let s = serde_json::to_string(&c).unwrap();
        let back: Command = serde_json::from_str(&s).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn query_parse_normalizes() {
        let q = QueryContext::parse("  Foo   BAR ");
        assert_eq!(q.normalized, "foo bar");
        assert_eq!(q.tokens, vec!["foo", "bar"]);
    }

    #[test]
    fn manifest_parse_and_validate() {
        let ok = r#"{"id":"echo","name":"Echo","executable":"echo.exe"}"#;
        assert!(PluginManifest::parse(ok).is_ok());

        let missing_exec = r#"{"id":"echo","name":"Echo"}"#;
        assert!(PluginManifest::parse(missing_exec).is_err());

        let bad_version = r#"{"id":"e","name":"E","executable":"x","api_version":"9.9"}"#;
        assert!(PluginManifest::parse(bad_version).is_err());

        let bad_timeout = r#"{"id":"e","name":"E","executable":"x","timeout_ms":0}"#;
        assert!(PluginManifest::parse(bad_timeout).is_err());
    }

    #[test]
    fn manifest_v2_schema_and_runtime_fields() {
        let ok = r#"{
            "schema_version": 1,
            "id": "com.example.calc",
            "name": "Calc",
            "version": "1.0.0",
            "api_version": "0.1",
            "executable": "plugin-calculator.exe",
            "runtime": { "type": "process", "executable": "bin/calc.exe", "args": ["--stdio"] }
        }"#;
        let m = PluginManifest::parse(ok).unwrap();
        assert_eq!(m.effective_executable(), "bin/calc.exe");
        assert_eq!(m.spawn_args(), ["--stdio"]);
        assert_eq!(m.version.as_deref(), Some("1.0.0"));

        let bad_schema = r#"{"id":"e","name":"E","executable":"x","schema_version":2}"#;
        assert!(PluginManifest::parse(bad_schema).is_err());

        let bad_runtime = r#"{"id":"e","name":"E","executable":"x","runtime":{"type":"wasm"}}"#;
        assert!(PluginManifest::parse(bad_runtime).is_err());

        // legacy flat manifest still loads; runtime.executable wins when set
        let legacy =
            PluginManifest::parse(r#"{"id":"e","name":"E","executable":"legacy.exe"}"#).unwrap();
        assert_eq!(legacy.effective_executable(), "legacy.exe");
        assert!(legacy.spawn_args().is_empty());
    }

    #[test]
    fn capabilities_use_dotted_contract_names() {
        let m: PluginManifest = serde_json::from_value(serde_json::json!({
            "id": "p", "name": "P", "executable": "x",
            "capabilities": ["network.connect", "context.location.read", "store.read"]
        }))
        .unwrap();
        assert!(m.requests(Capability::Network));
        assert!(m.requests(Capability::ContextLocationRead));
        assert!(m.requests(Capability::StoreRead));
        assert!(!m.requests(Capability::FilesystemWrite));

        // wire roundtrip keeps the dotted names
        assert_eq!(
            serde_json::to_value(Capability::ContextSelectionRead).unwrap(),
            "context.selection.read"
        );

        // legacy snake_case wire names are rejected (ADR-0007)
        assert!(serde_json::from_value::<PluginManifest>(serde_json::json!({
            "id": "p", "name": "P", "executable": "x", "capabilities": ["filesystem_read"]
        }))
        .is_err());
    }

    #[test]
    fn resolve_descriptor_system_types() {
        let d = |ty: &str, input: serde_json::Value| ActionDescriptor {
            id: "a".into(),
            title: None,
            action_type: ty.into(),
            input,
            requires: vec![],
            shortcut: None,
            confirmation: None,
        };
        let a = resolve_descriptor(
            &d(
                "system.copy_to_clipboard",
                serde_json::json!({"text": "80"}),
            ),
            &[],
        )
        .unwrap();
        assert_eq!(a.kind, ActionKind::Copy);

        let a = resolve_descriptor(
            &d("system.open", serde_json::json!({"target": "C:\\\\x.txt"})),
            &[],
        )
        .unwrap();
        assert_eq!(a.kind, ActionKind::Open);

        // unknown type is never executed; plugin.* without host routing
        // (own_plugin=None) stays UnknownActionType (legacy wrapper)
        assert!(matches!(
            resolve_descriptor(&d("plugin.foo.magic", serde_json::json!({})), &[]),
            Err(DescriptorError::UnknownActionType(_))
        ));
        // missing input
        assert!(matches!(
            resolve_descriptor(&d("system.copy_to_clipboard", serde_json::json!({})), &[]),
            Err(DescriptorError::InvalidInput(_))
        ));
    }

    #[test]
    fn owns_plugin_namespace_boundary_matrix() {
        let owns = owns_plugin_namespace;
        let own = "com.example.calc";
        // exact owner and segment-boundary extensions are owned
        assert!(owns(own, "com.example.calc"));
        assert!(owns(own, "com.example.calc.echo"));
        assert!(owns(own, "com.example.calc.v2.echo"));
        // same prefix WITHOUT the dot separator is a different plugin
        assert!(!owns(own, "com.example.calculator"));
        assert!(!owns(own, "com.example.calculator.echo"));
        // short prefixes and unrelated ids are not owned
        assert!(!owns(own, "com.example"));
        assert!(!owns(own, "com.other.calc.echo"));
        // case variants are distinct (ids are case-sensitive)
        assert!(!owns(own, "Com.Example.Calc.echo"));
        // empty either side is never owned
        assert!(!owns(own, ""));
        assert!(!owns("", "com.example.calc.echo"));
    }

    #[test]
    fn plugin_action_routing_mvp4() {
        let d = |ty: &str| ActionDescriptor {
            id: "export".into(),
            title: None,
            action_type: ty.into(),
            input: serde_json::json!({"format": "pdf"}),
            requires: vec![],
            shortcut: None,
            confirmation: None,
        };
        // own plugin + plugin.invoke granted -> PluginInvoke resolved
        let a = resolve_descriptor_for(
            &d("plugin.calc.export"),
            &[Capability::PluginInvoke],
            Some("calc"),
        )
        .unwrap();
        assert_eq!(a.kind, ActionKind::PluginInvoke);

        // plugin.invoke is implicitly required (INV-027)
        assert!(matches!(
            resolve_descriptor_for(&d("plugin.calc.export"), &[], Some("calc")),
            Err(DescriptorError::CapabilityDenied(_))
        ));

        // cross-plugin targets are denied (host identity binding)
        assert!(matches!(
            resolve_descriptor_for(
                &d("plugin.other.export"),
                &[Capability::PluginInvoke],
                Some("calc")
            ),
            Err(DescriptorError::InvalidInput(_))
        ));
    }

    /// MVP4.3 Phase 5 (MCP-040/041, review 41 §5.2): `plugin.mcp.invoke`
    /// resolves only against the host-granted `mcp.invoke` capability —
    /// `plugin.invoke` is a different ownership domain and never substitutes.
    #[test]
    fn resolve_descriptor_mcp_invoke_routing() {
        let d = ActionDescriptor {
            id: "invoke".into(),
            title: Some("Run".into()),
            action_type: "plugin.mcp.invoke".into(),
            input: serde_json::json!({
                "server_id": "calc",
                "tool_name": "evaluate",
                "arguments": {"expression": "1 + 1"}
            }),
            requires: vec![],
            shortcut: None,
            confirmation: None,
        };
        // no host grant -> denied
        assert!(matches!(
            resolve_descriptor(&d, &[]),
            Err(DescriptorError::CapabilityDenied(missing))
            if missing == vec![Capability::McpInvoke]
        ));
        // host grant -> ready, engine-classified as PluginInvoke (routing
        // classification, review 41 §7.2)
        let a = resolve_descriptor(&d, &[Capability::McpInvoke]).unwrap();
        assert_eq!(a.kind, ActionKind::PluginInvoke);
        assert!(a.disabled_reason.is_none());
        // plugin.invoke alone does NOT authorize mcp.invoke
        assert!(matches!(
            resolve_descriptor(&d, &[Capability::PluginInvoke]),
            Err(DescriptorError::CapabilityDenied(_))
        ));
        // non-object input is rejected (defense line 1 of §7.5)
        let bad = ActionDescriptor {
            input: serde_json::json!("nope"),
            ..d.clone()
        };
        assert!(matches!(
            resolve_descriptor(&bad, &[Capability::McpInvoke]),
            Err(DescriptorError::InvalidInput(_))
        ));
        // untrusted annotations can never grant: resolver input has no
        // metadata channel at all (MCP-042 authority rule)
        assert_eq!(d.requires, Vec::<Capability>::new());
    }

    #[test]
    fn resolve_descriptor_capability_monotonicity() {
        let d = ActionDescriptor {
            id: "copy".into(),
            title: None,
            action_type: "system.copy_to_clipboard".into(),
            input: serde_json::json!({"text": "80"}),
            requires: vec![Capability::ClipboardWrite],
            shortcut: None,
            confirmation: None,
        };
        // manifest declares nothing -> denied (no auto-grant, INV-027)
        assert!(matches!(
            resolve_descriptor(&d, &[]),
            Err(DescriptorError::CapabilityDenied(_))
        ));
        // manifest declares it -> ready
        assert!(resolve_descriptor(&d, &[Capability::ClipboardWrite]).is_ok());
    }

    #[test]
    fn capability_check() {
        let m: PluginManifest = serde_json::from_str(
            r#"{"id":"e","name":"E","executable":"x","capabilities":["network.connect"]}"#,
        )
        .unwrap();
        assert!(m.requests(Capability::Network));
        assert!(!m.requests(Capability::ShellExecute));
    }
}

/// Canonical identity form for filesystem paths (P2.1-B, review 70 §14/29):
/// purely LEXICAL — lowercase + forward/back separator fold + `.`/`..`
/// segment collapse. Never touches the disk (no realpath), so junctions and
/// symlinks cannot smuggle traversal into identity resolution. Used ONLY for
/// identity comparison/dedup; display and execution keep the original path
/// (Windows path semantics are unchanged).
/// Device/Volume namespace (`\.\PhysicalDrive0`, `\.\C:`): NOT a
/// filesystem path (review 74 §3) — never a valid FileId input. Callers
/// index/search real files only; the identity fn keeps these verbatim
/// (lowercased) so they cannot collide with any filesystem identity.
pub fn is_device_namespace(raw: &str) -> bool {
    raw.starts_with(r"\\.\")
}

pub fn normalize_path_identity(raw: &str) -> String {
    if is_device_namespace(raw) {
        // device namespace survives UNTOUCHED (shape-preserved, case
        // folded) — it can never equal a real filesystem path identity
        let mut c = String::with_capacity(raw.len());
        for ch in raw.chars() {
            c.extend(ch.to_lowercase());
        }
        return c;
    }
    // Windows rules (review 72 §8), all LEXICAL:
    //  - `\\?\` device prefix is stripped (`\\?\C:\foo` == `C:\foo`);
    //  - `\\?\UNC\server\share` == `\\server\share`;
    //  - UNC paths keep their leading separators and can NEVER equal a
    //    drive path (`\\server\share` != `c:\...`);
    //  - trailing separators fold (empty segments drop);
    //  - `.` segments drop, `..` pops.
    let raw = if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        let mut s = String::from(r"\\");
        s.push_str(rest);
        s
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        raw.to_string()
    };
    let leading = raw.len() - raw.trim_start_matches(['/', '\\']).len();
    let mut out: Vec<&str> = Vec::new();
    for seg in raw.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let joined = out.join("\\");
    let mut canon = String::with_capacity(joined.len() + 2);
    if leading > 0 {
        // preserve the UNC double-separator shape (single for drive-root)
        let sep_count = if leading >= 2 { 2 } else { 1 };
        canon.push_str(&"\\".repeat(sep_count));
    }

    for ch in joined.chars() {
        canon.extend(ch.to_lowercase());
    }
    canon
}

#[cfg(test)]
mod identity_tests {
    use super::normalize_path_identity as n;

    /// review 74 §3: device namespace paths are NOT filesystem identities —
    /// kept verbatim (case-folded) and never equal a real path identity.
    #[test]
    fn device_namespace_is_not_file_identity() {
        assert!(super::is_device_namespace(r"\\.\PhysicalDrive0"));
        assert!(!super::is_device_namespace(r"C:\foo"));
        assert_ne!(n(r"\\.\PhysicalDrive0"), n(r"C:\PhysicalDrive0"));
        assert_eq!(n(r"\\.\PhysicalDrive0"), n(r"\\.\physicaldrive0"));
    }

    /// review 72 §8: trailing separators fold.
    #[test]
    fn trailing_separator_folds() {
        assert_eq!(n(r"C:\Foo\"), n(r"C:\Foo"));
        assert_eq!(n("C:/Foo/"), n(r"C:\Foo"));
    }

    /// review 72 §8: `\\?\` device prefix is transparent.
    #[test]
    fn device_prefix_stripped() {
        assert_eq!(n(r"\\?\C:\foo"), n(r"C:\foo"));
        assert_eq!(n(r"\\?\UNC\server\share\f"), n(r"\\server\share\f"));
    }

    /// review 72 §8: UNC identities never equal drive identities.
    #[test]
    fn unc_distinct_from_drive_paths() {
        assert_ne!(n(r"\\server\share\foo"), n(r"C:\foo"));
        assert_ne!(n(r"\\server\share\foo"), n(r"\server\share\foo"));
        assert_eq!(n(r"\\SERVER\Share\FOO"), n(r"\\server\share\foo"));
    }

    #[test]
    fn lexical_normalization() {
        assert_eq!(n(r"D:\Foo\Bar.txt"), n("d:/foo/bar.txt"));
        assert_eq!(n(r"D:\Foo\.\Bar.txt"), n(r"d:\foo\bar.txt"));
        assert_eq!(n(r"D:\Foo\Sub\..\Bar.txt"), n(r"d:\foo\bar.txt"));
        assert_ne!(n(r"D:\Foo\Bar.txt"), n(r"D:\Foo\Baz.txt"));
    }
}

pub mod system;
pub mod system_adapter;
pub mod system_process_window;
