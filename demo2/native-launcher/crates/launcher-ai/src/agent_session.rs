//! Agent Session state machine (P27-C01/C02, spec `P2.7 开发设计规范`
//! §22-§24): formalizes the run-status lifecycle into a session with
//! WHITELISTED transitions, a step budget guard, and a bounded turn record.
//!
//! The state machine is ORCHESTRATION bookkeeping: it never executes —
//! proposals still flow through the frozen Resolver → Engine chain and
//! approvals stay in the approval flow.

use crate::agent::AgentRunStatus;

/// Legal transitions (§23 agent loop + §24 budget): Created → Observing →
/// Planning → Executing → (Completed | Failed | Replanning | Waiting...) —
/// BudgetExhausted is reachable from any active state; Cancelled likewise.
fn legal(from: AgentRunStatus, to: AgentRunStatus) -> bool {
    use AgentRunStatus::*;
    if to == Cancelled || to == BudgetExhausted {
        return !matches!(from, Completed | Failed | Cancelled | BudgetExhausted);
    }
    matches!(
        (from, to),
        (Created, Observing)
            | (Observing, Planning)
            | (Planning, Executing)
            | (Executing, Completed)
            | (Executing, Failed)
            | (Executing, Replanning)
            | (Executing, WaitingForConfirmation)
            | (Replanning, Planning)
            | (Replanning, Executing)
            | (WaitingForConfirmation, Executing)
            | (WaitingForConfirmation, Replanning)
    )
}

/// A terminal-state probe used by the runtime loop.
pub fn is_terminal(s: AgentRunStatus) -> bool {
    matches!(
        s,
        AgentRunStatus::Completed
            | AgentRunStatus::Failed
            | AgentRunStatus::Cancelled
            | AgentRunStatus::BudgetExhausted
    )
}

/// One agent session: id, current state, turn counter (§24 step budget is
/// enforced by `BudgetController`; this counter is the session-level view).
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub session_id: String,
    state: AgentRunStatus,
    pub turns: u64,
    max_turns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    Illegal { from: AgentRunStatus, to: AgentRunStatus },
    StepBudgetExhausted,
    AlreadyTerminal,
}

impl AgentSession {
    pub fn new(session_id: &str, max_turns: u64) -> Self {
        Self {
            session_id: session_id.to_string(),
            state: AgentRunStatus::Created,
            turns: 0,
            max_turns: max_turns.max(1),
        }
    }

    pub fn state(&self) -> AgentRunStatus {
        self.state
    }

    /// C02: validated transition. Terminal states are frozen; the step
    /// budget gates every turn-consuming transition.
    pub fn transition(
        &mut self,
        to: AgentRunStatus,
        consumes_turn: bool,
    ) -> Result<AgentRunStatus, TransitionError> {
        if is_terminal(self.state) {
            return Err(TransitionError::AlreadyTerminal);
        }
        if !legal(self.state, to) {
            return Err(TransitionError::Illegal { from: self.state, to });
        }
        if consumes_turn {
            if self.turns >= self.max_turns {
                return Err(TransitionError::StepBudgetExhausted);
            }
            self.turns += 1;
        }
        self.state = to;
        Ok(self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentRunStatus::*;

    #[test]
    fn happy_loop_transitions_legal() {
        let mut s = AgentSession::new("s1", 10);
        s.transition(Observing, true).unwrap();
        s.transition(Planning, true).unwrap();
        s.transition(Executing, true).unwrap();
        s.transition(Replanning, false).unwrap();
        s.transition(Planning, true).unwrap();
        s.transition(Executing, true).unwrap();
        s.transition(Completed, false).unwrap();
        assert!(is_terminal(s.state()));
        // terminal states are frozen
        assert!(matches!(
            s.transition(Observing, true),
            Err(TransitionError::AlreadyTerminal)
        ));
    }

    /// Illegal shortcuts (Created → Executing) are rejected.
    #[test]
    fn illegal_shortcut_rejected() {
        let mut s = AgentSession::new("s2", 10);
        assert!(matches!(
            s.transition(Executing, true),
            Err(TransitionError::Illegal { .. })
        ));
    }

    /// §24: exceeding the step budget stops turn-consuming transitions.
    #[test]
    fn step_budget_enforced() {
        let mut s = AgentSession::new("s3", 3);
        s.transition(Observing, true).unwrap();
        s.transition(Planning, true).unwrap();
        s.transition(Executing, true).unwrap();
        s.transition(Replanning, false).unwrap();
        assert!(matches!(
            s.transition(Planning, true),
            Err(TransitionError::StepBudgetExhausted)
        ));
        // non-turn transitions still allowed (bookkeeping)
        s.transition(Cancelled, false).unwrap();
    }
}
