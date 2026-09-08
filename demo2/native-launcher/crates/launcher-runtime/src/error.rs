//! Runtime-level errors (review 53 §7/§16/§19): the runtime reports WHAT
//! happened to the process/IO — never protocol or business semantics.
//! Upper layers (McpError / PluginError / FailureClass) do the mapping.

use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("process did not become ready within {0:?}")]
    StartupTimeout(Duration),
    #[error("io timed out after {0:?}")]
    IoTimeout(Duration),
    #[error("{stream} exceeded its {limit}-byte cap (allocation-bound enforcement)")]
    OutputLimitExceeded { stream: &'static str, limit: usize },
    #[error("process pipe closed (transport broken)")]
    TransportBroken,
    #[error("process exited: {code:?}")]
    ProcessExited { code: Option<i32> },
    #[error("process did not exit within the shutdown grace period")]
    ShutdownTimeout,
    #[error("invalid launch spec: {0}")]
    InvalidSpec(String),
}

impl RuntimeError {
    /// True when the process is gone (exited on its own) — upper layers use
    /// this to distinguish crash from timeout without knowing semantics.
    pub fn is_process_gone(&self) -> bool {
        matches!(self, RuntimeError::ProcessExited { .. } | RuntimeError::TransportBroken)
    }
}
