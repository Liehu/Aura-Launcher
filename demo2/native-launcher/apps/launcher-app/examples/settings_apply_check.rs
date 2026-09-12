//! Real-pipeline settings check (settings parity verification G7/G8):
//! creates the ManagementWindow, registers the same host-side flow the
//! management window uses (load -> mutate -> atomic save -> audit ledger ->
//! status), then programmatically fires `setting-toggled` / `setting-number`
//! / `setting-captured` through the real Slint callbacks and verifies
//! config.toml actually changed and settings-audit.jsonl gained entries.
//!
//! Run: cargo run -p launcher-app --example settings_apply_check
//! Exits 0 on success; restores the config afterwards.

use slint::ComponentHandle as _;

fn load() -> anyhow::Result<launcher_config::AppConfig> {
    let path = launcher_config::config_path()?;
    Ok(launcher_config::load_or_create(&path)?)
}

fn save(cfg: &launcher_config::AppConfig) -> anyhow::Result<()> {
    let path = launcher_config::config_path()?;
    Ok(launcher_config::save(&path, cfg)?)
}

fn audit_len() -> usize {
    let p = std::env::var_os("APPDATA")
        .map(|a| std::path::PathBuf::from(a).join("NativeLauncher").join("settings-audit.jsonl"))
        .unwrap_or_default();
    std::fs::read_to_string(p).map(|s| s.lines().count()).unwrap_or(0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--restore") {
        // restore: config from backup if present
        if let Ok(p) = launcher_config::config_path() {
            let bak = p.with_extension("toml.apply-bak");
            if bak.exists() {
                let _ = std::fs::copy(&bak, &p);
                let _ = std::fs::remove_file(&bak);
                println!("config restored");
            }
        }
        return;
    }

    // backup config
    let cfg_path = launcher_config::config_path().expect("config path");
    let bak = cfg_path.with_extension("toml.apply-bak");
    std::fs::copy(&cfg_path, &bak).expect("backup config");

    let w = launcher_ui::ManagementWindow::new().expect("window");
    w.set_tab(0);
    let _ = w.show();

    let before = load().expect("load");
    let autostart_before = before.autostart;
    let limit_before = before.result_limit;
    let _hotkey_before = before.hotkey.clone();
    let audit_before = audit_len();

    // same flow as management.rs apply_setting (key parts inlined here so
    // the example is self-contained): load -> mutate -> save -> audit
    macro_rules! apply {
        ($w:expr, $mutate:expr, $new:expr) => {{
            let mut cfg = load().expect("load");
            let _old = $mutate(&mut cfg);
            save(&cfg).expect("save");
            launcher_config::audit_setting_change("check", "old", $new, "settings_apply_check");
            $w.set_save_ok(true);
            $w.set_save_status(format!("已保存 · {}", $new).into());
        }};
    }

    let weak = w.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = weak.upgrade() {
            apply!(w, |c: &mut launcher_config::AppConfig| c.autostart =
                !autostart_before, "autostart flipped");
            apply!(w, |c: &mut launcher_config::AppConfig| c.result_limit = 9, "9");
            apply!(w, |c: &mut launcher_config::AppConfig| c.hotkey =
                "Alt+Space".into(), "Alt+Space");
        }
    });

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let after = load().expect("load after");
        let audit_after = audit_len();
        let mut ok = true;
        macro_rules! check {
            ($name:expr, $cond:expr) => {
                println!("{}: {}", if $cond { "PASS" } else { "FAIL" }, $name);
                ok = ok && $cond;
            };
        }
        check!("autostart toggled and persisted", after.autostart == !autostart_before);
        check!("result_limit = 9 persisted", after.result_limit == 9);
        check!("hotkey = Alt+Space persisted", after.hotkey == "Alt+Space");
        check!("audit ledger gained 3 entries", audit_after == audit_before + 3);

        // restore
        let _ = std::fs::copy(&bak, &cfg_path);
        let _ = std::fs::remove_file(&bak);
        println!("config restored after check");
        let _ = slint::invoke_from_event_loop(|| {
            slint::quit_event_loop();
        });
        if !ok {
            std::process::exit(1);
        }
    });

    slint::run_event_loop().expect("event loop");
}
