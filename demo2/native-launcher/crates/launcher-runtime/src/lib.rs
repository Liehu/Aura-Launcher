//! launcher-runtime (MVP4.4 P0-A, reviews 51/52/53): external process
//! lifecycle infrastructure shared by the Plugin broker and the MCP
//! executor.
//!
//! Boundary (frozen, review 53 §2/§17): this crate knows HOW to run an
//! external process — spawn (Windows: CREATE_SUSPENDED → Job Object →
//! resume), bounded stdio, timeouts, graceful/forced shutdown, process-tree
//! reap — and NOTHING else. It must never know:
//!
//! ```text
//! ActionProposal / ActionResolver / ActionEngine / Effect / Capability /
//! Confirmation / Workflow / MCP protocol / Plugin protocol
//! ```
//!
//! It also NEVER mints execution ids (RT-SEC-001): correlation ids belong
//! to the caller (the execution orchestrator). stdout/stderr are bounded
//! byte/line transports with independent limits — framing above lines and
//! all protocol semantics stay in the protocol owners.
//!
//! ```text
//! PluginBroker / McpExecutor   = WHAT the bytes mean
//! launcher-runtime             = HOW the process lives and dies
//! ```

pub mod error;
pub mod io;
pub mod lifecycle;
pub mod process;
pub mod spec;
pub mod runtime_manager;
pub mod supervisor;
pub mod types;

#[cfg(windows)]
pub mod windows;

pub use error::RuntimeError;
pub use lifecycle::LifecycleState;
pub use process::ProcessSession;
pub use spec::{LaunchSpec, RuntimeLimits};
pub use runtime_manager::{RuntimeKey, RuntimeManager, RuntimeManagerError, RuntimeMode, RuntimePolicy, RestartPolicy};
pub use supervisor::ProcessSupervisor;
pub use types::{ProcessTermination, RuntimeId};
