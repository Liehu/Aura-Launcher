//! MVP4.3 Phase 10 — Security Hardening suite root (review 45 §14/§15).
//!
//! Threat model (§1): the MCP server, tool metadata, tool results, AI
//! output and process/protocol behavior are all UNTRUSTED. The only
//! authority path is Host config → policy → ActionResolver → ActionEngine
//! → effect routing. Total principle (§21): **no external artifact may
//! increase authority** — external world may only add information.

#[path = "security/architecture.rs"]
mod architecture;
#[path = "security/capability.rs"]
mod capability;
#[path = "security/common/mod.rs"]
mod common;
#[path = "security/confirmation.rs"]
mod confirmation;
#[path = "security/confused_deputy.rs"]
mod confused_deputy;
#[path = "security/identity.rs"]
mod identity;
#[path = "security/metadata.rs"]
mod metadata;
#[path = "security/properties.rs"]
mod properties;
#[path = "security/prompt_injection.rs"]
mod prompt_injection;
#[path = "security/resource_exhaustion.rs"]
mod resource_exhaustion;
