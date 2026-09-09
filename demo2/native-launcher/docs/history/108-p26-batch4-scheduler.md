# 108 — P2.6 Batch 4：Durable Scheduler（B02/B03/B05/B06 核心闭环）

> 日期：2026-09-09。范围：P2.6 第五批（`P2.6 开发设计规范` §14-§18/§35）。
> 输入基线：history/107（718 tests）。

## 交付

- `crates/launcher-workflow/src/scheduler.rs`（新）+ `VariableStore::
  snapshot()/restore()`（B02 数据面）：
  - **B03 执行循环**：`run_graph(graph, store, run_id, executor, cancel)`
    ——ready_nodes → 节点条件求值（false 永久 skip）→ 顺序执行（v1）→
    输出写入 output_variables → **每节点后 checkpoint（B02）**——崩溃点
    即恢复点。
  - **恢复**：启动时从 store 加载 checkpoint（graph id/version 匹配才
    接受），finished/skipped/变量全部还原——**已完成节点绝不重执行**。
  - **B04/B06 暂停**：取消标志在节点间轮询（批内也检查），置位即
    checkpoint+Paused。
  - **B05 v1 失败策略**：步骤失败即在 checkpoint 点 Failed（无重试；
    Retry Policy 随 A 线 retry_policy 字段在后续批启用）。
  - 停滞诊断：无 runnable 且 skip 无进展 → Failed + "scheduler stall"。
- 测试 4 条：完成+checkpoint 全量、失败停在 checkpoint、**暂停→重开进程
  语义恢复→只执行剩余节点（b 不重跑）**、step output→variables→checkpoint
  全链路。

## Gate 结果

- `cargo test --workspace`：**722 passed / 0 failed**（718 → 722，+4）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.6 剩余

B04 Parallel（并发执行 ready 集合）、B06 恢复运行时的宿主接线
（launcher-app 恢复扫描 + resume 触发源）、C 线 Approval、D 线 Trigger、
E 线 Editor、F/G 线收口。
