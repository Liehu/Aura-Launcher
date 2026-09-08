//! BudgetController (review 59 §17/§70/§71): the ONLY component that
//! decides whether the agent loop may continue. Independent of the
//! planner — the planner can never consume, reset or extend budgets.

use std::time::{Duration, Instant};

use launcher_domain::workflow::MAX_STEPS_PER_RUN;

use super::types::AgentLimits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetVerdict {
    Continue,
    /// (exhausted dimension)
    Exhausted(&'static str),
}

/// Tracks consumption against [`AgentLimits`]. The planner can neither
/// read-modify-write budgets nor reset them via replan (§60/§61).
#[derive(Debug)]
pub struct BudgetController {
    limits: AgentLimits,
    turns: u32,
    executions: usize,
    failures: usize,
    replans: usize,
    started_at: Instant,
}

impl BudgetController {
    pub fn new(limits: AgentLimits) -> Self {
        Self { limits, turns: 0, executions: 0, failures: 0, replans: 0, started_at: Instant::now() }
    }

    pub fn limits(&self) -> &AgentLimits {
        &self.limits
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

    /// May another turn start? (turn + wall-time budgets)
    pub fn can_start_turn(&self) -> BudgetVerdict {
        if self.turns >= self.limits.max_turns {
            return BudgetVerdict::Exhausted("max_turns");
        }
        if self.started_at.elapsed() >= self.limits.max_wall_time {
            return BudgetVerdict::Exhausted("max_wall_time");
        }
        BudgetVerdict::Continue
    }

    /// May another proposal be executed? (execution budget)
    pub fn can_execute(&self) -> BudgetVerdict {
        if self.executions >= self.limits.max_total_executions {
            return BudgetVerdict::Exhausted("max_total_executions");
        }
        if self.failures >= self.limits.max_failures {
            return BudgetVerdict::Exhausted("max_failures");
        }
        BudgetVerdict::Continue
    }

    /// May the agent replan after a failure? (bounded replan budget)
    pub fn can_replan(&self) -> BudgetVerdict {
        if self.replans >= self.limits.max_replans {
            return BudgetVerdict::Exhausted("max_replans");
        }
        BudgetVerdict::Continue
    }

    pub fn record_turn(&mut self) {
        self.turns += 1;
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
}
