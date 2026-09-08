# PERFORMANCE.md

## Recorded baseline (benchmarks/baseline.json)

Recorded 2026-09-03, debug harness on dev machine, 200 app catalog entries +
1000 indexed files:

| metric | p50 | p95 | design target |
|---|---|---|---|
| app search | 0.70 ms | 0.88 ms | 5 ms / 10 ms |
| file search | 0.77 ms | 1.17 ms | 20 ms / 50 ms |

## Idle memory (release build)

| build | private bytes | budget |
|---|---|---|
| femtovg GPU renderer (default Slint features) | 72.4 MB | ≤ 80 MB hard |
| **renderer-software + tray/config/recent providers (current)** | **12.6 MB** | ≤ 50 MB target ✓ |

口径：**Launcher Idle Private Bytes**（release / software renderer / 21 万条索引已打开 /
特定 provider 集 / 本机 Windows）——不是"框架内存占用"的通用常数。

The GPU renderer + glutin/resvg stack cost ~63 MB; switching to the Slint
software renderer (same change demo1 validated) reaches the 50 MB target with
large margin. Debug builds are not representative (~110 MB) — always measure
release.

## How to run

```bash
cargo run -p launcher-bench          # print + target check
cargo run -p launcher-bench record   # rewrite baseline
cargo run -p launcher-bench check    # >10% regression => exit 1
```

Rules (test plan 13): >5% regression = warning, >10% = fail unless the PR
updates the baseline with justification.
