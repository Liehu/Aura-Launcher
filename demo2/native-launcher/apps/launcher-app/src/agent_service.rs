//! P2.7-C03–C07 host wiring (spec `P2.7 开发设计规范` §22-§26): composes the
//! launcher-ai Agent Loop (`run_agent`) with the app's live Core — the same
//! pattern as `workflow_service`: a trigger source (the `agent:` provider
//! namespace, below) requests a run; run creation, the LLM call, step
//! execution and session persistence belong to this service, never the UI.
//!
//! Execution boundary: every plan step goes through the frozen Resolver →
//! Engine chain via `CoreAgentHost` (launcher-core) — the loop decides WHAT
//! next, the Core decides HOW an effect happens (INV-EXEC-001 family).
//!
//! Persistence (C07): each run is an `AgentSession` record in
//! `agents.db` (SQLite/WAL, corruption-rebuild policy of the store).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use launcher_ai::agent::AgentExecutionHost as _;
use launcher_ai::agent_loop::{ApprovalSink, LoopStop, TurnExecutor, run_agent};
use launcher_ai::approval::{ApprovalDecision, Decision as ApprovalChoice};
use launcher_ai::memory::{RunRecord, RunMemory, SessionMemory};
use launcher_ai::privacy::RemoteAiPolicy;
use launcher_ai::agent_session::AgentSession;
use launcher_ai::agent_session_store::{AgentSessionRecord, AgentSessionStore};
use launcher_ai::clarification::ClarificationPolicy;
use launcher_ai::llm::{LlmProvider, OpenAiCompatibleProvider};
use launcher_config::LlmConfig;
use slint::{ComponentHandle as _, Weak};

use crate::AppState;

/// Step budget per run (§24: session-level turn cap; v1 single-turn runs).
const MAX_TURNS: u64 = 8;
/// Prompt/context budget in chars (§23 bounded-context contract).
const CONTEXT_BUDGET: usize = 4096;

/// The active run's cancel flag (Esc on the runtime surface cancels BETWEEN
/// steps only — an in-flight effect is never interrupted, INV-048 family).
static ACTIVE_CANCEL: std::sync::OnceLock<Mutex<Option<Arc<AtomicBool>>>> =
    std::sync::OnceLock::new();

/// Called from the UI dismiss path: cancels the active agent run at the
/// next between-step checkpoint. No active run = no-op.
pub fn cancel_active_agent() {
    if let Some(slot) = ACTIVE_CANCEL.get_or_init(|| Mutex::new(None)).lock().ok() {
        if let Some(flag) = slot.as_ref() {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

/// D02: confirm channel for a pending agent approval (the runtime surface's
/// confirm button approves the shown plan; Esc/timeout declines).
static APPROVAL_TX: std::sync::OnceLock<Mutex<Option<Sender<()>>>> = std::sync::OnceLock::new();

/// Called from the UI confirm path; approves the pending agent plan.
pub fn confirm_active_approval() {
    if let Some(cell) = APPROVAL_TX.get() {
        if let Ok(slot) = cell.lock() {
            if let Some(tx) = slot.as_ref() {
                let _ = tx.send(());
            }
        }
    }
}

/// D02 sink: blocks the agent-run thread on the user's decision while the
/// runtime surface shows the plan. Approve → Approve; Esc/cancel/timeout →
/// None (the gate then fails closed). The approval boundary stays intact:
/// only the shown plan is authorized, once.
struct UiApprovalSink {
    ui: slint::Weak<launcher_ui::AppWindow>,
    cancel: Arc<AtomicBool>,
    goal: String,
}

impl ApprovalSink for UiApprovalSink {
    fn request_approval(
        &mut self,
        request: &launcher_ai::approval::ApprovalRequest,
    ) -> Option<ApprovalDecision> {
        let items: Vec<launcher_ui::WorkflowItem> = request
            .steps
            .iter()
            .map(|s| launcher_ui::WorkflowItem {
                step_id: s.step_id.clone().into(),
                symbol: if s.requires_approval { "!" } else { "◇" }.into(),
                title: s.action_ref.clone().into(),
                state_text: if s.requires_approval {
                    "needs approval".into()
                } else {
                    "pending".into()
                },
                error: String::new().into(),
                current: false,
                execution_id: String::new().into(),
                failure_class: String::new().into(),
            })
            .collect();
        let ui = self.ui.clone();
        let title = format!("Approval required · {}", self.goal);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui.upgrade() {
                let _ = ui.show();
                ui.set_wf_title(title.into());
                ui.set_wf_status("Enter = approve · Esc = reject".into());
                ui.set_wf_items(slint::ModelRc::new(std::rc::Rc::new(
                    slint::VecModel::from(items),
                )));
                ui.set_wf_visible(true);
            }
        });
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        if let Ok(mut slot) = APPROVAL_TX.get_or_init(|| Mutex::new(None)).lock() {
            *slot = Some(tx);
        }
        let outcome = loop {
            if self.cancel.load(Ordering::SeqCst) {
                break None; // Esc path cancels the whole run
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            if now > request.expires_at_ms {
                break None; // D05 expiry
            }
            match rx.recv_timeout(std::time::Duration::from_millis(150)) {
                Ok(()) => {
                    break Some(ApprovalDecision {
                        request_id: request.request_id.clone(),
                        decision: ApprovalChoice::Approve,
                    });
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break None,
            }
        };
        if let Ok(mut slot) = APPROVAL_TX.get_or_init(|| Mutex::new(None)).lock() {
            *slot = None;
        }
        outcome
    }
}

/// Host-side step executor: resolves `action_ref` (=`provider|command|action`,
/// the ref format published in the catalog projection) into an ActionProposal
/// and runs it through the frozen chain via `CoreAgentHost`. F04: pushes the
/// executing step to the runtime surface status line.
struct CoreTurnExecutor {
    state: Arc<Mutex<AppState>>,
    ui: slint::Weak<launcher_ui::AppWindow>,
}

impl TurnExecutor for CoreTurnExecutor {
    fn execute(
        &mut self,
        action_ref: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let parts: Vec<&str> = action_ref.splitn(3, '|').collect();
        let [provider_id, command_id, action_id] = parts.as_slice() else {
            return Err(format!("malformed action_ref: {action_ref}"));
        };
        {
            let ui = self.ui.clone();
            let progress = format!("executing {action_ref}");
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.set_wf_status(progress.into());
                }
            });
        }
        let proposal = launcher_workflow::proposal::ActionProposal {
            provider_id: (*provider_id).to_string(),
            command_id: (*command_id).to_string(),
            action_id: (*action_id).to_string(),
            input: input.clone(),
        };
        let mut st = self.state.lock().map_err(|_| "state lock poisoned")?;
        let mut host = launcher_core::agent_host::CoreAgentHost::new(&mut st.core);
        let records = host.execute(std::slice::from_ref(&proposal));
        match records.into_iter().next() {
            Some(r) if r.ok => Ok(r.result.unwrap_or(serde_json::Value::Null)),
            Some(r) => Err(r.error.unwrap_or_else(|| "step failed".into())),
            None => Err("no execution record".into()),
        }
    }
}

/// Catalog projection for the planner (B01): the unified Tool Catalog over
/// the action catalog + installed workflows, rendered through the
/// launcher-ai projection (bounded, deterministic, `ref`-keyed).
fn catalog_json(state: &Arc<Mutex<AppState>>) -> String {
    let (actions, workflows) = match state.lock() {
        Ok(mut st) => {
            let actions = st.core.action_catalog_items();
            let workflows = std::fs::read_dir(crate::data_dir().join("workflows"))
                .map(|rd| {
                    rd.flatten()
                        .filter(|e| {
                            e.path().extension().and_then(|x| x.to_str()) == Some("json")
                        })
                        .filter_map(|e| {
                            let raw = std::fs::read_to_string(e.path()).ok()?;
                            let def: launcher_domain::WorkflowDefinition =
                                serde_json::from_str(&raw).ok()?;
                            Some(launcher_ai::tool_catalog::WorkflowToolSummary {
                                definition_id: def.id,
                                name: def.name,
                                description: None,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (actions, workflows)
        }
        Err(_) => (Vec::new(), Vec::new()),
    };
    let cat = launcher_ai::tool_catalog::ToolCatalog::project(&actions, &workflows);
    let (rendered, _) = cat.render_json(&launcher_ai::tool_catalog::ProjectionLimits::default());
    rendered
}

fn provider_from_config(cfg: &LlmConfig) -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider {
        base_url: cfg.base_url.clone(),
        model: cfg.model.clone(),
        api_key: std::env::var("LAUNCHER_LLM_API_KEY").ok().filter(|v| !v.is_empty()),
        timeout: std::time::Duration::from_secs(cfg.timeout_secs.max(1)),
    }
}

/// LLM endpoint, captured from config at startup (main). `None` = Agent
/// disabled: commands surface a clear status instead of running.
static LLM_CFG: std::sync::OnceLock<Option<LlmConfig>> = std::sync::OnceLock::new();

pub fn set_llm_config(cfg: Option<LlmConfig>) {
    let _ = LLM_CFG.set(cfg);
}

/// Start an agent run for `goal` (from the `agent:` provider command target):
/// persists the session (C07), shows the runtime surface, drives `run_agent`
/// on a worker thread, and pushes the outcome back to the surface + store.
pub fn start_agent_run(
    state: Arc<Mutex<AppState>>,
    ui_weak: Weak<launcher_ui::AppWindow>,
    goal: String,
) {
    let llm_cfg = LLM_CFG.get().cloned().flatten();
    let Some(llm_cfg) = llm_cfg else {
        crate::set_status(
            ui_weak,
            "⚠ Agent needs an [llm] section in config.toml".into(),
        );
        return;
    };
    // E05 local-first gate: remote endpoints need the explicit opt-in.
    let policy = if llm_cfg.allow_remote_data {
        RemoteAiPolicy::allow_remote()
    } else {
        RemoteAiPolicy::default()
    };
    if let Err(notice) = policy.ensure_remote_allowed() {
        tracing::info!("agent.remote_blocked");
        crate::set_status(ui_weak, format!("⚠ {notice}"));
        return;
    }
    let provider = provider_from_config(&llm_cfg);
    let run_id = format!("agent-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0));

    // C07: persist the session as Running before any step executes.
    let store_path = crate::data_dir().join("agents.db");
    let store = match AgentSessionStore::open(&store_path) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!(error = %e, "agent session store unavailable; run proceeds unpersisted");
            // the run continues; persistence is observation-only in v1
            None
        }
    }.map(Arc::new);
    if let Some(store) = &store {
        let _ = store.save(&AgentSessionRecord {
            session_id: run_id.clone(),
            goal: goal.clone(),
            turn: 0,
            status: "running".into(),
            updated_at_ms: now_ms(),
        });
    }

    show_agent_surface(&ui_weak, &goal, "▶ planning");

    let cancel = Arc::new(AtomicBool::new(false));
    if let Ok(mut slot) = ACTIVE_CANCEL.get_or_init(|| Mutex::new(None)).lock() {
        *slot = Some(cancel.clone());
    }

    let state2 = state.clone();
    let ui_weak2 = ui_weak.clone();
    let run_id2 = run_id.clone();
    let goal2 = goal.clone();
    std::thread::Builder::new()
        .name("agent-run".into())
        .spawn(move || {
            let catalog = catalog_json(&state2);
            // E01: bounded session memory supplies recent goals as context
            let context = {
                let mem = memory_slot().lock().expect("memory lock");
                let recent: String = mem
                    .entries("global")
                    .iter()
                    .rev()
                    .take(5)
                    .map(|e| format!("- {}", e.text))
                    .collect::<Vec<_>>()
                    .join("\n");
                launcher_ai::privacy::sanitize_untrusted(&recent, 1024)
            };
            memory_slot()
                .lock()
                .expect("memory lock")
                .push("global", "goal", &goal2, now_ms());
            let mut session = AgentSession::new(&run_id2, MAX_TURNS);
            let mut exec = CoreTurnExecutor { state: state2.clone(), ui: ui_weak2.clone() };
            let mut telemetry = launcher_ai::telemetry::Telemetry::default();
            let llm = |prompt: String| -> Result<String, String> {
                provider
                    .generate(&launcher_ai::llm::LlmRequest {
                        system_prompt: String::new(),
                        user_prompt: prompt,
                    })
                    .map(|r| r.text)
                    .map_err(|e| e.to_string())
            };
            let mut sink = UiApprovalSink {
                ui: ui_weak2.clone(),
                cancel: cancel.clone(),
                goal: goal2.clone(),
            };
            let stop = run_agent(
                &mut session,
                &goal2,
                &context, // E01: sanitized recent goals from session memory
                &catalog,
                &llm,
                &mut exec,
                &cancel,
                &ClarificationPolicy::default(),
                CONTEXT_BUDGET,
                &mut sink,
                &mut telemetry,
            );
            if let Some(store) = &store {
                let _ = store.save(&AgentSessionRecord {
                    session_id: run_id2.clone(),
                    goal: goal2.clone(),
                    turn: session.turns,
                    status: "running".into(), // replaced by finish() below
                    updated_at_ms: now_ms(),
                });
                let _ = store.finish(&run_id2);
            }
            let msg = match &stop {
                LoopStop::Completed => format!("✓ Agent completed: {goal2}"),
                LoopStop::Cancelled => format!("⏹ Agent cancelled: {goal2}"),
                LoopStop::Failed { step_id, error } => {
                    format!("✗ Agent failed at {step_id}: {error}")
                }
            };
            tracing::info!(run = %run_id2, stop = ?stop, turns = session.turns, "agent.finished");
            // §37/§38 observability: dump the event ring + metrics to the log
            for r in telemetry.log.records() {
                tracing::info!(at = r.at_ms, event = r.event.name(), detail = %r.detail, "agent.event");
            }
            let m = &telemetry.metrics;
            tracing::info!(
                llm_ms = m.llm_latency_ms,
                plan_ms = m.planning_latency_ms,
                approval_wait_ms = m.approval_wait_ms,
                run_ms = m.agent_run_latency_ms,
                steps = m.tool_selection_count,
                replans = m.replan_count,
                model_errors = m.model_error_count,
                "agent.metrics"
            );
            remember_goal(&goal2);
            // E02: record the run outcome in bounded run memory
            runs_slot().lock().expect("run memory lock").record(RunRecord {
                run_id: run_id2.clone(),
                goal: goal2.clone(),
                status: format!("{stop:?}"),
                turns: session.turns,
                at_ms: now_ms(),
            });
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak2.upgrade() {
                    ui.set_wf_visible(false);
                    ui.set_status(msg.into());
                }
            });
            // clear the cancel slot only if this run still owns it
            if let Some(cell) = ACTIVE_CANCEL.get() {
                if let Ok(mut slot) = cell.lock() {
                    if slot.as_ref().map(|f| Arc::ptr_eq(f, &cancel)).unwrap_or(false) {
                        *slot = None;
                    }
                }
            }
        })
        .expect("spawn agent thread");
}

/// Reuse the Workflow Runtime Surface as the agent progress view: the loop
/// phases rendered as workflow-style rows (same projection VR-013 uses).
fn show_agent_surface(ui_weak: &Weak<launcher_ui::AppWindow>, goal: &str, status: &str) {
    let _ = slint::invoke_from_event_loop({
        let ui_weak = ui_weak.clone();
        let goal = goal.to_string();
        let status = status.to_string();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let _ = ui.show();
                ui.set_wf_title(format!("Agent · {goal}").into());
                ui.set_wf_status(status.into());
                ui.set_wf_items(slint::ModelRc::new(std::rc::Rc::new(
                    slint::VecModel::from(vec![
                        launcher_ui::WorkflowItem {
                            step_id: "plan".into(),
                            symbol: "◈".into(),
                            title: "Plan (LLM pipeline)".into(),
                            state_text: "running".into(),
                            error: String::new().into(),
                            current: true,
                            execution_id: String::new().into(),
                            failure_class: String::new().into(),
                        },
                        launcher_ui::WorkflowItem {
                            step_id: "execute".into(),
                            symbol: "◇".into(),
                            title: "Execute steps (Resolver → Engine)".into(),
                            state_text: "pending".into(),
                            error: String::new().into(),
                            current: false,
                            execution_id: String::new().into(),
                            failure_class: String::new().into(),
                        },
                    ]),
                )));
                ui.set_wf_visible(true);
            }
        }
    });
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// E01: bounded session memory (goals become context for later runs).
static MEMORY: std::sync::OnceLock<Mutex<SessionMemory>> = std::sync::OnceLock::new();

fn memory_slot() -> &'static Mutex<SessionMemory> {
    MEMORY.get_or_init(|| Mutex::new(SessionMemory::new()))
}

/// E02: bounded run memory.
static RUNS: std::sync::OnceLock<Mutex<RunMemory>> = std::sync::OnceLock::new();

fn runs_slot() -> &'static Mutex<RunMemory> {
    RUNS.get_or_init(|| Mutex::new(RunMemory::new()))
}

/// F05: the last goal, for `agent retry`.
static LAST_GOAL: std::sync::OnceLock<Mutex<Option<String>>> = std::sync::OnceLock::new();

/// F05: remember a goal for a later `agent retry`.
pub fn remember_goal(goal: &str) {
    if let Ok(mut slot) = LAST_GOAL.get_or_init(|| Mutex::new(None)).lock() {
        *slot = Some(goal.to_string());
    }
}

/// The most recent goal (empty string when none yet).
pub fn last_goal() -> String {
    LAST_GOAL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .unwrap_or_default()
}
