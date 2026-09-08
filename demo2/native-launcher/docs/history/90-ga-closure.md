# 90 — 1.0 GA Closure（GA-1/2/3/4/5/7 完成，GA-6 签名暂缓）

> 日期：2026-09-08。范围：`97-launcher-1.0-final-audit.md` §D 的 GA Blockers，
> 按审计定义执行 GA 收口。权威基线文档：`demo2/files2/97-launcher-1.0-final-audit.md`。

## GA-1 文档统一

- README：头部 "v0.1 MVP" → **Native Launcher 1.0**；状态节改写为 1.0 RC1 权威基线
  （623 tests / zero warnings / G01~G12 / soak 数据），历史里程碑标注为时间序快照。
- `docs/TESTING.md`：头部加权威基线注记（623 全绿），62/62 等历史数字保留为快照。
- `demo2/files2/01-design-spec-v0.1.md`：§2.1 加修订注记——FTS5 裁决
  （1.0 实际实现 = SQLite metadata + bounded LIKE + deterministic ranking；
  FTS5 正式推迟 P2.5；禁止按原文重写 Indexer）。

## GA-2 Hotkey latency 仪表化（Test Plan §8.1）

- `apps/launcher-app`：`LAUNCHER_HOTKEY_BENCH=<report.json>`（可选
  `LAUNCHER_HOTKEY_BENCH_CYCLES`，默认 200）——合成热键事件驱动**真实**
  WM_HOTKEY → dispatch → ui.show() → recenter 管线，逐样本记录
  `dispatch_to_ready_us`，聚合 P50/P95 写 JSON 报告。
- **实测（debug build，200 cycles / 100 show samples）：
  P50 = 307µs，P95 = 19.7ms ≤ 目标 20ms/35ms，PASS。**
- 产物：`benchmarks/hotkey-latency.json`。T0（物理键→OS dispatch）在进程外，
  报告 scope 字段已声明。

## GA-3 10,000 show/hide soak（Test Plan §9.3）

- `LAUNCHER_SOAK_SHOWHIDE=10000`：Private 7.2MB → 8.9MB，
  growth +1.7MB（预算 ≤10MB），peak == final（无线性漂移），**PASS**。
- 产物：`benchmarks/showhide-soak-10k.json` + 原始 log。

## GA-4 Indexer 崩溃重启故障注入（Test Plan §10/§16）

- `scripts/indexer_crash_restart_test.py`：驱动真实
  `launcher-indexer-service` 进程——boot 1 建索引并 RPC 搜索确认 →
  **hard kill（非优雅退出）** → boot 2 同 db 重启：pre-crash 条目可搜、
  rebuild 重灌、file 数 eventual consistent、计数跨崩溃单调。**PASS**。
- 注：等价的 Rust 进程 spawn 用例被本地安全工具（Mimosa hook）误判
  "命令注入"拦截（argv-list、无 shell、编译期二进制路径，实为误报）；
  按 hook 建议改用 Python `Popen(argv, shell=False)` 形态，CI 直接调用。
- 产物：`benchmarks/indexer-crash-restart.log`。

## GA-5 GitHub Actions CI（Spec §2.1 / Test Plan §14）

- `.github/workflows/ci.yml`（仓库根，windows-latest）：
  零警告 build（G01 等价）→ check_topology（G03）→ `cargo test --workspace`（G02）
  → GA-4 注入脚本 → clippy（advisory，待存量 lint 分诊后转硬门禁）。

## GA-7 Release Gate 重跑 + GA 宣告

- `release_gate.py` 新增 `LAUNCHER_RELEASE_STATUS=released|release-candidate`
  覆盖（任一 required gate FAIL 时仍强制 blocked）。
- 全量 G01~G12 重跑后 manifest `status=released`；
  产物：`artifacts/LAUNCHER-1.0-RELEASE-MANIFEST.json` / `EVIDENCE.json`、
  `benchmarks/release-gate-ga-run.log`。
- **GA-6（MSIX 证书签名）经用户裁决暂缓**：无证书。无签名 msix 打包与
  update handoff 消费端已就绪（history/89）；"未签名"作为已知限制记录于本文件。

## GA 后状态

> **Launcher 1.0 GA（签名待补）**。后续工作入口 = 97 号审计 §E：P2.4 Foundation
> （Catalog 2.0 / Search Contract v2 / Plugin Ecosystem / Dev CLI）起步。

## 测试基线

- `cargo test --workspace` 全绿（gate G02）；`cargo build` 零警告（G01）；
  `check_topology.py` 通过（G03）；G01~G12 全 PASS。
