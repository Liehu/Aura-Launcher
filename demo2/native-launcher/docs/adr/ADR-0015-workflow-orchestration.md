# ADR-0015: Workflow Orchestration（MVP4.1 Contract Freeze）

日期：2026-09-05　状态：Accepted　来源：review 24 §15/§16（INV-049/050 预定）+ WORKFLOW-CONTRACT-v0.1 草案 + 内部评审（R1–R4/m1–m5）+ review 25 收敛

## Context

MVP4.0 冻结后，Workflow 是第一个超出 Native UI 的 Action Producer。风险：Workflow 演化为第二 Effect 执行体系，架空 Resolver/Engine 边界（INV-049/050 为此前预定的护栏）。

## Decision

1. **WORKFLOW-CONTRACT-v0.1 冻结**（`docs/WORKFLOW-CONTRACT-v0.1.md`）。核心：WorkflowDefinition / WorkflowRun / ActionInvocation / FailurePolicy / Re-resolution 五对象；线性 Step v0.1（无 DAG/条件/循环/并行）。
2. **WF-001~010 十条冻结原则**（见契约 §0）：orchestration only、Definition 持久化逻辑引用而非 ResolvedAction、Run 无可执行 Effect、ActionInvocation producer-agnostic、执行前重解析、Reference 双源解析、Inline descriptor 永不可信、无 Capability Authority / Effect Gateway、Retry 新 execution_id 且 Step 级、Confirmation 暂停 + 源会话恢复 + 恢复必重解析。
3. **双 Resolver 分离**：ReferenceResolver（在哪里找到 Action：Popup Results → Fresh Provider Query → CommandNotFound）与 ActionResolver（当前是否允许执行）职责分离，Runner 不接触 Provider/Search 细节。
4. **Step 双态**：`WorkflowAction::Reference | Inline(ActionDescriptor)`——system.* 步骤可表达；两路径汇聚到同一 Resolver/Engine，无第二执行通道；Inline 永远是不可信持久声明（INV-056）。
5. **Failure Classification 冻结 8 类**（CapabilityDenied / InvalidInput / StaleContext / Timeout / BusinessError / ProtocolViolation / PluginUnavailable / CommandNotFound）+ native error 映射表 + 默认策略矩阵；`FailureAction = Stop|Retry|Skip|ReResolve`。
6. **Confirmation = 暂停语义**：`Paused(ConfirmationRequired)`，仅源 UI 会话可恢复，恢复必重解析；confirmed 状态永不持久化。
7. **at-least-once 声明**：Timeout 重试不保证 exactly-once；需要更强语义由 Effect provider 自带 idempotency。
8. **INV-049/050 转为生效**；新增 INV-052~057（见下）。

## Invariants（本 ADR 固化）

| ID | 内容 |
|---|---|
| INV-049（生效） | Workflow 持久化 Action references/proposals，不是 trusted ResolvedAction。 |
| INV-050（生效） | ActionInvocation 执行前必须针对当前 capability/policy/context 重解析。 |
| INV-052 | WorkflowRun MUST NOT 包含可直接执行的 Effect 对象。 |
| INV-053 | ActionInvocation 是 producer-agnostic 的（无 Workflow 身份字段），AI/MCP 复用同一类型。 |
| INV-054 | Retry 产生新 execution_id 且是 Step 级状态；run 级无 Retrying。 |
| INV-055 | Reference 解析来源冻结：Popup Results → Fresh Provider Query → CommandNotFound（默认 Stop）。 |
| INV-056 | Inline ActionDescriptor 是持久化声明，永远作为不可信输入重解析。 |
| INV-057 | Workflow 无 Capability Authority、无 Effect Gateway；失败策略只能决定编排行为。 |

（review 25 建议的 INV-052~057 内容按"不留重复语义"原则合并映射：其 052→INV-049/052、053→INV-052、054→INV-053、055→INV-054、056→INV-050（已存在，转生效）、057→INV-057。）

## 明确不做（v0.1）

Parallel / 条件 / 循环 / 变量 / 子工作流 / 补偿回滚 / 分布式 / 持久队列 / exactly-once / 版本迁移 / 人工任务服务 / AI 生成工作流 / MCP 触发工作流。

## Consequences

- AI Planner（MVP4.2）与 MCP Adapter（MVP4.3）复用 ActionInvocation + ReferenceResolver + ActionResolver，零 Core 改动。
- 实现顺序（冻结后）：domain types → reference resolution → failure classification → runner 状态机 → calculator-plus 混合 Reference+Inline 三步工作流 → CAT-WF-001~012。

---

## Addendum（review 26，WF-A1~A4，冻结后微修订）

1. **WF-A1 输入权威**：`WorkflowStep.input` 是唯一权威执行输入；Inline descriptor 的 input 降级为声明/默认值（被 step.input 覆盖）。ActionInvocation.input 权威规则同样适用于未来 AI/MCP 复用。
2. **WF-A2 Inline 域约束**：`WorkflowAction::Inline` MUST 仅携带 `system.*` descriptor；`plugin.*` 必须走 Reference。Reference = external capability reference；Inline = Host-native declarative action。plugin 身份绑定（INV-047）只在 Reference 路径生效。
3. **WF-A3 Fresh Query = empty-query discovery**：不新增 RPC；empty-text query 对 Reference Resolution 而言 MUST 返回可发布 Command 发现集。属 Plugin Contract additive 澄清；未实现 discovery 的插件降级为 CommandNotFound（Stop）。
4. **WF-A4**：`step_id` 在 Definition 内唯一。
5. **malformed provider command 响应 → ProtocolViolation**（非 CommandNotFound）。

以上为契约 Addendum 1（不重开评审）；INV-056 措辞同步收紧（Inline 仅限 system.*）。
