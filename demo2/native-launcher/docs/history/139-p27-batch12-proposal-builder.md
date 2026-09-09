# 139 — P2.7 Batch 12：Action Proposal Builder（B04）

> 日期：2026-09-10。范围：P2.7 第十二批（`P2.7 开发设计规范` §7/B04）。
> 输入基线：history/138（806 tests）。

## 交付

- `crates/launcher-ai/src/proposal_builder.rs`（新）：
  `build_proposal(session_id, seq, &ParsedRequest, confidence) ->
  Result<AgentProposal>`——从 A03 的确定性解析结果构建符合冻结契约的
  AgentProposal（经 validate_proposal 合约校验）。
- action_ref 按 intent 映射（search:query / command:open / command:execute /
  workflow:run / explain:context / noop）；非 read-only intent 自动
  requires_approval（fail-closed）。
- 测试 5 条：search/open/workflow 构建+合约校验、confidence 钳制、
  确定性。

## Gate 结果

- `cargo test --workspace`：**811 passed / 0 failed**（806 → 811，+5）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
