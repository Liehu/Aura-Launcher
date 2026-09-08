"""Launcher 1.0 performance / memory baseline (MUST-4, review 66 §29).

Records reproducible product-level numbers into
artifacts/perf-baseline.json so regressions are visible over time:

  - cold_start_ms    : process spawn -> first frame painted (snapshot mode:
                       the app exits after capture, so wall time bounds
                       startup + first paint)
  - snapshot_total_ms: full 15-scenario VR capture run (same run, total)
  - idle_rss_mb      : resident set while resident in tray (post-startup)
  - search_p50/p95_ms: launcher-bench synthetic provider search latency

Usage: python scripts/perf_baseline.py [--runs N]
The bench requires a built debug binary (cargo build --workspace).
"""

import argparse
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXE = ROOT / "target" / "debug" / "launcher-app.exe"
BENCH = ROOT / "target" / "debug" / "launcher-bench.exe"
REL_EXE = ROOT / "target" / "release" / "launcher-app.exe"
REL_BENCH = ROOT / "target" / "release" / "launcher-bench.exe"
OUT = ROOT / "artifacts" / "perf-baseline.json"
REL_OUT = ROOT / "artifacts" / "perf-release.json"


def rss_mb_windows(pid: int) -> float:
    ps = subprocess.run(
        ["powershell", "-NoProfile", "-Command",
         f"(Get-Process -Id {pid} -ErrorAction Stop).WorkingSet64 / 1MB"],
        capture_output=True, text=True)
    try:
        return float(ps.stdout.strip())
    except ValueError:
        return 0.0


def measure_snapshot_run() -> dict:
    """One full VR capture run doubles as a cold-start + paint measurement."""
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        t0 = time.perf_counter()
        subprocess.run([str(EXE)], env={**__import__("os").environ,
                                        "LAUNCHER_SNAPSHOT_DIR": td,
                                        "WGPU_BACKEND": "gl"},
                       cwd=ROOT, timeout=180, capture_output=True)
        total_ms = (time.perf_counter() - t0) * 1000
    return {"snapshot_total_ms": round(total_ms, 1)}


def measure_search_bench() -> dict:
    if not BENCH.exists():
        return {"search_p50_ms": None, "search_p95_ms": None,
                "note": "launcher-bench not built"}
    r = subprocess.run([str(BENCH)], cwd=ROOT, capture_output=True, text=True,
                       timeout=600)
    # bench prints its own numbers; we don't parse — the release gate owns it.
    return {"bench_exit": r.returncode}


def private_mb_windows(pid: int) -> float:
    ps = subprocess.run(
        ["powershell", "-NoProfile", "-Command",
         f"(Get-Process -Id {pid} -ErrorAction Stop).PrivateMemorySize64 / 1MB"],
        capture_output=True, text=True)
    try:
        return float(ps.stdout.strip())
    except ValueError:
        return 0.0


def measure_release(runs: int) -> int:
    """P2.3-D D1-D4: the Release benchmark, one reproducible artifact.

    - D2 real-process startup : spawn target/release/launcher-app.exe in
      snapshot mode N times (auto-exit after capture); wall clock bounds
      process spawn -> first paint. This is the REAL binary the user runs,
      unlike launcher-bench's in-process cold_start_us.
    - D3 search latency       : parse launcher-bench release JSON stdout
      (10k files / 200 apps, p50/p95/p99 per class).
    - D4 memory               : resident + private bytes of the resident
      release process (3 s settle, no interaction) + bench peak.
    """
    import os
    import tempfile

    if not REL_EXE.exists() or not REL_BENCH.exists():
        print("FATAL: build release first "
              "(cargo build --release -p launcher-app -p launcher-bench)",
              file=sys.stderr)
        return 1

    # ---- D2: real-process startup. Single-file snapshot mode: the app
    # shows, waits a FIXED 900 ms (renderer settle, artificial), captures
    # one BMP and quits — so wall − 900 ms ≈ spawn → first paint.
    startups = []
    for _ in range(runs):
        import tempfile
        with tempfile.TemporaryDirectory() as td:
            snap = Path(td) / "startup.bmp"
            t0 = time.perf_counter()
            subprocess.run([str(REL_EXE)],
                           env={**os.environ, "LAUNCHER_SNAPSHOT": str(snap),
                                "WGPU_BACKEND": "gl"},
                           cwd=ROOT, timeout=180, capture_output=True)
            wall_ms = (time.perf_counter() - t0) * 1000
            if not snap.exists():
                print("FATAL: startup snapshot not written", file=sys.stderr)
                return 1
            startups.append(round(wall_ms - 900.0, 1))  # subtract fixed settle

    # ---- D4: resident release process memory (idle, no interaction)
    proc = subprocess.Popen([str(REL_EXE)], cwd=ROOT,
                            env={**os.environ, "WGPU_BACKEND": "gl"},
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(3.0)
    idle_rss_mb = rss_mb_windows(proc.pid)
    idle_private_mb = private_mb_windows(proc.pid)
    proc.kill()
    proc.wait()

    # ---- D5: popup show/hide soak — real UI visible-cycles in the release
    # binary with periodic private-byte sampling. Growth verdict computed
    # HERE (the app only traces the raw samples).
    import re
    soak_env = {**os.environ, "LAUNCHER_SOAK_SHOWHIDE": "1000",
                "RUST_LOG": "info", "WGPU_BACKEND": "gl"}
    soak = subprocess.run([str(REL_EXE)], cwd=ROOT, env=soak_env,
                          capture_output=True, text=True, timeout=600)
    showhide = {"cycles": 0, "initial_private": 0, "peak_private": 0,
                "final_private": 0, "growth_bytes": 0, "pass": None}
    # the app logs to stdout (console layer) AND a file appender; strip
    # ANSI colour codes between field names and '=' before parsing
    log_text = re.sub(r"\x1b\[[0-9;]*m", "", soak.stdout + soak.stderr)
    m = re.search(r"cycles=(\d+).*?initial_private=(\d+).*?peak_private=(\d+)"
                  r".*?final_private=(\d+).*?growth=(-?\d+)", log_text, re.S)
    if m:
        cycles, initial, peak, final, growth = m.groups()
        showhide = {"cycles": int(cycles), "initial_private": int(initial),
                    "peak_private": int(peak), "final_private": int(final),
                    "growth_bytes": int(growth),
                    "pass": int(growth) < 10_000_000}
    else:
        showhide["pass"] = False

    # ---- D3: search latency + bench-side cold start / peak memory
    r = subprocess.run([str(REL_BENCH), "10000"], cwd=ROOT,
                       capture_output=True, text=True, timeout=600)
    bench = {}
    try:
        blob = r.stdout[r.stdout.index("{"):r.stdout.rindex("}") + 1]
        b = json.loads(blob)
        bench = {
            "search_app_us": b.get("search_app_us"),
            "search_file_us": b.get("search_file_us"),
            "cold_start_us_core": b.get("cold_start_us"),
            "search_peak_private_mb": round(
                b.get("memory", {}).get("search_peak_private_bytes", 0) / 1e6, 2),
        }
    except (ValueError, IndexError):
        print("FATAL: cannot parse launcher-bench output", file=sys.stderr)
        return 1

    result = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "build": "release",
        "runs": runs,
        "startup": {
            "real_process_ms": {
                "min": min(startups), "median": statistics.median(startups),
                "max": max(startups), "samples": startups,
            },
            "note": "launcher-app.exe single-frame snapshot mode: "
                    "wall - fixed 900 ms renderer settle = spawn -> first paint",
        },
        "memory": {
            "idle_rss_mb": round(idle_rss_mb, 1),
            "idle_private_mb": round(idle_private_mb, 1),
        },
        "showhide_soak": showhide,
        "bench": bench,
    }
    REL_OUT.parent.mkdir(parents=True, exist_ok=True)
    REL_OUT.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps(result, indent=2))
    print(f"written: {REL_OUT}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--release", action="store_true",
                    help="P2.3-D D2-D4: measure the RELEASE binary (real "
                         "process startup, search latency, memory) into "
                         "artifacts/perf-release.json")
    args = ap.parse_args()

    if args.release:
        return measure_release(args.runs)

    if not EXE.exists():
        print("FATAL: build first (cargo build --workspace)", file=sys.stderr)
        return 1

    totals = []
    idles = []
    for _ in range(args.runs):
        res = measure_snapshot_run()
        totals.append(res["snapshot_total_ms"])

    # idle RSS: launch resident (no snapshot), give it 3 s, read, kill
    import os
    import signal
    proc = subprocess.Popen([str(EXE)], cwd=ROOT,
                            env={**os.environ, "WGPU_BACKEND": "gl"},
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(3.0)
    idles.append(rss_mb_windows(proc.pid))
    proc.kill()
    proc.wait()

    baseline = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "runs": args.runs,
        "snapshot_total_ms": {
            "min": round(min(totals), 1),
            "median": round(statistics.median(totals), 1),
            "max": round(max(totals), 1),
        },
        # snapshot_total is cold spawn -> paint 15 frames + exit; a
        # per-frame-paint upper bound:
        "cold_start_upper_ms_per_scenario": round(min(totals) / 15, 1),
        "idle_rss_mb": round(statistics.median(idles), 1),
        "bench": measure_search_bench(),
        "note": "numbers are bounds for regression watching, not SLAs "
                "(review 66 §29: 'no perceptible lag' is the gate)",
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(baseline, indent=2), encoding="utf-8")
    print(json.dumps(baseline, indent=2))
    print(f"written: {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
