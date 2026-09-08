//! LaunchSpec + RuntimeLimits (review 53 §7/§8/§16).
//!
//! LaunchSpec is THE security boundary: it contains only OS-level launch
//! facts. Authority (capabilities/effects/providers) never enters this
//! type — the caller (an executor behind the ActionEngine) is responsible
//! for having decided that this program may run.

use std::path::PathBuf;
use std::time::Duration;

/// How to launch one external process. Deliberately dumb.
#[derive(Debug, Clone, Default)]
pub struct LaunchSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Explicit env overrides layered over the inherited environment.
    /// The runtime never injects PATH/TOKEN/AUTHORIZATION or anything else
    /// on its own (review 53 §8).
    pub env: Vec<(String, String)>,
    /// Working directory for the child; None inherits the caller's.
    pub working_dir: Option<PathBuf>,
}

impl LaunchSpec {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self { program: program.into(), ..Default::default() }
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.args.push(a.into());
        self
    }

    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
}

/// Per-runtime resource limits (review 53 §13/§14/§16). stdout and stderr
/// are capped INDEPENDENTLY so a hostile diagnostics stream can never crowd
/// out the protocol channel. Enforcement is at the allocation boundary:
/// the reader never buffers more than the cap before declaring a violation.
#[derive(Debug, Clone)]
pub struct RuntimeLimits {
    pub startup_timeout: Duration,
    pub io_timeout: Duration,
    pub shutdown_grace: Duration,
    pub max_stdout_line_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(2),
            io_timeout: Duration::from_secs(5),
            shutdown_grace: Duration::from_millis(200),
            max_stdout_line_bytes: 256 * 1024,
            max_stderr_bytes: 64 * 1024,
        }
    }
}

impl RuntimeLimits {
    pub fn with_io_timeout(mut self, timeout: Duration) -> Self {
        self.io_timeout = timeout;
        self
    }

    pub fn with_max_stdout_line_bytes(mut self, cap: usize) -> Self {
        self.max_stdout_line_bytes = cap;
        self
    }
}
