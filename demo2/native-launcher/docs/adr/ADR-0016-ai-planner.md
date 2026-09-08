# ADR-0016: AI Planner（MVP4.2）— ActionProposal 生产者

日期：2026-09-05　状态：Accepted　来源：review 27-mvp5-0.3

## Decision

1. **AI 是普通 ActionProposal 生产者，不是新执行系统**。Planner trait：`plan(input, catalog) -> Vec<ActionProposal>`；catalog 是只读命令目录（AI 不监听、不解释 Context，INV-046 精神延续）。
2. **ActionProposal 字段即信任边界**：只有 `provider_id / command_id / action_id / input` 是权威的。`authorized / confirmed / granted_capabilities / trust_level` 等字段在类型上不存在——反序列化天然丢弃（INV-048 的结构化实现）。内嵌 ResolvedAction/Effect 对象无法通过验证。
3. **执行零新通道**：proposal → `to_step()` → WorkflowDefinition → ReferenceResolver → ActionResolver → ActionEngine（`execute_proposals` 管道）。Core 无任何 AI 分支；Workflow 失败策略/重解析机制原样适用。
4. **参考实现**：`KeywordPlanner`（确定性关键词匹配，只提案 Ready action）；LLM planner 作为同一 trait 的后续实现，Core 不感知。
5. **不做**：agent loop、Run Store（按需能力，非 MVP4.2 前置）、MCP（MVP4.3 同模式）。

## Consequences

- MCP Adapter（MVP4.3）= Tool → ActionProposal 翻译层，预计零 Core 改动。
- LLM 接入只需实现 `ActionPlanner`；伪造字段测试（`ai_planner.rs`）作为其验收一部分。

---

## Addendum（review 28-mvp5-0.4，MVP4.2 关账确认，2026-09-05）

1. **Catalog 只读原则**：`ActionPlanner::plan(input, catalog)` 中 catalog 永远只读。未来 AI 需要"创建临时能力"时，必须另设计 Capability/Provider 注册机制，禁止污染 Planner（禁止 plan → modify catalog → register → execute 路径）。
2. **DISCOVERY-TODO-001（技术债，见 docs/KNOWN-ISSUES.md）**：参与 Workflow/AI/MCP 的外部 provider MUST 实现冻结的 ReferenceResolver discovery 契约（empty-text discovery）。当前 AI Planner 的 live-query-snapshot 适配禁止悄悄变成永久双路径。
3. **MVP4.3 MCP 身份预埋**：MCP 的 `server_id / tool_name` 不得直接冒充 Launcher 的 `ProviderId/CommandId/ActionId`；必须经 MCP Adapter 映射为 Host 分配的 provider identity（沿用 MVP4.0 INV-029 身份权威）。若 MVP4.3 出现"需要改 ActionEngine"的情况，视为架构回归信号。
