//! Agent state machine + observation model (review 59 §6/§8-§11/§32).
//! The observation is a HOST-BUILT SNAPSHOT of untrusted data — the agent
//! never touches fs/network/process/MCP directly, and observation data can
//! never redefine host policy (INV-AI-001..005).

use std::time::Instant;

use serde::{Deserialize, Serialize};

use launcher_workflow::proposal::ActionProposal;

/// Bounded observation limits (review 59 §74/§75).
#[derive(Debug, Clone, PartialEq)]
pub struct ObservationLimits {
    pub max_actions: usize,
    pub max_execution_records: usize,
    pub max_result_bytes: usize,
}

impl Default for ObservationLimits {
    fn default() -> Self {
        Self { max_actions: 50, max_execution_records: 8, max_result_bytes: 4096 }
    }
}

/// Host-redacted execution summary — what the agent may see about one
/// executed proposal.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionSummary {
    pub execution_id: String,
    pub action_id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

/// The per-turn observation snapshot (review 59 §9). Only
/// `ActionCatalogItem`-shaped descriptions, redacted execution summaries
/// and minimal context — never Effect internals, credentials, capability
/// grants or runtime handles (§10).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Observation {
    pub turn: u32,
    pub user_goal: Option<String>,
    pub available_actions: Vec<crate::catalog_item::AgentCatalogItem>,
    pub recent_executions: Vec<ExecutionSummary>,
}

impl Observation {
    /// Bounded serialization of the untrusted sections — used by prompt
    /// builders. Truncation is marked so the agent knows it saw a partial
    /// view (review 59 §75).
    pub fn bounded_view(&self, limits: &ObservationLimits) -> String {
        let mut out = String::from("<OBSERVATION_DATA>\n");
        let actions = self.available_actions.iter().take(limits.max_actions);
        for a in actions {
            out.push_str(&format!(
                "- {}/{} ({})\n",
                a.provider_id, a.command_id, a.action_type
            ));
        }
        if self.available_actions.len() > limits.max_actions {
            out.push_str(&format!(
                "(... {} more actions omitted ...)\n",
                self.available_actions.len() - limits.max_actions
            ));
        }
        for e in self.recent_executions.iter().take(limits.max_execution_records) {
            let text = e.result.as_deref().unwrap_or("");
            let truncated: String = text.chars().take(limits_max_result_bytes()).collect();
            out.push_str(&format!(
                "- exec {} {} ok={} result={:.width$}\n",
                e.execution_id,
                e.action_id,
                e.ok,
                truncated,
                width = limits.max_result_bytes
            ));
        }
        out.push_str("</OBSERVATION_DATA>");
        out
    }
}

fn limits_max_result_bytes() -> usize {
    4096
}

/// Agent plan (review 59 §12): proposals + optional rationale. The
/// rationale is diagnostics only — the host never treats it as policy.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentPlan {
    pub objective: String,
    pub rationale: Option<String>,
    pub proposals: Vec<ActionProposal>,
}

/// A serialized proposal for schema-strict parsing at the agent boundary
/// (review 59 §40): `deny_unknown_fields` rejects forged
/// authorized/confirmed/effect/capability fields at the SCHEMA layer.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrictAgentProposal {
    pub provider_id: String,
    pub command_id: String,
    pub action_id: String,
    #[serde(default)]
    pub input: serde_json::Value,
}

/// Strict agent-plan response schema (review 59 §40): only objective +
/// proposals; anything else is a schema violation.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentPlanResponse {
    pub objective: String,
    pub proposals: Vec<StrictAgentProposal>,
}

/// Agent run status snapshot for diagnostics (§50): no prompt raw content,
/// no credentials, no tokens.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentDiagnostics {
    pub turn: u32,
    pub executions: usize,
    pub replans: usize,
    pub failures: usize,
}

/// Trust boundary marker type: observation data is UNTRUSTED (§37/§38).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UntrustedData;

/// Timestamp helper for audit events (§51) — the audit chain records the
/// sequence Goal → Observe → Plan → Propose → Execute → Result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditStamp {
    pub at: Instant,
}
