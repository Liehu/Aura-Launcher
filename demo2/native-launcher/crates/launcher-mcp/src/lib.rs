//! MCP Adapter (MVP4.3, spec `40-mvp8-0.1`): Model Context Protocol support
//! as a Producer, not an execution framework.
//!
//! Layering (Spec section 4):
//! - [`protocol`]: MCP protocol DTOs (JSON-RPC 2.0, initialize, tools)
//! - [`transport`]: [`transport::McpTransport`] trait + stdio implementation
//! - [`types`]: catalog/identity types (server, tool, snapshot)
//! - [`identity`]: stable route-identity helpers (INV-MCP-006)
//! - [`adapter`]: McpTool → Launcher `Command` projection (Action Proposal)
//! - [`executor`]: McpExecutor — the effect executor for approved
//!   `plugin.mcp.invoke` effects (MVP4.3 Phase 6)
//! - [`error`]: normalized MCP errors → Workflow failure classes
//!
//! Boundary (Spec section 3): the adapter/catalog side produces
//! proposal-shaped data only and must never depend on launcher-action,
//! launcher-plugin-host, or launcher-workflow — MCP has no execution
//! authority (ADR-0018). Execution authority lives exclusively in
//! [`executor`], reached only through the ActionEngine's effect routing
//! (review 41 §7): Provider = Discovery/Proposal, Executor = Effect
//! Executor, MCP Server = External Untrusted Capability Provider.

pub mod adapter;
pub mod auth;
pub mod catalog;
pub mod compat;
pub mod error;
pub mod executor;
pub mod identity;
pub mod protocol;
pub mod transport;
pub mod types;

pub use error::McpError;

/// MCP protocol version of the **Legacy Compatibility Profile** implemented
/// by [`transport::stdio`] (initialize → initialized → tools/* over a
/// stateful stdio session).
///
/// Boundary note (ADR-0018 Addendum / review 32): MCP 2026-07-28 removed the
/// protocol-level session handshake in favor of a stateless request model.
/// That future adapter plugs in BEHIND [`transport::McpTransport`] — catalog,
/// identity, adapter and projection types in this crate are version-neutral
/// and MUST NOT be reshaped around either profile.
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
