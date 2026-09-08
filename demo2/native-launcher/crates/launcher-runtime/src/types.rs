//! Runtime identity + termination types (review 53 §4/§5/§18).
//!
//! RuntimeId = ONE live process instance. It is deliberately a different
//! namespace from ExecutionId (one Effect execution attempt): the runtime
//! never mints execution ids (RT-SEC-001) — it only ever receives them as
//! caller-supplied correlation metadata, which it does not even store.

use std::fmt;

/// Monotonic process-instance counter shared by all supervisors in the
/// process. Ids are opaque: `r-<n>`.
static RUNTIME_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuntimeId(u64);

impl RuntimeId {
    pub fn mint() -> Self {
        let n = RUNTIME_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        RuntimeId(n)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r-{}", self.0)
    }
}

/// How a process ended (review 53 §18): Windows has no Unix signal
/// semantics, so termination is modeled generically — never as
/// BusinessError/ProtocolViolation, which are protocol concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessTermination {
    /// Exited by itself with the given (optional) exit code.
    Exited { code: Option<i32> },
    /// Killed by the supervisor (grace expired / forced kill).
    Killed,
    /// Exit status could not be observed.
    Unknown,
}

impl ProcessTermination {
    pub fn from_exit_code(code: Option<i32>) -> Self {
        ProcessTermination::Exited { code }
    }
}
