//! Agent Observability (P27-F-adjacent, spec §37/§38): the §37 event log
//! and §38 metrics for agent runs.
//!
//! Red line (§37): full sensitive prompts are NEVER persisted here — the
//! event ring stores event kinds, step/action identifiers and durations
//! only. Metrics are plain counters/durations for diagnostics and the
//! release gate.

/// §37 event kinds (exact set required by the spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentEvent {
    SessionCreated,
    PlanGenerated,
    PlanRejected,
    ApprovalRequested,
    ApprovalReceived,
    StepStarted,
    StepCompleted,
    StepFailed,
    Replanned,
    BudgetExceeded,
    RunSucceeded,
    RunFailed,
    RunCancelled,
}

impl AgentEvent {
    /// Stable snake_case name (matches the §37 names verbatim).
    pub fn name(&self) -> &'static str {
        match self {
            Self::SessionCreated => "agent_session_created",
            Self::PlanGenerated => "agent_plan_generated",
            Self::PlanRejected => "agent_plan_rejected",
            Self::ApprovalRequested => "agent_approval_requested",
            Self::ApprovalReceived => "agent_approval_received",
            Self::StepStarted => "agent_step_started",
            Self::StepCompleted => "agent_step_completed",
            Self::StepFailed => "agent_step_failed",
            Self::Replanned => "agent_replanned",
            Self::BudgetExceeded => "agent_budget_exceeded",
            Self::RunSucceeded => "agent_run_succeeded",
            Self::RunFailed => "agent_run_failed",
            Self::RunCancelled => "agent_run_cancelled",
        }
    }
}

/// One event: kind + optional non-sensitive detail (step/action ids only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub at_ms: i64,
    pub event: AgentEvent,
    /// Non-sensitive detail: step ids / action refs, never prompt text.
    pub detail: String,
}

/// §37 bounded event ring. Prompt content must never be recorded here.
#[derive(Debug, Default)]
pub struct EventLog {
    records: Vec<EventRecord>,
}

pub const EVENT_LOG_CAP: usize = 128;

impl EventLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, at_ms: i64, event: AgentEvent, detail: &str) {
        if self.records.len() >= EVENT_LOG_CAP {
            self.records.remove(0);
        }
        // defensive: keep details short and identifier-shaped
        let detail: String = detail.chars().take(128).collect();
        self.records.push(EventRecord { at_ms, event, detail });
    }

    pub fn records(&self) -> &[EventRecord] {
        &self.records
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

/// §38 metrics: the required counters and durations.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentMetrics {
    pub llm_latency_ms: u64,
    pub planning_latency_ms: u64,
    pub validation_latency_ms: u64,
    pub approval_wait_ms: u64,
    pub agent_run_latency_ms: u64,
    pub tool_selection_count: u64,
    pub replan_count: u64,
    pub budget_exceeded_count: u64,
    pub model_error_count: u64,
}

impl AgentMetrics {
    /// Add another run's metrics into an aggregate (release-gate rollup).
    pub fn accumulate(&mut self, other: &AgentMetrics) {
        self.llm_latency_ms += other.llm_latency_ms;
        self.planning_latency_ms += other.planning_latency_ms;
        self.validation_latency_ms += other.validation_latency_ms;
        self.approval_wait_ms += other.approval_wait_ms;
        self.agent_run_latency_ms += other.agent_run_latency_ms;
        self.tool_selection_count += other.tool_selection_count;
        self.replan_count += other.replan_count;
        self.budget_exceeded_count += other.budget_exceeded_count;
        self.model_error_count += other.model_error_count;
    }
}

/// Per-run telemetry: event ring + metrics. Passed into `run_agent`.
#[derive(Debug, Default)]
pub struct Telemetry {
    pub log: EventLog,
    pub metrics: AgentMetrics,
}

impl Telemetry {
    pub fn event(&mut self, at_ms: i64, event: AgentEvent, detail: &str) {
        self.log.record(at_ms, event, detail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §37: the full required event set exists with the spec's names.
    #[test]
    fn event_names_cover_spec() {
        let all = [
            AgentEvent::SessionCreated,
            AgentEvent::PlanGenerated,
            AgentEvent::PlanRejected,
            AgentEvent::ApprovalRequested,
            AgentEvent::ApprovalReceived,
            AgentEvent::StepStarted,
            AgentEvent::StepCompleted,
            AgentEvent::StepFailed,
            AgentEvent::Replanned,
            AgentEvent::BudgetExceeded,
            AgentEvent::RunSucceeded,
            AgentEvent::RunFailed,
            AgentEvent::RunCancelled,
        ];
        assert_eq!(all.len(), 13);
        assert_eq!(all[0].name(), "agent_session_created");
        assert_eq!(all[6].name(), "agent_step_completed");
        assert_eq!(all[12].name(), "agent_run_cancelled");
        // every name is unique
        let names: Vec<_> = all.iter().map(|e| e.name()).collect();
        assert_eq!(names.len(), std::collections::HashSet::<_>::from_iter(names.iter()).len());
    }

    /// §37: the ring is bounded (oldest dropped) and details bounded.
    #[test]
    fn event_ring_bounded() {
        let mut log = EventLog::new();
        for i in 0..EVENT_LOG_CAP + 10 {
            log.record(i as i64, AgentEvent::StepStarted, &format!("step{i}"));
        }
        assert_eq!(log.records().len(), EVENT_LOG_CAP);
        assert_eq!(log.records()[0].detail, "step10");
        log.record(0, AgentEvent::StepStarted, &"x".repeat(500));
        assert!(log.records().last().unwrap().detail.chars().count() <= 128);
    }

    /// §38: metrics accumulate for the release-gate rollup.
    #[test]
    fn metrics_accumulate() {
        let mut total = AgentMetrics::default();
        let mut run = AgentMetrics {
            llm_latency_ms: 100,
            replan_count: 1,
            ..Default::default()
        };
        run.approval_wait_ms = 40;
        total.accumulate(&run);
        total.accumulate(&run);
        assert_eq!(total.llm_latency_ms, 200);
        assert_eq!(total.replan_count, 2);
        assert_eq!(total.approval_wait_ms, 80);
    }
}
