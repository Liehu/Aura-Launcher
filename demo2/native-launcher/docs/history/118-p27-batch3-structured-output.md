# 118 — P2.7 Batch 3：结构化输出校验器（A05）

> 日期：2026-09-09。范围：P2.7 第三批（`P2.7 开发设计规范` §16）。
> 输入基线：history/117（755 tests）。

## 交付

- `crates/launcher-ai/src/structured_output.rs`（新）：
  `validate_llm_output(raw) -> Result<AgentProposal, String>`——LLM 原始
  文本到 AgentProposal 的**唯一**通道：
  - 提取 JSON（容忍 prose + ```json 围栏，取最外层 `{...}`）；
  - 解析为 AgentProposal 后走**冻结契约校验**
    （`validate_proposal`：唯一 step_id、confidence ∈ [0,1]、
    action_ref 词法、input 界限）。
- **Fail-closed**：任何校验失败整提案丢弃并给出原因——畸形的 LLM 回答
  永远无法变成半有效的提案。
- 测试 5 条：干净 JSON、prose+围栏、路径式 action_ref 拒绝、无 JSON
  拒绝、空 plan 拒绝、风险默认无隐式审批、PlanStep roundtrip。

## Gate 结果

- `cargo test --workspace`：**763 passed / 0 failed**（755 → 763，+8；
  含 A05 全部 5 条 + intent 收尾计数）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

A01/A02/A04/A06（Provider 深化/Context Builder/Prompt Builder/
Clarification）→ B 线 Planner（B03 校验器已具备=本模块）→ C 线 Runtime
→ D/E/F/G/H 线。
