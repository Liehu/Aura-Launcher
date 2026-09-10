//! Management window host (P3.1-B0, ADR-0019): owns the standalone
//! `ManagementWindow` lifecycle and projects host state (plugins /
//! workflows / general settings) into it. Pure projection rules apply —
//! the window renders models and forwards actions upward; all logic lives
//! here and in the APIs it calls.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use launcher_domain::Category;
use slint::{ComponentHandle as _, Weak};

use launcher_domain::{Command, QueryContext};

use crate::AppState;

pub const PROVIDER_ID: &str = "management";

thread_local! {
    /// P3.1 lifecycle fix: the STRONG window handle lives in the UI thread's
    /// thread-local (Slint handles are !Send — a static Mutex of the handle
    /// is illegal). Close = park off-screen; reopen = show + recenter. The
    /// window is created once and reused for the process lifetime.
    static WIN: RefCell<Option<launcher_ui::ManagementWindow>> =
        const { RefCell::new(None) };
}

static STATE: std::sync::OnceLock<Arc<Mutex<AppState>>> = std::sync::OnceLock::new();

/// P3.1-B3 (EXPERIMENTAL): desktop-pinned plugin windows (WorkerW re-parent).
static DESKTOP: std::sync::OnceLock<Mutex<std::collections::HashSet<isize>>> =
    std::sync::OnceLock::new();

/// Entry: open (or raise) the management window and refresh its models.
/// Dispatches to the UI thread (Slint windows are UI-thread objects).
pub fn open(state: Arc<Mutex<AppState>>) {
    let _ = STATE.set(state.clone());
    let _ = slint::invoke_from_event_loop(open_on_ui_thread);
}

fn open_on_ui_thread() {
    WIN.with(|slot| {
        if slot.borrow().is_none() {
            match launcher_ui::ManagementWindow::new() {
                Ok(w) => {
                    wire(w.as_weak());
                    *slot.borrow_mut() = Some(w);
                }
                Err(e) => {
                    tracing::error!(error = %e, "management window create failed");
                    return;
                }
            }
        }
        if let Some(w) = slot.borrow().as_ref() {
            let _ = w.show();
            crate::foreground::recenter_and_repaint(w.window());
            refresh(w);
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
    // P3.1-B2: follow-set / follow-clear (target-anchored topmost)
    let cb_set = weak.clone();
    let cb_clear = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_follow_set(move |pin, target, title| {
            let Ok(pin) = pin.to_string().parse::<isize>() else { return };
            let Ok(target) = target.to_string().parse::<isize>() else { return };
            match crate::win_platform::follow_set(pin, target, title.to_string()) {
                Ok(()) => {
                    tracing::info!(pin, target, "management.follow_set");
                    if let Some(w) = cb_set.upgrade() {
                        refresh(&w);
                    }
                }
                Err(e) => post_status(&cb_set, &format!("⚠ follow failed: {e}")),
            }
        });
        w.on_follow_clear(move |pin| {
            let Ok(pin) = pin.to_string().parse::<isize>() else { return };
            if crate::win_platform::follow_clear(pin) {
                tracing::info!(pin, "management.follow_cleared");
                if let Some(w) = cb_clear.upgrade() {
                    refresh(&w);
                }
            }
        });
    });
    // P3.1-B3: desktop pin (WorkerW re-parent, EXPERIMENTAL)
    let cb_desk = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_desktop_pin_toggled(move |hwnd, desktop| {
            let Ok(h) = hwnd.to_string().parse::<isize>() else { return };
            let res = if desktop {
                crate::win_platform::pin_desktop(h)
            } else {
                crate::win_platform::unpin_desktop(h)
            };
            let Some(w) = cb_desk.upgrade() else { return };
            match res {
                Ok(()) => {
                    if let Some(set) = DESKTOP.get() {
                        if let Ok(mut m) = set.lock() {
                            if desktop {
                                m.insert(h);
                            } else {
                                m.remove(&h);
                            }
                        }
                    }
                    refresh(&w);
                }
                Err(e) => {
                    tracing::warn!(hwnd = h, error = %e, "desktop.pin_failed");
                    refresh(&w);
                }
            }
        });
    });
    let weak2 = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_closed(move || {
            // P3.1 reopen fix: park off-screen, never hide/destroy (winit
            // show/hide cycles leave windows invalid — popup lesson)
            if let Some(x) = weak2.upgrade() {
                park(&x);
            }
        });
    });
    // P3.1-B1: pin/unpin a discovered plugin window (topmost toggle)
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_pin_toggled(move |hwnd, pin| {
            let Ok(h) = hwnd.to_string().parse::<isize>() else { return };
            match crate::win_platform::set_topmost(h, pin) {
                Ok(()) => {
                    crate::win_platform::pin_mark(h, pin);
                    tracing::info!(hwnd = h, pin, "management.pin_toggled");
                    if let Some(w) = cb.upgrade() {
                        refresh(&w);
                    }
                }
                Err(e) => post_status(&cb, &format!("⚠ pin failed: {e}")),
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
    let mut st = state.lock().expect("state lock");
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

    // P3.1-B1: discovered plugin windows (pid-boundary: only pids the host
    // spawned, and only manifests that declared "window": true)
    let mut windows: Vec<launcher_ui::WindowEntry> = Vec::new();
    // P3.1-B2: running windows the user may pick as a follow target
    // (everything visible and titled EXCEPT our own process — the user
    // explicitly picks targets, so this list is user-authorized data)
    let mut targets: Vec<launcher_ui::TargetEntry> = Vec::new();
    {
        let own = std::process::id();
        for winfo in crate::win_platform::enumerate_visible_windows() {
            if winfo.pid == own {
                continue;
            }
            targets.push(launcher_ui::TargetEntry {
                hwnd: format!("{}", winfo.hwnd).into(),
                title: winfo.title.clone().into(),
            });
        }
    }
    let follow = crate::win_platform::follow_current();
    let desktop_set = DESKTOP.get_or_init(|| Mutex::new(std::collections::HashSet::new()));
    for (id, pid, window_ui) in st.core.running_plugins() {
        if !window_ui {
            continue;
        }
        for winfo in crate::win_platform::visible_windows_of_pid(pid) {
            let pinned = crate::win_platform::pin_is_marked(winfo.hwnd);
            let desktop = desktop_set
                .lock()
                .map(|m| m.contains(&winfo.hwnd))
                .unwrap_or(false);
            let follow_target = follow
                .as_ref()
                .filter(|(pin, _, _)| *pin == winfo.hwnd)
                .map(|(_, _, title)| title.clone())
                .unwrap_or_default();
            windows.push(launcher_ui::WindowEntry {
                hwnd: format!("{}", winfo.hwnd).into(),
                title: winfo.title.into(),
                plugin_id: id.clone().into(),
                pinned,
                desktop,
                follow_target: follow_target.into(),
            });
        }
    }
    w.set_windows(slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(
        windows,
    ))));
    w.set_targets(slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(
        targets,
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
/// Park off-screen (window stays mapped; reopen = show + recenter).
fn park(ui: &launcher_ui::ManagementWindow) {
    crate::foreground::park(ui.window());
}

fn refresh_threaded(_state: Arc<Mutex<AppState>>, ui: Option<Weak<launcher_ui::ManagementWindow>>) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = ui.and_then(|w| w.upgrade()) {
            refresh(&w);
            return;
        }
        WIN.with(|slot| {
            if let Some(w) = slot.borrow().as_ref() {
                refresh(w);
            }
        });
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
