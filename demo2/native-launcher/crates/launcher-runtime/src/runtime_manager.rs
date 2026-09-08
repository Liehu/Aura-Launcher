//! Runtime persistence foundation (MVP4.4 P1-A, review 56): the
//! RuntimeManager decides WHICH runtime stays alive; the
//! ProcessSupervisor/ProcessSession (P0-A) keep deciding HOW processes
//! live and die.
//!
//! Frozen rules (§5/§7/§13/§18-§20):
//! - Runtime persistence MUST NOT grant execution authority; the manager
//!   stores opaque caller-supplied values (e.g. a protocol transport) and
//!   understands nothing about them.
//! - RuntimeId ≠ ExecutionId; the manager never mints execution ids and
//!   crash recovery NEVER replays executions (Workflow decides retry).
//! - One protocol operation per runtime at a time: a busy runtime REJECTS
//!   new work (`RuntimeBusy`) instead of queueing (no unbounded queues).
//! - Idle runtimes are evicted on explicit sweep; the removed value is
//!   returned to the owner for protocol-aware shutdown.
//! - Restarts are bounded with backoff (Never/OnCrash), tracked per key;
//!   the manager never restarts anything itself — it only counts and
//!   admits.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use thiserror::Error;

/// Runtime persistence mode (review 56 §4): only two — no warm/hot/
/// daemon expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeMode {
    /// Fresh process per execution (the P0-A behavior, unchanged default).
    Ephemeral,
    /// Process kept alive across executions until idle sweep / shutdown.
    Persistent,
}

/// Logical runtime identity (review 56 §6): namespace + server/plugin id
/// + optional profile, so the same capability with a different profile
/// never accidentally shares a runtime.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeKey {
    pub namespace: &'static str,
    pub id: String,
    pub profile: Option<String>,
}

impl RuntimeKey {
    pub fn plugin(id: impl Into<String>) -> Self {
        Self { namespace: "plugin", id: id.into(), profile: None }
    }
    pub fn mcp(id: impl Into<String>, profile: Option<String>) -> Self {
        Self { namespace: "mcp", id: id.into(), profile }
    }
    pub fn storage_key(&self) -> String {
        match &self.profile {
            Some(p) => format!("{}/{}/{}", self.namespace, self.id, p),
            None => format!("{}/{}", self.namespace, self.id),
        }
    }
}

/// Restart policy (review 56 §11): P1-A ships Never and OnCrash only.
/// `OnFailure` is deliberately absent — a business failure is not a
/// runtime failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestartPolicy {
    #[default]
    Never,
    OnCrash,
}

/// Persistence policy (review 56 §9/§21/§28): explicit idle timeout and a
/// bounded restart budget with linear-backoff slots (values configurable
/// later; boundedness is the invariant).
#[derive(Debug, Clone)]
pub struct RuntimePolicy {
    pub idle_timeout: Option<Duration>,
    pub restart_policy: RestartPolicy,
    pub max_restarts: u32,
    /// Backoff slots, cycled at the last entry: restart i waits
    /// backoff[min(i, len-1)].
    pub restart_backoff: Vec<Duration>,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            idle_timeout: Some(Duration::from_secs(30 * 60)),
            restart_policy: RestartPolicy::OnCrash,
            max_restarts: 3,
            restart_backoff: vec![
                Duration::from_millis(100),
                Duration::from_millis(500),
                Duration::from_secs(2),
            ],
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum RuntimeManagerError {
    /// Busy-reject (review 56 §19): one protocol operation per runtime;
    /// the caller retries later — the manager never queues.
    #[error("runtime is busy with another operation")]
    Busy,
    #[error("runtime crashed and the restart budget is exhausted")]
    RestartBudgetExhausted,
    #[error("runtime crashed; restart blocked by policy")]
    RestartBlocked,
    #[error("runtime not found")]
    NotFound,
}

struct Entry<V> {
    value: V,
    /// Retained on the entry for observability (manager API may expose
    /// mode stats later); policy decisions live in RuntimePolicy.
    #[allow(dead_code)]
    mode: RuntimeMode,
    last_activity: Instant,
    busy: bool,
    /// Restart count observed for this entry's current lifetime.
    #[allow(dead_code)]
    restart_count: u32,
}

/// Generic persistent-runtime registry: `RuntimeKey → opaque value` with
/// busy-exclusion, idle sweep and bounded restart accounting. The value is
/// opaque (e.g. a boxed protocol transport) — the manager knows nothing
/// about protocols, effects, capabilities or execution ids.
pub struct RuntimeManager<V> {
    entries: HashMap<String, Entry<V>>,
    /// Bounded restart accounting per storage key (survives eviction so a
    /// crash-looping runtime cannot reset its budget by stopping).
    restart_counts: HashMap<String, u32>,
}

/// Guard returned by [`RuntimeManager::use_runtime`]: marks the runtime
/// busy for its lifetime and refreshes last_activity on drop.
pub struct RuntimeGuard<'a, V> {
    entry: &'a mut Entry<V>,
    pub key: String,
}

impl<'a, V> std::ops::Deref for RuntimeGuard<'a, V> {
    type Target = V;
    fn deref(&self) -> &V {
        &self.entry.value
    }
}

impl<'a, V> std::ops::DerefMut for RuntimeGuard<'a, V> {
    fn deref_mut(&mut self) -> &mut V {
        &mut self.entry.value
    }
}

impl<'a, V> Drop for RuntimeGuard<'a, V> {
    fn drop(&mut self) {
        self.entry.busy = false;
        self.entry.last_activity = Instant::now();
    }
}

impl<V> Default for RuntimeManager<V> {
    fn default() -> Self {
        Self { entries: HashMap::new(), restart_counts: HashMap::new() }
    }
}

impl<V> RuntimeManager<V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn live_keys(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Busy flag for one runtime (§18 observability: callers can check
    /// before attempting work; use_runtime remains the enforced gate).
    pub fn is_busy(&self, key: &RuntimeKey) -> bool {
        self.entries.get(&key.storage_key()).map(|e| e.busy).unwrap_or(false)
    }

    /// Test/observability hook: mark an entry busy without holding the
    /// guard (used to prove the idle sweep skips busy runtimes).
    #[doc(hidden)]
    pub fn mark_busy(&mut self, key: &RuntimeKey) {
        if let Some(e) = self.entries.get_mut(&key.storage_key()) {
            e.busy = true;
        }
    }

    /// Get the persistent runtime for `key`, spawning it via `spawn` when
    /// absent. Busy-excluded (§18/§19): a second concurrent `use_runtime`
    /// on the same key gets `RuntimeManagerError::Busy`.
    pub fn use_runtime(
        &mut self,
        key: &RuntimeKey,
        spawn: impl FnOnce() -> Result<V, crate::error::RuntimeError>,
    ) -> Result<RuntimeGuard<'_, V>, RuntimeManagerError> {
        let sk = key.storage_key();
        if self.entries.get(&sk).map(|e| e.busy).unwrap_or(false) {
            return Err(RuntimeManagerError::Busy);
        }
        let exists = self.entries.contains_key(&sk);
        if !exists {
            let value = match spawn() {
            Ok(v) => v,
            Err(_) => return Err(RuntimeManagerError::NotFound),
        };
            self.entries.insert(
                sk.clone(),
                Entry {
                    value,
                    mode: RuntimeMode::Persistent,
                    last_activity: Instant::now(),
                    busy: false,
                    restart_count: 0,
                },
            );
        }
        let entry = self.entries.get_mut(&sk).expect("just inserted or checked");
        entry.busy = true;
        entry.last_activity = Instant::now();
        Ok(RuntimeGuard { entry, key: sk })
    }

    /// Account a crash for `key` and decide whether a restart is allowed
    /// (policy + bounded budget). Returns the backoff to wait before the
    /// next spawn attempt. The manager NEVER re-runs the execution that
    /// crashed — recovery of the attempt is the caller's/Workflow's job
    /// (INV-RUNTIME-002).
    pub fn on_crash(
        &mut self,
        key: &RuntimeKey,
        policy: &RuntimePolicy,
    ) -> Result<Duration, RuntimeManagerError> {
        let sk = key.storage_key();
        self.entries.remove(&sk);
        if policy.restart_policy == RestartPolicy::Never {
            return Err(RuntimeManagerError::RestartBlocked);
        }
        let count = self.restart_count(&sk);
        let new_count = count + 1;
        if new_count > policy.max_restarts {
            return Err(RuntimeManagerError::RestartBudgetExhausted);
        }
        self.restart_counts.insert(sk, new_count);
        let slot = ((new_count - 1) as usize).min(policy.restart_backoff.len().saturating_sub(1));
        Ok(policy
            .restart_backoff
            .get(slot)
            .copied()
            .unwrap_or(Duration::from_millis(100)))
    }

    /// Evict persistent runtimes idle beyond `policy.idle_timeout`.
    /// Returns the removed values so the OWNER can perform protocol-aware
    /// shutdown — the manager itself never touches the value's semantics
    /// (§7: process alive ≠ protocol session alive).
    pub fn sweep_idle(&mut self, policy: &RuntimePolicy) -> Vec<(String, V)> {
        let Some(timeout) = policy.idle_timeout else {
            return Vec::new();
        };
        let now = Instant::now();
        let mut removed = Vec::new();
        let keys: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, e)| !e.busy && now.duration_since(e.last_activity) > timeout)
            .map(|(k, _)| k.clone())
            .collect();
        for k in keys {
            if let Some(e) = self.entries.remove(&k) {
                removed.push((k, e.value));
            }
        }
        removed
    }

    /// Explicitly stop one runtime; returns the value for owner-side
    /// shutdown (§15).
    pub fn release(&mut self, key: &RuntimeKey) -> Option<V> {
        let sk = key.storage_key();
        if self.entries.get(&sk).map(|e| e.busy).unwrap_or(false) {
            return None; // busy runtimes cannot be reclaimed (§9)
        }
        self.entries.remove(&sk).map(|e| e.value)
    }

    /// Stop everything; busy runtimes are force-included (process exit
    /// path). Returns all values for owner-side shutdown.
    pub fn shutdown_all(&mut self) -> Vec<(String, V)> {
        let all: Vec<(String, Entry<V>)> = self.entries.drain().collect();
        all.into_iter().map(|(k, e)| (k, e.value)).collect()
    }

    fn restart_count(&self, storage_key: &str) -> u32 {
        self.restart_counts.get(storage_key).copied().unwrap_or(0)
    }
}
