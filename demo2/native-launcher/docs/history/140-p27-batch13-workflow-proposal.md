# 140 — P2.7 Batch 13：Workflow Proposal Builder（B05）

> 日期：2026-09-10。范围：P2.7 第十三批（`P2.7 开发设计规范` B05）。
> 输入基线：history/139（811 tests）。

## 交付

- `crates/launcher-ai/src/workflow_proposal.rs`（新）：
  `build_workflow_proposal(session_id, seq, workflow_id, variables,
  confidence)`——构建 workflow 触发型 AgentProposal（B05）。
  fail-closed：空 workflow_id 拒绝；confidence 钳制 [0,1]。
- 测试 4 条：构建+合约校验、空 id 拒绝、confidence 钳制。

## Gate 结果

- `cargo test --workspace`：**814 passed / 0 failed**（811 → 814，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
