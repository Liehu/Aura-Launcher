# 119 — P2.7 Batch 4：Clarification Engine（A06）

> 日期：2026-09-09。范围：P2.7 第四批（`P2.7 开发设计规范` §18）。
> 输入基线：history/118（763 tests）。

## 交付

- `crates/launcher-ai/src/clarification.rs`（新）：
  `needs_clarification(intent, entities, confidence, policy) ->
  Option<Clarification>`——确定性澄清决策 v0.1：
  - **空输入** → 必问（"你想做什么？"）；
  - **歧义目标**（Execute 且目标 <3 字符）→ 问具体可执行目标；
  - **低置信**（confidence < min_confidence，默认 0.6）→ 请用户补充；
  - 其余放行。澄清是 UX 安全阀，**永不产生授权语义**。
- `ClarificationPolicy` 可配置阈值；100x 确定性重放测试。
- 测试 4 条：空输入、歧义目标、低/高置信、确定性。

## Gate 结果

- `cargo test --workspace`：**767 passed / 0 failed**（763 → 767，+4）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

A01/A02/A04（Provider 深化/Context Builder/Prompt Builder——llm.rs 与
prompt.rs 已有基础待对齐 A 线契约）、B 线 Planner 演进、C 线 Runtime
产品化（agent.rs Budget/ExecutionHost 已有）、D/E/F/G/H 线。
