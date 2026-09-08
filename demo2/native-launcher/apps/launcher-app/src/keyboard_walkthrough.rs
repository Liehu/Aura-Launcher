//! P2.3-F Keyboard Walkthrough (UI-CONTRACT §99 Main-mode ladder):
//! drives the SAME Slint key-pressed pipeline physical keys take —
//! `Window::dispatch_event(KeyPressed)` reaches the capture-phase handler
//! in app.slint exactly like a real keystroke — then asserts observable
//! UI state transitions and writes a JSON report.
//!
//! The query text itself is injected through the `query-changed` callback
//! (the identical path the SearchBox fires); typed-char entry into the
//! LineEdit is Slint-internal behavior, while the CONTRACT covers the
//! navigation/state ladder, which is what this walkthrough asserts.
//!
//! Modifier chords (Ctrl+Down panel toggle, Ctrl+D favorite) are NOT
//! injectable through this API (no modifier field on WindowEvent); their
//! logic is covered by `crates/launcher-core/tests/ui_interaction.rs` and
//! the manual GUI checklist (docs/UI-CONTRACT-CONFORMANCE).
//!
//! `LAUNCHER_KEYBOARD_WALKTHROUGH=<report.json>` enables the mode; the app
//! runs it once after startup and quits.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle as _, Model as _, SharedString};

use crate::AppWindow;

struct Step {
    name: &'static str,
    ok: bool,
    detail: String,
}

pub fn run(ui_weak: slint::Weak<AppWindow>, visible: Arc<AtomicBool>, report: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        let mut steps: Vec<Step> = Vec::new();
        let mut pass = true;

        // -- show popup (same call the tray/hotkey path makes) + focus input
        {
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let _ = ui.show();
                    ui.invoke_focus_input();
                }
            });
            std::thread::sleep(Duration::from_millis(400));
        }

        // -- set the query through the SearchBox's own callback (real search
        // path); retry until results arrive — on a cold start provider
        // enumeration may lag the first query
        let mut count = 0usize;
        for _ in 0..100 {
            {
                let ui_weak = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_query_changed("a".into());
                    }
                });
            }
            std::thread::sleep(Duration::from_millis(300));
            count = read_ui(&ui_weak, |ui| ui.get_results().row_count() as i32).unwrap_or(0) as usize;
            if count > 0 {
                break;
            }
        }
        steps.push(Step {
            name: "query_yields_results",
            ok: count > 0,
            detail: format!("results={count}"),
        });
        pass &= count > 0;

        // -- ↓ moves selection (contract §99) through the REAL key pipeline
        press(&ui_weak, key_text(Key::DownArrow));
        let after_down = read_ui(&ui_weak, |ui| ui.get_selected_index()).unwrap_or(-1);
        steps.push(Step {
            name: "arrow_down_moves_selection",
            ok: count >= 2 && after_down == 1,
            detail: format!("selected_index={after_down} (results={count})"),
        });
        pass &= count >= 2 && after_down == 1;

        // -- ↑ moves back
        press(&ui_weak, key_text(Key::UpArrow));
        let after_up = read_ui(&ui_weak, |ui| ui.get_selected_index()).unwrap_or(-1);
        steps.push(Step {
            name: "arrow_up_moves_selection_back",
            ok: after_up == 0,
            detail: format!("selected_index={after_up}"),
        });
        pass &= after_up == 0;

        // -- Enter executes primary (real effect through on_execute →
        // ActionEngine → history; the effect lands in the app log)
        press(&ui_weak, key_text(Key::Return));
        std::thread::sleep(Duration::from_millis(500));
        steps.push(Step {
            name: "enter_dispatched_primary",
            ok: true,
            detail: "Enter dispatched on selection 0".into(),
        });

        // -- Esc dismisses (contract §99: Esc closes Launcher)
        press(&ui_weak, key_text(Key::Escape));
        std::thread::sleep(Duration::from_millis(300));
        let hidden = !visible.load(Ordering::SeqCst);
        steps.push(Step {
            name: "escape_dismisses_popup",
            ok: hidden,
            detail: format!("visible_flag_false={hidden}"),
        });
        pass &= hidden;

        let report_json = serde_json::json!({
            "mode": "keyboard-walkthrough",
            "pass": pass,
            "steps": steps.iter().map(|s| serde_json::json!({
                "step": s.name, "ok": s.ok, "detail": s.detail,
            })).collect::<Vec<_>>(),
        });
        let _ = std::fs::create_dir_all(report.parent().unwrap_or(std::path::Path::new(".")));
        let _ = std::fs::write(&report, serde_json::to_string_pretty(&report_json).unwrap_or_default());
        tracing::info!(pass, report = %report.display(), "keyboard.walkthrough.done");
        let _ = slint::quit_event_loop();
    });
}

/// P2.3-F DPI/scale walkthrough: inject the SAME ScaleFactorChanged event
/// winit dispatches when the user changes display scaling (125% / 150%),
/// then assert `apply_ui_scale` responds with the documented formula
/// (physical_h/1080/slint_scale snapped to 0.25, clamped 1.0..2.0) and the
/// window survives the transition without panic.
pub fn run_dpi(ui_weak: slint::Weak<AppWindow>, report: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        let mut steps: Vec<Step> = Vec::new();
        let mut pass = true;

        {
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let _ = ui.show();
                }
            });
            std::thread::sleep(Duration::from_millis(400));
        }

        let physical_h = 1600.0f32; // GetSystemMetrics(SM_CYSCREEN) on this host
        for factor in [1.0f32, 1.25, 1.5] {
            {
                let ui_weak = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.window().dispatch_event(WindowEvent::ScaleFactorChanged {
                            scale_factor: factor,
                        });
                    }
                });
            }
            std::thread::sleep(Duration::from_millis(300));
            // apply_ui_scale runs on the SHOW path (winit DPI now known);
            // re-apply here mirrors the production re-apply on recenter
            {
                let ui_weak = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        crate::apply_ui_scale(&ui);
                    }
                });
            }
            let ui_scale = read_ui(&ui_weak, |ui| {
                ui.global::<launcher_ui::Theme>().get_ui_scale()
            })
            .unwrap_or(0.0);
            let slint_scale =
                read_ui(&ui_weak, |ui| ui.window().scale_factor() as f32).unwrap_or(0.0);
            let expected =
                ((physical_h / 1080.0 / slint_scale * 4.0).round() / 4.0).clamp(1.0, 2.0);
            let ok = (ui_scale - expected).abs() < 0.001;
            steps.push(Step {
                name: "scale_factor_applied",
                ok,
                detail: format!(
                    "factor={factor} slint_scale={slint_scale} ui_scale={ui_scale} expected={expected}"
                ),
            });
            pass &= ok;
            let alive = read_ui(&ui_weak, |ui| ui.window().is_visible()).unwrap_or(false);
            steps.push(Step {
                name: "window_survives_dpi_change",
                ok: alive,
                detail: format!("factor={factor} visible={alive}"),
            });
            pass &= alive;
        }

        let report_json = serde_json::json!({
            "mode": "dpi-walkthrough",
            "pass": pass,
            "steps": steps.iter().map(|s| serde_json::json!({
                "step": s.name, "ok": s.ok, "detail": s.detail,
            })).collect::<Vec<_>>(),
        });
        let _ = std::fs::create_dir_all(report.parent().unwrap_or(std::path::Path::new(".")));
        let _ = std::fs::write(&report, serde_json::to_string_pretty(&report_json).unwrap_or_default());
        tracing::info!(pass, report = %report.display(), "dpi.walkthrough.done");
        let _ = slint::quit_event_loop();
    });
}

fn key_text(key: Key) -> SharedString {
    key.into()
}

/// Dispatch one key press+release on the event loop (real pipeline).
fn press(ui_weak: &slint::Weak<AppWindow>, text: SharedString) {
    let ui_weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.window().dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            ui.window().dispatch_event(WindowEvent::KeyReleased { text });
        }
    });
    std::thread::sleep(Duration::from_millis(150));
}

/// Read a value from the UI ON the event loop (avoids cross-thread model
/// access races).
fn read_ui<T: Send + 'static>(
    ui_weak: &slint::Weak<AppWindow>,
    read: impl Fn(&AppWindow) -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    let ui_weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let _ = tx.send(read(&ui));
        }
    });
    rx.recv_timeout(Duration::from_secs(2)).ok()
}
