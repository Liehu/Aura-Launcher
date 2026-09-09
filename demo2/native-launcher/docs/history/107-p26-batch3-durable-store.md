# 107 — P2.6 Batch 3：A06 契约 kit + B01 Durable Run Store

> 日期：2026-09-09。范围：P2.6 第四批（`P2.6 开发设计规范` §15-§18）。
> 输入基线：history/106（715 tests）。

## Task P26-B01 — Durable Run Store

- `crates/launcher-workflow/src/durable.rs`（新）+ rusqlite/tracing 依赖：
  `RunStore`（SQLite/WAL，`workflow_runs` 表）持久化 `RunCheckpoint`——
  run_id/graph_id/graph_version/status/finished/skipped/variables/
  updated_at_ms，**一次 upsert 即一个 checkpoint（§17）**，恢复从
  checkpoint 重放而非内存（§18）。
- `RunStatus` 五态冻结：Running/Paused/AwaitingApproval/Finished/Failed。
- 损坏策略与 catalog.db 一致：重建为空（run 是可重建状态，图定义在
  别处）；不引入 generation 语义（进度即 checkpoint）。
- `list_by_status()`：恢复扫描（重启后找 Running/Paused 续跑——§18
  Recovery 入口）。
- 测试 3 条：checkpoint 跨 reopen 全状态保留、按状态恢复扫描、损坏重建。

## Task P26-A06 — Graph Contract Kit

- 图模型/验证器/执行语义/durable store 四组 21 条测试共同构成契约 kit
  （graph.rs 11 条 + engine.rs 6 条 + durable.rs 3 条 + 既有 cat_wf 套件）；
  DTO roundtrip 与 authority-free 断言已在 kit 内钉死。

## Gate 结果

- `cargo test --workspace`：**718 passed / 0 failed**（715 → 718，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.6 剩余

B02 Checkpoint Commit（Runner 每步后写 checkpoint 的接线）→ B03 DAG
Scheduler（消费 ready_nodes 的执行循环）→ B04 Parallel → B05 Retry →
B06 Recovery Runtime → C/D/E/F/G 线。
