//! In-app workflow trigger chain (MVP4.1 integration closeout, review 32).
//!
//! Minimal trigger model: a trigger source (tray menu today; hotkey/plugin/
//! AI/schedule later) requests a run; **run creation and execution belong to
//! this service + WorkflowRunner**, never to the UI. The UI is a pure
//! Runtime Surface: it receives `WorkflowRunView` updates and sends back
//! confirm/dismiss events.
//!
//! Lifecycle: trigger → WorkflowDefinition → WorkflowRun → runner (with live
//! per-step progress pushed to the surface) → Paused(confirmation) waits on
//! a channel → confirm resumes with re-resolution → final status push.

use std::sync::atomic::AtomicU64;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use launcher_domain::{
    Action, ActionKind, ActionPayload, WorkflowDefinition, WorkflowRun, WorkflowRunStatus,
};
use launcher_ui::WorkflowRunView;
use slint::Weak;

use crate::AppState;

/// Confirmed-channel for the currently displayed paused run (WF-010: resume
/// only from the originating UI session; one active run at a time in v0.1).
static CONFIRM_TX: std::sync::OnceLock<Mutex<Option<Sender<()>>>> = std::sync::OnceLock::new();
static RUN_SEQ: AtomicU64 = AtomicU64::new(0);

fn set_confirm_tx(tx: Sender<()>) {
    let cell = CONFIRM_TX.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = cell.lock() {
        *slot = Some(tx);
    }
}

/// Called by the UI confirm event; resumes the paused run if one is waiting.
pub fn confirm_active_run() {
    if let Some(cell) = CONFIRM_TX.get() {
        if let Ok(slot) = cell.lock() {
            if let Some(tx) = slot.as_ref() {
                let _ = tx.send(());
            }
        }
    }
}

/// Adapter: drives engine + broker against app state while streaming live
/// step progress to the runtime surface.
struct AppStateBackend {
    state: Arc<Mutex<AppState>>,
    ui: Weak<launcher_ui::AppWindow>,
    run_id: String,
    name: String,
    steps: Vec<launcher_domain::StepRun>,
}

impl AppStateBackend {
    fn push_progress(&self, current_idx: usize) {
        let view = self.view(current_idx);
        if let Some(ui) = self.ui.upgrade() {
            launcher_ui::show_workflow_run(&ui, &view);
        }
    }

    fn view(&self, current_idx: usize) -> WorkflowRunView {
        let steps = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let status = if i < current_idx {
                    launcher_domain::StepRunStatus::Complete
                } else if i == current_idx {
                    launcher_domain::StepRunStatus::Executing
                } else {
                    launcher_domain::StepRunStatus::Pending
                };
                launcher_domain::StepRun {
                    step_id: s.step_id.clone(),
                    status,
                    attempt: 1,
                    last_error: None,
                    last_execution_id: None,
                    resolved_context_generation: None,
                    skip_reason: None,
                    output: None,
                }
            })
            .collect();
        let run = WorkflowRun {
            workflow_run_id: self.run_id.clone(),
            definition_id: "demo".into(),
            definition_version: 1,
            status: launcher_domain::WorkflowRunStatus::Running,
            paused_reason: None,
            current_step: self.steps.get(current_idx).map(|s| s.step_id.clone()),
            steps,
            variables: None,
        };
        launcher_ui::to_workflow_run_view(&run, &self.name)
    }
}

impl launcher_workflow::CommandSource for AppStateBackend {
    fn in_session(&self, provider_id: &str, command_id: &str) -> Option<launcher_domain::Command> {
        self.state
            .lock()
            .ok()?
            .current_results
            .iter()
            .find(|c| c.provider_id == provider_id && c.id == command_id)
            .cloned()
    }

    fn fresh_query(
        &mut self,
        _provider_id: &str,
    ) -> Result<
        Vec<launcher_domain::Command>,
        (launcher_domain::workflow::WorkflowFailureClass, String),
    > {
        // v0.1 demo workflows are Inline-only; external discovery is
        // DISCOVERY-TODO-001
        Ok(Vec::new())
    }
}

impl launcher_workflow::ActionExecutor for AppStateBackend {
    fn execute(
        &mut self,
        action: Action,
        provider_id: Option<String>,
        generation: u64,
        _confirmed: bool,
    ) -> Result<launcher_workflow::ExecutionOutcome, launcher_workflow::WfFailure> {
        let fail = |c: launcher_domain::workflow::WorkflowFailureClass, r: String| {
            launcher_workflow::WfFailure::new(c, r)
        };
        if action.kind == ActionKind::PluginInvoke {
            let plugin_id = provider_id
                .as_deref()
                .and_then(|p| p.strip_prefix("plugin:").map(str::to_string))
                .ok_or_else(|| {
                    fail(
                        launcher_domain::workflow::WorkflowFailureClass::ProtocolViolation,
                        "plugin action without provider identity".into(),
                    )
                })?;
            let input = match action.payload.as_ref() {
                Some(launcher_domain::ActionPayload::Json(v)) => v.clone(),
                _ => serde_json::Value::Null,
            };
            let action_id = action.id.clone().unwrap_or_default();
            // P1-FIX-01: mint the attempt's execution_id here (Core is the
            // single mint point) and carry it through the whole RPC chain.
            let outcome = self.state.lock().ok().and_then(|mut st| {
                let execution_id = st.core.next_execution_id();
                let res = st.core.execute_plugin_action(
                    &plugin_id,
                    &action_id,
                    &input,
                    &execution_id,
                    generation,
                );
                res.ok().map(|r| (r, execution_id))
            });
            return match outcome {
                Some((result, execution_id)) => Ok(launcher_workflow::ExecutionOutcome {
                    execution_id: Some(execution_id),
                    plugin_result: Some(result),
                }),
                None => Err(fail(
                    launcher_domain::workflow::WorkflowFailureClass::BusinessError,
                    "plugin action failed".into(),
                )),
            };
        }
        match launcher_action::execute(&action) {
            Ok(_) => Ok(launcher_workflow::ExecutionOutcome {
                execution_id: None,
                plugin_result: None,
            }),
            Err(e) => Err(fail(
                launcher_domain::workflow::WorkflowFailureClass::InvalidInput,
                e.to_string(),
            )),
        }
    }
}

/// Start a workflow run: creates the WorkflowRun (Core/service-owned, not
/// UI), spawns the runner on a background thread, and streams live progress
/// to the runtime surface. Confirmation pauses stream a confirm prompt; the
/// originating UI session resumes via `confirm_active_run()`.
pub fn start_workflow(
    state: Arc<Mutex<AppState>>,
    ui: Weak<launcher_ui::AppWindow>,
    def: WorkflowDefinition,
) {
    // review 68 §15: installed workflows are USER input — an invalid
    // definition must degrade to a log + no-op, never a panic.
    if let Err(e) = def.validate() {
        tracing::error!(error = %e, "workflow definition invalid; run aborted");
        return;
    }
    let n = RUN_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let run_id = format!("wr-{n}");
    let name = def.name.clone();
    let step_ids: Vec<String> = def.steps.iter().map(|s| s.step_id.clone()).collect();
    std::thread::Builder::new()
        .name(format!("workflow-{run_id}"))
        .spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            set_confirm_tx(tx);
            let backend = AppStateBackend {
                state: state.clone(),
                ui: ui.clone(),
                run_id: run_id.clone(),
                name: name.clone(),
                steps: step_ids
                    .iter()
                    .map(|id| launcher_domain::StepRun {
                        step_id: id.clone(),
                        status: launcher_domain::StepRunStatus::Pending,
                        attempt: 0,
                        last_error: None,
                        last_execution_id: None,
                        resolved_context_generation: None,
                        skip_reason: None,
                        output: None,
                    })
                    .collect(),
            };
            backend.push_progress(0);
            let mut runner = launcher_workflow::WorkflowRunner::new(backend);
            let mut run = match runner.run(&def, run_id.clone(), 0) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(workflow = %run_id, error = %e, "workflow.invalid");
                    return;
                }
            };
            // Paused(ConfirmationRequired) loop: wait for the originating
            // session's confirm event, then re-resolve + continue (WF-010)
            while run.status == WorkflowRunStatus::Paused {
                tracing::info!(workflow = %run_id, "workflow.waiting_confirmation");
                if rx
                    .recv_timeout(std::time::Duration::from_secs(300))
                    .is_err()
                {
                    tracing::info!(workflow = %run_id, "workflow.confirm_timeout_keep_paused");
                    break; // keep paused; Esc/timeout never cancels effects
                }
                run = match runner.resume(&def, run, 0) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!(workflow = %run_id, error = %e, "workflow.resume_failed");
                        return;
                    }
                };
            }
            if let Some(ui) = ui.upgrade() {
                let view = launcher_ui::to_workflow_run_view(&run, &name);
                launcher_ui::show_workflow_run(&ui, &view);
            }
            tracing::info!(workflow = %run_id, status = ?run.status, "workflow.finished");
        })
        .expect("spawn workflow thread");
}

/// Built-in demo definition: full lifecycle in three steps — normal,
/// confirmation-required (exercises Paused → Confirm), normal.
pub fn demo_definition() -> WorkflowDefinition {
    serde_json::from_value(serde_json::json!({
        "id": "wf.demo",
        "version": 1,
        "name": "Demo Workflow",
        "steps": [
            { "step_id": "s1", "action": { "Inline": {
                "id": "s1", "title": "Copy step 1", "type": "system.copy_to_clipboard",
                "input": {"text": "workflow step 1"} } }, "input": {"text": "workflow step 1"} },
            { "step_id": "s2", "action": { "Inline": {
                "id": "s2", "title": "Copy step 2 (confirm)", "type": "system.copy_to_clipboard",
                "input": {"text": "workflow step 2"}, "confirmation": "confirm" } },
              "input": {"text": "workflow step 2"} },
            { "step_id": "s3", "action": { "Inline": {
                "id": "s3", "title": "Copy step 3", "type": "system.copy_to_clipboard",
                "input": {"text": "workflow step 3"} } }, "input": {"text": "workflow step 3"} }
        ]
    }))
    .expect("demo workflow definition")
}

#[allow(unused)]
fn placeholder_payload_kind(k: ActionKind, p: ActionPayload) {}
