//! Agent Runtime types (MVP4.4 P1-D, review 59 §4-§14/§22): the agent is a
//! stateful decision loop with NO execution authority.
//!
//! ```text
//! AgentRunId ≠ TurnId ≠ ExecutionId
//! ```
//!
//! The agent never mints execution ids (those come from the host inside
//! ExecutionRecords), never holds capabilities, never satisfies
//! confirmation by self-declaration, and its observation data can never
//! alter host policy (INV-AGENT-001..015).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use launcher_workflow::proposal::ActionProposal;

/// Static agent configuration (review 59 §5). Deliberately contains NO
/// capability grants, tool allow-lists or credentials — the definition can
/// never become an authorization vehicle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub limits: AgentLimits,
}

/// Five independent budgets (review 59 §17): turn / proposal / execution /
/// failure / wall-time. Any one exhausting stops the agent — multiple
/// budgets are required because `max_turns` alone allows unbounded
/// proposal fan-out.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentLimits {
    pub max_turns: u32,
    pub max_proposals_per_turn: usize,
    pub max_total_executions: usize,
    pub max_failures: usize,
    pub max_replans: usize,
    pub max_wall_time: Duration,
    pub planner_timeout: Duration,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            max_turns: 8,
            max_proposals_per_turn: 8,
            max_total_executions: 16,
            max_failures: 4,
            max_replans: 4,
            max_wall_time: Duration::from_secs(5 * 60),
            planner_timeout: Duration::from_secs(30),
        }
    }
}

/// Agent run lifecycle (review 59 §6/§32). Transitions are explicit; the
/// runner validates each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentRunStatus {
    Created,
    Observing,
    Planning,
    Executing,
    WaitingForConfirmation,
    Replanning,
    Completed,
    Failed,
    Cancelled,
    BudgetExhausted,
}

/// Why the agent entered a replan (review 59 §56): only enumerated,
/// host-recognized reasons may trigger a replan — never "every observation
/// → arbitrary replan".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReplanReason {
    ExecutionFailure,
    StaleContext,
    UnexpectedResult,
    GoalNotSatisfied,
}

/// The planner's opinion that its goal is satisfied (review 59 §57/§58):
/// this is an AGENT DECISION — the UI/Host reports "agent reports
/// completion", never "goal verified".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GoalStatus {
    Continue,
    Complete,
    Blocked,
}

/// Summary of one host-executed proposal, as visible to the agent
/// (review 59 §21). No Effect internals, no capability state, no
/// credentials — host-redacted summaries only.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionSummary {
    pub execution_id: String,
    pub action_id: String,
    pub ok: bool,
    /// Host-redacted result summary (truncated, secrets stripped).
    pub result: Option<String>,
}

/// One turn of the agent loop (review 59 §7): Observe → Plan → Propose →
/// (host) Execute → Result.
#[derive(Debug, Clone)]
pub struct AgentTurn {
    pub turn: u32,
    pub objective: String,
    pub proposals: Vec<ActionProposal>,
    pub execution_summaries: Vec<ExecutionSummary>,
    pub goal: GoalStatus,
}

/// Bounded-replan reasons actually tracked per run.
#[derive(Debug, Clone, Default)]
pub struct TurnLog {
    pub turns: Vec<AgentTurn>,
}

impl TurnLog {
    pub fn executed_count(&self) -> usize {
        self.turns.iter().map(|t| t.execution_summaries.len()).sum()
    }
}
