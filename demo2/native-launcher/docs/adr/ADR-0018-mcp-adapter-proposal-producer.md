# ADR-0018: MCP Adapter as Proposal Producer（MVP4.3）

日期：2026-09-05　状态：Accepted　来源：spec `40-mvp8-0.1`（MVP4.3 MCP Adapter v0.1，FROZEN）

## Decision

1. **MCP = Producer**（与 UI/AI 同层级），不是 Execution Framework：
   - `MCP ≠ ActionEngine`、`MCP ≠ Effect Gateway`、`MCP ≠ Capability Authority`、`MCP ≠ WorkflowRunner`。
   - MCP Server 暴露的 tools 投影为 Launcher Command/Action，最终经 ActionProposal → Resolver → Engine 执行。
2. **新 crate `launcher-mcp`**（依赖仅 launcher-domain，无 action/plugin-host/workflow 依赖——MCP 无执行权）：
   - `protocol`：JSON-RPC 2.0 DTO（initialize / notifications/initialized / tools/list / tools/call）+ `MCP_PROTOCOL_VERSION = "2025-06-18"`。
   - `transport::McpTransport` trait + stdio 实现（子进程 + NDJSON；单 in-flight 请求；id 不匹配 = ProtocolViolation；通知跳过；超时=Timeout）。HTTP/remote auth 架构预留。
   - `adapter`：`tool_to_command`（McpTool → Domain Command，route = `mcp:<server>` / tool.name / `invoke`）；invoke 动作在 Phase 7 前带 disabled_reason（可见不可执行）。
   - `error::McpError::failure_class()`：8 类 FailureClass 的单一映射点（Timeout/BusinessError/ProtocolViolation/PluginUnavailable/CommandNotFound/InvalidInput）。
3. **launcher-config**：`[[mcp.servers]]`（id/transport/program/args）；transport 仅 stdio；凭据禁止入配置（Spec section 21）。
4. **launcher-core McpProvider**（Phase 4）：empty query = catalog 全量发现（**正式闭合 DISCOVERY-TODO-001 的 MCP 半边**——WF-006 option ② fresh-query 从此对 MCP 可用）；非空 query 按名称/标题过滤。Provider 只查本地 catalog 缓存，refresh 归 adapter（tools/list）。
5. **不新增 MCP 专属 capability**：MCP annotations/description/instructions 一律为不可信 metadata/policy input；授权仍由 Manifest/Policy/GrantedCapabilities 决定（INV-MCP-004/005）。`confirmation` 由既有 Resolver 统一裁决。
6. **Effect 执行域（Phase 7 预留）**：`plugin.mcp.invoke` 属 Plugin Effect Domain，经 `Effect::PluginInvoked → Core → McpExecutor → MCP Server`；Adapter ≠ Executor（McpExecutor trait 已在 crate 内预留类型）。execution_id = 一次 Effect attempt（INV-MCP-008），逐 attempt 新分配。
7. **只做 Tools**：Resources / Prompts / Tasks / Sampling / Elicitation / MCP Apps 全部 ❌（Spec section 23）。

## Invariants（INV-MCP-001~010，Spec section 33）

INV-MCP-001 proposal-only；002 经 Resolver；003 经 Engine；004 metadata 不授予 capability；005 description/instructions/annotations 不改 authorization；006 catalog identity ≠ authorization identity；007 Workflow 永不持久化 MCP ResolvedAction；008 每个 MCP Effect attempt 新 execution_id；009 失败映射统一 FailureClass；010 MCP 无顶层 UI Mode。

## Consequences

- empty-text discovery TODO 的 MCP 半边闭合：`McpProvider.query("")` 返回 catalog 全量（DISCOVERY-TODO-001 对 plugin 侧仍开放）。
- MCP Server = UNTRUSTED EXTERNAL PROVIDER：description/instructions/annotations 为数据非授权（Spec section 29）。
- 新增测试：protocol DTO roundtrip、identity scoping（注入/冲突不可行）、catalog discovery/refresh、adapter 投影与禁执行、fixture E2E 6 项（discovery/route/disabled/filter/unavailable/exit）。

---

## Addendum 2（2026-09-05）：MCP 版本策略 — Legacy Compatibility Profile

MCP 官方 2026-07-28 规范移除了 initialize/initialized 协议级 session，转向无状态请求模型 + 可缓存列表结果 + server/discover。**不重做 Phase 1–4**，而是把当前实现明确定位为：

```text
MVP4.3 Phase 1–4
    ├── MCP legacy compatibility profile（2025-06-18, stateful stdio session）← 当前实现
    └── transport abstraction（McpTransport trait）→ future 2026-07-28 stateless adapter
```

- 当前 stdio fixture（mcp-calculator）继续工作；`launcher-mcp` 的 catalog/identity/adapter/projection 类型**版本中立**，不得围绕任一 profile 重塑。
- 未来 2026-07-28 adapter 的接入点 = 新的 `McpTransport` 实现（无状态请求，无 initialize），catalog/identity/adapter 层零改动。
- 新增 MCP effect domain 时同样只经 `plugin.mcp.invoke` namespace 字符串，不扩张 ActionKind。
