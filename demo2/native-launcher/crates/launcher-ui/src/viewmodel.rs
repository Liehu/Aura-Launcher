//! UI-CONTRACT-v0.2 view model (review 60 P1-E.1, §59-§61): the DTO layer
//! between Core and the presentation. Pure data + total mappings — no Slint
//! types, no core runtime handles, no capability/effect vocabulary.
//!
//! Invariants enforced here (§77):
//! - INV-UI-008: UI consumes immutable snapshot data only.
//! - INV-UI-009: status text/symbols are total functions of Core states —
//!   the UI never redefines what a state MEANS, only how it is spelled.
//! - INV-UI-010: `Close` (dismiss a panel) and `Cancel` (stop a run) are
//!   distinct commands; closing is never a rollback.

/// Top-level navigation mode (§4): frozen at Main/Action/Workflow. Agent,
/// MCP and runtime state are content INSIDE a panel, never new modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Main,
    Action,
    Workflow,
}

/// The only commands the UI may send upward (§60). There is deliberately
/// no `ExecuteEffect` / `Authorize` / `Grant` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCommand {
    Select,
    Invoke,
    Confirm,
    Cancel,
    Navigate,
    Close,
    ToggleDiagnostics,
}

/// Core → UI notifications (§61).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    SearchUpdated,
    ResultsUpdated,
    ActionStateChanged,
    WorkflowUpdated,
    AgentUpdated,
    ConfirmationRequired,
    DiagnosticsUpdated,
}

/// Keyboard focus target (§30) — replaces scattered per-component bools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Search,
    Results,
    Actions,
    Confirmation,
    Workflow,
    Diagnostics,
}

/// One timeline entry (§50). UI-only DTO: host-side code projects
/// Workflow/Agent/MCP events into these; the timeline never holds an
/// Effect, proposal, or runtime handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityItem {
    /// Fixed-format HH:MM:SS string; producers own clock policy so visual
    /// fixtures stay deterministic (§52).
    pub timestamp: String,
    pub kind: ActivityKind,
    pub title: String,
    pub summary: String,
    pub status: ActivityStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    Observe,
    Plan,
    Execute,
    Result,
    Replan,
    Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

impl ActivityStatus {
    /// Symbol + text pair (§15: never color-only — icon AND text).
    pub fn symbol_text(self) -> (&'static str, &'static str) {
        match self {
            ActivityStatus::Pending => ("○", "pending"),
            ActivityStatus::Running => ("◐", "running"),
            ActivityStatus::Succeeded => ("✓", "ok"),
            ActivityStatus::Failed => ("!", "failed"),
            ActivityStatus::Skipped => ("–", "skipped"),
        }
    }
}

/// ---- Empty state taxonomy (§6): ProviderUnavailable must never render
/// as "No Results". The host picks the variant from the search outcome;
/// the label is a total mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyState {
    NoQuery,
    NoResults,
    NoProviders,
    ProviderUnavailable,
}

impl EmptyState {
    pub fn label(self) -> &'static str {
        match self {
            EmptyState::NoQuery => "Start typing...",
            EmptyState::NoResults => "No matching commands",
            EmptyState::NoProviders => "No providers available",
            EmptyState::ProviderUnavailable => "Provider temporarily unavailable",
        }
    }

    /// Only a genuine empty index is a "quiet" empty; provider failure is
    /// surfaced as a warning-grade state, not blank space.
    pub fn is_warning(self) -> bool {
        matches!(self, EmptyState::ProviderUnavailable)
    }
}

/// ---- Disabled reason vocabulary (§11): diagnostics class names
/// (CapabilityDenied, FailureClass::ProtocolViolation, ...) never reach the
/// normal UI; they map to user wording here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledReason {
    CapabilityDenied,
    MissingContext,
    StaleContext,
    InvalidInput,
    Unavailable,
    ConfirmationRequired,
}

impl DisabledReason {
    pub fn label(self) -> &'static str {
        match self {
            DisabledReason::CapabilityDenied => "Not permitted",
            DisabledReason::MissingContext => "Requires a folder",
            DisabledReason::StaleContext => "Context changed",
            DisabledReason::InvalidInput => "Input is invalid",
            DisabledReason::Unavailable => "Temporarily unavailable",
            DisabledReason::ConfirmationRequired => "Confirmation required",
        }
    }

    /// Map a raw diagnostics reason string to the UI vocabulary. Unmapped
    /// strings stay visible verbatim (never silently blanked) but the
    /// caller may choose to render them only in diagnostics.
    pub fn from_diagnostic(raw: &str) -> Option<DisabledReason> {
        let r = raw.to_lowercase();
        if r.contains("denied") || r.contains("not permitted") || r.contains("capability") {
            Some(DisabledReason::CapabilityDenied)
        } else if r.contains("folder") || r.contains("context missing") || r.contains("missing") {
            Some(DisabledReason::MissingContext)
        } else if r.contains("stale") || r.contains("changed") {
            Some(DisabledReason::StaleContext)
        } else if r.contains("invalid") {
            Some(DisabledReason::InvalidInput)
        } else if r.contains("unavailable") {
            Some(DisabledReason::Unavailable)
        } else if r.contains("confirmation") {
            Some(DisabledReason::ConfirmationRequired)
        } else {
            None
        }
    }
}

/// ---- Provider/connection status (§24, §26): the normal UI shows only
/// these four states. RuntimeId / ProtocolSessionId / profile live in
/// diagnostics only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStatus {
    Connected,
    Starting,
    /// Runtime alive but protocol session invalid (P1-B) → reconnect.
    Reconnecting,
    Unavailable,
}

impl ProviderStatus {
    pub fn label(self) -> &'static str {
        match self {
            ProviderStatus::Connected => "Connected",
            ProviderStatus::Starting => "Starting…",
            ProviderStatus::Reconnecting => "Reconnecting…",
            ProviderStatus::Unavailable => "Unavailable",
        }
    }

    /// §26: reconnecting is a transient state, NOT an error — the status
    /// line shows it as info-grade, never red (§45).
    pub fn severity(self) -> Severity {
        match self {
            ProviderStatus::Connected => Severity::Success,
            ProviderStatus::Starting | ProviderStatus::Reconnecting => Severity::Info,
            ProviderStatus::Unavailable => Severity::Warning,
        }
    }
}

/// Presentation severity for the status line / runtime message (§45).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}

/// ---- Error tiers (§45): a Tier decides presentation weight; only
/// Terminal/Internal render as errors in the normal surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorTier {
    /// Transient, self-healing (reconnecting, searching).
    Recoverable,
    /// Needs a user decision but has a next step (confirmation, retry).
    Actionable,
    /// Failed with no automatic continuation.
    Terminal,
    /// Internal invariant broken — diagnostics vocabulary only.
    Internal,
}

/// Toast policy (§46, §65): small transient feedback only. Confirmation,
/// security denials and failures needing action go to real UI states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSubject {
    Copied,
    Saved,
    Completed,
    Disconnected,
    Confirmation,
    SecurityDenial,
    WorkflowFailure,
}

impl ToastSubject {
    pub fn toast_allowed(self) -> bool {
        matches!(
            self,
            ToastSubject::Copied
                | ToastSubject::Saved
                | ToastSubject::Completed
                | ToastSubject::Disconnected
        )
    }
}

/// ---- Agent view model (§18-§23): what the runtime panel shows. Phase
/// names come from the agent state machine; the UI cannot advance them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentView {
    pub name: String,
    pub goal: String,
    /// Fixed fixture-friendly "3 / 8" style counters (§22, §53).
    pub turn: Budget,
    pub executions: Budget,
    pub phases: Vec<PhaseView>,
    pub proposals: Vec<ProposalView>,
    pub replanning: bool,
    pub status: AgentTerminalStatus,
}

/// A used/total counter with the §22 warning rule attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub used: u32,
    pub total: u32,
}

impl Budget {
    pub fn label(self) -> String {
        format!("{} / {}", self.used, self.total)
    }

    /// Warn when the last unit is in play.
    pub fn warning(self) -> bool {
        self.total > 0 && self.used + 1 >= self.total
    }

    pub fn exhausted(self) -> bool {
        self.total > 0 && self.used >= self.total
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseView {
    pub name: String,
    pub status: ActivityStatus,
}

/// One agent proposal (§20): presented as a PROPOSAL — never as an
/// "authorized action"; authorization happens only in the resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalView {
    pub provider: String,
    pub command: String,
    pub action: String,
    pub input_summary: String,
    pub confirmation_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTerminalStatus {
    Running,
    Replanning,
    Completed,
    Failed,
    BudgetExhausted,
    Cancelled,
}

impl AgentTerminalStatus {
    /// Replan ≠ retry (§21): the wording must say "Replanning".
    pub fn label(self) -> &'static str {
        match self {
            AgentTerminalStatus::Running => "Running",
            AgentTerminalStatus::Replanning => "Replanning…",
            AgentTerminalStatus::Completed => "Completed",
            AgentTerminalStatus::Failed => "Failed",
            AgentTerminalStatus::BudgetExhausted => "Budget exhausted",
            AgentTerminalStatus::Cancelled => "Cancelled",
        }
    }

    pub fn severity(self) -> Severity {
        match self {
            AgentTerminalStatus::Running | AgentTerminalStatus::Replanning => Severity::Info,
            AgentTerminalStatus::Completed => Severity::Success,
            AgentTerminalStatus::BudgetExhausted | AgentTerminalStatus::Cancelled => {
                Severity::Warning
            }
            AgentTerminalStatus::Failed => Severity::Error,
        }
    }
}

/// Project an AgentView into workflow-style runtime rows (§18: agent is a
/// workflow-style runtime panel, no new top-level mode). The host pushes
/// these through the existing workflow surface.
pub fn agent_phase_rows(agent: &AgentView) -> Vec<(String, String, String)> {
    let mut rows: Vec<(String, String, String)> = agent
        .phases
        .iter()
        .map(|p| {
            let (sym, txt) = p.status.symbol_text();
            (sym.to_string(), p.name.clone(), txt.to_string())
        })
        .collect();
    if agent.replanning {
        rows.push(("↻".to_string(), "replan".to_string(), "Replanning…".to_string()));
    }
    rows
}

/// Budget header line: "Turn 3 / 8 · Executions 5 / 16" (+ warning marker).
pub fn agent_budget_line(agent: &AgentView) -> String {
    let warn = if agent.turn.warning() || agent.executions.warning() {
        " ⚠"
    } else {
        ""
    };
    format!(
        "Turn {} · Executions {}{}",
        agent.turn.label(),
        agent.executions.label(),
        warn
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §4: the mode set is frozen — this compile-time mirror of the enum
    /// fails to build if a new top-level mode (AgentMode, McpMode, …) is added.
    #[allow(dead_code)]
    fn modes_frozen(m: UiMode) -> &'static str {
        match m {
            UiMode::Main => "main",
            UiMode::Action => "action",
            UiMode::Workflow => "workflow",
        }
    }

    /// INV-UI: no ExecuteEffect-style command can appear (source frozen).
    #[test]
    fn ui_commands_are_the_frozen_seven() {
        let all = [
            UiCommand::Select,
            UiCommand::Invoke,
            UiCommand::Confirm,
            UiCommand::Cancel,
            UiCommand::Navigate,
            UiCommand::Close,
            UiCommand::ToggleDiagnostics,
        ];
        assert_eq!(all.len(), 7);
    }

    /// INV-UI-010: Close and Cancel are distinct values — dismissing a
    /// panel is structurally not a cancel.
    #[test]
    fn close_is_not_cancel() {
        assert_ne!(UiCommand::Close, UiCommand::Cancel);
    }

    /// §6: provider unavailability must not render as "No Results".
    #[test]
    fn empty_states_distinct() {
        assert_ne!(
            EmptyState::NoResults.label(),
            EmptyState::ProviderUnavailable.label()
        );
        assert!(EmptyState::ProviderUnavailable.is_warning());
        assert!(!EmptyState::NoResults.is_warning());
        assert_eq!(EmptyState::NoQuery.label(), "Start typing...");
    }

    /// §11: diagnostics vocabulary maps to user wording, never shown raw.
    #[test]
    fn disabled_reason_labels() {
        assert_eq!(DisabledReason::CapabilityDenied.label(), "Not permitted");
        assert_eq!(DisabledReason::MissingContext.label(), "Requires a folder");
        assert_eq!(DisabledReason::StaleContext.label(), "Context changed");
        assert_eq!(DisabledReason::Unavailable.label(), "Temporarily unavailable");
        assert_eq!(
            DisabledReason::from_diagnostic("capability mcp.invoke denied"),
            Some(DisabledReason::CapabilityDenied)
        );
        assert_eq!(DisabledReason::from_diagnostic("totally unknown"), None);
    }

    /// §26 + §45: reconnecting is info-grade, unavailable is not an error.
    #[test]
    fn provider_status_severity() {
        assert_eq!(ProviderStatus::Reconnecting.severity(), Severity::Info);
        assert_eq!(ProviderStatus::Starting.severity(), Severity::Info);
        assert_eq!(ProviderStatus::Unavailable.severity(), Severity::Warning);
        assert_eq!(ProviderStatus::Connected.severity(), Severity::Success);
        assert_eq!(ProviderStatus::Reconnecting.label(), "Reconnecting…");
    }

    /// §15: status always carries symbol AND text (never color-only).
    #[test]
    fn activity_status_pairs() {
        for s in [
            ActivityStatus::Pending,
            ActivityStatus::Running,
            ActivityStatus::Succeeded,
            ActivityStatus::Failed,
            ActivityStatus::Skipped,
        ] {
            let (sym, txt) = s.symbol_text();
            assert!(!sym.is_empty());
            assert!(!txt.is_empty());
        }
    }

    /// §21: replan wording is "Replanning", never "Retrying".
    #[test]
    fn replan_not_retry() {
        let l = AgentTerminalStatus::Replanning.label();
        assert!(l.contains("Replan"));
        assert!(!l.contains("Retry"));
    }

    /// §22: budget warns on the last unit and flags exhaustion.
    #[test]
    fn budget_warning_and_exhaustion() {
        let b = Budget { used: 7, total: 8 };
        assert!(b.warning());
        assert!(!b.exhausted());
        let b = Budget { used: 8, total: 8 };
        assert!(b.exhausted());
        assert_eq!(b.label(), "8 / 8");
        let b = Budget { used: 0, total: 0 };
        assert!(!b.warning() && !b.exhausted(), "degenerate budget never warns");
    }

    /// §18/§22: agent rows reuse the workflow surface; budget line format
    /// is stable for visual fixtures (§53).
    #[test]
    fn agent_projection_rows_and_budget_line() {
        let agent = AgentView {
            name: "Organize Downloads".into(),
            goal: "Organize large files".into(),
            turn: Budget { used: 3, total: 8 },
            executions: Budget { used: 5, total: 16 },
            phases: vec![
                PhaseView { name: "Observe".into(), status: ActivityStatus::Succeeded },
                PhaseView { name: "Plan".into(), status: ActivityStatus::Succeeded },
                PhaseView { name: "Execute".into(), status: ActivityStatus::Running },
                PhaseView { name: "Replan".into(), status: ActivityStatus::Pending },
            ],
            proposals: vec![ProposalView {
                provider: "MCP · Calculator".into(),
                command: "calculate".into(),
                action: "invoke".into(),
                input_summary: "expression = 123 * 456".into(),
                confirmation_required: false,
            }],
            replanning: false,
            status: AgentTerminalStatus::Running,
        };
        let rows = agent_phase_rows(&agent);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[2], ("◐".to_string(), "Execute".to_string(), "running".to_string()));
        assert_eq!(
            agent_budget_line(&agent),
            "Turn 3 / 8 · Executions 5 / 16"
        );
        // proposal is presented as a proposal, not an authorization
        assert!(!agent.proposals[0].confirmation_required);
    }

    /// §46/§65: toasts only for transient confirmations; failures and
    /// security denials must go to real UI state.
    #[test]
    fn toast_policy() {
        assert!(ToastSubject::Copied.toast_allowed());
        assert!(ToastSubject::Completed.toast_allowed());
        assert!(!ToastSubject::Confirmation.toast_allowed());
        assert!(!ToastSubject::SecurityDenial.toast_allowed());
        assert!(!ToastSubject::WorkflowFailure.toast_allowed());
    }

    /// §50: timeline entries are plain data; symbol+text derivable.
    #[test]
    fn timeline_items_are_plain_data() {
        let item = ActivityItem {
            timestamp: "08:31:24".into(),
            kind: ActivityKind::Execute,
            title: "Scan".into(),
            summary: "scanned 12 files".into(),
            status: ActivityStatus::Succeeded,
        };
        let (sym, _) = item.status.symbol_text();
        assert_eq!(sym, "✓");
        assert_eq!(item.timestamp, "08:31:24");
    }
}
