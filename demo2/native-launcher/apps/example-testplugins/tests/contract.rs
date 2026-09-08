//! Plugin Contract Test Kit — conformance suite for the plugin protocol
//! (PLUGIN-CONTRACT-v0.1 §29.17): normal / slow / crash / malformed / flood,
//! handshake & version negotiation, query_id echo, manifest profiles, stdout
//! red line, frame limits, graceful shutdown, process-tree cleanup.
//! Every SDK (Rust / Python / future Node / WASM) MUST pass this suite.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use launcher_domain::PluginManifest;
use launcher_plugin_host::{PluginError, PluginHandle};

fn bin(name: &str) -> PathBuf {
    let exe = env!(concat!("CARGO_BIN_EXE_", "plugin-normal"));
    let p = PathBuf::from(exe);
    let _ = name;
    p.parent().map(PathBuf::from).unwrap_or_default()
}

fn exe_name(name: &str) -> String {
    format!("{name}.exe")
}

fn manifest(exe: &str, timeout_ms: u64) -> (PluginManifest, PathBuf) {
    let m: PluginManifest = serde_json::from_value(serde_json::json!({
        "id": "contract",
        "name": "Contract",
        "executable": exe,
        "timeout_ms": timeout_ms
    }))
    .unwrap();
    (m, bin(exe))
}

#[test]
fn normal_query_returns_results() {
    let (m, base) = manifest(&exe_name("plugin-normal"), 2000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    let cmds = h.query("hello").unwrap();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].title, "normal: hello");
    assert_eq!(cmds[0].provider_id, "plugin:contract");
    assert!(!cmds[0].actions.is_empty());
}

#[test]
fn slow_plugin_times_out_and_does_not_block_host() {
    let (m, base) = manifest(&exe_name("plugin-slow"), 1500);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    let started = Instant::now();
    match h.query("x") {
        Err(PluginError::Timeout(d)) => {
            assert!(d <= Duration::from_millis(2000));
            assert!(started.elapsed() < Duration::from_millis(4000));
        }
        other => panic!("expected timeout, got {other:?}"),
    }
    // process must be killed by the host after timeout
    h.wait_exit();
}

#[test]
fn crash_plugin_does_not_take_down_host() {
    let (m, base) = manifest(&exe_name("plugin-crash"), 2000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    // crash plugin exits without valid stdout -> malformed / no response, host survives
    let _err = h.query("x");
    // host is still alive and can spawn again
    let (m, base) = manifest(&exe_name("plugin-normal"), 2000);
    let mut h2 = PluginHandle::spawn(m, &base).unwrap();
    assert!(!h2.query("again").unwrap().is_empty());
}

#[test]
fn malformed_response_is_rejected() {
    let (m, base) = manifest(&exe_name("plugin-malformed"), 2000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    assert!(matches!(h.query("x"), Err(PluginError::Malformed(_))));
}

#[test]
fn flood_is_truncated() {
    let (m, base) = manifest(&exe_name("plugin-flood"), 5000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    let cmds = h.query("x").unwrap();
    assert_eq!(cmds.len(), launcher_ipc::MAX_PLUGIN_RESULTS);
}

#[test]
fn cold_vs_warm_plugin_query_timing() {
    // P1 benchmark (PERFORMANCE-CONTRACT): cold = spawn + query, warm =
    // query on the already-running process. Run with --nocapture to read.
    let (m, base) = manifest(&exe_name("plugin-normal"), 5000);

    // cold
    let t0 = Instant::now();
    let mut h = PluginHandle::spawn(m.clone(), &base).unwrap();
    let cmds = h.query("cold").unwrap();
    assert_eq!(cmds.len(), 1);
    let cold_us = t0.elapsed().as_micros();

    // warm x5
    let mut warm_us = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        assert!(!h.query("warm").unwrap().is_empty());
        warm_us.push(t.elapsed().as_micros());
    }
    warm_us.sort();
    eprintln!(
        "PLUGIN COLD (spawn+query): {}us | WARM min/median: {}/{}us",
        cold_us,
        warm_us[0],
        warm_us[warm_us.len() / 2]
    );
    // sanity bounds, not tight gates (CI noise)
    assert!(cold_us < 5_000_000);
    assert!(warm_us[2] < 100_000);
}

#[test]
fn spawn_query_kill_soak_is_stable() {
    let (m, base) = manifest(&exe_name("plugin-normal"), 2000);
    for i in 0..50 {
        let mut h = PluginHandle::spawn(m.clone(), &base).unwrap();
        let cmds = h.query(&format!("q{i}")).unwrap();
        assert_eq!(cmds.len(), 1);
        drop(h); // Drop kills the child; no accumulation
    }
}

#[test]
fn version_negotiation_rejects_unsupported_protocol() {
    let (m, base) = manifest(&exe_name("plugin-badversion"), 2000);
    match PluginHandle::spawn(m, &base) {
        Err(PluginError::VersionMismatch(msg)) => assert!(msg.contains("9.9")),
        Ok(_) => panic!("plugin with unsupported protocol version was accepted"),
        Err(other) => panic!("expected VersionMismatch, got {other}"),
    }
}

#[test]
fn wrong_query_id_echo_is_a_protocol_violation() {
    let (m, base) = manifest(&exe_name("plugin-badecho"), 2000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    match h.query("x") {
        Err(PluginError::Malformed(msg)) => assert!(msg.contains("query_id")),
        Ok(cmds) => panic!("wrong echo accepted: {cmds:?}"),
        Err(other) => panic!("expected Malformed, got {other}"),
    }
}

#[test]
fn manifest_v2_runtime_form_spawns() {
    let m: PluginManifest = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "id": "contract-v2",
        "name": "Contract V2",
        "version": "1.0.0",
        "api_version": "0.1",
        "runtime": { "type": "process", "executable": exe_name("plugin-normal") },
        "capabilities": ["store.read"],
        "timeout_ms": 2000
    }))
    .unwrap();
    assert_eq!(m.effective_executable(), exe_name("plugin-normal"));
    let mut h = PluginHandle::spawn(m, &bin("plugin-normal")).unwrap();
    assert!(h.requests(launcher_domain::Capability::StoreRead));
    let cmds = h.query("v2").unwrap();
    assert_eq!(cmds[0].title, "normal: v2");
}

#[test]
fn graceful_shutdown_lets_plugin_exit_cleanly() {
    let (m, base) = manifest(&exe_name("plugin-normal"), 2000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    h.shutdown();
    // the plugin served `shutdown` and exited by itself (exit code 0);
    // force-kill would also give Some(...) but the serve() loop breaks first
    assert_eq!(h.wait_exit(), Some(0));
}

// --- protocol-boundary contract tests (review 11-mvp2-0.7 §6/§8/§10/§11/§13) ---

#[test]
fn legacy_profile_manifest_tolerates_bare_array() {
    // manifest() has no schema_version -> legacy profile -> flood array OK
    let (m, base) = manifest(&exe_name("plugin-flood"), 5000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    assert_eq!(
        h.query("x").unwrap().len(),
        launcher_ipc::MAX_PLUGIN_RESULTS
    );
}

#[test]
fn v1_manifest_rejects_bare_array_response() {
    // same flood plugin, but a v1 manifest: legacy array = protocol violation
    let m: PluginManifest = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "id": "contract", "name": "Contract",
        "executable": exe_name("plugin-flood"),
        "timeout_ms": 5000
    }))
    .unwrap();
    let mut h = PluginHandle::spawn(m, &bin("plugin-flood")).unwrap();
    match h.query("x") {
        Err(PluginError::Malformed(msg)) => assert!(msg.contains("bare-array")),
        Ok(_) => panic!("v1 manifest accepted a legacy bare-array response"),
        Err(other) => panic!("expected Malformed, got {other}"),
    }
}

#[test]
fn initialize_malformed_response_rejected_at_spawn() {
    let (m, base) = manifest(&exe_name("plugin-initmalformed"), 2000);
    match PluginHandle::spawn(m, &base) {
        Err(PluginError::Malformed(_)) => {}
        Ok(_) => panic!("malformed initialize accepted"),
        Err(other) => panic!("expected Malformed, got {other}"),
    }
}

#[test]
fn stdout_noise_violates_protocol_and_fails_handshake() {
    let (m, base) = manifest(&exe_name("plugin-badstdout"), 2000);
    match PluginHandle::spawn(m, &base) {
        Err(PluginError::Malformed(_)) => {}
        Ok(_) => panic!("plugin printing non-protocol stdout was accepted"),
        Err(other) => panic!("expected Malformed, got {other}"),
    }
}

#[test]
fn oversized_frame_is_a_violation_not_truncated() {
    let (m, base) = manifest(&exe_name("plugin-bigframe"), 2000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    match h.query("x") {
        Err(PluginError::Malformed(msg)) => assert!(msg.contains("frame too large")),
        Ok(_) => panic!("oversized frame accepted"),
        Err(other) => panic!("expected Malformed, got {other}"),
    }
}

#[test]
fn killing_plugin_reaps_whole_process_tree() {
    let (m, base) = manifest(&exe_name("plugin-childspawner"), 2000);
    let mut h = PluginHandle::spawn(m, &base).unwrap();
    let cmds = h.query("spawn").unwrap();
    assert_eq!(cmds[0].title, "child spawned");
    std::thread::sleep(std::time::Duration::from_millis(300)); // let ping.exe start
    assert!(ping_running(), "precondition: ping.exe grandchild alive");
    h.kill();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(
        !ping_running(),
        "grandchild ping.exe must be reaped by the Job Object"
    );
}

fn ping_running() -> bool {
    // grandchild runs as PING.EXE (spawned via cmd /C ping); this and the
    // parent test are Windows-only by nature of the Job Object under test.
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq PING.EXE", "/FO", "CSV", "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("PING.EXE"))
        .unwrap_or(false)
}
