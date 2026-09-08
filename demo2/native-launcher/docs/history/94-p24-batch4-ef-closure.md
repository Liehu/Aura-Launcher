# 94 — P2.4 收口（Batch 4：E01–E05 诊断打通 + F Release Closure）

> 日期：2026-09-08。范围：P2.4 最后一批（`demo2/files2/01-P2.4-DESIGN-SPEC.md` §8/§9）。
> 输入基线：history/93（654 tests → 本批 655）。

## Task P24-E01/E05 — 诊断接线（Provider 失败/成功路径 → 结构化记录）

- `PluginProvider::query` 全路径接入 `DiagnosticLog`（P2.4-C06 的全局有界
  ring，512 条）：
  - spawn 失败 → `spawn_failed`；超时/无响应 → `timeout`；进程崩溃 →
    `crash`；畸形/超限响应 → `malformed`（E05 的 crash/timeout/flood 分类
    全部经由 PluginError→DiagnosticClass 映射，词表与 Plugin Contract 一致）；
  - 成功查询 → `ok` + result_count。
- 纯观测：记录发生在既有 warn 日志点旁，不改变任何执行/quarantine 语义。

## Task P24-E03 — Diagnostic Snapshot 落盘

- `global_dump_json()`：确定性 JSON 快照（全字段：class/query_id/
  runtime_id/session_id/elapsed/frame_size/result_count）。
- launcher-app 托盘退出时写入 `<data_dir>/plugin-diagnostics.json`
  （支持包/开发工具直接消费）。

## Task P24-E02/E04 — RPC Inspector / Replay（CLI）

- `launcher-plugin replay <dir> --queries <file>`：把保存的输入逐条重放，
  每条走真实 host spawn/shutdown 路径（隔离即被测契约）。
- E02 的 RPC 观测面由 E01 的结构化记录 + `inspect`/快照 JSON 承担。
- 测试 `replay_runs_saved_queries`：真实 Python 插件（SDK scaffold 修正为
  子类化 `Plugin` + `.run()`）双 query 重放，9/9 CLI 测试全绿。

## Task P24-F — Release Closure

- `release_gate.py` 新增 **G13 "P2.4 foundation conformance"**（required）：
  5 个命名 conformance 套件入证据链——catalog（providers lib）、
  root_lifecycle（indexer e2e）、incremental_e2e（P2.1-B 契约回归）、
  plugin_registry（core lib）、launcher-plugin-cli 全套。
- 全量 gate 重跑：**G01~G13 全 PASS**（G02=655 tests；G10 perf 基线不变：
  app p95 7µs / file p95 23µs / 启动 189.6ms / idle RSS 27.4MB）。
- manifest status 保持 `release-candidate`（1.0 GA 已另行宣告于 90 号；
  P2.4 是 1.0 之后的演进批次，不翻动 1.0 manifest）。

## P2.4 完成宣告

```text
P2.4-0  Baseline Closure        ✅ (= 1.0 GA Closure, history/90)
P2.4-A  Catalog 2.0             ✅ (A01–A06, history/91/92)
P2.4-B  Index Maintenance 2.0   ✅ (B01–B06, history/92)
P2.4-C  Plugin Control Plane    ✅ (C01–C06, history/89/91/93)
P2.4-D  Plugin SDK / CLI        ✅ (D01–D06, history/93)
P2.4-E  Dev Diagnostics         ✅ (E01–E05, history/94)
P2.4-F  Release Closure         ✅ (G13 入 gate, history/94)
```

## Gate 结果

- `cargo test --workspace`：**655 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：ok（17 crates + 9 apps）
- Release Gate：G01~G13 全 PASS（含新 G13）

## 下一步（按 `100-roadmap.md` / 97 号审计 §E）

P2.5 Search Intelligence（Query Intent / Context Intelligence / Personal
Ranking / Candidate Merge v2 / Search Contract v2）为 2.x 第一主线；
Plugin Ecosystem 产品化（Registry 管理 UI、Marketplace 前置）为第二主线。
