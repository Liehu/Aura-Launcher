//! Protocol session identity + state (review 57 §3-§7): the protocol
//! session is a THIRD identity, distinct from RuntimeId (P0-A) and
//! ExecutionId (caller-owned).
//!
//! ```text
//! RuntimeId       = "which OS process instance"   (P0-A / P1-A)
//! ProtocolSession = "is the protocol state on that process usable"(P1-B)
//! ExecutionId     = "one Effect execution attempt" (caller-owned)
//! ```
//!
//! Frozen rules (§5/§6/§7/§13/§15):
//! - A live runtime does NOT imply a valid session (and vice versa: session
//!   failure does not automatically kill the runtime).
//! - `ProtocolSession` records the ASSOCIATION to a runtime (runtime_id) —
//!   it does not own the ProcessSession; lifecycle ownership stays in the
//!   Runtime layer.
//! - Persistence is process-lifetime + session-memory only: application
//!   restart ⇒ runtime gone ⇒ session gone. No disk persistence.
//! - Session state can never grant execution authority.

use std::fmt;
use std::time::{Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::compat::McpProtocolProfile;
use launcher_runtime::RuntimeId;

/// Opaque protocol-session identity. Distinct type from RuntimeId and from
/// any execution id — never a type alias over either (review 57 §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtocolSessionId(u64);

static SESSION_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl ProtocolSessionId {
    pub fn mint() -> Self {
        let n = SESSION_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        ProtocolSessionId(n)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for ProtocolSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s-{}", self.0)
    }
}

/// The complete protocol identity of one session (review 57 §4): reuse is
/// decided by the FULL key — same process + different profile ⇒ different
/// session (no accidental cross-profile reuse).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolSessionKey {
    pub namespace: String,
    pub endpoint_id: String,
    pub profile: McpProtocolProfile,
}

impl ProtocolSessionKey {
    /// MCP namespace key: `mcp / <server_id> / <profile>`.
    pub fn mcp(server_id: &str, profile: McpProtocolProfile) -> Self {
        Self {
            namespace: "mcp".into(),
            endpoint_id: server_id.into(),
            profile,
        }
    }

    pub fn storage_key(&self) -> String {
        format!(
            "{}/{}/{}",
            self.namespace, self.endpoint_id, self.profile.as_str()
        )
    }
}

/// Session state machine (review 57 §5), compressed to the necessary
/// states. `Failed` is protocol-failure-at-establish — distinct from a
/// dead runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolSessionState {
    /// Created, no protocol traffic yet.
    New,
    /// Establish in progress (initialize exchange).
    Initializing,
    /// Usable: protocol state valid, operations may acquire.
    Ready,
    /// Previously Ready; protocol failure invalidated the state. The
    /// underlying runtime MAY still be alive — re-establish is allowed.
    Invalid,
    /// Establish failed (e.g. process dead at initialize). Not the same as
    /// "runtime is dead" — the runtime layer owns that verdict.
    Failed,
    /// Explicitly closed; cannot be reused.
    Closed,
}

impl ProtocolSessionState {
    pub fn can_acquire_operation(&self) -> bool {
        matches!(self, ProtocolSessionState::Ready)
    }

    pub fn can_re_establish(&self) -> bool {
        matches!(
            self,
            ProtocolSessionState::New | ProtocolSessionState::Invalid | ProtocolSessionState::Failed
        )
    }
}

/// One protocol session record: identity + state + the association to its
/// runtime. It does NOT own the ProcessSession (review 57 §7) — lifecycle
/// ownership stays in the Runtime layer.
#[derive(Debug, Clone)]
pub struct ProtocolSession {
    pub session_id: ProtocolSessionId,
    pub runtime_id: Option<RuntimeId>,
    pub key: ProtocolSessionKey,
    pub state: ProtocolSessionState,
    pub created_at: SystemTime,
    pub last_activity_at: Instant,
}

impl ProtocolSession {
    pub fn new(key: ProtocolSessionKey, runtime_id: Option<RuntimeId>) -> Self {
        Self {
            session_id: ProtocolSessionId::mint(),
            runtime_id,
            key,
            state: ProtocolSessionState::New,
            created_at: SystemTime::now(),
            last_activity_at: Instant::now(),
        }
    }

    pub fn touch(&mut self) {
        self.last_activity_at = Instant::now();
    }
}

/// Session-key consistency (review 57 §18): a session may only ever be
/// used under its own key — cross-server/cross-profile lookup is refused
/// by exact key equality here.
pub fn keys_match(a: &ProtocolSessionKey, b: &ProtocolSessionKey) -> bool {
    a == b
}

/// P1-B serialization shape (§15): session is memory-only; this struct
/// exists for diagnostics and is deliberately minimal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: u64,
    pub state: String,
    pub runtime_id: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SESSION-012/013 (INV-MCP-SESSION-001): three distinct identities.
    #[test]
    fn identities_are_distinct() {
        let sid = ProtocolSessionId::mint();
        let rid = RuntimeId::mint();
        // different counters, different Display namespaces (s-N vs r-N);
        // the raw counters are independent sequences and MAY numerically
        // collide — identity comes from the type + namespace, not the number
        assert_eq!(format!("{sid}"), format!("s-{}", sid.as_u64()));
        assert_eq!(format!("{rid}"), format!("r-{}", rid.as_u64()));
        assert_ne!(
            std::any::type_name_of_val(&sid),
            std::any::type_name_of_val(&rid)
        );
    }

    /// SESSION-005/006 (INV-MCP-SESSION-009): exact key identity — same
    /// server + different profile = different session; different server +
    /// same profile = different session.
    #[test]
    fn session_key_exact_identity() {
        let k1 = ProtocolSessionKey::mcp("calc", McpProtocolProfile::V2025_06_18);
        let k2 = ProtocolSessionKey::mcp("calc", McpProtocolProfile::V2026_07_28);
        let k3 = ProtocolSessionKey::mcp("github", McpProtocolProfile::V2025_06_18);
        assert!(keys_match(&k1, &k1));
        assert!(!keys_match(&k1, &k2), "profile change = no reuse");
        assert!(!keys_match(&k1, &k3), "server change = no reuse");
        assert_ne!(k1.storage_key(), k2.storage_key());
    }

    /// §5: state predicates.
    #[test]
    fn state_predicates() {
        assert!(ProtocolSessionState::Ready.can_acquire_operation());
        for st in [
            ProtocolSessionState::New,
            ProtocolSessionState::Initializing,
            ProtocolSessionState::Invalid,
            ProtocolSessionState::Failed,
            ProtocolSessionState::Closed,
        ] {
            assert!(!st.can_acquire_operation());
        }
        for st in [
            ProtocolSessionState::New,
            ProtocolSessionState::Invalid,
            ProtocolSessionState::Failed,
        ] {
            assert!(st.can_re_establish(), "{st:?} should allow re-establish");
        }
        assert!(!ProtocolSessionState::Ready.can_re_establish());
        assert!(!ProtocolSessionState::Closed.can_re_establish());
    }
}
