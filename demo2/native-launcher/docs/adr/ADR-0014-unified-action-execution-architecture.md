# ADR-0014: Unified Action Execution Architecture（统一行动执行架构）

日期：2026-09-04　状态：Accepted　来源：review 21-mvp4（MVP4.0 Plugin-owned Action RPC）

## 核心原则（冻结）

> **所有"想做什么"的参与者都是 Action Producer（UI / Plugin / Workflow / AI / MCP / Hotkey / Shortcut）；所有"实际做什么"都必须经 ActionResolver 与 ActionEngine。ActionEngine 是唯一的 mutation/Effect 执行网关。**

Producer 提出做什么；Resolver 决定现在是否允许做；ActionEngine 决定如何产生 Effect。任何上层系统不得绕过后两者。

## Decision

1. **`plugin.*` 成为唯一新增的 Effect Domain**；workflow 仅编排（ActionInvocation 序列）、AI 仅提案（ActionProposal）、MCP 仅适配器（Tool ↔ Command/Action 翻译）——都不引入新执行机制。
2. **PluginBroker 是 Effect Executor，不是独立执行网关**（INV-047）：`ActionDescriptor → resolve_descriptor_for(own_plugin) → ResolvedAction(PluginInvoke) → ActionEngine(验证/策略) → Effect::PluginInvoked → Core::execute_plugin_action → execute_action RPC → 插件进程`。
3. **execute_action 独立于 query RPC**（Plugin Contract v0.1.x additive）：独立方法、独立 `execution_id` 空间（`e-<seq>`，与 `q-<seq>` 完全分离）、必须回显 execution_id、context_generation 随行（INV-044）。参数只传 `ExecutionContext`（action input + 允许的 context 字段 + generation），不暴露 Host 内部 ContextSnapshot。
4. **身份绑定**（延续 INV-029）：`plugin.<id>.*` 仅当 `<id>` 与 producer 的 manifest id 匹配才解析（reverse-DNS 含点号，前缀匹配 `own` 或 `own.`）；跨插件调用一律 InvalidInput。provider_id 永远由 Host 分配。
5. **Capability**：新增 `plugin.invoke`（点分 `plugin.invoke`），`plugin.*` action 隐式要求它（叠加在声明的 requires 上，INV-027 单调性不变）。不以 `plugin.<id>.*` 作 capability。
6. **Plugin 级错误 ≠ 协议违规**：`execute_action` 的业务失败是 Action Execution Result `EffectFailed`（`PluginError::ActionFailed`），插件进程保持存活；kill 只用于 timeout/malformed/协议违规。
7. **插件级错误响应**（-32603 等）不终止进程；SDK（`serve_with_actions`）自动处理 envelope/回显/panic 隔离，作者仍只写 `|action_id, input, generation| -> Result`。

## 明确不做

Workflow / AI Planner / MCP Adapter 的运行时（MVP4.1+）；`plugin.invoke.self/other` 细分；取消/审计。

## Invariants

| ID | 内容 |
|---|---|
| INV-044 | Plugin-owned Actions MUST execute through ActionEngine → PluginBroker。 |
| INV-045 | PluginBroker MUST NOT be an independent effect gateway。 |
| INV-046 | Workflow/AI/MCP 是 ActionProposal 生产者/适配器，MUST NOT 绕过 ActionResolver。 |
| INV-047 | Context-bound 的 ActionInvocation MUST 携带 context generation；stale resolution MUST NOT 静默执行。 |
| INV-048 | 外部提供的 authorized/confirmed 状态 MUST NOT 被信任；授权只来自 Manifest/Policy/GrantedCapabilities。 |

（对应 review 21 提议的 INV-046~051 合并去重：046+047→INV-044/045，048+049→INV-046，050→INV-047，051→INV-048。）

## 实现

`launcher-ipc::{method::EXECUTE_ACTION, ExecuteActionParams/Result}`、`launcher-plugin-host::PluginHandle::execute_action`（echo 校验）、`launcher-core::{Provider::execute_action/plugin_identity, Core::execute_plugin_action}`、`launcher-plugin-api::serve_with_actions`、`launcher-domain::resolve_descriptor_for`（plugin 路由 + `ActionKind::PluginInvoke` + `Capability::PluginInvoke`）、`launcher-action::Effect::PluginInvoked`。calculator-plus 升级为 MVP4 Reference（`plugin.com.example.calculator.plus.echo`）。验收：`apps/calculator-plus/tests/mvp4_acceptance.rs`（4 项）。

---

## Addendum（review 23-mvp4-0.2，MVP4.0 冻结确认，2026-09-04）

1. **Effect 抽象层级**：`Effect::PluginInvoked` 是 Rust 内部的**执行域/路由分类**，不是插件业务能力的枚举——禁止演化出 `PluginPdfCompress/PluginOcr/...` 式变体。长期形态：`Effect { effect_type(开放 namespace 字符串), input, target, context }`，`system.*`/`plugin.*` 是 namespace，不是 enum 成员。
2. **namespace 所有权纯函数**：`launcher_domain::owns_plugin_namespace(owner, target)`（exact / `owner.` 段边界；同前缀无点、短前缀、大小写变体、空串全部拒绝），配完整边界测试矩阵（`owns_plugin_namespace_boundary_matrix`）。`resolve_descriptor_for` 改为调用该函数。
3. **execution_id 生命周期语义**：`execution_id` 标识**一次最终 Effect execution attempt**，不是一次用户 Action。Workflow/AI 的上层对象（ActionInvocation 等）保存自己的 id（`workflow_run_id → step_id → execution_id` 分层），不得让多个 Effect 共用一个 execution_id。
4. **context_generation ≠ 事务锁**：它只回答"这个 Action 是否仍针对我看到的那个 Context"。执行链分为三层：generation check → precondition validation → actual Effect；generation 一致不保证文件存在/窗口存活/网络在线/插件状态未变。

## MVP4.1 预定 Invariants（Workflow 实现时生效）

- Workflow MUST persist Action references/proposals, never trusted ResolvedAction or executable Effect objects（防绕过 Resolver）。
- Every Workflow ActionInvocation MUST be re-resolved before execution against current capability/policy/context state。

## MVP4.0 冻结清单

plugin namespace ✅ / plugin.invoke ✅ / identity binding（纯函数+矩阵）✅ / execute_action RPC ✅ / execution_id ✅ / context_generation ✅ / PluginBroker ✅ / business-error isolation ✅。

---

## Addendum 2（review 24-mvp4-0.3，MVP4.0 Frozen，2026-09-04）

**MVP4.0 冻结清单**：plugin namespace / plugin.invoke / `owns_plugin_namespace()` 安全边界 / identity binding / execute_action RPC / 独立 execution_id / context_generation / generation-precondition-effect 三层语义 / PluginBroker / business-error isolation / Effect 抽象 Addendum（INV-051）/ Workflow re-resolution 设计约束（INV-049/050）——全部 ✅。

1. **`owns_plugin_namespace` 复用原则**：它是 plugin identity 的唯一安全原语。今后任何涉及 plugin 身份判断的路径（manifest 校验、broker 路由、未来的 workflow/ai/mcp 适配层）MUST 复用它，禁止复制一份稍有差异的字符串前缀判断。
2. **Effect 脱离 enum 思维的架构收益**：新增插件能力只改 Manifest + Plugin，不改 launcher-domain/Resolver/Engine、不重新发布 Host——Host 只理解通用执行协议（INV-051 为此护栏）。
3. **execution_id 重试语义**：重试产生**新的** execution_id（`execution-102 timeout → execution-103 retry success`），旧 id 的语义永不篡改；Workflow 上层用 step_id 关联同一逻辑步骤的多次 attempt。

## MVP4.1 设计评审输入：Failure Policy 分类矩阵（本轮只记录，不实现）

| 失败类别 | 默认策略倾向 |
|---|---|
| CapabilityDenied | Stop |
| StaleContext | Re-resolve |
| Plugin Timeout | Retry（新 execution_id） |
| Business Error（EffectFailed） | 按 Workflow policy |
| Protocol Violation | Stop |
| InvalidInput | Stop / Skip |

MVP4.1 需回答的五个问题：WorkflowDefinition（编排什么）/ WorkflowRun（运行到哪）/ ActionInvocation（调用哪个 Action）/ Failure Policy（失败后怎么办）/ Re-resolution（为何执行前仍有效）。
