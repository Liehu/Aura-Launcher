# 134 — P2.7 Batch 8：Agent Pipeline 串联（C03 前置/§3 管线落地）

> 日期：2026-09-09。范围：P2.7 第八批（`P2.7 开发设计规范` §3/§16/§18/
> §19 管线串联）。输入基线：history/133（793 tests）。

## 交付

- `crates/launcher-ai/src/pipeline.rs`（新）：
  `run_pipeline(input, context, catalog, llm, policy, budget) ->
  PipelineOutcome`——六段组装：
  1. **A03 Intent 解析**（确定性规则）；
  2. **A06 澄清门**（空输入/歧义在任何 LLM 调用**之前**短路——省钱且
     安全）；
  3. **A04/A02 Prompt 组装**（清洗+围栏+预算）；
  4. **注入式 LLM 调用**（闭包——测试用 mock，生产用 OpenAI 兼容
     provider；本模块拥有编排而非传输）；
  5. **A05 结构化输出校验**（validate_llm_output 唯一通道）；
  6. **B06 风险分类**（逐 step，L3/L4 自动补 requires_approval）+
     intent 一致性收敛（确定性解析优先，LLM 只细化 HOW）。
- `PipelineOutcome`：Proposal{proposal, risks, prompt} | Clarify{question}。
- 测试 4 条：分类提案产出（含 prompt 断言）、**澄清门在 LLM 之前短路
  （LLM 闭包 panic 证明）**、LLM 失败显式报错、L3 强制审批。

## Gate 结果

- `cargo test --workspace`：**797 passed / 0 failed**（793 → 797，+4）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

A01（Provider 深化——llm.rs 已有 mock/OpenAI）、B01-B05（Tool Catalog
投影/Plan Schema=agent_contract 已备/校验器=structured_output 已备/
Proposal 构建器/Risk=本批）、C03-C07（把 AgentSession/Budget 接入本
管线）、D/E/F/G/H 线。
