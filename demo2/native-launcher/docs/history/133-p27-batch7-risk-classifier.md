# 133 — P2.7 Batch 7：Risk Classifier（B06）

> 日期：2026-09-09。范围：P2.7 第七批（`P2.7 开发设计规范` §19）。
> 输入基线：history/121（773 tests；P2.8/P2.9/P2.6 并行批至 779）。

## 交付

- `crates/launcher-ai/src/risk_classifier.rs`（新）：
  `classify(&PlanStep) -> RiskLevel`——按 action_ref 词法前缀的确定性
  分类（search/explain=L0、command:app/file=L1、clipboard/workflow=L2、
  shell/power=L3、privileged=L4）；**未知词法 fail-closed 到 L4**
  （最需审批）。
- `requires_approval(&PlanStep)`：L3/L4 强制审批（§19 默认表）；
  实际闸门仍由 ActionResolver/Policy 决定——分类只是 AI 侧提示，
  只能提高审批可能性。
- 测试 2 条：§19 默认策略全表 + 未知词法 fail-closed。

## Gate 结果

- `cargo test --workspace`：**793 passed / 0 failed**（791 → 793，+2）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

A01（Provider 深化）、B01-B05（Tool Catalog 投影——launcher-core
catalog.rs 已有基础/Plan Schema=agent_contract 已备/校验器=structured_
output 已备/Proposal 构建器/Risk=本批）、C03-C07（Loop 接线——
AgentSession 已备）、D/E/F/G/H 线。
