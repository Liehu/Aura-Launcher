//! Core-side [`launcher_ai::agent::AgentExecutionHost`] implementation
//! (review 59 §46/§47): the agent's proposals enter through the frozen
//! pipeline — ReferenceResolver → ActionResolver → ActionEngine — with the
//! execution boundary owned by launcher-core, never by launcher-ai.

use launcher_ai::agent::{AgentExecutionHost, ExecutionRecord};
use crate::workflow_backend::CoreWorkflowBackend;
use crate::Core;
use launcher_domain::Command;
use launcher_workflow::proposal::ActionProposal;

/// Host adapter over a live Core: the agent's proposals execute through
/// the frozen orchestration path exactly as every other producer's do.
/// Each turn's proposals run through one WorkflowRunner instance.
pub struct CoreAgentHost<'a> {
    core: &'a mut Core,
    session: Vec<Command>,
}

impl<'a> CoreAgentHost<'a> {
    pub fn new(core: &'a mut Core) -> Self {
        Self { core, session: Vec::new() }
    }

    /// Seed the popup-session results (R1 priority for the resolver).
    pub fn with_session(mut self, session: Vec<Command>) -> Self {
        self.session = session;
        self
    }
}

impl AgentExecutionHost for CoreAgentHost<'_> {
    fn catalog(&mut self) -> Vec<Command> {
        self.core.action_catalog()
    }

    fn execute(&mut self, proposals: &[ActionProposal]) -> Vec<ExecutionRecord> {
        let mut records = Vec::new();
        for (i, proposal) in proposals.iter().enumerate() {
            let step = proposal.to_step(format!("agent-step-{i}"));
            let def = launcher_domain::WorkflowDefinition {
                id: format!("wf-agent-{}", i),
                version: 1,
                name: format!("agent turn step {i}"),
                steps: vec![step],
                failure_policy: Default::default(),
                entry_step: None,
                variables: Vec::new(),
                inputs: Vec::new(),
            };
            let session = std::mem::take(&mut self.session);
            let mut runner = launcher_workflow::WorkflowRunner::new(CoreWorkflowBackend {
                core: self.core,
                session_results: &session,
                discovery_limit: 50,
            });
            let run = match runner.run(&def, format!("wr-agent-{i}"), 1) {
                Ok(r) => r,
                Err(e) => {
                    records.push(ExecutionRecord {
                        execution_id: None,
                        action_id: proposal.action_id.clone(),
                        ok: false,
                        error: Some(e),
                        result: None,
                    });
                    self.session = session;
                    continue;
                }
            };
            let sr = &run.steps[0];
            records.push(ExecutionRecord {
                execution_id: sr.last_execution_id.clone(),
                action_id: proposal.action_id.clone(),
                ok: sr.status == launcher_domain::StepRunStatus::Complete,
                error: sr.last_error.clone(),
                result: sr.output.clone(),
            });
            self.session = session;
        }
        records
    }
}
