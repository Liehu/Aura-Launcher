//! P2.4-D CLI tests: drive `run_cli` IN-PROCESS (the CLI is lib+bin), so no
//! test spawns a process (the dev-driver `run` subcommand's process handling
//! is the production host path covered by plugin-host E2E).

use launcher_plugin_cli::run_cli;

use std::fs;
use std::path::PathBuf;

fn scratch(tag: &str) -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("nl_cli_{tag}_{}_{}", std::process::id(), n));
    fs::create_dir_all(&d).unwrap();
    d
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// P24-D02/D03 happy path: init scaffold passes validate.
#[test]
fn init_then_validate_ok() {
    let dir = scratch("init");
    assert_eq!(run_cli(&args(&["init", dir.to_str().unwrap(), "--id", "dev.ok"])), 0);
    assert_eq!(run_cli(&args(&["validate", dir.to_str().unwrap()])), 0);
    fs::remove_dir_all(&dir).ok();
}

/// D03 fail-closed: validate rejects a manifest whose entrypoint is missing.
#[test]
fn validate_rejects_missing_entrypoint() {
    let dir = scratch("noentry");
    assert_eq!(run_cli(&args(&["init", dir.to_str().unwrap(), "--id", "dev.noentry"])), 0);
    fs::remove_file(dir.join("main.py")).unwrap();
    assert_eq!(run_cli(&args(&["validate", dir.to_str().unwrap()])), 1);
    fs::remove_dir_all(&dir).ok();
}

/// D03 fail-closed: invalid plugin ids are rejected at init.
#[test]
fn init_rejects_invalid_id() {
    let dir = scratch("badid");
    assert_eq!(
        run_cli(&args(&["init", dir.to_str().unwrap(), "--id", "../evil"])),
        1
    );
    fs::remove_dir_all(&dir).ok();
}

/// D03/D05: package → inspect roundtrip; envelope is deterministic JSON with
/// contract version.
#[test]
fn package_and_inspect_roundtrip() {
    let dir = scratch("pkg");
    let pkg = dir.join("out.nlpkg");
    assert_eq!(run_cli(&args(&["init", dir.to_str().unwrap(), "--id", "dev.pkg"])), 0);
    assert_eq!(
        run_cli(&args(&[
            "package",
            dir.to_str().unwrap(),
            "--out",
            pkg.to_str().unwrap()
        ])),
        0
    );
    assert_eq!(run_cli(&args(&["inspect", pkg.to_str().unwrap()])), 0);
    fs::remove_dir_all(&dir).ok();
}

/// D04: staged install lands the plugin under <root>/<id>; reinstall replaces.
#[test]
fn install_stages_and_replaces() {
    let dir = scratch("inst");
    let root = dir.join("plugins");
    let pkg = dir.join("out.nlpkg");
    assert_eq!(run_cli(&args(&["init", dir.to_str().unwrap(), "--id", "dev.inst"])), 0);
    assert_eq!(
        run_cli(&args(&[
            "package",
            dir.to_str().unwrap(),
            "--out",
            pkg.to_str().unwrap()
        ])),
        0
    );
    assert_eq!(
        run_cli(&args(&[
            "install",
            pkg.to_str().unwrap(),
            "--root",
            root.to_str().unwrap()
        ])),
        0
    );
    assert!(root.join("dev.inst").join("plugin.json").exists());
    assert!(!root.join("dev.inst.staging").exists(), "staging cleaned up");
    // reinstall (replace) still works
    assert_eq!(
        run_cli(&args(&[
            "install",
            pkg.to_str().unwrap(),
            "--root",
            root.to_str().unwrap()
        ])),
        0
    );
    // uninstall removes it
    assert_eq!(
        run_cli(&args(&[
            "uninstall",
            "dev.inst",
            "--root",
            root.to_str().unwrap()
        ])),
        0
    );
    assert!(!root.join("dev.inst").exists());
    fs::remove_dir_all(&dir).ok();
}

/// D04 fail-closed: a package with a path-traversal file name is rejected
/// before anything touches the plugins root.
#[test]
fn install_rejects_path_traversal() {
    let dir = scratch("trav");
    let root = dir.join("plugins");
    fs::create_dir_all(&root).unwrap();
    let manifest = r#"{"id":"dev.trav","name":"T","runtime":{"type":"python","executable":"main.py"}}"#;
    let envelope = serde_json::json!({
        "contract_version": "0.1",
        "manifest": serde_json::from_str::<serde_json::Value>(manifest).unwrap(),
        "files": { "../evil.py": "" }
    });
    let pkg = dir.join("evil.nlpkg");
    fs::write(&pkg, serde_json::to_vec(&envelope).unwrap()).unwrap();
    assert_eq!(
        run_cli(&args(&[
            "install",
            pkg.to_str().unwrap(),
            "--root",
            root.to_str().unwrap()
        ])),
        1
    );
    assert!(!dir.join("evil.py").exists(), "traversal file never written");
    assert!(fs::read_dir(&root).unwrap().next().is_none(), "root untouched");
    fs::remove_dir_all(&dir).ok();
}

/// D04 fail-closed: unsupported contract versions are rejected.
#[test]
fn install_rejects_unknown_contract_version() {
    let dir = scratch("contract");
    let root = dir.join("plugins");
    fs::create_dir_all(&root).unwrap();
    let manifest = r#"{"id":"dev.contract","name":"C","runtime":{"type":"python","executable":"main.py"}}"#;
    let envelope = serde_json::json!({
        "contract_version": "9.9",
        "manifest": serde_json::from_str::<serde_json::Value>(manifest).unwrap(),
        "files": { "main.py": "cHJpbnQoaGkp" }
    });
    let pkg = dir.join("bad.nlpkg");
    fs::write(&pkg, serde_json::to_vec(&envelope).unwrap()).unwrap();
    assert_eq!(
        run_cli(&args(&[
            "install",
            pkg.to_str().unwrap(),
            "--root",
            root.to_str().unwrap()
        ])),
        1
    );
    fs::remove_dir_all(&dir).ok();
}

/// Unknown commands fail with exit code 1.
#[test]
fn unknown_command_fails() {
    assert_eq!(run_cli(&args(&["definitely-not-a-command"])), 1);
}

/// P2.4-E04: replay drives real queries through the host spawn path
/// (requires a Python interpreter like python_sdk_e2e; skipped otherwise).
#[test]
fn replay_runs_saved_queries() {
    // interpreter probe identical in spirit to python_sdk_e2e
    let py = std::env::var("LAUNCHER_PYTHON").unwrap_or_else(|_| "python".into());
    let ok = std::process::Command::new(&py)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("skipping: no python interpreter");
        return;
    }
    let dir = scratch("replay");
    assert_eq!(
        run_cli(&args(&["init", dir.to_str().unwrap(), "--id", "dev.replay"])),
        0
    );
    // make the scaffold runnable: point the SDK import at the repo SDK
    let main_py = dir.join("main.py");
    let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/python");
    let body = fs::read_to_string(&main_py).unwrap().replace(
        r#"<path-to-plugins-python-sdk>"#,
        &sdk.to_string_lossy(),
    );
    fs::write(&main_py, body).unwrap();

    let queries = dir.join("queries.txt");
    fs::write(&queries, "hello
world
").unwrap();
    let code = run_cli(&args(&[
        "replay",
        dir.to_str().unwrap(),
        "--queries",
        queries.to_str().unwrap(),
    ]));
    assert_eq!(code, 0, "replay through the real host path should succeed");
    fs::remove_dir_all(&dir).ok();
}
