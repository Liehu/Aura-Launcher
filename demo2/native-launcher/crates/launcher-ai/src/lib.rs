//! launcher-ai (MVP4.4 P0-D, review 55): LLM planning infrastructure.
//!
//! The LLM is a PROPOSAL PRODUCER, never an executor (§1): it may see a
//! readonly catalog view and user intent, and it outputs structured
//! proposals that re-enter the frozen pipeline
//! (`execute_proposals → ReferenceResolver → ActionResolver → Engine`).
//! This crate must never know McpExecutor, McpTransport, capabilities,
//! confirmation state or the credential store (LLM-ARCH-001..005).

pub mod agent;
pub mod agent_contract;
pub mod agent_loop;
pub mod agent_session;
pub mod clarification;
pub mod proposal_builder;
pub mod risk_classifier;
pub mod intent;
pub mod pipeline;
pub mod structured_output;
pub mod llm;
pub mod planner;
pub mod prompt;
pub mod provider_caps;

pub use llm::{LlmError, LlmProvider, LlmRequest, LlmResponse, MockLlmProvider};
pub use planner::{LlmPlanner, PlannerDiagnostics, PlannerResult};
