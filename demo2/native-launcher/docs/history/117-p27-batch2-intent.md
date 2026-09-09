# 117 — P2.7 Batch 2：Intent/Entity 模型（A03，确定性规则解析）

> 日期：2026-09-09。范围：P2.7 第二批（`P2.7 开发设计规范` §9）。
> 输入基线：history/101（705 tests 起，含 P2.6/P2.8 并行批至 751）。

## 交付

- `crates/launcher-ai/src/intent.rs`（新）：`parse_intent(input) ->
  ParsedRequest { intent, entities }`——确定性规则解析 v0.1：
  - 动词前缀路由：`find/search` → Search（query 实体）、`open` → Open
    （target）、`run` → Execute（target）、`workflow` → Workflow
    （workflow_id）；
  - 未匹配输入安全回退为 Search + 全文 query（自然语言在此层**永远**只
    生成提案输入，不可能被误读为特权操作）；
  - 大小写不敏感、100x 确定性重放、空输入安全。
- **无 LLM/网络/I/O**——LLM 未来可替换解析器，输出走同一结构化契约
  （由 agent_contract 校验）。
- 测试 4 条：动词路由与实体抽取、回退、确定性+大小写、空输入。

## Gate 结果

- `cargo test --workspace`：**755 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

A01（Provider 抽象深化——llm.rs 已有 mock/OpenAI 基础）、A02 Context
Builder、A04 Prompt Builder、A05 结构化输出校验、A06 Clarification →
B 线 Planner 演进 → C 线 Runtime 产品化 → D/E/F/G/H 线。
