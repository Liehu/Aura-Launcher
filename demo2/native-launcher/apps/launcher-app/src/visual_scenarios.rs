// Visual Regression scenario baselines (review 39-mvp7-0.3, Spec §51;
// P1-E additions per review 60 §51):
// 15 frozen UI states, each captured at 640x420 / Dark / 100% scale.
// `LAUNCHER_SNAPSHOT_DIR=<dir>` writes VR-001..VR-015 BMPs; byte-diff them
// against the previous run for the visual regression gate.

use crate::snapshot;
use launcher_domain::{Category, Command};
use launcher_ui::WorkflowItem;

/// Deterministic demo dataset for a named visual scenario.
pub fn scenario_data(name: &str) -> DemoState {
    let base_results: Vec<Command> = (0..3)
        .map(|i| Command {
            id: format!("demo.cmd{i}"),
            title: format!("Demo Command {i}"),
            subtitle: Some(format!("demo subtitle {i}")),
            icon: String::new().into(),
            provider_id: "demo".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![],
            target: None,
        })
        .collect();

    let action = |id: &str, title: &str, enabled: bool| {
        launcher_ui::ActionItem {
            action_id: id.into(),
            title: title.into(),
            enabled,
            reason: if enabled {
                String::new().into()
            } else {
                "Requires shell.execute".into()
            },
            shortcut: if id == "copy" { "Ctrl+Shift+C".into() } else { "".into() },
            is_primary: false,
        }
    };

    match name {
        // VR-001 Main.Empty
        "VR-001" => DemoState { results: vec![], actions: vec![], status: String::new(), ..Default::default() },
        // VR-002 Main.Results
        "VR-002" => DemoState { results: base_results, ..Default::default() },
        // VR-003 Main.Selected (second row carries the marker)
        "VR-003" => DemoState {
            results: base_results,
            selected: 1,
            ..Default::default()
        },
        // VR-004 Main.Error — distinct from Empty (UI-CONTRACT §3.1)
        "VR-004" => DemoState {
            status: "⚠ Search failed: plugin:calc — Timeout".into(),
            ..Default::default()
        },
        // VR-005 Action.Normal (panel open, all enabled, primary badge)
        "VR-005" => DemoState {
            results: base_results,
            actions: vec![
                action("copy", "Copy result", true),
                action("paste", "Paste at cursor", true),
                action("open", "Open History", true),
            ],
            panel_visible: true,
            ..Default::default()
        },
        // VR-006 Action.Disabled
        "VR-006" => DemoState {
            results: base_results,
            actions: vec![
                action("copy", "Copy result", true),
                action("exec", "Execute command", false),
            ],
            panel_visible: true,
            ..Default::default()
        },
        // VR-007 Action.Confirmation
        "VR-007" => DemoState {
            results: base_results,
            actions: vec![action("exec", "Execute command", true)],
            panel_visible: true,
            confirmation_pending: true,
            ..Default::default()
        },
        // VR-008 Workflow.Running
        "VR-008" => DemoState {
            wf_items: wf_steps("running", None),
            wf_title: "Organize Downloads".into(),
            wf_status: "⌛ Running · 2 / 4".into(),
            wf_visible: true,
            ..Default::default()
        },
        // VR-009 Workflow.Paused(ConfirmationRequired)
        "VR-009" => DemoState {
            wf_items: wf_steps("paused", None),
            wf_title: "Organize Downloads".into(),
            wf_status: "⏸ Waiting for your confirmation".into(),
            wf_visible: true,
            ..Default::default()
        },
        // VR-010 Workflow.Failed
        "VR-010" => DemoState {
            wf_items: wf_steps("failed", Some("Plugin is not installed or could not be started".into())),
            wf_title: "Organize Downloads".into(),
            wf_status: "⚠ Failed · step 3 / 4".into(),
            wf_visible: true,
            wf_diagnostics: true,
            ..Default::default()
        },
        // ---- P1-E additions (review 60 §51: VR-011..015) ----
        // VR-011 Workflow.Success — every step complete, green-free ✓ text.
        "VR-011" => DemoState {
            wf_items: wf_steps("success", None),
            wf_title: "Organize Downloads".into(),
            wf_status: "✓ Completed · 4 / 4".into(),
            wf_visible: true,
            ..Default::default()
        },
        // VR-012 Workflow.Branch — condition-false step skipped with the
        // taken branch spelled out (§16), diagnostics show the goto.
        "VR-012" => DemoState {
            wf_items: wf_steps("branch", None),
            wf_title: "Organize Downloads".into(),
            wf_status: "✓ Completed · branch: finish".into(),
            wf_visible: true,
            wf_diagnostics: true,
            ..Default::default()
        },
        // VR-013 Agent.Running — agent as a workflow-style runtime panel
        // (§18) with the §22 budget line in the status row.
        "VR-013" => DemoState {
            wf_items: agent_steps(),
            wf_title: "Agent: Organize Downloads".into(),
            wf_status: "⌛ Running · Turn 3 / 8 · Executions 5 / 16".into(),
            wf_visible: true,
            ..Default::default()
        },
        // VR-014 MCP.Reconnecting — info-grade transient state, NOT an
        // error presentation (§26/§45).
        "VR-014" => DemoState {
            results: base_results,
            status: "● MCP Calculator — Reconnecting…".into(),
            ..Default::default()
        },
        // VR-015 Main.ProviderUnavailable — distinct empty state (§6),
        // never rendered as "No Results".
        "VR-015" => DemoState {
            status: "⚠ Provider temporarily unavailable".into(),
            ..Default::default()
        },
        _ => Default::default(),
    }
}

fn wf_steps(state: &str, error: Option<String>) -> Vec<WorkflowItem> {
    let wf = |id: &str, symbol: &str, title: &str, state_text: &str, err: String, current: bool| WorkflowItem {
        step_id: id.into(),
        symbol: symbol.into(),
        title: title.into(),
        state_text: state_text.into(),
        error: err.into(),
        current,
        execution_id: if id == "s2" { "e-102".into() } else { "".into() },
        failure_class: if id == "s3" { "PluginUnavailable".into() } else { "".into() },
    };
    match state {
        "running" => vec![
            wf("s1", "✓", "Scan files", "done", String::new(), false),
            wf("s2", "●", "Classify files", "running", String::new(), true),
            wf("s3", "○", "Generate report", "pending", String::new(), false),
            wf("s4", "○", "Move files", "pending", String::new(), false),
        ],
        "paused" => vec![
            wf("s1", "✓", "Scan files", "done", String::new(), false),
            wf("s2", "⏸", "Move files", "waiting for your confirmation", String::new(), true),
            wf("s3", "○", "Generate report", "pending", String::new(), false),
        ],
        "failed" => vec![
            wf("s1", "✓", "Scan files", "done", String::new(), false),
            wf("s2", "⚠", "Compress files",
               "failed",
               error.clone().unwrap_or_else(|| "Plugin is not installed or could not be started".into()),
               false),
            wf("s3", "⊘", "Move files", "skipped", String::new(), false),
        ],
        // P1-E (§15/§16): success + branch states for VR-011/VR-012.
        "success" => vec![
            wf("s1", "✓", "Scan files", "done", String::new(), false),
            wf("s2", "✓", "Classify files", "done", String::new(), false),
            wf("s3", "✓", "Move files", "done", String::new(), false),
            wf("s4", "✓", "Finish", "done", String::new(), false),
        ],
        "branch" => vec![
            wf("s1", "✓", "Check file count", "done", String::new(), false),
            wf("s2", "⊘", "Compress", "condition false → finish",
               String::new(), false),
            wf("s3", "✓", "Finish", "done", String::new(), false),
        ],
        _ => vec![],
    }
}

/// VR-013 (§18): agent phases rendered as workflow-style rows.
fn agent_steps() -> Vec<WorkflowItem> {
    let ph = |sym: &str, name: &str, state: &str, cur: bool| WorkflowItem {
        step_id: name.into(),
        symbol: sym.into(),
        title: name.into(),
        state_text: state.into(),
        error: "".into(),
        current: cur,
        execution_id: if name == "Execute" { "e-103".into() } else { "".into() },
        failure_class: "".into(),
    };
    vec![
        ph("✓", "Observe", "ok", false),
        ph("✓", "Plan", "ok", false),
        ph("◐", "Execute", "running", true),
        ph("○", "Replan", "pending", false),
    ]
}

/// The named visual regression scenarios, in frozen order (VR-001..VR-015).
pub fn scenarios() -> Vec<(&'static str, DemoState)> {
    vec![
        ("VR-001-Main-Empty", scenario_data("VR-001")),
        ("VR-002-Main-Results", scenario_data("VR-002")),
        ("VR-003-Main-Selected", scenario_data("VR-003")),
        ("VR-004-Main-Error", scenario_data("VR-004")),
        ("VR-005-Action-Normal", scenario_data("VR-005")),
        ("VR-006-Action-Disabled", scenario_data("VR-006")),
        ("VR-007-Action-Confirmation", scenario_data("VR-007")),
        ("VR-008-Workflow-Running", scenario_data("VR-008")),
        ("VR-009-Workflow-Paused", scenario_data("VR-009")),
        ("VR-010-Workflow-Failed", scenario_data("VR-010")),
        // P1-E (review 60 §51)
        ("VR-011-Workflow-Success", scenario_data("VR-011")),
        ("VR-012-Workflow-Branch", scenario_data("VR-012")),
        ("VR-013-Agent-Running", scenario_data("VR-013")),
        ("VR-014-MCP-Reconnecting", scenario_data("VR-014")),
        ("VR-015-Main-ProviderUnavailable", scenario_data("VR-015")),
    ]
}

// ---- shared demo state container ----

#[derive(Clone, Default)]
pub struct DemoState {
    pub results: Vec<Command>,
    pub actions: Vec<launcher_ui::ActionItem>,
    pub wf_items: Vec<WorkflowItem>,
    pub wf_title: String,
    pub wf_status: String,
    pub wf_visible: bool,
    pub wf_diagnostics: bool,
    pub panel_visible: bool,
    pub confirmation_pending: bool,
    pub status: String,
    pub selected: usize,
}

/// Apply a scenario's state to the AppWindow.
pub fn apply(ui: &launcher_ui::AppWindow, st: &DemoState) {
    let results: Vec<launcher_ui::ResultItem> = st
        .results
        .iter()
        .map(|c| launcher_ui::ResultItem {
            command_id: c.id.clone().into(),
            title: c.title.clone().into(),
            subtitle: c.subtitle.clone().unwrap_or_default().into(),
            icon: String::new().into(),
            score: String::new().into(),
            icon_data: slint::Image::default(),
        })
        .collect();
    ui.set_results(slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(
        results,
    ))));
    ui.set_selected_index(st.selected as i32);

    let actions: Vec<launcher_ui::ActionItem> = st
        .actions
        .iter()
        .map(|a| launcher_ui::ActionItem {
            action_id: a.action_id.clone().into(),
            title: a.title.clone().into(),
            enabled: a.enabled,
            reason: a.reason.clone().into(),
            shortcut: a.shortcut.clone().into(),
            is_primary: a.is_primary,
        })
        .collect();
    ui.set_actions(slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(
        actions,
    ))));
    ui.set_panel_visible(st.panel_visible);
    ui.set_confirmation_pending(st.confirmation_pending);

    let wf: Vec<launcher_ui::WorkflowItem> = st
        .wf_items
        .iter()
        .map(|w| launcher_ui::WorkflowItem {
            step_id: w.step_id.clone().into(),
            symbol: w.symbol.clone().into(),
            title: w.title.clone().into(),
            state_text: w.state_text.clone().into(),
            error: w.error.clone().into(),
            current: w.current,
            execution_id: w.execution_id.clone().into(),
            failure_class: w.failure_class.clone().into(),
        })
        .collect();
    ui.set_wf_items(slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(
        wf,
    ))));
    ui.set_wf_title(wf_title_of(st).into());
    ui.set_wf_status(wf_status_of(st).into());
    ui.set_wf_visible(st.wf_visible);
    ui.set_wf_diagnostics(st.wf_diagnostics);
    ui.set_status(st.status.clone().into());
    ui.set_context_hint("📁 demo-context".into());
}

fn wf_title_of(st: &DemoState) -> String {
    if st.wf_visible {
        st.wf_title.clone()
    } else {
        String::new()
    }
}

fn wf_status_of(st: &DemoState) -> String {
    if st.wf_visible {
        st.wf_status.clone()
    } else {
        String::new()
    }
}

/// VR capture helper: raise the launcher above every other window without
/// needing foreground activation (denied to background processes). A fully
/// occluded winit+GL window never presents a surface, so both PrintWindow
/// and screen BitBlt would capture black; topmost makes DWM composite it.
#[cfg(windows)]
fn force_topmost() {
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsIconic, IsWindowVisible, SetWindowPos, ShowWindow, HWND_TOPMOST,
        SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_RESTORE,
    };
    unsafe {
        let Ok(hwnd) = FindWindowW(None, &windows::core::HSTRING::from("Launcher")) else {
            tracing::warn!("vr.window_not_found");
            return;
        };
        // hide->show cycles leave the window iconic (main.rs foreground notes);
        // an iconic window has a valid client rect but no rendered surface.
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        tracing::info!(
            visible = IsWindowVisible(hwnd).as_bool(),
            iconic = IsIconic(hwnd).as_bool(),
            "vr.window_state"
        );
    }
}

#[cfg(not(windows))]
fn force_topmost() {}

/// Capture every frozen state (VR-001..VR-010) into `dir` as BMPs.
pub fn capture_all(
    ui: slint::Weak<launcher_ui::AppWindow>,
    dir: std::path::PathBuf,
) {
    // The capture loop runs on a worker thread; every UI mutation is posted
    // to the event loop and acked, so the loop must already be running. We
    // probe with a no-op dispatch until the UI thread responds.
    std::thread::spawn(move || {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::error!(?dir, error = %e, "vr.create_dir_failed");
        }
        for _ in 0..100 {
            if slint::invoke_from_event_loop(|| {}).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // Show/raise the window ON THE UI THREAD: upgrading the Weak from a
        // worker thread yields None, so a previous off-thread show() silently
        // never ran and every capture came out black.
        {
            let (stx, srx) = std::sync::mpsc::channel::<()>();
            let ui2 = ui.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = ui2.upgrade() {
                    use slint::ComponentHandle as _;
                    let _ = w.show();
                    // Center and raise: a fully occluded or iconic window has
                    // a valid client rect but no presented surface, so both
                    // PrintWindow and screen BitBlt capture black.
                    crate::foreground::recenter_and_repaint(w.window());
                    // focus the root key scope, NOT the search box: the
                    // LineEdit caret blinks on a wall-clock phase, which made
                    // captures nondeterministic (§52: fixed visual data only)
                    w.invoke_focus_keys();
                    crate::foreground::take_foreground("Launcher");
                    force_topmost();
                }
                let _ = stx.send(());
            });
            let _ = srx.recv_timeout(std::time::Duration::from_millis(2000));
        }

        for (name, state) in scenarios() {
            // apply the state ON the UI thread (INV-001) and wait for the ack
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            let ui2 = ui.clone();
            let st = state.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = ui2.upgrade() {
                    apply(&w, &st);
                }
                let _ = tx.send(());
            });
            let _ = rx.recv_timeout(std::time::Duration::from_millis(1000));

            // let the renderer paint, then capture the client area
            std::thread::sleep(std::time::Duration::from_millis(350));
            let hwnd = snapshot::find_launcher_hwnd().unwrap_or(0);
            match snapshot::capture_client_to_bmp(hwnd) {
                Some(bmp) => {
                    let path = dir.join(format!("{name}.bmp"));
                    match std::fs::write(&path, &bmp) {
                        Ok(()) => tracing::info!(scenario = name, ?path, bytes = bmp.len(), "vr.captured"),
                        Err(e) => tracing::error!(scenario = name, error = %e, "vr.write_failed"),
                    }
                }
                None => tracing::error!(scenario = name, "vr.capture_failed"),
            }
        }
        let _ = slint::invoke_from_event_loop(|| {
            let _ = slint::quit_event_loop();
        });
    });
}
