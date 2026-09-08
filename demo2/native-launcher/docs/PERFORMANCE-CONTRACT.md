# PERFORMANCE-CONTRACT.md — 性能契约（v3，采纳 06/07/08 评审后冻结版）

所有对外报告的性能数字必须来自本文件定义的测量方法；禁止凭感觉报告。

## 指标定义

### Memory（Windows）
| 指标 | 采集方式 | 定位 |
|---|---|---|
| Private Bytes | `GetProcessMemoryInfo` → `PrivateUsage`（commit charge） | **主预算指标** |
| Working Set | `Process.WorkingSet64` | 用户可见即时 RAM 参考 |
| Peak Private | 同 Private 口径，运行期间最大值 | 瞬时峰值（search/plugin/popup 场景） |
| ~~Commit~~ | **v0.2 起撤销为正式指标**：`PagedMemorySize64` ≠ Windows Commit，宁缺勿错 | 如需恢复须用 ETW/Performance Counter 单独定义 |

采样时机：进程启动后 ≥ 60s、窗口隐藏、无插件运行、索引已建立；连续 3 次采样偏差 < 5% 才可记录。

### Plugin 进程
- 正确口径是 **`plugin_process_count`**（进程是否存在）而非 "Plugin Host Private = 0"——"进程不存在"与"进程存在但报 0 MB"不是一回事
- 附加计数：`plugin_spawn_count` / `plugin_kill_count` / `orphan_process_count`（引入 Python/Node 宿主后用于验证整个进程树清理）

### CPU
| 指标 | 预算 |
|---|---|
| Idle CPU%（60s 窗口平均，隐藏态） | ≤ 0.1% |
| Search / Indexing CPU | 记录，不设预算 |

### Latency（v2：拆分应用内延迟与用户感知延迟）
```text
T0: 物理按键 → Windows 热键分发（进程外，OS 调度）
T1: 热键分发（hotkey 线程 recv）→ show 请求提交      [应用内，已埋点: popup.latency t1_dispatch_to_show_us]
T2: show 请求 → show 调用返回（注意：≠ first frame）  [应用内，已埋点: t2_show_call_us]
T3: 物理按键 → first presented frame                 [用户感知，P2 measurement]
```
**概念修正（08-mvp2-0.4 §1）**：`T1 + T2` 描述的是"从热键接收到 show() 返回"的进程内延迟，
是 T3 的**不完整内部代理 / 下界参考**——first frame 还要经过 render → compositor →
present → display。**T1+T2 不得作为 T3 的上界**，更不得当作 T3 报告。

T3 定义与测量工具解耦：定义是 *Physical Input → First Presented Frame*；测量手段为
ETW / PresentMon / 兼容的 frame-present instrumentation，高速摄像仅作一次性的
instrumentation validation，不作为长期 benchmark。

| Benchmark | 定义 | 预算 |
|---|---|---|
| Cold Popup（T1+T2，首次 show 含窗口创建） | `popup.latency` cold=true | 实测 T1 0.16ms / T2 20.2ms；目标 ≤ 35ms |
| Warm Popup（复显） | `popup.latency` cold=false | 目标 P50 ≤ 5ms / P95 ≤ 10ms（待批量采样） |
| T3 Physical → First Frame | 待外部测量手段（ETW/高速摄像） | ≤ 35 ms（设计目标） |
| Query → Result（app/file） | `Core::search` 全程 | ≤ 10ms / ≤ 50ms |
| Enter → Effect | execute() → Spawn 返回（待埋点） | ≤ 50 ms |
| Plugin cold / warm query | spawn→首条结果 / 已运行→首条结果（P1 待入 bench，两数差异大必须分开报） | TBD |
| Cold start query | core 构建后首次查询（另有 Warm/Repeated/Different Query 区分，P1） | 记录 |

### User-facing SLO（唯一的产品门禁，百分位统一标注）

| 指标 | SLO |
|---|---|
| Cold Popup | **P95**(T1+T2) ≤ 35 ms |
| Warm Popup | P50 ≤ 5 ms，**P95** ≤ 10 ms |
| App Search | **P95** ≤ 10 ms |
| File Search | **P95** ≤ 50 ms |
| Enter → Effect | **P95** ≤ 50 ms（待埋点） |

### Diagnostic（不作为 UX 门禁）

```text
T1 / T2 / T3   —— T1/T2 已埋点（诊断分解），T3 为 P2 measurement
P50 / P95      —— Regression Gate 适用
P99 / Max      —— 尾部观测；Max 用于发现"平均漂亮但偶发卡顿"
```

### 两种 Gate 的区分（08-mvp2-0.4 §6）

- **Regression Gate**（对 baseline）：P50/P95，相对 >10% **且** 绝对 >100µs
- **Budget Gate**（绝对 SLO）：与 baseline 无关，如 baseline=9ms/budget=10ms 时
  9.5ms 无 regression 但已需关注趋势；baseline=1ms 涨到 1.2ms（+20%）虽远低于预算
  也必须触发 regression

### 采样量（08-mvp2-0.4 §5）

```text
Microbenchmark（P50 参考）: N = 50/100
Latency gate（P95 门禁）:   N ≥ 200   ← bench 当前取值
Tail analysis（P99/Max）:   N ≥ 1000
```

## Soak / 内存趋势（v2：趋势检测而非终点对比）

仅比较 final vs initial 会漏掉中间尖峰（12→300→14 MB 也可能"通过"）。soak 必须输出完整趋势签名（`launcher-bench soak [ops]` 自动生成 `benchmarks/soak-latest.json`）：

```text
initial / min / median / p95 / peak / final / slope_bytes_per_op / last_third_growth_bytes / peak_to_final_bytes
verdict: staircase_suspected | final_within_20pct | slope_under_1kb_per_op

`peak_to_final_bytes` 区分 temporary allocation 与 retained memory：peak=80MB→final=13MB
是临时分配（无害）；peak=80MB→final=35MB 则值得调查。
```

瞬时峰值场景矩阵（人工/半自动，P1 自动化）：

| 场景 | Peak Private | After settle |
|---|---|---|
| Idle | 记录 | 记录 |
| App / File search | bench 采集 | 记录 |
| Plugin query | 待埋点 | 记录 |
| Popup show/hide ×1000 | 待埋点 | 记录 |

规模计划（P1）：10k show/hide、100k query、1k plugin cycle；数据规模曲线 10K/100K/250K/1M/5M files（latency + memory 双曲线——核心卖点是"规模增大后 Launcher 本身仍然轻"）。

## 基线契约（baseline.json v1）

baseline 不再只是数字，带环境元数据（不同机器/硬件/数据规模的数字不可直接比较）：

```json
{ "version": 1,
  "environment": { "os", "cpu", "build", "commit", "bench_version" },
  "dataset": { "files", "apps" },
  "memory": { "idle_private_bytes", "search_peak_private_bytes" },
  "search_app_us": { "p50", "p95", "p99", "max" },
  "search_file_us": { "p50", "p95", "p99", "max" } }
```

## 性能规范分级（User SLO 与 Diagnostic 分离，08-mvp2-0.4 §3/§11）

```text
P0 User SLO:      Cold Popup P95 / Warm Popup P50+P95 / App+File Search P95 / Enter P95
P0 Memory:        Idle Private / Idle Working Set / Peak Private / Search Peak
P1 Diagnostic:    T1 / T2 / T3（分层分解，非门禁）
P1 Plugin:        Cold Start / Warm Query / Shutdown / Process Tree Cleanup
P1 Scale:         10K → 100K → 250K → 1M → 5M files（latency + memory 曲线）
P1 Stability:     10K show/hide / 100K query / 1K plugin
P1 State Matrix:  Hidden Idle / Visible Idle / Searching / Plugin Running（各配 memory+CPU 基线；
                  Visible Idle CPU ≤ 0.1% 比 Hidden Idle 更值得关注）
P2 Measurement:   T3 physical → first presented frame（PresentMon/ETW）
P2 Diag:          Handles / Threads / GPU / ETW
```

## Soak verdict（结构化 boolean，非互斥）

soak 报告 verdict 为结构化结果，`pass` 聚合门禁项；`slope_bytes_per_op` 为诊断值
不设统一硬门槛（1KB/op 在 100k query=100MB 不可接受，在 1k plugin cycle=1MB 或许可接受，
阈值应按 scenario 配置）：

```json
{ "verdict": { "pass": true, "staircase_suspected": false, "final_within_20pct": true } }
{ "peak_to_final_bytes": ..., "peak_to_final_ratio": 0.1625 }
```

## 测量工具与流程

1. `cargo run --release -p launcher-bench`（release 构建；debug 数字无效；每桶 50 轮）
2. `record` 写 baseline；`check` 门禁 P50/P95 **相对 >10% 且绝对 >100µs** → exit 1（亚毫秒指标的纯相对门禁会被调度噪声误报，实测 299→333µs；P99/Max 记录不门禁）
3. `soak [ops]` 输出趋势签名，`staircase_suspected=true` 即失败
4. `attribution` 输出逐组件内存归因矩阵（benchmarks/memory-attribution.json）
5. UI show/hide soak：`LAUNCHER_SOAK_SHOWHIDE=N cargo run -p launcher-app`，日志 `showhide.soak.report`
6. 内存人工采样：PowerShell `Get-Process`，release、真实数据目录、≥60s 稳定后连续 3 次偏差 <5%

## 口径警示

内存数字必须表述为 "**Launcher Idle Private Bytes**（release / software renderer /
index open / 特定 provider 集 / 本机）"，不得简化为"框架内存占用"——我们证明的是
当前这套架构在特定环境下的空闲占用，不是 Slint 或 Rust 的通用常数。

## 记录规则（对齐 test plan 13）

- 性能 PR 必须附 bench 对比结果；>5% 回归 = warning，>10% = fail（除非 PR 更新 baseline 并记录前后数值）
- 每个 Agent 任务的完成报告必须含 `Performance Impact` 段（见 AGENTS.md）
