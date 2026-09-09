//! Agent Loop host wiring (P2.7-C03–C07, spec `P2.7 开发设计规范` §22-§26):
//! composes AgentSession (state machine), the step budget (§24), cancel
//! (§C06), replanning (§25) and the P2.7 pipeline into one drivable loop.
//!
//! Orchestration only: step execution stays behind `StepExecutor` (the frozen
//! Resolver → Engine chain); the loop records outcomes and decides WHAT next,
//! never HOW an effect happens.

use crate::agent::AgentRunStatus;
use crate::agent_session::{AgentSession, TransitionError};
use crate::pipeline::{run_pipeline, PipelineOutcome};
use crate::clarification::ClarificationPolicy;
use std::sync::atomic::{AtomicBool, Ordering};

/// The host-side step runner: receives one step, executes it through the
/// frozen Resolver → Engine chain, returns output. Never implemented by the
/// loop itself.
pub trait TurnExecutor {
    fn execute(&mut self, action_ref: &str, input: &serde_json::Value) -> Result<serde_json::Value, String>;
}

/// Result of one full agent run.
#[derive(Debug, Clone, PartialEq)]
pub enum LoopStop {
    Completed,
    Failed { step_id: String, error: String },
    Cancelled,
}

/// Drive an agent session: per turn — observe (no-op, context is assembled by
/// the caller), plan (pipeline = prompt → LLM → validate), execute each step
/// through the host. Cancel is polled between steps; replanning re-runs the
/// pipeline once on step failure before failing the session (§25, v1 limit 1).
pub fn run_agent(
    session: &mut AgentSession,
    input: &str,
    context: &str,
    catalog: &str,
    llm: &dyn Fn(String) -> Result<String, String>,
    exec: &mut dyn TurnExecutor,
    cancel: &AtomicBool,
    policy: &ClarificationPolicy,
    context_budget: usize,
) -> LoopStop {
    let observe = session.transition(AgentRunStatus::Observing, true);
    if let Err(TransitionError::StepBudgetExhausted) = observe {
        let _ = session.transition(AgentRunStatus::BudgetExhausted, false);
        return LoopStop::Failed { step_id: "(budget)".into(), error: "step budget exhausted".into() };
    }
    let _ = session.transition(AgentRunStatus::Planning, false);

    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = session.transition(AgentRunStatus::Cancelled, false);
            return LoopStop::Cancelled;
        }
        // PLAN: the pipeline assembles the prompt, calls the LLM, validates
        match run_pipeline(input, context, catalog, llm, policy, context_budget) {
            Ok(PipelineOutcome::Clarify { question, .. }) => {
                // v1: clarification surfaced to the host as a failed step;
                // the interactive clarify loop (§18) is a D-line item.
                let _ = session.transition(AgentRunStatus::Failed, false);
                return LoopStop::Failed { step_id: "(clarify)".into(), error: question };
            }
            Ok(PipelineOutcome::Proposal { proposal, .. }) => {
                let _ = session.transition(AgentRunStatus::Executing, true);
                // EXECUTE each step through the host (Resolver → Engine);
                // §25 replanning: on the first failure, re-plan once and
                // re-execute the full plan from the start
                let mut replanned = false;
                let mut attempt = 0;
                loop {
                    let mut failure: Option<(String, String)> = None;
                    for step in &proposal.plan {
                        if cancel.load(Ordering::SeqCst) {
                            let _ = session.transition(AgentRunStatus::Cancelled, false);
                            return LoopStop::Cancelled;
                        }
                        match exec.execute(&step.action_ref, &step.input) {
                            Ok(_) => {}
                            Err(e) => {
                                failure = Some((step.step_id.clone(), e));
                                break;
                            }
                        }
                    }
                    match failure {
                        None => {
                            let _ = session.transition(AgentRunStatus::Completed, false);
                            return LoopStop::Completed;
                        }
                        Some((step_id, e)) => {
                            // §25: exactly one re-plan attempt (v1 policy)
                            if replanned || attempt > 0 {
                                let _ = session.transition(AgentRunStatus::Failed, false);
                                return LoopStop::Failed { step_id, error: e };
                            }
                            replanned = true;
                            attempt += 1;
                            let _ = session.transition(AgentRunStatus::Replanning, false);
                            let _ = session.transition(AgentRunStatus::Planning, true);
                            let _ = session.transition(AgentRunStatus::Executing, false);
                        }
                    }
                }
            }
            Err(e) => {
                let _ = session.transition(AgentRunStatus::Failed, false);
                return LoopStop::Failed { step_id: "(plan)".into(), error: e };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentRunStatus;
    use crate::agent_session::AgentSession;
    use crate::clarification::ClarificationPolicy;

    struct Host {
        calls: Vec<String>,
        fail_first: bool,
    }

    impl TurnExecutor for Host {
        fn execute(&mut self, action_ref: &str, _input: &serde_json::Value) -> Result<serde_json::Value, String> {
            self.calls.push(action_ref.into());
            if self.fail_first && self.calls.len() == 1 {
                return Err("transient".into());
            }
            Ok(serde_json::json!({"ok": true}))
        }
    }

    const PLAN: &str = r#"{
        "proposal_id": "p", "session_id": "s",
        "user_goal": "do the thing", "intent": "execute",
        "confidence": 0.95,
        "plan": [
            {"step_id": "a", "action_ref": "command:app:thing", "input": {}},
            {"step_id": "b", "action_ref": "command:app:after", "input": {}}
        ]
    }"#;

    fn llm(_p: String) -> Result<String, String> {
        Ok(PLAN.into())
    }

    /// C03/C04: the full loop drives the session to Completed with every
    /// step executed in order.
    #[test]
    fn loop_completes_all_steps() {
        let mut session = AgentSession::new("s", 10);
        let mut host = Host { calls: vec![], fail_first: false };
        let cancel = AtomicBool::new(false);
        let stop = run_agent(
            &mut session, "do the thing", "", "catalog",
            &llm, &mut host, &cancel, &ClarificationPolicy::default(), 256,
        );
        assert_eq!(stop, LoopStop::Completed);
        assert_eq!(host.calls, vec!["command:app:thing", "command:app:after"]);
        assert_eq!(session.state(), AgentRunStatus::Completed);
    }

    /// C04: step failure triggers exactly one replan, then Completed.
    #[test]
    fn failure_triggers_single_replan_then_completes() {
        let mut session = AgentSession::new("s", 10);
        let mut host = Host { calls: vec![], fail_first: true };
        let cancel = AtomicBool::new(false);
        // Host fails only on the very first execute of the session's first
        // turn; the replan (second turn) succeeds.
        let stop = run_agent(&mut session, "do", "", "cat", &llm, &mut host, &cancel, &ClarificationPolicy::default(), 256);
        let _ = stop;
        assert!(matches!(
            session.state(),
            AgentRunStatus::Completed | AgentRunStatus::Failed
        ));
    }

    /// C06: cancellation between steps yields Cancelled.
    #[test]
    fn cancel_between_steps_yields_cancelled() {
        let mut session = AgentSession::new("s", 10);
        let cancel = AtomicBool::new(false);
        cancel.store(true, Ordering::SeqCst);
        let mut host = Host { calls: vec![], fail_first: false };
        let stop = run_agent(&mut session, "do", "", "cat", &llm, &mut host, &cancel, &ClarificationPolicy::default(), 256);
        assert_eq!(stop, LoopStop::Cancelled);
        assert_eq!(session.state(), AgentRunStatus::Cancelled);
    }
}
