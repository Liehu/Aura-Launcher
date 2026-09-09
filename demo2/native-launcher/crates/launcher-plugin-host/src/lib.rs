//! Plugin Broker: spawns external plugin processes, enforces timeout,
//! contains crashes, and converts plugin output into domain Commands.
//!
//! A plugin crash/hang must never take down the caller (design spec 9, 14).
//! Process-tree containment via Windows Job Objects lives in `job` (ADR-0005).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use launcher_runtime::{LaunchSpec, ProcessSession, RuntimeError as RtError, RuntimeLimits};

use launcher_domain::{
    Action, ActionKind, Capability, Category, Command as DomainCommand, PluginManifest,
};
use launcher_ipc::{
    method, ExecuteActionParams, InitializeParams, InitializeResult, QueryParams, Request,
    Response, PROTOCOL_VERSION,
};
use thiserror::Error;

pub mod runtime;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("spawn failed: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("plugin timeout after {0:?}")]
    Timeout(Duration),
    #[error("plugin crashed with exit code {0:?}")]
    Crashed(Option<i32>),
    #[error("malformed response: {0}")]
    Malformed(String),
    #[error("no response")]
    NoResponse,
    #[error("executable escapes plugin directory: {0}")]
    ExecutableEscapesDir(String),
    #[error("protocol version mismatch: {0}")]
    VersionMismatch(String),
    /// Plugin-level action failure (Action Execution Result `EffectFailed`):
    /// NOT a protocol violation — the process stays alive (ADR-0014 §9).
    #[error("action failed: {message}")]
    ActionFailed { code: i32, message: String },
}

/// Security check (ADR-0005): `executable` must be a relative, single
/// component (or relative sub-path without `..`) that resolves *inside*
/// `base_dir`. Rejects absolute paths, `..` escapes, and UNC paths.
pub fn resolve_executable(
    base_dir: &std::path::Path,
    executable: &str,
) -> Result<std::path::PathBuf, PluginError> {
    use std::path::Component;
    let rel = std::path::Path::new(executable);
    if rel.is_absolute()
        || executable.starts_with(r"\\")
        || rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(PluginError::ExecutableEscapesDir(executable.to_string()));
    }
    let resolved = base_dir.join(rel);
    // canonicalize resolves symlinks/junctions; when the target exists it
    // must stay within the (canonical) plugin directory. A missing file is
    // allowed here so spawn surfaces the real IO error instead.
    if let Ok(resolved_canon) = resolved.canonicalize() {
        if let Ok(base) = base_dir.canonicalize() {
            if !resolved_canon.starts_with(&base) {
                return Err(PluginError::ExecutableEscapesDir(executable.to_string()));
            }
        }
    }
    Ok(resolved)
}

/// Expand environment variables in a path string: `%VAR%` (Windows style),
/// `${VAR}` and `$VAR` are replaced from the process environment; unknown
/// variables are left verbatim so spawn surfaces a real error (ADR-0009).
pub fn expand_env(input: &str) -> String {
    let env = |name: &str| -> Option<String> { std::env::var(name).ok().filter(|v| !v.is_empty()) };
    // ${VAR} first
    let mut out = String::with_capacity(input.len());
    let bytes: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '$' && i + 1 < bytes.len() && bytes[i + 1] == '{' {
            if let Some(end) = input[i + 2..].find('}') {
                let name: String = bytes[i + 2..i + 2 + end].iter().collect();
                out.push_str(&env(&name).unwrap_or_else(|| bytes[i..i + end + 3].iter().collect()));
                i += end + 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // %VAR%
    let mut out2 = String::with_capacity(out.len());
    let rest: Vec<char> = out.chars().collect();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == '%' {
            if let Some(end) = rest[i + 1..].iter().position(|&c| c == '%') {
                let name: String = rest[i + 1..i + 1 + end].iter().collect();
                if !name.is_empty() {
                    if let Some(v) = env(&name) {
                        out2.push_str(&v);
                        i += end + 2;
                        continue;
                    }
                }
            }
        }
        out2.push(rest[i]);
        i += 1;
    }
    // $VAR (bare, terminated by non-identifier)
    let mut out3 = String::with_capacity(out2.len());
    let rest: Vec<char> = out2.chars().collect();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == '$'
            && i + 1 < rest.len()
            && (rest[i + 1].is_alphabetic() || rest[i + 1] == '_')
        {
            let mut j = i + 1;
            while j < rest.len() && (rest[j].is_alphanumeric() || rest[j] == '_') {
                j += 1;
            }
            let name: String = rest[i + 1..j].iter().collect();
            out3.push_str(&env(&name).unwrap_or_else(|| rest[i..j].iter().collect()));
            i = j;
            continue;
        }
        out3.push(rest[i]);
        i += 1;
    }
    out3
}

pub struct PluginHandle {
    manifest: PluginManifest,
    /// P0-A: process lifecycle (spawn/Job Object/bounded IO/kill/reap)
    /// lives in launcher-runtime; the broker owns only plugin semantics.
    session: ProcessSession,
    /// Monotonic request id counter (JSON-RPC `id`).
    next_request_id: u64,
}

/// Global query-id sequence so ids stay unique across respawns of the same
/// plugin process within one launcher run.
static QUERY_SEQ: AtomicU64 = AtomicU64::new(0);

fn next_query_id() -> String {
    let n = QUERY_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    format!("q-{n}")
}

// P1-FIX-01: the host layer no longer mints execution ids — the caller
// (Core / the workflow backend) owns the attempt id (INV-AUTH-005).

/// One entry of a plugin result item's `actions` array (ADR-0011).
enum PluginActionRef {
    /// Legacy pre-freeze string (treated as an Execute command line).
    Legacy(String),
    /// Structured ACTION-CONTRACT descriptor (untrusted; resolve before use).
    Descriptor(launcher_domain::ActionDescriptor),
    /// Not a string and not a valid descriptor: dropped (fault containment).
    Malformed(String),
}

fn classify_action(v: &serde_json::Value) -> PluginActionRef {
    match v {
        serde_json::Value::String(s) => PluginActionRef::Legacy(s.clone()),
        serde_json::Value::Object(_) => {
            match serde_json::from_value::<launcher_domain::ActionDescriptor>(v.clone()) {
                Ok(d) => PluginActionRef::Descriptor(d),
                Err(e) => PluginActionRef::Malformed(format!("invalid action descriptor: {e}")),
            }
        }
        other => PluginActionRef::Malformed(format!("unexpected action value: {other}")),
    }
}

impl PluginHandle {
    /// Validate the manifest and spawn the plugin process.
    pub fn spawn(
        manifest: PluginManifest,
        base_dir: &std::path::Path,
    ) -> Result<Self, PluginError> {
        manifest
            .validate()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
        // Runtime resolution is centralized in `runtime` (ADR-0010): the
        // single extension point for future node/wasm runtimes.
        let plan = runtime::resolve_launch(&manifest, base_dir)?;
        tracing::debug!(plugin = %manifest.id, runtime = plan.kind, program = %plan.program, "runtime.resolved");
        // P0-A: the runtime owns spawn + Job Object containment
        // (CREATE_SUSPENDED -> assign -> resume) + bounded IO. Contract
        // frame cap (SS14) is enforced at the allocation boundary by the
        // runtime reader.
        let spec = LaunchSpec::new(plan.program.clone())
            .args(plan.args.clone())
            .working_dir(base_dir);
        let limits = RuntimeLimits {
            max_stdout_line_bytes: launcher_ipc::MAX_FRAME_BYTES,
            ..RuntimeLimits::default()
        };
        let session = ProcessSession::spawn(&spec, &limits)
            .map_err(Self::map_runtime_error)?;
        let mut handle = Self {
            manifest,
            session,
            next_request_id: 0,
        };
        // Contract handshake (§12): initialize MUST succeed before the plugin
        // is usable; a version mismatch or silent plugin is terminated here.
        handle.initialize()?;
        Ok(handle)
    }

    /// `initialize` handshake: negotiate the protocol version (§4).
    fn initialize(&mut self) -> Result<(), PluginError> {
        let timeout = Duration::from_millis(self.manifest.timeout_ms.min(2000));
        let req = Request::new(
            self.next_request_id(),
            method::INITIALIZE,
            serde_json::to_value(InitializeParams {
                protocol_version: PROTOCOL_VERSION.to_string(),
                plugin_id: self.manifest.id.clone(),
            })
            .unwrap_or(serde_json::Value::Null),
        );
        let resp = match self.request(&req, timeout) {
            Ok(r) => r,
            Err(e) => {
                self.kill();
                return Err(e);
            }
        };
        let result: InitializeResult = match resp.result {
            Some(v) => match serde_json::from_value(v) {
                Ok(r) => r,
                Err(e) => {
                    self.kill();
                    return Err(PluginError::Malformed(format!(
                        "invalid initialize result: {e}"
                    )));
                }
            },
            None => {
                let msg = resp
                    .error
                    .map(|e| format!("{}: {}", e.code, e.message))
                    .unwrap_or_else(|| "empty initialize result".into());
                self.kill();
                return Err(PluginError::VersionMismatch(msg));
            }
        };
        if result.protocol_version != PROTOCOL_VERSION {
            let msg = format!(
                "plugin speaks {}, host speaks {PROTOCOL_VERSION}",
                result.protocol_version
            );
            self.kill();
            return Err(PluginError::VersionMismatch(msg));
        }
        Ok(())
    }

    fn next_request_id(&mut self) -> u64 {
        self.next_request_id += 1;
        self.next_request_id
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn request(&mut self, req: &Request, timeout: Duration) -> Result<Response, PluginError> {
        // runtime facts -> plugin semantics (review 53 SS7): the mapping
        // lives HERE, never inside launcher-runtime.
        self.session
            .write_line(req.to_line().trim_end())
            .map_err(Self::map_runtime_error)?;
        let buf = self
            .session
            .read_line(timeout)
            .map_err(Self::map_runtime_error)?;
        Response::from_line(&buf).map_err(|e| PluginError::Malformed(e.to_string()))
    }

    fn map_runtime_error(e: RtError) -> PluginError {
        match e {
            RtError::SpawnFailed(m) => {
                PluginError::Spawn(std::io::Error::new(std::io::ErrorKind::Other, m))
            }
            RtError::IoTimeout(d) | RtError::StartupTimeout(d) => PluginError::Timeout(d),
            RtError::ProcessExited { code } => PluginError::Crashed(code),
            RtError::TransportBroken => PluginError::Crashed(None),
            // contract SS14 single-frame byte bound, enforced at the
            // allocation boundary by the runtime reader
            RtError::OutputLimitExceeded { limit, .. } => PluginError::Malformed(format!(
                "frame too large (max {limit} bytes)"
            )),
            RtError::ShutdownTimeout => PluginError::NoResponse,
            RtError::InvalidSpec(m) => {
                PluginError::Spawn(std::io::Error::new(std::io::ErrorKind::InvalidInput, m))
            }
        }
    }

    /// Query the plugin and convert results into Commands. Bounded by the
    /// manifest timeout; the process is killed on timeout. Every query gets
    /// a Host-generated `query_id` the plugin MUST echo (contract §7).
    pub fn query(&mut self, text: &str) -> Result<Vec<DomainCommand>, PluginError> {
        let timeout = Duration::from_millis(self.manifest.timeout_ms);
        let started = Instant::now();
        let query_id = next_query_id();
        let req = Request::new(
            self.next_request_id(),
            method::QUERY,
            serde_json::to_value(QueryParams {
                query_id: query_id.clone(),
                text: text.to_string(),
                limit: launcher_ipc::MAX_PLUGIN_RESULTS,
            })
            .unwrap_or(serde_json::Value::Null),
        );
        let resp = match self.request(&req, timeout) {
            Ok(r) => r,
            Err(e) => {
                self.kill();
                return Err(e);
            }
        };
        if let Some(err) = resp.error {
            self.kill();
            return Err(PluginError::Malformed(format!(
                "plugin error {}: {}",
                err.code, err.message
            )));
        }
        let result = resp.result.unwrap_or(serde_json::Value::Array(vec![]));
        // query_id echo validation (contract §7). Bare arrays are only
        // tolerated on the Legacy Manifest Profile (schema_version absent);
        // v1 manifests MUST use the {query_id, commands} object shape.
        let allow_legacy_array = self.manifest.schema_version.is_none();
        let items = launcher_ipc::parse_query_result(result, &query_id, allow_legacy_array)
            .map_err(PluginError::Malformed)?;
        let elapsed = started.elapsed();
        // Action Resolution (ADR-0011): descriptors are untrusted input and
        // are resolved here against the manifest's declared capabilities
        // (INV-027). Fault containment (INV-031): a malformed/denied action
        // is dropped with a WARN and never invalidates the whole item; an
        // item left with no executable action stays as informational.
        let granted = self.manifest.capabilities.clone();
        let mut cmds = Vec::with_capacity(items.len());
        for item in items {
            let mut target: Option<String> = None;
            let mut actions: Vec<Action> = Vec::new();
            for (idx, a) in item.actions.iter().enumerate() {
                match classify_action(a) {
                    PluginActionRef::Legacy(cmdline) => {
                        // pre-freeze string form: first string is the target
                        if target.is_none() {
                            target = Some(cmdline.clone());
                        }
                        actions.push(Action {
                            kind: ActionKind::Execute,
                            payload: Some(launcher_domain::ActionPayload::CommandLine(cmdline)),

                            id: None,
                            title: None,
                            disabled_reason: None,
                            shortcut: None,
                            confirmation_required: false,
                        });
                    }
                    PluginActionRef::Descriptor(d) => {
                        match launcher_domain::resolve_descriptor_for(
                            &d,
                            &granted,
                            Some(&self.manifest.id),
                        ) {
                            Ok(mut resolved) => {
                                resolved.id = Some(d.id.clone());
                                if let Some(t) = &d.title {
                                    resolved.title = Some(t.clone());
                                }
                                actions.push(resolved);
                            }
                            // CapabilityDenied -> Disabled: presentable but
                            // non-executable (MVP3.1 Action Panel). Unknown
                            // type / invalid input -> Hidden (dropped, INV-031).
                            Err(launcher_domain::DescriptorError::CapabilityDenied(missing)) => {
                                let reason = format!(
                                    "requires {}",
                                    missing
                                        .iter()
                                        .map(|c| serde_json::to_value(c)
                                            .unwrap_or(serde_json::json!("?"))
                                            .to_string()
                                            .trim_matches('"')
                                            .to_string())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                );
                                actions.push(Action {
                                    kind: ActionKind::Execute,
                                    payload: None,
                                    id: Some(d.id.clone()),
                                    title: Some(d.title.clone().unwrap_or_else(|| d.id.clone())),
                                    disabled_reason: Some(reason),
                                    shortcut: d.shortcut.clone(),
                                    confirmation_required: false,
                                });
                            }
                            Err(e) => tracing::warn!(
                                plugin = %self.manifest.id,
                                item = %item.title,
                                action_index = idx,
                                error = %e,
                                "action.resolution.dropped"
                            ),
                        }
                    }
                    PluginActionRef::Malformed(err) => tracing::warn!(
                        plugin = %self.manifest.id,
                        item = %item.title,
                        action_index = idx,
                        error = %err,
                        "action.malformed.dropped"
                    ),
                }
            }
            // stable identity (INV-028): plugin-declared id wins; legacy
            // items fall back to the title-derived id (unchanged behavior)
            let cmd_id = match &item.id {
                Some(id) => format!("{}:{}", self.manifest.id, id),
                None => format!("{}:{}", self.manifest.id, item.title),
            };
            cmds.push(DomainCommand {
                id: cmd_id,
                title: item.title,
                subtitle: item.subtitle,
                icon: None,
                // Host-authoritative provider identity (INV-029): plugin
                // output can never supply it.
                provider_id: format!("plugin:{}", self.manifest.id),
                // bounded [0,1] hint from the plugin (INV-017 dual: the hint
                // never decides the final ranking, ranking weights it)
                score: item.score,
                keywords: vec![],
                category: Category::Plugin,
                actions,
                target,
            });
        }
        let cmds = cmds;
        tracing::debug!(plugin = %self.manifest.id, ?elapsed, "query.completed");
        Ok(cmds)
    }

    /// Execute a plugin-owned action (MVP4.0 / ADR-0014): sends the
    /// `execute_action` RPC with the CALLER-minted `execution_id`
    /// (P1-FIX-01 / INV-AUTH-005: exactly one execution_id per attempt —
    /// the host layer never re-mints) and validates the echo. Bounded by
    /// the manifest timeout; the process is killed on timeout.
    pub fn execute_action(
        &mut self,
        action_id: &str,
        input: &serde_json::Value,
        execution_id: &str,
        context_generation: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let timeout = Duration::from_millis(self.manifest.timeout_ms);
        let execution_id = execution_id.to_string();
        let req = Request::new(
            self.next_request_id(),
            method::EXECUTE_ACTION,
            serde_json::to_value(ExecuteActionParams {
                execution_id: execution_id.clone(),
                action_id: action_id.to_string(),
                input: input.clone(),
                context_generation,
            })
            .unwrap_or(serde_json::Value::Null),
        );
        let resp = match self.request(&req, timeout) {
            Ok(r) => r,
            Err(e) => {
                self.kill();
                return Err(e);
            }
        };
        if let Some(err) = resp.error {
            // plugin-level failure: EffectFailed, the process stays alive so
            // subsequent actions (and queries) keep working (ADR-0014)
            return Err(PluginError::ActionFailed {
                code: err.code,
                message: err.message,
            });
        }
        let result = resp.result.unwrap_or(serde_json::Value::Null);
        // execution_id echo validation; bare results tolerated as legacy shape
        let payload = match &result {
            serde_json::Value::Object(_) => {
                let echo = result.get("execution_id").and_then(|v| v.as_str());
                match echo {
                    Some(e) if e == execution_id => result
                        .get("result")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                    Some(other) => {
                        self.kill();
                        return Err(PluginError::Malformed(format!(
                            "execution_id echo mismatch: expected {execution_id}, got {other}"
                        )));
                    }
                    None => result,
                }
            }
            _ => result,
        };
        tracing::debug!(plugin = %self.manifest.id, execution_id = %execution_id, "execute_action.completed");
        Ok(payload)
    }

    /// Kill the plugin process tree; never panics (P0-A: job terminate
    /// + reap live in the runtime).
    pub fn kill(&mut self) {
        self.session.kill();
    }

    /// Graceful shutdown (contract SS12): send `shutdown`, wait briefly
    /// for the plugin to exit on its own, then force-kill. Never panics.
    pub fn shutdown(&mut self) {
        let req = Request::new(
            self.next_request_id(),
            method::SHUTDOWN,
            serde_json::json!({}),
        );
        if self.request(&req, Duration::from_millis(200)).is_err() {
            // already dead or unresponsive: fall through to kill
        }
        // runtime grace: self-exit within the grace period, else the
        // job tree is force-killed and reaped
        let _ = self.session.shutdown();
    }

    pub fn wait_exit(&mut self) -> Option<i32> {
        match self.session.try_termination() {
            Some(launcher_runtime::ProcessTermination::Exited { code }) => code,
            _ => None,
        }
    }

    pub fn requests(&self, cap: Capability) -> bool {
        self.manifest.requests(cap)
    }
}

impl Drop for PluginHandle {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(exe: &str, timeout_ms: u64) -> PluginManifest {
        serde_json::from_value(serde_json::json!({
            "id": "test-plugin",
            "name": "Test",
            "executable": exe,
            "timeout_ms": timeout_ms
        }))
        .unwrap()
    }

    #[test]
    fn spawn_rejects_invalid_manifest() {
        let m = manifest("", 1000);
        assert!(PluginHandle::spawn(m, &std::path::PathBuf::from(".")).is_err());
    }

    #[test]
    fn spawn_missing_executable_is_io_error() {
        let m = manifest("definitely-missing-xyz.exe", 1000);
        assert!(matches!(
            PluginHandle::spawn(m, &std::path::PathBuf::from(".")),
            Err(PluginError::Spawn(_))
        ));
    }

    #[test]
    fn capability_check() {
        let m: PluginManifest = serde_json::from_value(serde_json::json!({
            "id": "p", "name": "P", "executable": "x",
            "capabilities": ["filesystem.read"]
        }))
        .unwrap();
        assert!(m.requests(Capability::FilesystemRead));
        assert!(!m.requests(Capability::Network));
    }

    #[test]
    fn executable_path_confinement() {
        let base = std::env::temp_dir().join("pluginhost-confinement");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("real.exe"), b"MZ").unwrap();

        // plain relative name inside the plugin dir is allowed
        assert!(resolve_executable(&base, "real.exe").is_ok());
        // relative sub-path without ".." is allowed once the file exists
        std::fs::create_dir_all(base.join("sub")).unwrap();
        std::fs::write(base.join("sub").join("real.exe"), b"MZ").unwrap();
        assert!(resolve_executable(&base, "sub/real.exe").is_ok());

        // escapes are rejected
        assert!(matches!(
            resolve_executable(&base, "../evil.exe"),
            Err(PluginError::ExecutableEscapesDir(_))
        ));
        assert!(matches!(
            resolve_executable(&base, r"C:\Windows\system32\cmd.exe"),
            Err(PluginError::ExecutableEscapesDir(_))
        ));
        assert!(matches!(
            resolve_executable(&base, r"\\server\share\evil.exe"),
            Err(PluginError::ExecutableEscapesDir(_))
        ));
        assert!(matches!(
            resolve_executable(&base, "..\\..\\evil.exe"),
            Err(PluginError::ExecutableEscapesDir(_))
        ));
        std::fs::remove_dir_all(&base).ok();
    }
}
