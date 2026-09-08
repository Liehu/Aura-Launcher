//! Benchmark harness (test plan 13 + docs/PERFORMANCE-CONTRACT.md):
//! records perf baselines with environment metadata, latency percentiles
//! (P50/P95/P99/Max), peak-private sampling, and a soak mode with trend
//! detection (initial/min/median/p95/peak/final/slope).
//!
//!   launcher-bench record          write a new baseline (with env metadata)
//!   launcher-bench check           compare vs baseline, exit 1 on >10% regression (P50/P95)
//!   launcher-bench soak [ops]      memory trend soak (default 10_000 queries)
//!   launcher-bench (default)       print results

use std::time::Instant;

use launcher_core::{Core, Provider};
use launcher_domain::{Action, ActionKind, Category, Command, QueryContext};
use launcher_indexer::Indexer;

struct BenchProvider {
    id: String,
    commands: Vec<Command>,
}

impl Provider for BenchProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if q.normalized.is_empty() {
            Vec::new()
        } else {
            self.commands.clone()
        }
    }
}

fn percentile(mut v: Vec<u64>, p: f64) -> u64 {
    v.sort();
    let idx = ((v.len() as f64 - 1.0) * p).round() as usize;
    v.get(idx).copied().unwrap_or_default()
}

fn median(v: Vec<u64>) -> u64 {
    percentile(v, 0.5)
}

/// Current private (commit) bytes of this process, Windows only.
/// 0 when unavailable (non-Windows CI) — callers must treat 0 as "unknown".
#[cfg(windows)]
fn private_bytes() -> u64 {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    unsafe {
        let mut pmc = PROCESS_MEMORY_COUNTERS_EX {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        let ok = GetProcessMemoryInfo(
            windows::Win32::Foundation::HANDLE(-1isize as *mut _), // GetCurrentProcess() pseudo-handle
            &mut pmc as *mut _
                as *mut windows::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS,
            pmc.cb,
        );
        if ok.is_ok() {
            pmc.PrivateUsage as u64
        } else {
            0
        }
    }
}

#[cfg(not(windows))]
fn private_bytes() -> u64 {
    0
}

fn build_core(file_count: usize) -> (Core, std::path::PathBuf) {
    let tmp = std::env::temp_dir().join(format!("launcher-bench-{}", std::process::id()));
    let root = tmp.join("data");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for i in 0..file_count {
        std::fs::write(root.join(format!("document{i:05}.txt")), "x").unwrap();
    }
    // disk-backed index: matches production (real app idle 12.6MB @ 214k
    // files). In-memory SQLite would pin the whole DB in RAM and distort the
    // memory scale curve.
    let mut indexer = Indexer::open(&tmp.join("index.db")).unwrap();
    indexer.rebuild(std::slice::from_ref(&root)).unwrap();

    let mut core = Core::new();
    core.register(Box::new(BenchProvider {
        id: "apps".into(),
        commands: (0..200)
            .map(|i| Command {
                id: format!("app{i}"),
                title: format!("Application Number {i}"),
                subtitle: None,
                icon: None,
                provider_id: "apps".into(),
                score: 0.0,
                keywords: vec![],
                category: Category::Application,
                actions: vec![Action {
                    kind: ActionKind::Open,
                    payload: None,

                    id: None,
                    title: None,
                    disabled_reason: None,
                    shortcut: None,
                    confirmation_required: false,
                }],
                target: None,
            })
            .collect(),
    }));
    core.register(Box::new(launcher_core::providers::file::FileProvider::new(
        indexer,
    )));
    (core, tmp)
}

fn environment() -> serde_json::Value {
    // git commit when available; "nogit" otherwise (this repo is not a checkout)
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "nogit".into());
    serde_json::json!({
        "os": std::env::var("OS").unwrap_or_else(|_| std::cfg!(windows).then(|| "Windows").unwrap_or("unknown").to_string()),
        "cpu": std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unknown".into()),
        "build": "release",
        "commit": commit,
        "bench_version": 1
    })
}

/// MVP3.1 P1 performance gate (review 19): Action Panel latency for the
/// non-UI cores — presentation projection, enabled-only selection navigation,
/// and the id-based execute bridge. Slint paint time is excluded (it is
/// covered by the in-app popup.latency / T2 tracing).
fn action_panel_bench() -> serde_json::Value {
    use launcher_ui::to_action_presentations;

    // selected command with 50 actions, half capability-disabled (worst case
    // for skip-disabled navigation)
    let actions: Vec<Action> = (0..50)
        .map(|i| Action {
            kind: if i % 3 == 0 {
                ActionKind::Open
            } else {
                ActionKind::Copy
            },
            payload: None,
            id: Some(format!("action-{i}")),
            title: Some(format!("Action {i}")),
            disabled_reason: (i % 2 == 1).then(|| "requires clipboard.write".to_string()),
            shortcut: None,
            confirmation_required: false,
        })
        .collect();
    let cmd = Command {
        id: "calc.plus:= 80".into(),
        title: "= 80".into(),
        subtitle: Some("12+34*2".into()),
        icon: None,
        provider_id: "plugin:calc.plus".into(),
        score: 0.0,
        keywords: vec![],
        category: Category::Plugin,
        actions,
        target: None,
    };
    let results: Vec<Command> = (0..50)
        .map(|i| {
            let mut c = cmd.clone();
            c.id = format!("calc.plus:={i}");
            c
        })
        .collect();

    let rounds = 10_000u32;
    let mut open_us = Vec::new();
    let mut nav_us = Vec::new();
    let mut bridge_us = Vec::new();
    for r in 0..rounds {
        // 1) panel open: projection of the selected command's actions
        let t = Instant::now();
        let presentations = to_action_presentations(&cmd);
        open_us.push(t.elapsed().as_micros() as u64);

        // 2) selection navigation: move to the next enabled row (skip
        //    disabled), wrapping logic identical to launcher-app on_panel_nav
        let t = Instant::now();
        let mut sel = (r as usize) % presentations.len();
        loop {
            sel = (sel + 1) % presentations.len();
            if presentations[sel].enabled {
                break;
            }
        }
        nav_us.push(t.elapsed().as_micros() as u64);

        // 3) execute bridge: (command_id, action_id) resolution against the
        //    visible result set, then engine validation gate
        let t = Instant::now();
        let cmd_id = format!("calc.plus:={}", r % 50);
        let action_id = format!("action-{}", r % 50);
        let resolved = results.iter().find(|c| c.id == cmd_id).and_then(|c| {
            c.actions
                .iter()
                .find(|a| a.id.as_deref() == Some(action_id.as_str()))
        });
        if let Some(a) = resolved {
            let _ = launcher_action::validate(a);
        }
        bridge_us.push(t.elapsed().as_micros() as u64);
    }

    let stats = |v: Vec<u64>| {
        serde_json::json!({
            "p50": percentile(v.clone(), 0.5),
            "p95": percentile(v.clone(), 0.95),
            "p99": percentile(v.clone(), 0.99),
            "max": v.iter().copied().max().unwrap_or(0),
        })
    };
    serde_json::json!({
        "note": "non-UI cores of the Action Panel (paint excluded); iterations=10000; command with 50 actions, half disabled",
        "panel_open_us": stats(open_us),
        "selection_nav_us": stats(nav_us),
        "execute_bridge_us": stats(bridge_us),
    })
}

fn run_benchmarks(file_count: usize) -> serde_json::Value {
    let queries_app = ["application", "number", "app1", "zzz-nothing"];
    let queries_file = ["document", "doc00", "zzz-nothing"];

    let (mut core, tmp) = build_core(file_count);

    // cold start: the very first query after core construction
    let cold_start = {
        let t = Instant::now();
        let _ = core.search(queries_app[0], 50);
        t.elapsed()
    };

    let idle_private = private_bytes();
    let mut peak_private = idle_private;

    let mut sample = |queries: &[&str], rounds: usize| -> (u64, u64, u64, u64) {
        let mut samples: Vec<u64> = Vec::new();
        for _ in 0..rounds {
            for q in queries {
                let t = Instant::now();
                let _ = core.search(q, 50);
                samples.push(t.elapsed().as_micros() as u64);
            }
            peak_private = peak_private.max(private_bytes());
        }
        (
            percentile(samples.clone(), 0.5),
            percentile(samples.clone(), 0.95),
            percentile(samples.clone(), 0.99),
            samples.iter().copied().max().unwrap_or(0),
        )
    };

    let (app_p50, app_p95, app_p99, app_max) = sample(&queries_app, 200);
    let (file_p50, file_p95, file_p99, file_max) = sample(&queries_file, 200);
    let action_panel = action_panel_bench();
    let _ = std::fs::remove_dir_all(&tmp);

    serde_json::json!({
        "version": 1,
        "timestamp_unix_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        "environment": environment(),
        "dataset": {
            "files": file_count,
            "apps": 200
        },
        "cold_start_us": cold_start.as_micros() as u64,
        "memory": {
            "idle_private_bytes": idle_private,
            "search_peak_private_bytes": peak_private,
            "note": "private = commit charge via GetProcessMemoryInfo(PrivateUsage); 0 = unavailable"
        },
        "search_app_us": { "p50": app_p50, "p95": app_p95, "p99": app_p99, "max": app_max },
        "search_file_us": { "p50": file_p50, "p95": file_p95, "p99": file_p99, "max": file_max },
        "action_panel_us": action_panel,
        "targets": {
            "search_app_us": { "p50": 5000, "p95": 10000 },
            "search_file_us": { "p50": 20000, "p95": 50000 },
            "action_panel_us": { "panel_open_p95": 10000, "selection_p95": 5000 }
        }
    })
}

/// Memory-trend soak (docs/PERFORMANCE-CONTRACT.md §soak): repeated
/// queries with periodic private-byte sampling; reports the full trend
/// signature and flags a monotonic-growth slope ("memory staircase").
fn run_soak(ops: usize) -> anyhow::Result<()> {
    let queries = ["application", "number", "app1", "zzz", "document", "doc00"];
    let (mut core, tmp) = build_core(10_000);
    let sample_every = (ops / 20).max(1);
    let mut samples: Vec<u64> = Vec::new();

    let started = Instant::now();
    let mut done = 0usize;
    while done < ops {
        for q in queries {
            let _ = core.search(q, 50);
            done += 1;
        }
        if done % sample_every < queries.len() {
            let b = private_bytes();
            if b > 0 {
                samples.push(b);
            }
        }
    }
    let elapsed = started.elapsed();
    let _ = std::fs::remove_dir_all(&tmp);

    let initial = samples.first().copied().unwrap_or(0);
    let final_ = samples.last().copied().unwrap_or(0);
    let peak = samples.iter().copied().max().unwrap_or(0);
    let min = samples.iter().copied().min().unwrap_or(0);
    let med = median(samples.clone());
    let p95 = percentile(samples.clone(), 0.95);
    // slope: bytes per operation over the whole run
    let slope_b_per_op = if done > 0 && final_ >= initial {
        (final_ - initial) as f64 / done as f64
    } else {
        (final_ as f64 - initial as f64) / done as f64
    };
    // staircase detection: growth between consecutive sampled thirds
    let third = samples.len() / 3;
    let growth_first_to_last_third = if third > 0 {
        samples[samples.len() - 1] as i64 - samples[third - 1] as i64
    } else {
        0
    };
    // structured verdict (08-mvp2-0.4 §7): booleans are not mutually
    // exclusive; `pass` aggregates the gated checks. `slope_bytes_per_op` is
    // diagnostic only (a fixed 1KB/op threshold means different things at
    // 1k plugin cycles vs 100k queries).
    let staircase_suspected = growth_first_to_last_third > 5 * 1024 * 1024;
    let final_within_20pct = final_ as f64 <= initial as f64 * 1.2;
    let peak_to_final_bytes = peak as i64 - final_ as i64;
    let peak_to_final_ratio = if peak > 0 {
        ((final_ as f64 / peak as f64) * 10000.0).round() / 10000.0
    } else {
        0.0
    };
    let pass = !staircase_suspected && final_within_20pct;
    let report = serde_json::json!({
        "version": 2,
        "operations": done,
        "elapsed_ms": elapsed.as_millis() as u64,
        "sample_count": samples.len(),
        "memory_private_bytes": {
            "initial": initial,
            "min": min,
            "median": med,
            "p95": p95,
            "peak": peak,
            "final": final_
        },
        "peak_to_final_bytes": peak_to_final_bytes,
        "peak_to_final_ratio": peak_to_final_ratio,
        "slope_bytes_per_op": { "value": (slope_b_per_op * 100.0).round() / 100.0, "role": "diagnostic" },
        "last_third_growth_bytes": growth_first_to_last_third,
        "verdict": {
            "pass": pass,
            "staircase_suspected": staircase_suspected,
            "final_within_20pct": final_within_20pct
        }
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::create_dir_all("benchmarks")?;
    std::fs::write(
        "benchmarks/soak-latest.json",
        serde_json::to_string_pretty(&report)?,
    )?;
    println!("soak written to benchmarks/soak-latest.json");
    Ok(())
}

/// Memory attribution (07-mvp2-0.3 §6): build the core incrementally and
/// report private bytes after each stage, so future features can quote
/// "FTS likely +X MB" against measured per-component costs.
fn run_attribution() -> anyhow::Result<()> {
    let tmp = std::env::temp_dir().join(format!("launcher-attr-{}", std::process::id()));
    let root = tmp.join("data");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)?;
    for i in 0..1000 {
        std::fs::write(root.join(format!("document{i:05}.txt")), "x")?;
    }

    let mut stages: Vec<(&str, u64)> = Vec::new();
    let mut core = Core::new();
    stages.push(("empty_core", private_bytes()));

    // + app-style provider (200 in-memory commands)
    core.register(Box::new(BenchProvider {
        id: "apps".into(),
        commands: (0..200)
            .map(|i| Command {
                id: format!("app{i}"),
                title: format!("Application Number {i}"),
                subtitle: None,
                icon: None,
                provider_id: "apps".into(),
                score: 0.0,
                keywords: vec![],
                category: Category::Application,
                actions: vec![Action {
                    kind: ActionKind::Open,
                    payload: None,

                    id: None,
                    title: None,
                    disabled_reason: None,
                    shortcut: None,
                    confirmation_required: false,
                }],
                target: None,
            })
            .collect(),
    }));
    let _ = core.search("application", 50); // touch the catalog
    stages.push(("plus_app_provider", private_bytes()));

    // + file provider backed by SQLite index of 1000 files
    let mut indexer = Indexer::in_memory()?;
    indexer.rebuild(std::slice::from_ref(&root))?;
    core.register(Box::new(launcher_core::providers::file::FileProvider::new(
        indexer,
    )));
    let _ = core.search("document", 50);
    stages.push(("plus_file_provider_1k", private_bytes()));

    // + bounded history sink
    core.set_history(launcher_indexer::Indexer::in_memory()?);
    core.record_use("app1", "apps");
    stages.push(("plus_history", private_bytes()));

    let _ = std::fs::remove_dir_all(&tmp);

    let mut rows = Vec::new();
    for i in 0..stages.len() {
        let (name, bytes) = stages[i];
        let delta = if i > 0 {
            bytes as i64 - stages[i - 1].1 as i64
        } else {
            0
        };
        rows.push(serde_json::json!({
            "stage": name,
            "private_bytes": bytes,
            "delta_from_prev_bytes": delta
        }));
    }
    let report = serde_json::json!({ "version": 1, "stages": rows });
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::create_dir_all("benchmarks")?;
    std::fs::write(
        "benchmarks/memory-attribution.json",
        serde_json::to_string_pretty(&report)?,
    )?;
    println!("attribution written to benchmarks/memory-attribution.json");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "print".into());
    if mode == "attribution" {
        return run_attribution();
    }
    if mode == "soak" {
        let ops: usize = std::env::args()
            .nth(2)
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000);
        return run_soak(ops);
    }
    if mode == "scale" {
        // scale curve (P1): latency + memory vs dataset size
        let mut rows = Vec::new();
        for files in [10_000usize, 100_000, 250_000, 1_000_000] {
            let r = run_benchmarks(files);
            rows.push(serde_json::json!({
                "files": files,
                "cold_start_us": r["cold_start_us"],
                "app_p95_us": r["search_app_us"]["p95"],
                "file_p50_us": r["search_file_us"]["p50"],
                "file_p95_us": r["search_file_us"]["p95"],
                "file_max_us": r["search_file_us"]["max"],
                "idle_private_bytes": r["memory"]["idle_private_bytes"],
                "search_peak_private_bytes": r["memory"]["search_peak_private_bytes"]
            }));
        }
        let report = serde_json::json!({ "version": 1, "scale_curve": rows });
        println!("{}", serde_json::to_string_pretty(&report)?);
        std::fs::create_dir_all("benchmarks")?;
        std::fs::write(
            "benchmarks/scale-curve.json",
            serde_json::to_string_pretty(&report)?,
        )?;
        println!("scale curve written to benchmarks/scale-curve.json");
        return Ok(());
    }

    let file_count: usize = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let result = run_benchmarks(file_count);
    println!("{}", serde_json::to_string_pretty(&result)?);

    let baseline_path = std::path::PathBuf::from("benchmarks/baseline.json");
    match mode.as_str() {
        "record" => {
            std::fs::create_dir_all(baseline_path.parent().unwrap())?;
            std::fs::write(&baseline_path, serde_json::to_string_pretty(&result)?)?;
            println!("baseline written to {}", baseline_path.display());
        }
        "check" => {
            let baseline: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&baseline_path)?)?;
            let mut fail = false;
            // Two distinct gates (08-mvp2-0.4 §6):
            // 1. Regression Gate — vs baseline, P50/P95 only,
            //    relative >10% AND absolute >100us (sub-ms noise guard)
            // 2. Budget Gate (absolute SLO) — independent of baseline
            // P99/Max are tail observations, gated by neither.
            for key in ["search_app_us", "search_file_us"] {
                for p in ["p50", "p95"] {
                    let base = baseline[key][p].as_u64().unwrap_or(0);
                    let cur = result[key][p].as_u64().unwrap_or(0);
                    if base > 0 && cur as f64 > base as f64 * 1.10 && cur.saturating_sub(base) > 100
                    {
                        eprintln!(
                            "REGRESSION {key}.{p}: baseline {base}us, current {cur}us (>10% and >100us)"
                        );
                        fail = true;
                    }
                }
            }
            let slo = [
                ("search_app_us.p95", 10_000u64),
                ("search_file_us.p95", 50_000u64),
            ];
            for (key, budget) in slo {
                let (k, p) = key.split_once('.').unwrap();
                let cur = result[k][p].as_u64().unwrap_or(0);
                if cur > budget {
                    eprintln!("SLO MISS {key}: current {cur}us > budget {budget}us");
                    fail = true;
                }
            }
            if fail {
                std::process::exit(1);
            }
            println!("no regression vs baseline; SLO met");
        }
        _ => {
            let app_p95 = result["search_app_us"]["p95"].as_u64().unwrap_or(u64::MAX);
            let file_p95 = result["search_file_us"]["p95"].as_u64().unwrap_or(u64::MAX);
            println!(
                "target check: app p95 <= 10ms: {}, file p95 <= 50ms: {}",
                app_p95 <= 10_000,
                file_p95 <= 50_000
            );
        }
    }
    Ok(())
}
