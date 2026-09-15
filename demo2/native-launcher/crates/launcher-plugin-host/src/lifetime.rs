//! Plugin Lifetime Policy (ADR-0019 / Plugin Lifetime v0.1): the SINGLE
//! place where a validated manifest's `runtime.lifetime` decides how the
//! host launches and reclaims the plugin process (spec §12: lifecycle stays
//! executor/handle-local — Workflow / Action / MCP / AI never branch on it).
//!
//! Two lifetimes are supported in v0.1:
//! - `Ephemeral`: one invocation = one process (spawn → invoke → shutdown →
//!   bounded wait → force kill if needed → Job Object descendant cleanup).
//! - `Resident`: first call spawns, later calls reuse, idle timeout reclaims
//!   (legacy behavior; an absent manifest field maps to Resident).

use std::time::Duration;

use launcher_domain::{PluginLifetime, PluginManifest};

/// Host-owned two-phase shutdown grace (v0.1 §6): `shutdown` RPC, then a
/// bounded wait, then Job Object force-kill. This is HOST POLICY — it is
/// deliberately NOT manifest-declarable, so a plugin cannot extend its own
/// shutdown indefinitely.
pub const SHUTDOWN_GRACE_MS: u64 = 200;

/// How the host runs one plugin invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// One invocation = one process; the process must be absent afterwards.
    Ephemeral,
    /// First call spawns, later calls reuse, idle timeout reclaims.
    Resident,
}

/// Validated-manifest-derived launch policy (ADR-0019 §3/L3). Only the host
/// constructs it from a manifest that already passed `validate()`.
#[derive(Debug, Clone, Copy)]
pub struct PluginLifetimePolicy {
    mode: ExecutionMode,
    idle_timeout: Duration,
    window_ui: bool,
}

impl PluginLifetimePolicy {
    pub fn from_manifest(manifest: &PluginManifest) -> Self {
        Self {
            mode: match manifest.lifetime() {
                PluginLifetime::Ephemeral => ExecutionMode::Ephemeral,
                PluginLifetime::Resident => ExecutionMode::Resident,
            },
            idle_timeout: Duration::from_millis(manifest.idle_timeout_ms),
            window_ui: manifest.window_ui,
        }
    }

    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }

    pub fn is_ephemeral(&self) -> bool {
        self.mode == ExecutionMode::Ephemeral
    }

    /// Host-owned shutdown grace (never manifest-declared, v0.1 §6).
    pub fn shutdown_grace(&self) -> Duration {
        Duration::from_millis(SHUTDOWN_GRACE_MS)
    }

    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// Whether this resident plugin may be reclaimed after its idle timeout.
    /// Window plugins (P3.1) are excluded: their process lifetime is owned
    /// by window management while the user keeps the window — an idle
    /// reclaim here would be an undeclared legacy behavior change.
    pub fn idle_reclaimable(&self) -> bool {
        self.mode == ExecutionMode::Resident && !self.window_ui
    }
}

// ---- L7 observability ------------------------------------------------------
// Bounded counters only (no history cache, spec §13 / L7). The structured
// per-invocation details live in tracing events.

/// Global lifecycle counters (process-wide, monotonic).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LifecycleMetrics {
    pub spawn_count: u64,
    pub successful_spawn_count: u64,
    pub forced_kill_count: u64,
    pub ephemeral_invocations: u64,
}

use std::sync::atomic::{AtomicU64, Ordering};

static SPAWN_COUNT: AtomicU64 = AtomicU64::new(0);
static SUCCESSFUL_SPAWN_COUNT: AtomicU64 = AtomicU64::new(0);
static FORCED_KILL_COUNT: AtomicU64 = AtomicU64::new(0);
static EPHEMERAL_INVOCATIONS: AtomicU64 = AtomicU64::new(0);

pub fn lifecycle_metrics() -> LifecycleMetrics {
    LifecycleMetrics {
        spawn_count: SPAWN_COUNT.load(Ordering::Relaxed),
        successful_spawn_count: SUCCESSFUL_SPAWN_COUNT.load(Ordering::Relaxed),
        forced_kill_count: FORCED_KILL_COUNT.load(Ordering::Relaxed),
        ephemeral_invocations: EPHEMERAL_INVOCATIONS.load(Ordering::Relaxed),
    }
}

pub(crate) fn record_spawn(success: bool) {
    SPAWN_COUNT.fetch_add(1, Ordering::Relaxed);
    if success {
        SUCCESSFUL_SPAWN_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

pub(crate) fn record_forced_kill() {
    FORCED_KILL_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_ephemeral_invocation() {
    EPHEMERAL_INVOCATIONS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(lifetime: &str, window: bool) -> PluginManifest {
        serde_json::from_value(serde_json::json!({
            "id": "p", "name": "P", "window": window,
            "runtime": { "type": "process", "executable": "x.exe", "lifetime": lifetime },
            "idle_timeout_ms": 1234
        }))
        .unwrap()
    }

    #[test]
    fn policy_maps_validated_manifest() {
        assert!(PluginLifetimePolicy::from_manifest(&manifest("ephemeral", false)).is_ephemeral());
        assert!(!PluginLifetimePolicy::from_manifest(&manifest("resident", false)).is_ephemeral());
    }

    #[test]
    fn shutdown_grace_is_host_policy() {
        let p = PluginLifetimePolicy::from_manifest(&manifest("ephemeral", false));
        assert_eq!(p.shutdown_grace(), Duration::from_millis(SHUTDOWN_GRACE_MS));
    }

    #[test]
    fn idle_reclaim_resident_headless_only() {
        assert!(
            PluginLifetimePolicy::from_manifest(&manifest("resident", false)).idle_reclaimable()
        );
        assert!(
            !PluginLifetimePolicy::from_manifest(&manifest("resident", true)).idle_reclaimable()
        );
        assert!(
            !PluginLifetimePolicy::from_manifest(&manifest("ephemeral", false)).idle_reclaimable()
        );
    }

    #[test]
    fn metrics_counters_are_monotonic() {
        let before = lifecycle_metrics();
        record_spawn(true);
        record_spawn(false);
        record_forced_kill();
        record_ephemeral_invocation();
        let after = lifecycle_metrics();
        assert_eq!(after.spawn_count, before.spawn_count + 2);
        assert_eq!(
            after.successful_spawn_count,
            before.successful_spawn_count + 1
        );
        assert_eq!(after.forced_kill_count, before.forced_kill_count + 1);
        assert_eq!(
            after.ephemeral_invocations,
            before.ephemeral_invocations + 1
        );
    }
}
