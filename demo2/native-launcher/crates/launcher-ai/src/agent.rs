//! Agent Runtime (MVP4.4 P1-D, review 59): a stateful but
//! non-authoritative bounded decision loop.
//!
//! ```text
//! while budget.can_start_turn():
//!     observe → plan → propose → host resolve/authorize/execute → observe
//! ```
//!
//! Frozen boundaries (review 59 §62 INV-AGENT-001..015):
//! - The agent decides WHAT to propose; the ActionResolver decides WHETHER
//!   it is allowed; the ActionEngine decides HOW it executes; replan
//!   creates NEW proposals and never revives failed executions.
//! - The agent cannot construct Effects, cannot grant capabilities, cannot
//!   self-confirm, and cannot mint ExecutionIds — ids surface only inside
//!   host-produced ExecutionRecords.
//! - The loop is bounded by five independent budgets (§17); there is no
//!   hidden queue and no unbounded replan.

use std::time::{Duration, Instant};

use launcher_domain::Command;
use launcher_workflow::proposal::ActionProposal;

// ---------------- limits (§17/§33) ----------------

/// Five independent budgets + planner timeout (§17): any one exhausting
/// stops the agent. `max_turns` alone would allow unbounded proposal
/// fan-out.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentLimits {
    pub max_turns: u32,
    pub max_proposals_per_turn: usize,
    pub max_total_executions: usize,
    pub max_failures: usize,
    pub max_replans: usize,
    #[serde(default = "default_max_wall_time")]
    pub max_wall_time: Duration,
    #[serde(default = "default_planner_timeout")]
    pub planner_timeout: Duration,
}

fn default_max_wall_time() -> Duration {
    Duration::from_secs(300)
}
fn default_planner_timeout() -> Duration {
    Duration::from_secs(30)
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            max_turns: 8,
            max_proposals_per_turn: 8,
            max_total_executions: 16,
            max_failures: 4,
            max_replans: 4,
            max_wall_time: default_max_wall_time(),
            planner_timeout: default_planner_timeout(),
        }
    }
}

// ---------------- budget controller (§70/§71) ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetVerdict {
    Continue,
    Exhausted(&'static str),
}

/// The single budget authority (review 59 §70/§71): check / consume /
/// remaining. The planner can never consume, reset or extend budgets.
#[derive(Debug)]
pub struct BudgetController {
    limits: AgentLimits,
    turns: u32,
    proposals: usize,
    executions: usize,
    failures: usize,
    replans: usize,
    started_at: Instant,
}

impl BudgetController {
    pub fn new(limits: AgentLimits) -> Self {
        Self {
            limits,
            turns: 0,
            proposals: 0,
            executions: 0,
            failures: 0,
            replans: 0,
            started_at: Instant::now(),
        }
    }

    pub fn can_start_turn(&self) -> BudgetVerdict {
        if self.turns >= self.limits.max_turns {
            return BudgetVerdict::Exhausted("max_turns");
        }
        if self.started_at.elapsed() >= self.limits.max_wall_time {
            return BudgetVerdict::Exhausted("max_wall_time");
        }
        BudgetVerdict::Continue
    }

    pub fn can_execute(&self) -> BudgetVerdict {
        if self.executions >= self.limits.max_total_executions {
            return BudgetVerdict::Exhausted("max_total_executions");
        }
        if self.failures >= self.limits.max_failures {
            return BudgetVerdict::Exhausted("max_failures");
        }
        BudgetVerdict::Continue
    }

    pub fn can_replan(&self) -> BudgetVerdict {
        if self.replans >= self.limits.max_replans {
            return BudgetVerdict::Exhausted("max_replans");
        }
        BudgetVerdict::Continue
    }

    pub fn record_turn(&mut self) {
        self.turns += 1;
    }

    pub fn record_proposals(&mut self, n: usize) {
        self.proposals += n;
    }

    pub fn record_execution(&mut self) {
        self.executions += 1;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
    }

    pub fn record_replan(&mut self) {
        self.replans += 1;
    }

    pub fn turns(&self) -> u32 {
        self.turns
    }

    pub fn executions(&self) -> usize {
        self.executions
    }

    pub fn failures(&self) -> usize {
        self.failures
    }

    pub fn replans(&self) -> usize {
        self.replans
    }

    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn diagnostics(&self) -> AgentDiagnostics {
        AgentDiagnostics {
            turns: self.turns,
            executions: self.executions,
            replans: self.replans,
            failures: self.failures,
            elapsed: self.elapsed(),
        }
    }
}

// ---------------- state + records (§6/§7/§21) ----------------

/// Agent run lifecycle (review 59 §6/§32).
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

/// The planner's claim that the goal is satisfied — an AGENT DECISION; the
/// host reports "agent reports completion", never "goal verified"
/// (review 59 §57/§58).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GoalStatus {
    Continue,
    Complete,
    Blocked,
}

/// Host-redacted record of one executed proposal — the only view of an
/// execution the agent ever receives (review 59 §21).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionRecord {
    pub execution_id: Option<String>,
    pub action_id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentTurnRecord {
    pub turn: u32,
    pub objective: String,
    pub proposal_count: usize,
    pub executed: Vec<String>,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AgentDiagnostics {
    pub turns: u32,
    pub executions: usize,
    pub replans: usize,
    pub failures: usize,
    pub elapsed: Duration,
}

// ---------------- AgentHost port (§46/§47) ----------------

/// The host-mediated boundary. Implementations live OUTSIDE launcher-ai
/// (launcher-core wiring) so the agent never touches executors, runtimes
/// or protocol sessions. `execute` runs the frozen pipeline
/// (ReferenceResolver → ActionResolver → ActionEngine) for the turn's
/// proposals and returns host-produced ExecutionRecords.
pub trait AgentExecutionHost {
    /// Readonly catalog view for the planner (fresh empty-query discovery).
    fn catalog(&mut self) -> Vec<Command>;
    /// Execute the turn's proposals serially through the frozen pipeline.
    fn execute(&mut self, proposals: &[ActionProposal]) -> Vec<ExecutionRecord>;
    /// Whether the host requires UI confirmation for this proposal. The
    /// agent cannot self-confirm; the default is "no confirmation" so
    /// unauthenticated flows need no host override.
    fn confirmation_required(&self, _proposal: &ActionProposal) -> bool {
        false
    }
}

// ---------------- bounded loop (§15/§59/§70) ----------------

/// One completed agent turn.

/// Result of a full agent run.
#[derive(Debug, Clone)]
pub struct AgentRunOutcome {
    pub status: AgentRunStatus,
    pub turns: u32,
    pub executions: usize,
    pub failures: usize,
    pub replans: usize,
    pub turn_records: Vec<AgentTurnRecord>,
}

/// The bounded decision loop (§15/§70). There is no hidden `loop {}`:
/// every continuation is explicitly gated by `budget.can_*`.
pub fn run_bounded_agent<H: AgentExecutionHost, P: launcher_workflow::proposal::ActionPlanner>(
    host: &mut H,
    planner: &mut P,
    goal: &str,
    limits: &AgentLimits,
) -> AgentRunOutcome {
    let budget = &mut BudgetController::new(limits.clone());
    let mut turn_records: Vec<AgentTurnRecord> = Vec::new();
    let mut status = AgentRunStatus::Created;
    // P1-FIX-02: single source of truth — the counter lives ONLY in the
    // budget; AgentRunOutcome.replans reads it at the end.
    let mut observation = launcher_workflow::proposal::ReplanContext::default();
    let mut is_replan = false;

    while matches!(budget.can_start_turn(), BudgetVerdict::Continue) {
        budget.record_turn();

        // Observe: fresh readonly catalog (§9/§11 — snapshot, not live env)
        let catalog = host.catalog();
        // Plan (first turn) / Replan (subsequent turns, P1-FIX-03): the
        // planner sees the previous turn's failures, not just goal+catalog.
        let proposals: Vec<ActionProposal> = if is_replan {
            planner
                .replan(goal, &catalog, &observation)
                .into_iter()
                .take(limits.max_proposals_per_turn)
                .collect()
        } else {
            planner
                .plan(goal, &catalog)
                .into_iter()
                .take(limits.max_proposals_per_turn)
                .collect()
        };
        budget.record_proposals(proposals.len());

        // Empty plan: the agent reports nothing left to propose
        if proposals.is_empty() {
            status = AgentRunStatus::Completed;
            break;
        }

        // Execute serially via the host (§18/§19: order preserved, one
        // operation at a time, no queue, no parallel)
        let mut executed: Vec<String> = Vec::new();
        let mut failed: Vec<String> = Vec::new();
        let mut needs_confirmation = false;
        for p in &proposals {
            if host.confirmation_required(p) {
                needs_confirmation = true;
                break;
            }
            if budget.can_execute() != BudgetVerdict::Continue {
                break;
            }
            budget.record_execution();
            let records = host.execute(std::slice::from_ref(p));
            for r in records {
                let eid = r.execution_id.unwrap_or_default();
                if r.ok {
                    executed.push(eid);
                } else {
                    failed.push(eid);
                    // P1-FIX-03: record WHAT failed for the next replan
                    observation
                        .failed
                        .push((p.command_id.clone(), p.action_id.clone()));
                }
            }
        }
        if needs_confirmation {
            // §26/§27: the agent cannot self-confirm — surface to the host
            status = AgentRunStatus::WaitingForConfirmation;
            break;
        }
        if !failed.is_empty() {
            budget.record_failure();
        }

        let turn_record = AgentTurnRecord {
            turn: budget.turns(),
            objective: goal.into(),
            proposal_count: proposals.len(),
            executed: executed.clone(),
            failed: failed.clone(),
        };
        turn_records.push(turn_record);

        // All proposals executed successfully this turn → agent reports
        // completion (§57: agent decision, host may verify later)
        if failed.is_empty() && executed.len() == proposals.len() {
            status = AgentRunStatus::Completed;
            break;
        }

        // Failures happened: bounded replan (§24) — fresh observe, new
        // proposals; failed executions are never replayed. The next turn's
        // planner sees WHAT failed via the observation (P1-FIX-03).
        if budget.can_replan() != BudgetVerdict::Continue {
            status = AgentRunStatus::BudgetExhausted;
            break;
        }
        budget.record_replan();
        status = AgentRunStatus::Replanning;
        is_replan = true;
    }

    if status == AgentRunStatus::Created || status == AgentRunStatus::Replanning {
        status = AgentRunStatus::Failed;
    }

    AgentRunOutcome {
        status,
        turns: budget.turns(),
        executions: budget.executions(),
        failures: budget.failures(),
        replans: budget.replans(),
        turn_records,
    }
}
