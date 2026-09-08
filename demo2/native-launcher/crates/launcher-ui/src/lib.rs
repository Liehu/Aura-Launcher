//! Slint-based presentation layer for the launcher.
//!
//! The UI crate owns no business logic: it renders result models coming from
//! the core and forwards user input events upward.

slint::include_modules!();

use launcher_domain::StepRunStatus;

pub mod viewmodel;

/// Convert a domain command list into UI result items (bounded by `limit`).
pub fn to_result_items(
    items: impl IntoIterator<Item = launcher_domain::Command>,
    limit: usize,
) -> Vec<ResultItem> {
    items
        .into_iter()
        .take(limit)
        .map(|c| ResultItem {
            command_id: c.id.into(),
            title: c.title.into(),
            subtitle: c.subtitle.unwrap_or_default().into(),
            icon: c.icon.unwrap_or_default().into(),
            score: format!("{:.2}", c.score).into(),
            // P2.1-E5: filled asynchronously by the host icon pipeline;
            // empty = 20px placeholder slot (no layout reflow)
            icon_data: Default::default(),
        })
        .collect()
}

/// One presentable action of the selected command (MVP3.1 ActionPresentation,
/// review 18 section 3). The UI never sees ActionDescriptor/ActionKind or
/// capabilities (INV-036): only id/title/enabled plus an optional reason.
pub struct ActionPresentation {
    pub action_id: String,
    pub title: String,
    pub enabled: bool,
    pub reason: String,
    pub shortcut: String,
    /// First Ready action in declaration order (primary = Enter target).
    /// Computed here so the UI never derives it positionally (review 29 §9).
    pub is_primary: bool,
}

/// Project a command's resolved actions into presentation items, preserving
/// declaration order. Disabled (capability-denied) actions stay visible and
/// non-selectable; hidden ones were already dropped by the host (INV-031).
pub fn to_action_presentations(cmd: &launcher_domain::Command) -> Vec<ActionPresentation> {
    let primary_id = cmd.primary_action().and_then(|a| a.id.clone());
    cmd.actions
        .iter()
        .map(|a| {
            let action_id = a.id.clone().unwrap_or_default();
            ActionPresentation {
                is_primary: primary_id.as_deref() == Some(action_id.as_str())
                    && !action_id.is_empty(),
                action_id,
                // legacy string actions carry no title: fall back to the payload
                // text (e.g. "echo"), then the kind name as last resort
                title: a.title.clone().unwrap_or_else(|| match a.payload.as_ref() {
                    Some(launcher_domain::ActionPayload::Text(t))
                    | Some(launcher_domain::ActionPayload::CommandLine(t))
                    | Some(launcher_domain::ActionPayload::Path(t)) => t.clone(),
                    _ => format!("{:?}", a.kind),
                }),
                enabled: a.disabled_reason.is_none(),
                reason: a.disabled_reason.clone().unwrap_or_default(),
                shortcut: a.shortcut.clone().unwrap_or_default(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, ActionPayload, Category, Command};

    fn cmd(actions: Vec<Action>) -> Command {
        Command {
            id: "calc.plus:= 80".into(),
            title: "= 80".into(),
            subtitle: None,
            icon: None,
            provider_id: "plugin:calc.plus".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Plugin,
            actions,
            target: None,
        }
    }

    /// MVP3.1 presentation projection (review 18 section 3): the UI sees only
    /// id/title/enabled/reason, in declaration order, disabled kept visible.
    #[test]
    fn action_presentation_projection() {
        let actions = vec![
            Action {
                kind: ActionKind::Copy,
                payload: Some(ActionPayload::Text("80".into())),
                id: Some("copy".into()),
                title: Some("Copy".into()),
                disabled_reason: None,
                shortcut: Some("Ctrl+Shift+C".into()),
                confirmation_required: false,
            },
            Action {
                kind: ActionKind::Execute,
                payload: None,
                id: Some("magic".into()),
                title: Some("Magic".into()),
                disabled_reason: Some("requires clipboard.write".into()),
                shortcut: None,
                confirmation_required: false,
            },
        ];
        let items = to_action_presentations(&cmd(actions));
        assert_eq!(items.len(), 2);
        assert!(items[0].enabled);
        assert!(items[0].is_primary, "first Ready action is primary");
        assert!(!items[1].is_primary, "disabled action is never primary");
        assert_eq!(items[0].action_id, "copy");
        assert_eq!(items[0].title, "Copy");
        assert!(!items[1].enabled);
        assert!(items[1].reason.contains("clipboard.write"));
        assert_eq!(items[0].shortcut, "Ctrl+Shift+C");
    }
}

// ---------- Workflow Runtime Surface (MVP4.1 UI, UI-CONTRACT §13) ----------

/// One presentable workflow step (UI-CONTRACT §4.3): symbol + state text are
/// derived host-side so the UI never interprets runtime state (INV-062).
pub struct WorkflowStepView {
    pub step_id: String,
    pub title: String,
    pub symbol: String,
    pub state_text: String,
    pub error: Option<String>,
    pub current: bool,
    /// Level 3 diagnostics (Spec section 21): execution attempt identity and
    /// classified failure, rendered only when diagnostics are requested.
    pub execution_id: String,
    pub failure_class: String,
}

/// Presentable run header (UI-CONTRACT §4.4).
pub struct WorkflowRunView {
    pub run_id: String,
    pub name: String,
    pub status_symbol: String,
    pub status_text: String,
    pub progress: String,
    pub steps: Vec<WorkflowStepView>,
    /// Level 2 (UI-CONTRACT §21): the failing step, when any
    pub failed_detail: Option<FailedStepDetail>,
    /// Level 3 (UI-CONTRACT §21): monospace diagnostic lines
    pub diagnostic_lines: Vec<String>,
}

/// Level 2 failing-step detail (host presentation).
#[derive(Debug, Clone)]
pub struct FailedStepDetail {
    pub title: String,
    pub reason: String,
    pub attempt: u32,
}

/// Map a domain StepRun to its presentation (symbol vocabulary frozen in
/// UI-CONTRACT §15: ✓ complete / ● current / ○ pending / ⚠ failed / ⏸ paused).
/// Derive the classified failure name for Level 3 diagnostics from the
/// user-readable error (keyword mapping documented in UI-CONTRACT section 15).
fn classify_failure(error: Option<&String>) -> String {
    let Some(e) = error else { return String::new() };
    let e = e.to_lowercase();
    if e.contains("timeout") {
        "Timeout".into()
    } else if e.contains("unavailable") || e.contains("spawn") {
        "PluginUnavailable".into()
    } else if e.contains("denied") {
        "CapabilityDenied".into()
    } else if e.contains("protocol") || e.contains("malformed") || e.contains("crashed") {
        "ProtocolViolation".into()
    } else {
        "EffectFailed".into()
    }
}

fn step_view(
    step_id: &str,
    status: StepRunStatus,
    error: Option<&String>,
    current: bool,
) -> WorkflowStepView {
    let (symbol, state_text) = match status {
        StepRunStatus::Pending => ("○".to_string(), "pending".to_string()),
        StepRunStatus::Resolving | StepRunStatus::Executing => {
            ("●".to_string(), "running".to_string())
        }
        StepRunStatus::Resolved => ("●".to_string(), "resolved".to_string()),
        StepRunStatus::Complete => ("✓".to_string(), "complete".to_string()),
        StepRunStatus::Failed => ("⚠".to_string(), "failed".to_string()),
        StepRunStatus::Skipped => ("⊘".to_string(), "skipped".to_string()),
        StepRunStatus::WaitingForConfirmation => {
            ("⏸".to_string(), "waiting for your confirmation".to_string())
        }
    };
    WorkflowStepView {
        step_id: step_id.to_string(),
        title: step_id.to_string(),
        symbol,
        state_text,
        error: error.cloned(),
        current,
        execution_id: String::new(),
        failure_class: String::new(),
    }
}

/// Project a domain WorkflowRun + definition name into the presentation model.
/// Failure presentation follows the classified state: errors come from
/// StepRun.last_error (already user-readable strings from the runner).
pub fn to_workflow_run_view(run: &launcher_domain::WorkflowRun, name: &str) -> WorkflowRunView {
    let current_step = run.current_step.clone();
    let (status_symbol, status_text) = match run.status {
        launcher_domain::WorkflowRunStatus::Created
        | launcher_domain::WorkflowRunStatus::Running => ("⌛".to_string(), "Running".to_string()),
        launcher_domain::WorkflowRunStatus::Paused => (
            "⏸".to_string(),
            run.paused_reason.as_deref().unwrap_or("Paused").to_string(),
        ),
        launcher_domain::WorkflowRunStatus::Succeeded => ("✓".to_string(), "Completed".to_string()),
        launcher_domain::WorkflowRunStatus::Failed => ("⚠".to_string(), "Failed".to_string()),
        launcher_domain::WorkflowRunStatus::Cancelled => ("⊘".to_string(), "Cancelled".to_string()),
    };
    let completed = run
        .steps
        .iter()
        .filter(|s| s.status == StepRunStatus::Complete)
        .count();
    let steps: Vec<WorkflowStepView> = run
        .steps
        .iter()
        .map(|s| {
            let mut v = step_view(
                &s.step_id,
                s.status,
                s.last_error.as_ref(),
                Some(s.step_id.as_str()) == current_step.as_deref(),
            );
            v.execution_id = s.last_execution_id.clone().unwrap_or_default();
            v.failure_class = classify_failure(s.last_error.as_ref());
            v
        })
        .collect();
    // Level 2: first Failed step becomes the failure detail
    // Level 2: first Failed step becomes the failure detail
    let failed_detail = run.steps.iter().zip(steps.iter()).find_map(|(dom, view)| {
        (dom.status == StepRunStatus::Failed).then(|| FailedStepDetail {
            title: view.title.clone(),
            reason: view.error.clone().unwrap_or_default(),
            attempt: dom_attempt_of(run, &dom.step_id),
        })
    });
    let diagnostic_lines: Vec<String> = steps
        .iter()
        .filter(|s| !s.execution_id.is_empty() || !s.failure_class.is_empty())
        .map(|s| {
            format!(
                "⚙ {} · exec={} · class={}",
                s.step_id, s.execution_id, s.failure_class
            )
        })
        .collect();
    WorkflowRunView {
        run_id: run.workflow_run_id.clone(),
        name: name.to_string(),
        status_symbol,
        status_text,
        progress: format!("{} / {}", completed, run.steps.len()),
        steps,
        failed_detail,
        diagnostic_lines,
    }
}

fn dom_attempt_of(run: &launcher_domain::WorkflowRun, step_id: &str) -> u32 {
    run.steps
        .iter()
        .find(|s| s.step_id == step_id)
        .map(|s| s.attempt)
        .unwrap_or(1)
}

/// Push a workflow run view into the runtime surface (UI-CONTRACT §13).
pub fn show_workflow_run(ui: &AppWindow, view: &WorkflowRunView) {
    let items: Vec<WorkflowItem> = view
        .steps
        .iter()
        .map(|s| WorkflowItem {
            step_id: s.step_id.clone().into(),
            symbol: s.symbol.clone().into(),
            title: s.title.clone().into(),
            state_text: s.state_text.clone().into(),
            error: s.error.clone().unwrap_or_default().into(),
            current: s.current,
            execution_id: s.execution_id.clone().into(),
            failure_class: s.failure_class.clone().into(),
        })
        .collect();
    ui.set_wf_items(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(items),
    )));
    ui.set_wf_title(view.name.clone().into());
    ui.set_wf_status(format!("{} {}", view.status_symbol, view.status_text).into());
    ui.set_wf_visible(true);
}

pub fn hide_workflow_run(ui: &AppWindow) {
    ui.set_wf_visible(false);
}

// ---------- Agent Runtime Surface (P1-E.5, review 60 §18-§23) ----------
// The agent renders through the SAME workflow runtime panel (§18: agent is
// a workflow-style runtime panel, never a new top-level mode) — the host
// projects the viewmodel::AgentView into WorkflowRunView here.

/// Build the workflow-style run view for an agent. Phase rows use the
/// frozen symbol vocabulary; proposals appear as Level-3 monospace lines
/// labeled "proposed" (§20: a proposal is not an authorized action).
pub fn agent_run_view(agent: &viewmodel::AgentView) -> WorkflowRunView {
    let (symbol, text) = match agent.status {
        viewmodel::AgentTerminalStatus::Running => ("⌛", "Running"),
        viewmodel::AgentTerminalStatus::Replanning => ("↻", "Replanning"),
        viewmodel::AgentTerminalStatus::Completed => ("✓", "Completed"),
        viewmodel::AgentTerminalStatus::Failed => ("⚠", "Failed"),
        viewmodel::AgentTerminalStatus::BudgetExhausted => ("⏸", "Budget exhausted"),
        viewmodel::AgentTerminalStatus::Cancelled => ("⊘", "Cancelled"),
    };
    let steps: Vec<WorkflowStepView> = viewmodel::agent_phase_rows(agent)
        .into_iter()
        .map(|(symbol, title, state_text)| WorkflowStepView {
            step_id: title.clone(),
            title,
            symbol,
            state_text,
            error: None,
            current: false,
            execution_id: String::new(),
            failure_class: String::new(),
        })
        .collect();
    // §19: plan/proposal/result summary lines — never chain-of-thought.
    let mut diagnostic_lines: Vec<String> = agent
        .proposals
        .iter()
        .map(|p| {
            format!(
                "⚙ proposed {} · {} · {}{}",
                p.provider, p.command, p.input_summary,
                if p.confirmation_required { " · confirmation required" } else { "" }
            )
        })
        .collect();
    if agent.replanning {
        diagnostic_lines.push("⚙ execution failed · replanning (not retrying)".into());
    }
    WorkflowRunView {
        run_id: "agent".into(),
        name: agent.name.clone(),
        status_symbol: symbol.into(),
        status_text: text.into(),
        progress: viewmodel::agent_budget_line(agent),
        steps,
        failed_detail: None,
        diagnostic_lines,
    }
}

/// Push an agent run into the shared workflow runtime surface. The §22
/// budget line rides in the status row ("⌛ Running · Turn 3 / 8 · …").
pub fn show_agent_run(ui: &AppWindow, agent: &viewmodel::AgentView) {
    let view = agent_run_view(agent);
    show_workflow_run(ui, &view);
    ui.set_wf_status(
        format!(
            "{} {} · {}",
            view.status_symbol, view.status_text, view.progress
        )
        .into(),
    );
}
