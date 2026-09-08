//! ProtocolSessionManager (review 57 §20/§21): session identity/lifecycle
//! registry. It deliberately does NOT execute MCP initialize itself —
//! state/identity/lifecycle live here; the protocol exchange (initialize /
//! tools/call) is driven by the transport owner (the executor's persistent
//! path). `establish` records a successful protocol establishment AFTER
//! the transport-level handshake succeeded.
//!
//! Lifecycle orderings frozen here (§16):
//! - `invalidate` before runtime release (no dangling registry entries).
//! - `close_for_runtime` on runtime shutdown/eviction.

use std::collections::HashMap;

use launcher_runtime::RuntimeId;

use super::session::{
    ProtocolSession, ProtocolSessionId, ProtocolSessionKey, ProtocolSessionState,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error("session not found")]
    NotFound,
    #[error("session key mismatch (cross-identity lookup refused)")]
    KeyMismatch,
    #[error("session is closed and cannot be reused")]
    Closed,
    #[error("session is busy with another operation")]
    Busy,
    #[error("session is in a non-recoverable state")]
    NotRecoverable,
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub session: ProtocolSession,
    /// One protocol operation at a time (review 57 §11 / INV-MCP-SESSION-006).
    pub operation_active: bool,
}

#[derive(Default)]
pub struct ProtocolSessionManager {
    sessions: HashMap<ProtocolSessionKey, SessionRecord>,
}

impl ProtocolSessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// §20 `get`: current session id for a key, if any.
    pub fn get(&self, key: &ProtocolSessionKey) -> Option<ProtocolSessionId> {
        self.sessions
            .get(key)
            .filter(|r| r.session.state != ProtocolSessionState::Closed)
            .map(|r| r.session.session_id)
    }

    pub fn state(&self, session_id: &ProtocolSessionId) -> Option<ProtocolSessionState> {
        self.sessions
            .values()
            .find(|r| r.session.session_id == *session_id)
            .map(|r| r.session.state)
    }

    /// §20 `establish`: register a NEW session bound to `runtime_id`. The
    /// caller performs the protocol handshake separately and reports the
    /// outcome via [`ProtocolSessionManager::mark_ready`] /
    /// [`ProtocolSessionManager::mark_failed`] — establishment bookkeeping
    /// is not the protocol exchange itself (§20).
    pub fn establish(
        &mut self,
        key: ProtocolSessionKey,
        runtime_id: Option<RuntimeId>,
    ) -> Result<ProtocolSessionId, SessionError> {
        if let Some(existing) = self.sessions.get(&key) {
            if existing.session.state == ProtocolSessionState::Ready && !existing.operation_active
            {
                // idempotent: a Ready session is reused, not re-established
                return Ok(existing.session.session_id);
            }
        }
        let session = ProtocolSession::new(key.clone(), runtime_id);
        let id = session.session_id;
        self.sessions.insert(
            key,
            SessionRecord { session, operation_active: false },
        );
        Ok(id)
    }

    /// Record that the protocol handshake for `session_id` succeeded.
    pub fn mark_ready(&mut self, session_id: &ProtocolSessionId) -> Result<(), SessionError> {
        let record = Self::record_by_id(self, session_id)?;
        record.session.state = ProtocolSessionState::Ready;
        record.session.touch();
        Ok(())
    }

    /// Record establish failure (§5 `Failed`): distinct from runtime death.
    pub fn mark_failed(&mut self, session_id: &ProtocolSessionId) -> Result<(), SessionError> {
        let record = Self::record_by_id(self, session_id)?;
        record.session.state = ProtocolSessionState::Failed;
        Ok(())
    }

    /// §11 `acquire_operation`: Ready → one active operation; Busy/Invalid/
    /// Closed otherwise. No queue (INV-MCP-SESSION-006).
    pub fn acquire_operation(
        &mut self,
        key: &ProtocolSessionKey,
    ) -> Result<ProtocolSessionId, SessionError> {
        let record = self
            .sessions
            .get_mut(key)
            .ok_or(SessionError::NotFound)?;
        if record.session.key != *key {
            return Err(SessionError::KeyMismatch);
        }
        match record.session.state {
            ProtocolSessionState::Ready if !record.operation_active => {
                record.operation_active = true;
                record.session.touch();
                Ok(record.session.session_id)
            }
            ProtocolSessionState::Ready => Err(SessionError::Busy),
            ProtocolSessionState::Closed => Err(SessionError::Closed),
            ProtocolSessionState::Invalid | ProtocolSessionState::Failed => {
                Err(SessionError::NotRecoverable)
            }
            _ => Err(SessionError::Busy),
        }
    }

    pub fn release_operation(&mut self, session_id: &ProtocolSessionId) -> Result<(), SessionError> {
        let record = Self::record_by_id(self, session_id)?;
        record.operation_active = false;
        record.session.touch();
        Ok(())
    }

    /// §13: invalidate the session (protocol failure). The RUNTIME verdict
    /// is separate — this never kills a process.
    pub fn invalidate(&mut self, session_id: &ProtocolSessionId) -> Result<(), SessionError> {
        let record = Self::record_by_id(self, session_id)?;
        record.session.state = ProtocolSessionState::Invalid;
        record.operation_active = false;
        Ok(())
    }

    /// §16: close every session bound to `runtime_id` — called by the
    /// executor BEFORE releasing the runtime so no registry entry dangles
    /// (INV-MCP-SESSION-008). Returns the closed session ids.
    pub fn close_for_runtime(&mut self, runtime_id: &RuntimeId) -> Vec<ProtocolSessionId> {
        let mut closed = Vec::new();
        for record in self.sessions.values_mut() {
            if record.session.runtime_id.as_ref() == Some(runtime_id)
                && record.session.state != ProtocolSessionState::Closed
            {
                record.session.state = ProtocolSessionState::Closed;
                record.operation_active = false;
                closed.push(record.session.session_id);
            }
        }
        closed
    }

    /// Drop Closed records (registry hygiene; INV-MCP-SESSION-008).
    pub fn purge_closed(&mut self) {
        self.sessions
            .retain(|_, r| r.session.state != ProtocolSessionState::Closed);
    }

    pub fn session(&self, key: &ProtocolSessionKey) -> Option<&ProtocolSession> {
        self.sessions.get(key).map(|r| &r.session)
    }

    fn record_by_id(
        &mut self,
        session_id: &ProtocolSessionId,
    ) -> Result<&mut SessionRecord, SessionError> {
        self.sessions
            .values_mut()
            .find(|r| r.session.session_id == *session_id)
            .ok_or(SessionError::NotFound)
    }
}

/// Key-consistency guard for call sites that pass a session id + key pair
/// (review 57 §18): exact identity required.
pub fn validate_pair(
    manager: &ProtocolSessionManager,
    session_id: &ProtocolSessionId,
    key: &ProtocolSessionKey,
) -> Result<(), SessionError> {
    match manager.get(key) {
        Some(id) if id == *session_id => Ok(()),
        Some(_) => Err(SessionError::KeyMismatch),
        None => Err(SessionError::NotFound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::McpProtocolProfile;

    fn key(server: &str, profile: McpProtocolProfile) -> ProtocolSessionKey {
        ProtocolSessionKey::mcp(server, profile)
    }

    /// SESSION-001: New → (initialize) → Ready bookkeeping.
    #[test]
    fn session001_establish_ready() {
        let mut m = ProtocolSessionManager::new();
        let k = key("calc", McpProtocolProfile::V2026_07_28);
        let sid = m.establish(k.clone(), None).unwrap();
        assert_eq!(m.state(&sid), Some(ProtocolSessionState::New));
        m.mark_ready(&sid).unwrap();
        assert_eq!(m.state(&sid), Some(ProtocolSessionState::Ready));
    }

    /// SESSION-002 (§12): a Ready session is reused — establish is
    /// idempotent, initialize count stays 1 (executor-level proof below).
    #[test]
    fn session002_ready_reuse_idempotent() {
        let mut m = ProtocolSessionManager::new();
        let k = key("calc", McpProtocolProfile::V2026_07_28);
        let s1 = m.establish(k.clone(), None).unwrap();
        m.mark_ready(&s1).unwrap();
        let s2 = m.establish(k.clone(), None).unwrap();
        assert_eq!(s1, s2, "Ready session is reused (initialize stays 1)");
    }

    /// SESSION-003 (§13): invalid → re-establish creates a NEW session id
    /// (S1 ≠ S2) on the same runtime.
    #[test]
    fn session003_invalid_then_new_session() {
        let mut m = ProtocolSessionManager::new();
        let k = key("calc", McpProtocolProfile::V2025_06_18);
        let rid = RuntimeId::mint();
        let s1 = m.establish(k.clone(), Some(rid)).unwrap();
        m.mark_ready(&s1).unwrap();
        m.invalidate(&s1).unwrap();
        assert_eq!(m.state(&s1), Some(ProtocolSessionState::Invalid));
        let s2 = m.establish(k, Some(rid)).unwrap();
        assert_ne!(s1, s2, "S1 != S2 on the same runtime R1");
    }

    /// SESSION-004: Closed sessions cannot be reused or re-established.
    #[test]
    fn session004_closed_is_terminal() {
        let mut m = ProtocolSessionManager::new();
        let k = key("calc", McpProtocolProfile::V2025_06_18);
        let sid = m.establish(k.clone(), None).unwrap();
        m.mark_ready(&sid).unwrap();
        let closed = m.close_for_runtime(&RuntimeId::mint());
        // close_for_runtime only closes sessions of the GIVEN runtime —
        // an unrelated runtime id closes nothing
        assert!(closed.is_empty());
        // closing by the actual runtime: bind one first
        let rid = RuntimeId::mint();
        let k2 = key("calc2", McpProtocolProfile::V2025_06_18);
        let s2 = m.establish(k2.clone(), Some(rid)).unwrap();
        let closed = m.close_for_runtime(&rid);
        assert_eq!(closed, vec![s2]);
        assert_eq!(m.state(&s2), Some(ProtocolSessionState::Closed));
        assert!(matches!(
            m.acquire_operation(&k2),
            Err(SessionError::Closed)
        ));
        // re-establish after close creates a fresh session (new execution)
        assert_ne!(m.establish(k2, Some(rid)).unwrap(), s2);
    }

    /// SESSION-030/031: acquire on non-Ready → Busy/Invalid/Closed, never
    /// a hidden queue.
    #[test]
    fn session030_operation_acquire_states() {
        let mut m = ProtocolSessionManager::new();
        let k = key("calc", McpProtocolProfile::V2025_06_18);
        let sid = m.establish(k.clone(), None).unwrap();
        // New: not usable yet
        assert_eq!(m.acquire_operation(&k), Err(SessionError::Busy));
        m.mark_ready(&sid).unwrap();
        let op = m.acquire_operation(&k).unwrap();
        // second concurrent acquire → Busy
        assert!(matches!(m.acquire_operation(&k), Err(SessionError::Busy)));
        m.release_operation(&op).unwrap();
        // released: acquirable again (no queue existed)
        assert!(m.acquire_operation(&k).is_ok());
        // invalidate while op active → operation dropped, Invalid
        m.invalidate(&sid).unwrap();
        assert!(matches!(
            m.acquire_operation(&k),
            Err(SessionError::NotRecoverable)
        ));
    }

    /// SESSION-040: close_for_runtime removes all sessions of one runtime
    /// (idle-cleanup ordering: sessions close BEFORE runtime release).
    #[test]
    fn session040_close_for_runtime() {
        let mut m = ProtocolSessionManager::new();
        let rid = RuntimeId::mint();
        for server in ["a", "b"] {
            let k = key(server, McpProtocolProfile::V2026_07_28);
            let sid = m.establish(k, Some(rid)).unwrap();
            m.mark_ready(&sid).unwrap();
        }
        assert_eq!(m.close_for_runtime(&rid).len(), 2);
        assert!(m.close_for_runtime(&RuntimeId::mint()).is_empty());
    }
}
