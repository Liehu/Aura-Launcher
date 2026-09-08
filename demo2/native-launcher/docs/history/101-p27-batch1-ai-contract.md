# 101 — P2.7 Batch 1：评审 + AI Contract v1（P27-001）

> 日期：2026-09-09。范围：P2.7 第一批（`demo2/files2/P2.7 开发设计规范 —
> AI - Agent Productization.md` 等 4 份）。输入基线：history/100（701 tests）。

## 评审结论（四份文档）

1. **设计规范**（1112 行，40 任务/8 阶段）：阶段定位明确——P2.7 **必须
   建立在 P2.5 与 P2.6 已冻结的基础设施之上**。核心红线与全项目一致：
   AI 可以理解/规划/提案，但不能直接拥有执行权（§7 Proposal 是 DATA 不是
   AUTHORIZATION；§8 action_ref 是逻辑引用，不得保存 ResolvedAction）。
2. **依赖冲突（必须声明）**：P2.6 目前仅完成 Batch 1/5（图模型+验证器）。
   P27-B05（Workflow Proposal Builder）与 Workflow Integration 直接依赖
   P2.6 B 线（Durable Runtime）。**裁决：P2.7 按不依赖 P2.6 的子集先行**
   （契约/A 线/B01-B04/C 线核心），B05 与 Workflow Integration 在 P2.6
   B 线完成后补齐。
3. **资产盘点**：launcher-ai 已有 MVP4.4 遗产（LlmProvider mock/OpenAI、
   LLMPlanner、AgentLimits/BudgetController/AgentExecutionHost/
   AgentRunOutcome）——P2.7 是产品化而非从零开发。
4. Agentic/测试/验收规范与 P2.4-P2.6 同构，prompt injection 防御（E06）
   与 approval 安全（D03/G04）是本阶段新增的高优先测试面。

## Task P27-001 — AI Contract v1（本批落地）

- `crates/launcher-ai/src/agent_contract.rs`（新）：冻结 DTO——
  `AgentIntent`（Search/Open/Execute/Workflow/Explain/Unknown）、
  `PlanStep`（逻辑 action_ref + input + rationale + requires_approval，
  §8：**不得保存 ResolvedAction**）、`AgentProposal`（§7：proposal_id/
  session_id/user_goal/intent/plan/confidence/explanation）、
  `RiskLevel` L0-L4 + §19 默认审批策略（L0/L1 自动、L3/L4 强制）。
- `validate_proposal()`：确定性 fail-closed——非空 plan、唯一 step_id、
  confidence ∈ [0,1]、**action_ref 词法校验（路径形式引用拒绝）**、
  input 大小界限（64KB）。
- 测试 6 条：valid roundtrip、fail-closed 四例、风险等级默认策略、
  **authority-free 结构断言**（proposal JSON 永不含 capability/authority/
  resolved 词汇）。

## Gate 结果

- `cargo test --workspace`：**705 passed / 0 failed**（701 → 705，+4）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余（后续批次）

P27-000/002/003（基线/ID/安全模型）→ A 线（Provider 抽象深化/Context
Builder/Prompt Builder/结构化输出校验/Clarification）→ B01–B04/B06
（Tool Catalog 投影/Plan Schema/校验/Proposal 构建/Risk 分类——已有
LLMPlanner 演进）→ C 线（Agent Runtime 状态机/Loop/Replanning/Budget——
BudgetController 已有，产品化）→ D 线（Approval 模型/UI/安全/Edit Plan）
→ E 线（Memory/Privacy/Prompt Injection 防御）→ F 线（AI 搜索面/Chat 面/
Plan Preview）→ G/H 线（QA + Release）。
**B05 + Workflow Integration 等待 P2.6 B 线完成后补齐。**
