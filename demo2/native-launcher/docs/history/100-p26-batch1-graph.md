# 100 — P2.6 Batch 1: Workflow Graph Model + Validator（A01/A02）

> 日期：2026-09-09。范围：P2.6 第一批（`demo2/files2/P2.6 开发设计规范 —
> Workflow 2.0.md` §7-§11）。输入基线：P2.5 完成（history/99，690 tests）。

## 评审结论（三份 P2.6 文档）

- 设计规范 33 任务/7 阶段，与 roadmap 的 P2.6 定位一致；§11 DAG-only 政策
  明确（arbitrary loop 留 2.1/3.x），大幅缩小 Durable Scheduler 状态空间。
- 与现有资产衔接良好：§8 action_ref 走既有 ReferenceResolver/
  ActionResolver 链，§3/§4 condition/variables 明确为 DATA/CONTROL FLOW
  非 authority——无需动 Action 契约。
- 体量警告：E 线 Visual Editor（7 任务）+ D 线 Trigger Framework（6 任务）
  是两个独立大块；建议 Durable Runtime（B 线）先行，Editor 最后。
- Agentic 规范与测试方案与 P2.4/P2.5 同构，无新风险。

## Task P26-A01 — Graph Domain Model

- `crates/launcher-workflow/src/graph.rs`（新）：`WorkflowGraph`/
  `WorkflowNode`/`WorkflowEdge`/`ConditionExpr`（Eq/Ne/Gt/Lt）/
  `NodeId`——字段与规格 §7/§8/§9 一致；serde 稳定 roundtrip。
- `action_ref` 是稳定 action/command id，解析仍走既有 Resolver 链（无新
  执行路径）；condition/序列化 shape 断言不含 capability/authority。

## Task P26-A02 — Graph Validator

- `validate()`（§10 清单）：Empty → DuplicateNode → MissingEntryNode →
  DanglingEdge → SelfLoop → **UnreachableNode** → Cycle（迭代式 DFS 染色，
  §11 DAG-only）。
- 设计裁决：断连子图根归诊断为 UnreachableNode（更可操作），MultipleEntry
  变体保留但由 unreachable 检查覆盖（§10 的 multiple entry 在显式
  entry_node 模型下必然表现为断连子图）。
- 测试 11 条：diamond branch/join 合法、cycle/self-loop/dangling/
  duplicate/missing-entry/multi-entry/unreachable/empty 全拒绝、roundtrip、
  authority-free。

## Gate 结果

- `cargo test --workspace`：**701 passed / 0 failed**（690 → 701，+11）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.6 剩余（后续批次）

A03–A06（join 语义/条件引擎/变量模型/契约 kit）→ **B 线**（Durable Run
Store/Checkpoint/DAG Scheduler/Parallel/Retry/Recovery）→ **C 线**
（Human Approval：契约/存储/UI/Resume 安全/过期防重放）→ **D 线**
（Trigger Framework + 队列：hotkey/plugin/schedule/AI-MCP）→ **E 线**
（Visual Editor，7 任务）→ **F/G 线**（可观测/DAG stress/恢复测试/安全
测试/VR/性能 + CI/gate/文档/完成宣告）。
