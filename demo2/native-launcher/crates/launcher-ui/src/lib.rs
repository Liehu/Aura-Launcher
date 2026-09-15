//! Slint-based presentation layer for the launcher.
//!
//! The UI crate owns no business logic: it renders result models coming from
//! the core and forwards user input events upward.

slint::include_modules!();

use launcher_domain::StepRunStatus;

pub mod ui_state;
pub mod viewmodel;

/// Convert a domain command list into UI result items (bounded by `limit`).
pub fn to_result_items(
    items: impl IntoIterator<Item = launcher_domain::Command>,
    limit: usize,
) -> Vec<ResultItem> {
    to_result_items_with_query(items, limit, "")
}

/// P3-UX: to_result_items with match highlighting. Finds the first
/// case-insensitive occurrence of `query` in the title and sets
/// match_start/match_len for the Slint renderer to highlight.
pub fn to_result_items_with_query(
    items: impl IntoIterator<Item = launcher_domain::Command>,
    limit: usize,
    query: &str,
) -> Vec<ResultItem> {
    let q_lower = query.to_lowercase();
    let q_empty = q_lower.is_empty();
    items
        .into_iter()
        .take(limit)
        .map(|c| {
            // P3-UX: pre-split title into before/match/after for Slint
            let title_lower = c.title.to_lowercase();
            let (t_before, t_match, t_after) = if q_empty {
                (String::new(), String::new(), c.title.clone())
            } else {
                match title_lower.find(&q_lower) {
                    Some(byte_pos) => {
                        let chars: Vec<char> = c.title.chars().collect();
                        let mut char_idx = 0;
                        let mut before = String::new();
                        let mut matched = String::new();
                        for (ci, ch) in chars.iter().enumerate() {
                            if char_idx >= byte_pos && char_idx < byte_pos + q_lower.len() {
                                matched.push(*ch);
                            } else {
                                before.push(*ch);
                            }
                            char_idx += ch.len_utf8();
                            let _ = ci;
                        }
                        let after = if before.len() + matched.len() < c.title.len() {
                            c.title[before.len() + matched.len()..].to_string()
                        } else {
                            String::new()
                        };
                        (before, matched, after)
                    }
                    None => (String::new(), String::new(), c.title.clone()),
                }
            };
            let _has_match = !t_match.is_empty();
            ResultItem {
                command_id: c.id.into(),
                title: c.title.clone().into(),
                title_before: t_before.into(),
                title_match: t_match.into(),
                title_after: t_after.into(),
                subtitle: c.subtitle.unwrap_or_default().into(),
                icon: c.icon.unwrap_or_default().into(),
                score: format!("{:.2}", c.score).into(),
                icon_data: Default::default(),
                is_header: false,
                header_label: "".into(),
            }
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

// ---- P3-D: Result Grouping + focus model (spec §8) -------------------------

/// Group header labels in their FIXED presentation order (spec §8:
/// Applications → Commands → Files → Folders → Plugins → Web → Other; Web
/// has no Category equivalent in v1, so such results land in Other).
pub const RESULT_GROUP_ORDER: [&str; 6] = [
    "Applications",
    "Commands",
    "Files",
    "Folders",
    "Plugins",
    "Other",
];

fn group_label(c: &launcher_domain::Command) -> &'static str {
    match c.category {
        launcher_domain::Category::Application => "Applications",
        launcher_domain::Category::Command => "Commands",
        launcher_domain::Category::File => "Files",
        launcher_domain::Category::Folder => "Folders",
        launcher_domain::Category::Plugin => "Plugins",
    }
}

/// One contiguous, non-empty result section. FOCUS MODEL CONTRACT (P3-D):
/// sections are a *presentation* projection over the flat, ranked result
/// list — the keyboard cursor indexes RESULTS ONLY. Headers derived from
/// these sections are `focusable = false`; traversal A1→A2→B1→B2 is
/// guaranteed by construction because `start/len` never interleave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultSection {
    pub label: &'static str,
    /// Index of the first result in the flat ranked list.
    pub start: usize,
    /// Number of results (>= 1; empty groups are never emitted).
    pub len: usize,
}

impl ResultSection {
    pub fn end(&self) -> usize {
        self.start + self.len
    }
}

/// Project a ranked result list into contiguous group sections (spec §8):
/// runs are cut IN RANKED ORDER (grouping must never re-order results —
/// P3-D reject condition), empty groups never appear, and when all results
/// fall in a SINGLE group the header is suppressed (empty vec — "只有一
/// 个 group 时可隐藏 header"). The flat cursor space is unchanged.
pub fn group_sections(commands: &[launcher_domain::Command]) -> Vec<ResultSection> {
    let mut sections: Vec<ResultSection> = Vec::new();
    let mut i = 0;
    while i < commands.len() {
        let label = group_label(&commands[i]);
        let start = i;
        while i < commands.len() && group_label(&commands[i]) == label {
            i += 1;
        }
        sections.push(ResultSection {
            label,
            start,
            len: i - start,
        });
    }
    if sections.len() <= 1 {
        return Vec::new();
    }
    sections
}

/// Keyboard traversal contract core (P3-D test target): the cursor only
/// moves within [0, len) of the RESULT space — headers are never in that
/// space, so `cursor_step` needs no header-awareness by construction.
pub fn cursor_step(len: usize, current: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    let cur = current as i32 + delta;
    cur.clamp(0, len as i32 - 1) as usize
}

/// P3-I: build the interleaved DISPLAY model (headers + result rows) from
/// a ranked command list, with match highlighting. Returns the slint rows
/// and the row→result map used by navigation.
pub fn to_result_rows_with_query(
    items: impl IntoIterator<Item = launcher_domain::Command>,
    limit: usize,
    query: &str,
) -> (Vec<ResultItem>, Vec<Option<usize>>) {
    let commands: Vec<launcher_domain::Command> = items.into_iter().collect();
    let (list_rows, row_map) = result_rows(&commands, limit);
    let items = to_result_items_with_query(commands, limit, query);
    let mut rows: Vec<ResultItem> = Vec::with_capacity(list_rows.len());
    for r in &list_rows {
        match r {
            ResultListRow::Header(label) => rows.push(ResultItem {
                command_id: "".into(),
                title: "".into(),
                title_before: "".into(),
                title_match: "".into(),
                title_after: "".into(),
                subtitle: "".into(),
                icon: "".into(),
                score: "".into(),
                icon_data: Default::default(),
                is_header: true,
                header_label: (*label).into(),
            }),
            ResultListRow::Result(idx) => {
                rows.push(items[*idx].clone());
            }
        }
    }
    (rows, row_map)
}

/// One row of the DISPLAY model (P3-I): either a non-selectable section
/// header or a result row. Headers are a distinct visual component — never
/// a disabled ResultRow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultListRow {
    Header(&'static str),
    Result(usize),
}

/// Build the interleaved display model over a ranked result list:
/// group_sections projects the headers, results keep their ranked order.
/// Returns the rows plus `row_map` — `row_map[row] = Some(result_idx)` for
/// result rows, `None` for headers. The keyboard cursor lives in RESULT
/// space; `result_nav` translates it into row space for rendering.
pub fn result_rows(
    commands: &[launcher_domain::Command],
    limit: usize,
) -> (Vec<ResultListRow>, Vec<Option<usize>>) {
    let sections = group_sections(commands);
    let capped = commands.len().min(limit);
    let mut rows = Vec::new();
    let mut row_map = Vec::new();
    let mut result_idx = 0usize;
    let mut section_at = 0usize;
    while result_idx < capped {
        // emit the header when entering a new section
        if let Some(sec) = sections.get(section_at) {
            if sec.start == result_idx {
                rows.push(ResultListRow::Header(sec.label));
                row_map.push(None);
                section_at += 1;
            }
        }
        rows.push(ResultListRow::Result(result_idx));
        row_map.push(Some(result_idx));
        result_idx += 1;
    }
    (rows, row_map)
}

/// P3-D/P3-I navigation: move the RESULT cursor by `delta`, then translate
/// to the display row index. Never lands on a header (they are not in the
/// cursor space). Returns (result_idx, row_idx).
pub fn result_nav(
    row_map: &[Option<usize>],
    result_count: usize,
    current_result: usize,
    delta: i32,
) -> (usize, usize) {
    let next = cursor_step(result_count, current_result, delta);
    let row = row_map
        .iter()
        .position(|r| *r == Some(next))
        .unwrap_or(row_map.len().saturating_sub(1));
    (next, row)
}

/// Row index of a result index (Home/End/selection restore).
pub fn row_of_result(row_map: &[Option<usize>], result_idx: usize) -> usize {
    row_map
        .iter()
        .position(|r| *r == Some(result_idx))
        .unwrap_or(0)
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

    // ---- P3-D: grouping + focus model ----

    fn result(id: &str, category: Category) -> Command {
        Command {
            id: id.into(),
            title: id.into(),
            subtitle: None,
            icon: None,
            provider_id: "test".into(),
            score: 0.0,
            keywords: vec![],
            category,
            actions: vec![],
            target: None,
        }
    }

    /// P3-D focus model: headers are a projection — the flat cursor walk
    /// over ranked results yields A1→A2→B1→B2 (never a header index).
    #[test]
    fn p3d_cursor_walk_skips_headers() {
        let ranked = vec![
            result("A1", Category::Application),
            result("A2", Category::Application),
            result("B1", Category::Plugin),
            result("B2", Category::Plugin),
        ];
        let sections = group_sections(&ranked);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].label, "Applications");
        assert_eq!(sections[0].start, 0);
        assert_eq!(sections[0].len, 2);
        assert_eq!(sections[1].label, "Plugins");
        assert_eq!(sections[1].start, 2);
        // flat cursor walk = results only, in ranked order
        let mut idx = 0usize;
        let mut walk: Vec<&str> = Vec::new();
        for _ in 0..ranked.len() {
            walk.push(ranked[idx].id.as_str());
            idx = cursor_step(ranked.len(), idx, 1);
        }
        assert_eq!(walk, ["A1", "A2", "B1", "B2"]);
    }

    /// P3-D: single group → headers suppressed (empty vec).
    #[test]
    fn p3d_single_group_hides_header() {
        let one = vec![
            result("a", Category::Application),
            result("b", Category::Application),
        ];
        assert!(group_sections(&one).is_empty());
        assert!(group_sections(&[]).is_empty());
    }

    /// P3-D reject condition: grouping NEVER re-orders ranked results —
    /// interleaved categories stay in ranked order, cut into runs.
    #[test]
    fn p3d_grouping_preserves_ranked_order() {
        let ranked = vec![
            result("app1", Category::Application),
            result("file1", Category::File),
            result("app2", Category::Application),
        ];
        let sections = group_sections(&ranked);
        let covered: Vec<usize> = sections
            .iter()
            .flat_map(|s| s.start..s.end())
            .collect();
        assert_eq!(covered, (0..3).collect::<Vec<_>>(), "runs tile the list");
        assert_eq!(ranked[covered[0]].id, "app1");
        assert_eq!(ranked[covered[1]].id, "file1");
        assert_eq!(ranked[covered[2]].id, "app2");
    }

    // ---- P3-I: interleaved row model + result-space navigation ----

    fn p3i_row(id: &str, category: Category) -> Command {
        result(id, category)
    }

    /// Headers interleave at section starts; every result keeps its row.
    #[test]
    fn p3i_rows_interleave_headers_and_results() {
        let ranked = vec![
            p3i_row("A1", Category::Application),
            p3i_row("A2", Category::Application),
            p3i_row("B1", Category::Plugin),
            p3i_row("B2", Category::Plugin),
        ];
        let (rows, map) = result_rows(&ranked, 50);
        assert_eq!(rows.len(), 6, "2 headers + 4 results");
        assert_eq!(map.len(), 6);
        assert_eq!(rows[0], ResultListRow::Header("Applications"));
        assert_eq!(rows[1], ResultListRow::Result(0));
        assert_eq!(rows[2], ResultListRow::Result(1));
        assert_eq!(rows[3], ResultListRow::Header("Plugins"));
        assert_eq!(rows[4], ResultListRow::Result(2));
        assert_eq!(rows[5], ResultListRow::Result(3));
    }

    /// THE focus-model guarantee, end to end: navigating by rows via
    /// result_nav lands ONLY on result rows (A1→A2→B1→B2), never headers.
    #[test]
    fn p3i_nav_skips_headers() {
        let ranked = vec![
            p3i_row("A1", Category::Application),
            p3i_row("A2", Category::Application),
            p3i_row("B1", Category::Plugin),
            p3i_row("B2", Category::Plugin),
        ];
        let (rows, map) = result_rows(&ranked, 50);
        let mut idx = 0usize;
        let mut visited = Vec::new();
        for _ in 0..ranked.len() {
            visited.push(match rows[row_of_result(&map, idx)] {
                ResultListRow::Result(i) => ranked[i].id.clone(),
                ResultListRow::Header(_) => "<HEADER>".to_string(),
            });
            let (next, _row) = result_nav(&map, ranked.len(), idx, 1);
            idx = next;
        }
        assert_eq!(visited, ["A1", "A2", "B1", "B2"]);
    }

    /// Limit truncation applies to RESULTS; a header never dangles.
    #[test]
    fn p3i_row_limit_respects_results() {
        let ranked = vec![
            p3i_row("A1", Category::Application),
            p3i_row("B1", Category::Plugin),
            p3i_row("B2", Category::Plugin),
        ];
        let (rows, map) = result_rows(&ranked, 2);
        assert_eq!(map.iter().filter(|r| r.is_some()).count(), 2);
        assert_eq!(rows.len(), 4, "header+result per section (limit = results)");
    }

    /// cursor_step is bounded and deterministic (P3-D: no ambiguity).
    #[test]
    fn p3d_cursor_step_bounds() {
        assert_eq!(cursor_step(4, 0, 1), 1);
        assert_eq!(cursor_step(4, 3, 1), 3, "End-clamped");
        assert_eq!(cursor_step(4, 0, -1), 0);
        assert_eq!(cursor_step(4, 0, 99), 3);
        assert_eq!(cursor_step(0, 0, 1), 0);
    }

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
                p.provider,
                p.command,
                p.input_summary,
                if p.confirmation_required {
                    " · confirmation required"
                } else {
                    ""
                }
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
