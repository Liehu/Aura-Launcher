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
use std::sync::{Arc, Mutex};

use launcher_ai::agent::AgentExecutionHost as _;
use launcher_ai::agent_loop::{LoopStop, TurnExecutor, run_agent};
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

/// Host-side step executor: resolves `action_ref` (=`provider|command|action`,
/// the ref format published in the catalog projection) into an ActionProposal
/// and runs it through the frozen chain via `CoreAgentHost`.
struct CoreTurnExecutor {
    state: Arc<Mutex<AppState>>,
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

/// Catalog projection for the planner: the neutral `ActionCatalogItem`s plus
/// the `ref` key the model echoes back in plan steps. Same lossy projection
/// rules as the proposal layer (no authority state survives).
fn catalog_json(state: &Arc<Mutex<AppState>>) -> String {
    let items = state
        .lock()
        .ok()
        .map(|mut st| st.core.action_catalog_items())
        .unwrap_or_default();
    let projected: Vec<serde_json::Value> = items
        .iter()
        .map(|i| {
            serde_json::json!({
                "ref": format!("{}|{}|{}", i.provider_id, i.command_id, i.action_id),
                "title": i.title,
                "description": i.description,
                "action_type": i.action_type,
                "input_schema": i.input_schema,
            })
        })
        .collect();
    serde_json::to_string(&projected).unwrap_or_else(|_| "[]".into())
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
            let mut session = AgentSession::new(&run_id2, MAX_TURNS);
            let mut exec = CoreTurnExecutor { state: state2.clone() };
            let llm = |prompt: String| -> Result<String, String> {
                provider
                    .generate(&launcher_ai::llm::LlmRequest {
                        system_prompt: String::new(),
                        user_prompt: prompt,
                    })
                    .map(|r| r.text)
                    .map_err(|e| e.to_string())
            };
            let stop = run_agent(
                &mut session,
                &goal2,
                "", // context: host-supplied; v1 runs context-free
                &catalog,
                &llm,
                &mut exec,
                &cancel,
                &ClarificationPolicy::default(),
                CONTEXT_BUDGET,
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
