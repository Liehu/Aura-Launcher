//! Plugin SearchProvider (MVP4.0 / ADR-0014): wraps one external plugin for
//! discovery + effect execution. The broker boundary (INV-047): this provider
//! is reached only through the ActionEngine result path; execution goes
//! `PluginHandle::execute_action` RPC with the CALLER-minted execution_id
//! (P1-FIX-01, INV-AUTH-005).
//!
//! P2.2-D lifecycle state (review: P2.2-D §6/§59/§75): consecutive
//! protocol-level failures (not business errors) above QUARANTINE_THRESHOLD
//! quarantine the plugin — it stops participating in discovery until
//! explicitly re-enabled. Disabled plugins likewise produce no candidates
//! (§75: Disabled/Quarantined → no search candidate).

use std::path::PathBuf;
use std::time::Instant;

use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{Command, QueryContext};
use launcher_domain::PluginManifest;
use launcher_plugin_host::{PluginError, PluginHandle};
use super::plugin_diagnostics::{global_record, DiagnosticClass, DiagnosticEntry};
use tracing::warn;

use crate::Provider;

pub const QUARANTINE_THRESHOLD: u32 = 3;

pub struct PluginProvider {
    manifest: PluginManifest,
    base_dir: PathBuf,
    handle: Option<PluginHandle>,
    last_used: Option<Instant>,
    /// Last query failure (spawn/cooldown/RPC), surfaced via
    /// `Provider::take_last_error` for the Main.Error state (UI-CONTRACT §3.1).
    last_query_error: Option<String>,
    /// Spawn/handshake failure cooldown: a plugin that cannot start (e.g. an
    /// outdated pre-contract binary) must not be re-spawned on every
    /// keystroke. Cooldown = manifest idle_timeout, cleared on success.
    spawn_failed_until: Option<Instant>,
    /// P2.2-D: consecutive protocol-level failures (not business errors).
    protocol_failures: u32,
    quarantined: bool,
    disabled: bool,
    /// P2.2-D registry: persists enable/quarantine/failures across restarts.
    registry: Option<std::sync::Arc<super::plugin_registry::PluginRegistry>>,
}

/// P2.4-E05: PluginError → diagnostic classification (contract failure
/// taxonomy). Flood surfaces today as Malformed(output-limit) — the host
/// variant carries the limit in its payload.
/// P2.4-E01: structured diagnostic for one interaction (observation only —
/// never an execution path, spec P2.4-E).
fn record_diag(plugin_id: &str, class: DiagnosticClass, elapsed_ms: u64, results: Option<usize>) {
    global_record(DiagnosticEntry {
        plugin_id: plugin_id.to_string(),
        class,
        query_id: None,
        runtime_id: None,
        protocol_session_id: None,
        elapsed_ms,
        frame_size: None,
        result_count: results,
    });
}

fn classify(e: &PluginError) -> DiagnosticClass {
    match e {
        PluginError::Spawn(_) => DiagnosticClass::SpawnFailed,
        PluginError::Timeout(_) | PluginError::NoResponse => DiagnosticClass::Timeout,
        PluginError::Crashed(_) => DiagnosticClass::Crash,
        PluginError::Malformed(_) => DiagnosticClass::Malformed,
        _ => DiagnosticClass::Malformed,
    }
}

impl PluginProvider {
    /// Validate `plugin.json` and build a provider around it.
    pub fn from_manifest_file(path: &std::path::Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        let manifest: PluginManifest =
            serde_json::from_str(&raw).map_err(|e| format!("manifest: {e}"))?;
        manifest
            .validate()
            .map_err(|e| format!("invalid manifest: {e}"))?;
        let base_dir = path
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| "manifest has no parent dir".to_string())?;
        Ok(Self {
            manifest,
            base_dir,
            handle: None,
            last_used: None,
            last_query_error: None,
            spawn_failed_until: None,
            protocol_failures: 0,
            quarantined: false,
            disabled: false,
            registry: None,
        })
    }

    /// Override the package directory (tests: the packaged exe lives in the
    /// test-binary directory, not next to the manifest).
    pub fn set_base_dir(&mut self, dir: PathBuf) {
        self.base_dir = dir;
    }

    /// P2.2-D: enable/disable — a disabled plugin produces no candidates and
    /// its runtime is dropped at the boundary.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.disabled = !enabled;
        if self.disabled {
            self.handle = None; // graceful stop at the next use boundary
        }
    }

    pub fn is_available(&self) -> bool {
        !self.disabled && !self.quarantined
    }

    /// Attach the persistent plugin registry (P2.2-D): quarantine and
    /// failure state survive restarts.
    pub fn set_registry(&mut self, registry: std::sync::Arc<super::plugin_registry::PluginRegistry>) {
        // hydrate persisted state
        let st = registry.state(&self.manifest.id);
        self.disabled = !st.enabled;
        self.quarantined = st.quarantined;
        self.protocol_failures = st.protocol_failures;
        if self.quarantined {
            self.handle = None;
        }
        self.registry = Some(registry);
    }

    /// Manifest accessor (host logs the plugin id at registration).
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Python interpreter injection (ADR-0010 Runtime Discovery: host
    /// resolves config > LAUNCHER_PYTHON > PATH and records the source).
    pub fn set_interpreter_with_source(&mut self, interpreter: PathBuf, source: &str) {
        self.manifest.interpreter = Some(interpreter);
        self.manifest.interpreter_source = Some(source.to_string());
    }

    fn ensure_running(&mut self) -> Result<(), PluginError> {
        if self.handle.is_some() {
            return Ok(());
        }
        if let Some(until) = self.spawn_failed_until {
            if Instant::now() < until {
                return Err(PluginError::Spawn(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "spawn cooldown active",
                )));
            }
            self.spawn_failed_until = None;
        }
        let handle = PluginHandle::spawn(self.manifest.clone(), &self.base_dir)?;
        self.handle = Some(handle);
        Ok(())
    }

    /// Empty-text discovery (DISCOVERY-TODO-001 / WF-006 option 2): ask the
    /// plugin for its static catalog so `ReferenceResolver::fresh_query` can
    /// resolve ActionReferences without a user-typed query. Best-effort:
    /// failures yield an empty catalog (local failure, never surfaces).
    fn discover(&mut self) -> Vec<Command> {
        if let Err(e) = self.ensure_running() {
            warn!(plugin = %self.manifest.id, error = %e, "plugin.discovery_failed");
            return Vec::new();
        }
        match self.handle.as_mut().expect("checked above").query("") {
            Ok(cmds) => {
                self.last_used = Some(Instant::now());
                cmds
            }
            Err(e) => {
                warn!(plugin = %self.manifest.id, error = %e, "plugin.discovery_failed");
                self.handle = None;
                Vec::new()
            }
        }
    }
}

impl Provider for PluginProvider {
    fn id(&self) -> &str {
        "plugin"
    }

    fn plugin_identity(&self) -> Option<&str> {
        Some(self.manifest.id.as_str())
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        // P2.2-D §75/§142: disabled/quarantined plugins produce no candidates
        if self.disabled || self.quarantined {
            return Vec::new();
        }
        // DISCOVERY-TODO-001 closed: empty query = discovery request. Catalog
        // items are score-0.0 so popup ranking filters them; the workflow/AI
        // fresh_query path calls the provider directly and sees them.
        if q.normalized.is_empty() {
            return self.discover();
        }
        let t0 = std::time::Instant::now();
        if let Err(e) = self.ensure_running() {
            warn!(plugin = %self.manifest.id, error = %e, "plugin.failed");
            record_diag(&self.manifest.id, classify(&e), t0.elapsed().as_millis() as u64, None);
            self.last_query_error = Some(e.to_string());
            return Vec::new();
        }
        let Some(h) = self.handle.as_mut() else { return Vec::new() };
        match h.query(&q.normalized) {
            Ok(cmds) => {
                self.protocol_failures = 0;
                self.last_used = Some(Instant::now());
                record_diag(
                    &self.manifest.id,
                    DiagnosticClass::Ok,
                    t0.elapsed().as_millis() as u64,
                    Some(cmds.len()),
                );
                cmds
            }
            Err(e) => {
                warn!(plugin = %self.manifest.id, error = %e, "plugin query failed");
                record_diag(&self.manifest.id, classify(&e), t0.elapsed().as_millis() as u64, None);
                self.last_query_error = Some(e.to_string());
                // a query RPC failure is a protocol-level event exactly like
                // its execute_action counterpart (P2.3-C3): drop the process
                // and count the consecutive failure toward quarantine — a
                // crash-looping plugin must not be respawned forever
                self.handle = None;
                self.protocol_failures += 1;
                if let Some(reg) = self.registry.as_ref() {
                    self.quarantined =
                        reg.record_failure(&self.manifest.id).unwrap_or(self.quarantined);
                }
                if self.protocol_failures >= QUARANTINE_THRESHOLD {
                    self.quarantined = true;
                    warn!(
                        plugin = %self.manifest.id,
                        failures = self.protocol_failures,
                        "plugin quarantined (query path)"
                    );
                }
                Vec::new()
            }
        }
    }

    fn take_last_error(&mut self) -> Option<String> {
        self.last_query_error.take()
    }

    /// MVP4.0: effect execution via the plugin's `execute_action` RPC
    /// (PluginBroker role; reached only through the ActionEngine path).
    fn execute_action(
        &mut self,
        action_id: &str,
        input: &serde_json::Value,
        execution_id: &str,
        context_generation: u64,
    ) -> Result<serde_json::Value, (Class, String)> {
        use launcher_plugin_host::PluginError as E;
        if self.quarantined {
            return Err((Class::PluginUnavailable, "plugin quarantined".into()));
        }
        if self.disabled {
            return Err((Class::PluginUnavailable, "plugin disabled".into()));
        }
        if let Err(e) = self.ensure_running() {
            return Err(match &e {
                E::Spawn(_) => (Class::PluginUnavailable, e.to_string()),
                other => (Class::ProtocolViolation, other.to_string()),
            });
        }
        let h = self.handle.as_mut().expect("checked above");
        self.last_used = Some(Instant::now());
        // business failures (ActionFailed) keep the process alive; protocol
        // failures (timeout/malformed/crash) drop the handle for respawn
        h.execute_action(action_id, input, execution_id, context_generation)
            .map_err(|e| {
                let class = match &e {
                    E::Timeout(_) => Class::Timeout,
                    E::ActionFailed { .. } => Class::BusinessError,
                    E::Spawn(_) => Class::PluginUnavailable,
                    _ => Class::ProtocolViolation,
                };
                let kill = !matches!(e, E::ActionFailed { .. });
                if kill {
                    self.handle = None;
                    // P2.2-D: consecutive protocol failures → quarantine.
                    // With the registry attached, state persists across
                    // restarts; without, the in-memory counter applies.
                    if !matches!(class, Class::BusinessError) {
                        self.protocol_failures += 1;
                        if let Some(reg) = self.registry.as_ref() {
                            self.quarantined = reg
                                .record_failure(&self.manifest.id)
                                .unwrap_or(self.quarantined);
                        }
                        if self.protocol_failures >= QUARANTINE_THRESHOLD {
                            self.quarantined = true;
                            warn!(
                                plugin = %self.manifest.id,
                                failures = self.protocol_failures,
                                "plugin quarantined"
                            );
                        }
                    }
                } else {
                    self.protocol_failures = 0;
                    if let Some(reg) = self.registry.as_ref() {
                        let _ = reg.record_success(&self.manifest.id);
                    }
                }
                (class, e.to_string())
            })
    }
}
