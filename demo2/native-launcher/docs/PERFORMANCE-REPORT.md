# PERFORMANCE-REPORT — 性能测试报告

## Run 2（2026-09-04，当前代码：插件协议 v1 + ContextProvider + calculator-plugin，96/96 tests）

### 门禁结果

| 项 | 结果 |
|---|---|
| Regression Gate（P50/P95，>10% 且 >100µs） | 首跑报回归 → 确认为 MVP2.1 ContextProvider 注册进 Core 的预期代价（app P50 202→302µs），**更新 baseline 并记录原因**后 3 次运行通过 |
| Budget Gate（绝对 SLO） | app P95 0.35ms ≤ 10ms，file P95 0.46ms ≤ 50ms — **met** |
| 100K 查询 soak | **pass**（3.86→4.10MB，slope 2.83 B/op，无阶梯，peak_to_final=0） |
| 1000 show/hide soak | **pass**（5.89→6.68MB，peak_to_final=0） |
| 插件契约 17 项（含 50 轮 spawn soak） | **全部通过** |
| Idle Private（隐藏态，3 采样偏差 <1%） | **6.13–6.16 MB** ✅ |
| Idle CPU（隐藏 10s） | 0.000% ✅ |
| Visible Idle CPU（弹窗开 10s） | **0.625%** ⚠️ 较上轮 0.156% 上升，疑似新 UI 组件引入，需排查 |
| Visible Idle Private | 9.84 MB ✅ |
| Plugin cold / warm | 62 ms / 46–66 µs |
| Popup 冷启动（首次 show，内部代理） | T1+T2 ≈ 41.6 ms（预算 P95≤35ms ⚠️ 首次窗口创建） |
| **Warm Popup（13 样本）** | **T1+T2 P50=188µs / P95=511µs / Max=511µs** — 远优于 P50≤5ms / P95≤10ms ✅ |

### 规模曲线（磁盘库，当前代码）

| files | cold rebuild | app P95 | file P50 | file P95 | file Max | Idle Private |
|---:|---:|---:|---:|---:|---:|---:|
| 10K | 7.7 ms | 3.9 ms | 3.4 ms | 6.1 ms | 6.9 ms | 3.49 MB |
| 100K | 64.7 ms | 85.5 ms | 53.7 ms | 80.6 ms | 126.3 ms | 4.14 MB |
| 250K | 96.2 ms | 124.5 ms | 93.8 ms | 127.1 ms | 179.3 ms | 4.75 MB |
| 1M | 196.2 ms | 399.2 ms | 214.9 ms | 379.2 ms | 646.1 ms | 4.50 MB |

结论与 Run 1 一致并加强：**内存平坦（3.5→4.5MB @ 1M）**；file search 延迟线性劣化，
1M 时 P95 379ms ≫ 50ms 预算 → **FTS5/前缀索引为 Phase 2 必要项，MVP 预算口径 "≤ ~100K 文件"**。
（app P95 在 ≥100K 桶同步抬升，疑为新文件写入后 Defender 扫描/页缓存压力污染，记为观察项。）

---

## Run 1（2026-09-04，MVP2.1 代码，74/74 tests）— 历史留档

依据 `docs/PERFORMANCE-CONTRACT.md` 与测试方案（files2/03-test-plan）执行的全部性能测试结果。环境：本机 Windows 11（AMD64 Family 25），release 构建，磁盘库。

### 总览

| 类别 | 指标 | 结果 | 预算/目标 | 判定 |
|---|---|---:|---|---|
| 基准延迟 | App search P95 | 0.23 ms | ≤ 10 ms | ✅ |
| 基准延迟 | File search P95 | 0.32 ms | ≤ 50 ms | ✅ |
| 基准延迟 | Cold start query | 0.44 ms | 记录 | ✅ |
| 回归门禁 | P50/P95 相对+绝对双阈值 | 3 次运行通过 | >10% 且 >100µs fail | ✅ |
| 绝对 SLO | search P95 vs 预算 | SLO met | — | ✅ |
| 内存 | **Hidden Idle Private** | **5.97 MB** | ≤ 50 MB | ✅ |
| 内存 | Visible Idle Private（弹窗开） | 9.17 MB | ≤ 50 MB | ✅ |
| CPU | Hidden Idle CPU（10s） | 0.000 % | ≤ 0.1% | ✅ |
| CPU | Visible Idle CPU（弹窗开 10s） | 0.156 % | ≤ 0.1% | ⚠️ 略超，弹窗渲染开销 |
| Soak | 100K 查询趋势 | pass | 无阶梯/slope<1KB/op | ✅ |
| Soak | 1000 show/hide | pass | 无泄漏 | ✅ |
| Soak | 50 spawn→query→kill | pass | 子进程全回收 | ✅ |
| Plugin | Cold（spawn+query） | 60 ms | TBD | 记录 |
| Plugin | Warm query | 23–36 µs | TBD | 记录 |
| Popup | Cold（T1+T2，首次） | T1 0.11ms + T2 16ms | P95 ≤ 35 ms | ✅（内部代理口径） |

## 规模曲线（P1，磁盘库，10K→1M 文件）

| files | cold rebuild | app P95 | file P50 | file P95 | file Max | Idle Private | Peak Private |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 10K | 2.6 ms | 1.5 ms | 1.5 ms | 2.3 ms | 2.4 ms | 3.45 MB | 3.69 MB |
| 100K | 30.4 ms | 41.5 ms | 31.9 ms | 42.9 ms | 57.6 ms | 3.91 MB | 3.91 MB |
| 250K | 101.9 ms | 115.4 ms | 96.9 ms | 136.6 ms | 172.2 ms | 3.98 MB | 4.00 MB |
| 1M | 624.5 ms | 983.5 ms | 645.5 ms | 1035.0 ms | 2534.0 ms | 4.30 MB | 4.30 MB |

**结论 1（正面）**：磁盘库下 **Idle Private 几乎不随规模增长**（3.45 → 4.30 MB @ 1M）——"文件规模增大后 Launcher 本身仍然轻"成立。对比：in-memory SQLite 同规模为 228 MB，已从 bench 中修正为磁盘口径。

**结论 2（问题）**：file search 延迟随规模**线性劣化**（LIKE %q% 全表扫描）：100K 时 P95 43ms 已接近 50ms 预算；250K 超预算（137ms）；1M 严重超限（1035ms，Max 2.5s）。**行动项：≥ 100K 文件场景需要 SQLite FTS5 或前缀索引（Phase 2 / ADR）**，当前 MVP 预算口径应表述为"≤ ~100K 文件"。

## 内存归因（attribution，bench 内存口径）

| 阶段 | Private | 增量 |
|---|---:|---:|
| empty core | 0.69 MB | — |
| + app 目录（200 条） | 0.78 MB | +0.08 MB |
| + file provider（SQLite 1k 文件） | 1.46 MB | +0.68 MB |
| + history sink | 1.77 MB | +0.32 MB |

后续新增特性（如 FTS）可对照此矩阵预估内存代价。

## Soak 明细

- **100K 查询**：initial 3.86 → final 4.14 MB（+0.27MB），slope 2.83 B/op（诊断），无阶梯，peak=final，**verdict pass**
- **1000 show/hide**（真实窗口循环）：6.07 → 6.77 MB（+0.7MB 为首次窗口创建一次性成本），peak_to_final=0
- **插件 50 轮 spawn→query→kill**：全部通过，子进程无残留

## Plugin cold/warm（首次量化）

- Cold（spawn+query 首次）：**60 ms**
- Warm（已运行进程再查询）：**23–36 µs**
- 相差约 2000 倍——证明 idle_timeout 自动退出是低内存与响应速度的正确权衡：冷启动 60ms 在交互可接受范围，且空闲期零常驻。

## 备注

- Hidden Idle 5.97 MB 低于此前记录的 12.6 MB：本次为纯隐藏态、无插件运行、弹窗未打开过；12.6 MB 为弹窗打开过后的水平。两者均远低于预算。
- Visible Idle CPU 0.156% 略超 0.1% 目标：弹窗可见时的渲染刷新；列入观察，优化候选（弹窗静止时降帧）。
- Popup 冷启动 T2 = 16–20ms 为首次窗口创建（复显待批量采样）。
- 测试工件：`benchmarks/baseline.json`、`benchmarks/soak-latest.json`、`benchmarks/memory-attribution.json`、`benchmarks/scale-curve.json`。
