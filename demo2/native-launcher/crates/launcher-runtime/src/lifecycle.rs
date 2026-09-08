//! Lifecycle state (review 53 §6): process mechanics only. States like
//! Authorized / Confirmed / McpReady are NOT runtime concerns and must
//! never appear here.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Created,
    Starting,
    Running,
    Stopping,
    Stopped,
    /// Process ended by itself (crash or normal exit observed too late to
    /// distinguish) — the runtime only knows the process is gone.
    Crashed,
}

impl LifecycleState {
    pub fn is_alive(&self) -> bool {
        matches!(self, LifecycleState::Starting | LifecycleState::Running)
    }
}
