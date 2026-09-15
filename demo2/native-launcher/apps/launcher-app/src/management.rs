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
    /// Settings parity: the hotkey row currently capturing (None = idle).
    static CAPTURING: RefCell<Option<String>> = const { RefCell::new(None) };
}

static STATE: std::sync::OnceLock<Arc<Mutex<AppState>>> = std::sync::OnceLock::new();

/// P3.1-B3 (EXPERIMENTAL): desktop-pinned plugin windows (WorkerW re-parent).
static DESKTOP: std::sync::OnceLock<Mutex<std::collections::HashSet<isize>>> =
    std::sync::OnceLock::new();

/// Entry: open (or raise) the management window and refresh its models.
/// Dispatches to the UI thread (Slint windows are UI-thread objects).
pub fn open(state: Arc<Mutex<AppState>>) {
    open_tab(state, None);
}

/// P3-H (spec §15): tray one-hop entries land on a SPECIFIC tab
/// (0 = general/settings, 1 = plugins — ManagementWindow `tab` property).
pub fn open_tab(state: Arc<Mutex<AppState>>, tab: Option<i32>) {
    let _ = STATE.set(state.clone());
    let _ = slint::invoke_from_event_loop(move || open_on_ui_thread(tab));
}

fn open_on_ui_thread(tab: Option<i32>) {
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
            if let Some(tab) = tab {
                w.set_tab(tab);
            }
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

    // ---- settings parity callbacks (General page real controls) ----
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_setting_toggled(move |key, new| {
            let key = key.to_string();
            let new = new;
            if let Some(w) = cb.upgrade() {
                match key.as_str() {
                    "autostart" => {
                        apply_setting(&w, &key, if new { "true" } else { "false" }, |cfg| {
                            let old = cfg.autostart.to_string();
                            cfg.autostart = new;
                            old
                        })
                    }
                    "watch_enabled" => {
                        apply_setting(&w, &key, if new { "true" } else { "false" }, |cfg| {
                            let old = cfg.watch_enabled.to_string();
                            cfg.watch_enabled = new;
                            old
                        })
                    }
                    _ => set_save_status(&w, false, "未知的开关项"),
                }
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_setting_selected(move |key, option| {
            let key = key.to_string();
            let opt = option.to_string();
            if let Some(w) = cb.upgrade() {
                if key == "theme_mode" {
                    let opt2 = opt.clone();
                    apply_setting(&w, &key, &opt, move |cfg| {
                        let old = cfg.theme_mode.clone();
                        cfg.theme_mode = opt2;
                        old
                    });
                }
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_setting_number(move |key, v| {
            let key = key.to_string();
            let v = v.clamp(4, 16);
            if let Some(w) = cb.upgrade() {
                if key == "result_limit" {
                    apply_setting(&w, &key, &v.to_string(), move |cfg| {
                        let old = cfg.result_limit.to_string();
                        cfg.result_limit = v as usize;
                        old
                    });
                }
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_setting_capture_started(move |key| {
            let key = key.to_string();
            if key != "hotkey" {
                return;
            }
            CAPTURING.with(|c| *c.borrow_mut() = Some(key.clone()));
            if let Some(w2) = cb.upgrade() {
                refresh(&w2);
                set_save_status(&w2, true, "请按下新的组合键…（Esc 取消）");
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_setting_capture_cancelled(move |key| {
            let key = key.to_string();
            if key != "hotkey" {
                return;
            }
            CAPTURING.with(|c| *c.borrow_mut() = None);
            if let Some(w2) = cb.upgrade() {
                refresh(&w2);
                set_save_status(&w2, true, "已取消热键修改");
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_setting_captured(move |key, text, ctrl, alt, shift, meta| {
            let key = key.to_string();
            if key != "hotkey" {
                return;
            }
            let Some(main) = normalize_main_key(&text) else {
                return;
            };
            if !(ctrl || alt || shift || meta) {
                if let Some(w) = cb.upgrade() {
                    set_save_status(
                        &w,
                        false,
                        "热键至少需要一个修饰键（Ctrl / Alt / Shift / Win）",
                    );
                }
                return;
            }
            let mut combo = String::new();
            if ctrl {
                combo.push_str("Ctrl+");
            }
            if alt {
                combo.push_str("Alt+");
            }
            if shift {
                combo.push_str("Shift+");
            }
            if meta {
                combo.push_str("Win+");
            }
            combo.push_str(&main);
            CAPTURING.with(|c| *c.borrow_mut() = None);
            if let Some(w) = cb.upgrade() {
                let combo2 = combo.clone();
                apply_setting(&w, &key, &combo, move |cfg| {
                    let old = cfg.hotkey.clone();
                    cfg.hotkey = combo2;
                    old
                });
                set_save_status(&w, true, "热键已保存 · 重启后生效");
            }
        });
    });
    let cb = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |w| {
        w.on_setting_reset_default(move |key| {
            let key = key.to_string();
            if key != "hotkey" {
                return;
            }
            CAPTURING.with(|c| *c.borrow_mut() = None);
            if let Some(w) = cb.upgrade() {
                apply_setting(&w, &key, "Ctrl+Space", |cfg| {
                    let old = cfg.hotkey.clone();
                    cfg.hotkey = "Ctrl+Space".into();
                    old
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
            let Ok(pin) = pin.to_string().parse::<isize>() else {
                return;
            };
            let Ok(target) = target.to_string().parse::<isize>() else {
                return;
            };
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
            let Ok(pin) = pin.to_string().parse::<isize>() else {
                return;
            };
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
            let Ok(h) = hwnd.to_string().parse::<isize>() else {
                return;
            };
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
            let Ok(h) = hwnd.to_string().parse::<isize>() else {
                return;
            };
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
    tracing::info!(msg, "management.action");
    let msg = msg.to_string();
    let _ = ui.upgrade_in_event_loop(move |w| {
        let ok = !msg.starts_with('?');
        w.set_save_ok(ok);
        w.set_save_status(msg.into());
    });
}

/// Settings parity (G7): save-status strip helpers.
fn set_save_status(w: &launcher_ui::ManagementWindow, ok: bool, msg: &str) {
    w.set_save_ok(ok);
    w.set_save_status(msg.into());
}

/// Rebuild and push all models onto the window (UI thread).
fn refresh(w: &launcher_ui::ManagementWindow) {
    // Settings parity (2026-09 round 2): typed option model — groups, real
    // controls, current values from config.toml. The UI renders and reports;
    // every value change re-enters through the wire() callbacks below.
    let mut items: Vec<launcher_ui::SettingItem> = Vec::new();
    let mut focus_ix: i32 = 0;
    let capturing = CAPTURING.with(|c| c.borrow().clone());
    match launcher_config::load_or_create(&launcher_config::config_path().unwrap_or_default()) {
        Ok(cfg) => {
            items.push(group_item("外观"));
            items.push(item(
                "theme_mode",
                "主题模式",
                "弹窗与管理窗口配色，切换立即生效",
                "segmented",
                cfg.theme_mode.clone(),
                false,
                cfg.result_limit.clamp(4, 16) as i32,
                4,
                16,
                vec!["system".into(), "light".into(), "dark".into()],
                true,
                focus_ix,
            ));
            focus_ix += 1;
            items.push(group_item("行为"));
            items.push(item(
                "autostart",
                "开机自启",
                "登录时自动启动 Native Launcher",
                "toggle",
                String::new(),
                cfg.autostart,
                0,
                0,
                0,
                vec![],
                true,
                focus_ix,
            ));
            focus_ix += 1;
            items.push(group_item("性能"));
            items.push(item(
                "watch_enabled",
                "文件监视",
                "索引目录变化时增量更新（关闭后按周期重建）",
                "toggle",
                String::new(),
                cfg.watch_enabled,
                0,
                0,
                0,
                vec![],
                true,
                focus_ix,
            ));
            focus_ix += 1;
            items.push(item(
                "result_limit",
                "结果条数",
                "搜索结果最多显示的行数（重启后生效）",
                "slider",
                String::new(),
                false,
                cfg.result_limit.clamp(4, 16) as i32,
                4,
                16,
                vec![],
                true,
                focus_ix,
            ));
            focus_ix += 1;
            items.push(group_item("快捷键"));
            items.push(item(
                "hotkey",
                "呼出热键",
                "点击输入框后按下新组合（至少一个修饰键，Esc 取消）",
                "hotkey",
                cfg.hotkey.clone(),
                false,
                0,
                0,
                0,
                vec![],
                capturing.as_deref().map(|k| k != "hotkey").unwrap_or(true),
                focus_ix,
            ));
            items.push(group_item("索引目录"));
            let dirs = if cfg.index_dirs.is_empty() {
                "(默认: 文档 / 桌面 / 下载)".to_string()
            } else {
                cfg.index_dirs.join("  ·  ")
            };
            items.push(item(
                "index_dirs",
                "已配置索引目录",
                "删除：搜索 settings index；添加：编辑 config.toml",
                "info",
                dirs,
                false,
                0,
                0,
                0,
                vec![],
                true,
                -1,
            ));
            let ai = match &cfg.llm {
                None => "未配置（在 config.toml 添加 [llm]）".to_string(),
                Some(l) => format!(
                    "{} / {} · 远程数据 {}",
                    l.base_url,
                    l.model,
                    if l.allow_remote_data {
                        "已允许"
                    } else {
                        "仅本地"
                    }
                ),
            };
            items.push(item(
                "ai",
                "AI Agent",
                "状态与配置（allow_remote_data）",
                "info",
                ai,
                false,
                0,
                0,
                0,
                vec![],
                true,
                -1,
            ));
        }
        Err(e) => {
            items.push(item(
                "config_error",
                "配置读取失败",
                "将使用默认值；请检查 config.toml",
                "info",
                format!("{e}"),
                false,
                0,
                0,
                0,
                vec![],
                true,
                -1,
            ));
        }
    }
    w.set_setting_items(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(items),
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
    w.set_plugins(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(plugins),
    )));

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
    w.set_workflows(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(workflows),
    )));

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
    w.set_windows(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(windows),
    )));
    w.set_targets(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(targets),
    )));

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

fn group_item(label: &str) -> launcher_ui::SettingItem {
    launcher_ui::SettingItem {
        key: label.into(),
        label: label.into(),
        desc: String::new().into(),
        kind: "group".into(),
        value_str: String::new().into(),
        value_bool: false,
        value_num: 0,
        min: 0,
        max: 0,
        options: slint::ModelRc::default(),
        enabled: true,
        capturing: false,
        focus_ix: -1,
    }
}

#[allow(clippy::too_many_arguments)]
fn item(
    key: &str,
    label: &str,
    desc: &str,
    kind: &str,
    value_str: String,
    value_bool: bool,
    value_num: i32,
    min: i32,
    max: i32,
    options: Vec<slint::SharedString>,
    enabled: bool,
    focus_ix: i32,
) -> launcher_ui::SettingItem {
    launcher_ui::SettingItem {
        key: key.into(),
        label: label.into(),
        desc: desc.into(),
        kind: kind.into(),
        value_str: value_str.into(),
        value_bool,
        value_num,
        min,
        max,
        options: slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(options))),
        enabled,
        capturing: false,
        focus_ix,
    }
}

/// Settings parity (G7/G8): load -> mutate -> atomic save -> audit ledger ->
/// status line -> side effects -> refresh. Runs on the UI thread (config IO
/// is a small TOML file; existing settings flow does the same).
fn apply_setting(
    w: &launcher_ui::ManagementWindow,
    key: &str,
    new_display: &str,
    mutate: impl FnOnce(&mut launcher_config::AppConfig) -> String,
) {
    let applied = (|| -> anyhow::Result<()> {
        let path = launcher_config::config_path()?;
        let mut cfg = launcher_config::load_or_create(&path)?;
        let old = mutate(&mut cfg);
        launcher_config::save(&path, &cfg)?;
        launcher_config::audit_setting_change(key, &old, new_display, "management");
        Ok(())
    })();
    match applied {
        Ok(()) => {
            set_save_status(w, true, &format!("已保存 · {key} = {new_display}"));
            post_apply_side_effects(w, key, new_display);
            refresh(w);
        }
        Err(e) => set_save_status(w, false, &format!("保存失败：{e}")),
    }
}

fn post_apply_side_effects(w: &launcher_ui::ManagementWindow, key: &str, new: &str) {
    match key {
        "theme_mode" => {
            let light = crate::effective_light(new);
            w.global::<launcher_ui::Theme>().set_light_theme(light);
        }
        "autostart" => {
            if let Err(e) = crate::autostart::set_autostart(new == "true") {
                set_save_status(w, false, &format!("自启动注册失败：{e}"));
            }
        }
        _ => {}
    }
}

/// Only real keys with at least one modifier become a hotkey combo.
fn normalize_main_key(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_lowercase();
    if matches!(
        lower.as_str(),
        "control"
            | "ctrl"
            | "shift"
            | "alt"
            | "menu"
            | "meta"
            | "super"
            | "windows"
            | "system"
            | "lwin"
            | "rwin"
    ) {
        return None; // bare modifier: keep waiting for the full chord
    }
    Some(match t {
        " " => "Space".into(),
        s if s.chars().count() == 1 => s.to_uppercase(),
        s => s.to_string(),
    })
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
            || n.split_whitespace().all(|w| w == "settings") && !n.is_empty();
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
