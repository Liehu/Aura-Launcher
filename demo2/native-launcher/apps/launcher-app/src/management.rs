//! Management window host (P3.1-B0, ADR-0019): owns the standalone
//! `ManagementWindow` lifecycle and projects host state (plugins /
//! workflows / general settings) into it. Pure projection rules apply —
//! the window renders models and forwards actions upward; all logic lives
//! here and in the APIs it calls.

use std::sync::{Arc, Mutex};

use launcher_domain::Category;
use slint::{ComponentHandle as _, Weak};

use launcher_domain::{Command, QueryContext};

use crate::AppState;

pub const PROVIDER_ID: &str = "management";

/// The lazily created window. Slint windows must be created and touched on
/// the UI thread; the Weak lives here and every mutation is dispatched via
/// `slint::invoke_from_event_loop`.
static WINDOW: std::sync::OnceLock<Mutex<Option<Weak<launcher_ui::ManagementWindow>>>> =
    std::sync::OnceLock::new();
static STATE: std::sync::OnceLock<Arc<Mutex<AppState>>> = std::sync::OnceLock::new();

fn window_slot() -> &'static Mutex<Option<Weak<launcher_ui::ManagementWindow>>> {
    WINDOW.get_or_init(|| Mutex::new(None))
}

/// Entry: open (or raise) the management window and refresh its models.
pub fn open(state: Arc<Mutex<AppState>>) {
    let _ = STATE.set(state.clone());
    let _ = slint::invoke_from_event_loop(move || {
        let existing = window_slot()
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        let ui = match existing {
            Some(w) => w.upgrade(),
            None => match launcher_ui::ManagementWindow::new() {
                Ok(w) => {
                    wire(w.as_weak());
                    if let Ok(mut slot) = window_slot().lock() {
                        *slot = Some(w.as_weak());
                    }
                    Some(w)
                }
                Err(e) => {
                    tracing::error!(error = %e, "management window create failed");
                    return;
                }
            },
        };
        if let Some(w) = ui {
            let _ = w.show();
            refresh(&w);
        }
    });
}

/// Wire the once-per-window callbacks (actions re-enter the host).
fn wire(weak: Weak<launcher_ui::ManagementWindow>) {
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_setting_applied(move |action_id| {
            if let Some(state) = STATE.get() {
                let ui2 = cb.clone();
                let action_id = action_id.to_string();
                std::thread::spawn(move || match crate::settings_ui::apply(&action_id) {
                    Ok(msg) => {
                        tracing::info!(target = %action_id, "settings.applied");
                        post_status(&ui2, &msg);
                        refresh_threaded(state.clone(), Some(ui2));
                    }
                    Err(e) => post_status(&ui2, &format!("⚠ {e}")),
                });
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_plugin_toggled(move |id, enable| {
            if let Some(state) = STATE.get() {
                let st = state.lock().expect("state lock");
                let res = st
                    .plugins
                    .set_enabled(&id, enable)
                    .map_err(|e| e.to_string());
                drop(st);
                match res {
                    Ok(()) => {
                        tracing::info!(plugin = %id, enable, "management.plugin_toggled");
                        refresh_threaded(state.clone(), Some(cb.clone()));
                    }
                    Err(e) => post_status(&cb, &format!("⚠ {e}")),
                }
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_workflow_deleted(move |path| {
            let path = path.to_string();
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    tracing::info!(?path, "management.workflow_deleted");
                    if let Some(state) = STATE.get() {
                        refresh_threaded(state.clone(), Some(cb.clone()));
                    }
                }
                Err(e) => post_status(&cb, &format!("⚠ delete failed: {e}")),
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_closed(move || {
            if let Some(w) = cb.upgrade() {
                let _ = w.hide();
            }
        });
    });
}

fn post_status(ui: &Weak<launcher_ui::ManagementWindow>, msg: &str) {
    // v1: the management window surfaces action feedback through the
    // general page value column refresh; errors go to the log + popup.
    tracing::info!(msg, "management.action");
    let _ = ui;
}

/// Rebuild and push all models onto the window (UI thread).
fn refresh(w: &launcher_ui::ManagementWindow) {
    let general: Vec<launcher_ui::SettingRow> = vec![
        row("Theme", "cycles dark → light → system", "theme"),
        row("Autostart on login", "toggle registration", "autostart"),
        row(
            "AI Agent",
            "status + config (allow_remote_data)",
            "ai",
        ),
        row("Advanced options", "open config.toml", "config"),
    ];
    w.set_general_rows(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(general),
    )));

    let Some(state) = STATE.get() else { return };
    let st = state.lock().expect("state lock");
    let mut plugins: Vec<launcher_ui::PluginEntry> = Vec::new();
    // enumerate manifests from the plugins dir through the registry state
    if let Ok(entries) = std::fs::read_dir(crate::data_dir().join("plugins")) {
        for e in entries.flatten() {
            let mf = e.path().join("plugin.json");
            if !mf.exists() {
                continue;
            }
            let Ok(manifest) =
                launcher_core::providers::plugin::PluginProvider::from_manifest_file(&mf)
            else {
                continue;
            };
            let id = manifest.manifest().id.clone();
            let version = manifest.manifest().version.clone();
            let st_state = st.plugins.state(&id);
            let trust = st.plugins.trust(&id);
            plugins.push(launcher_ui::PluginEntry {
                id: id.into(),
                state_text: if st_state.quarantined {
                    "quarantined".into()
                } else if st_state.enabled {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
                trust: format!("{trust:?}").into(),
                version: version.unwrap_or_default().into(),
                enabled: st_state.enabled && !st_state.quarantined,
                quarantined: st_state.quarantined,
            });
        }
    }
    w.set_plugins(slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(
        plugins,
    ))));

    let mut workflows: Vec<launcher_ui::WorkflowEntry> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(crate::data_dir().join("workflows")) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(&p) else {
                continue;
            };
            let Ok(def) = serde_json::from_str::<launcher_domain::WorkflowDefinition>(&raw) else {
                continue;
            };
            workflows.push(launcher_ui::WorkflowEntry {
                name: def.name.into(),
                path: p.display().to_string().into(),
            });
        }
    }
    w.set_workflows(slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(
        workflows,
    ))));

    w.set_about_text(
        format!(
            "Native Launcher v{}\nAuthority chain: Producer → Resolver → Policy → Approval → Engine → Adapter.\nPlugins are Job-Object isolated; settings writes are atomic.",
            env!("CARGO_PKG_VERSION")
        )
        .into(),
    );
}

/// Refresh from a non-UI thread (dispatch + model rebuild).
fn refresh_threaded(
    state: Arc<Mutex<AppState>>,
    ui: Option<Weak<launcher_ui::ManagementWindow>>,
) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui.and_then(|w| w.upgrade()) {
            refresh(&ui);
        } else if let Some(slot) = window_slot().lock().ok().and_then(|s| s.clone()) {
            if let Some(w) = slot.upgrade() {
                let _ = state;
                refresh(&w);
            }
        }
    });
}

fn row(label: &str, value: &str, action: &str) -> launcher_ui::SettingRow {
    launcher_ui::SettingRow {
        label: label.into(),
        value: value.into(),
        action_id: action.into(),
    }
}

/// The searchable entry command ("management" / "管理").
pub struct ManagementCommandProvider;

impl launcher_core::Provider for ManagementCommandProvider {
    fn id(&self) -> &str {
        PROVIDER_ID
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        let n = q.normalized.clone();
        let hit = n.contains("management")
            || n.contains("管理")
            || n.split_whitespace().all(|w| w == "settings")
                && !n.is_empty();
        if hit && !n.is_empty() {
            vec![Command {
                id: "management:open".into(),
                title: "Open Management (settings · plugins · workflows)".into(),
                subtitle: Some("Standalone window: General / Plugins / Workflows / About".into()),
                icon: None,
                provider_id: PROVIDER_ID.into(),
                score: 0.9,
                keywords: vec![
                    "management".into(),
                    "管理".into(),
                    "设置".into(),
                    "插件".into(),
                    "工作流".into(),
                ],
                category: Category::Command,
                actions: vec![launcher_domain::Action {
                    kind: launcher_domain::ActionKind::Execute,
                    payload: None,
                    id: Some("open".into()),
                    title: Some("Open".into()),
                    disabled_reason: None,
                    shortcut: None,
                    confirmation_required: false,
                }],
                target: None,
            }]
        } else {
            vec![]
        }
    }
}
