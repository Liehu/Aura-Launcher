# WORKFLOW-CONTRACT-v0.1（FROZEN）

**Status:** FROZEN v0.1（2026-09-05；经内部评审 R1–R4/m1–m5 + review 25 收敛确认）
**Depends on:** ADR-0014 / ACTION-CONTRACT v0.1 / PLUGIN-CONTRACT v0.1.x / INV-049~051
**Freeze ADR:** ADR-0015（Workflow Orchestration）
**Primary invariant:** Workflow is orchestration only — no Effect authority, no Resolver authority, no broker access.

---

## 0. 冻结原则（ADR-0015 核心，WF-001~010）

| # | 原则 |
|---|---|
| WF-001 | Workflow is orchestration only. |
| WF-002 | WorkflowDefinition persists logical Action references/proposals, never ResolvedAction.（INV-049/052） |
| WF-003 | WorkflowRun contains execution state, not executable Effects.（INV-053） |
| WF-004 | ActionInvocation is producer-agnostic and carries no Workflow identity.（INV-054 关联部分） |
| WF-005 | Every ActionInvocation is re-resolved immediately before execution.（INV-050 生效） |
| WF-006 | ActionReference resolution: Popup Session Results → Fresh Provider Query → CommandNotFound.（INV-055） |
| WF-007 | Inline ActionDescriptor is untrusted persisted declaration and is re-resolved like any input.（INV-056） |
| WF-008 | Workflow has no Capability Authority and no Effect Gateway. |
| WF-009 | Retry creates a new execution_id and is Step-scoped.（INV-054） |
| WF-010 | ConfirmationRequired pauses the Run; resume only from the originating UI session and always re-resolves. |

## 1. 核心对象

```rust
struct WorkflowDefinition { id, version, name, steps: Vec<WorkflowStep>, failure_policy: FailurePolicy }
struct WorkflowStep { step_id, action: WorkflowAction, input: Value, failure_policy: StepFailurePolicy }

enum WorkflowAction {
    /// 逻辑引用（非物化对象）：解析来源见 §3
    Reference(ActionReference),        // { provider_id, command_id, action_id }
    /// Host-owned 内联声明：persisted declaration, NOT trusted executable state（INV-056）
    Inline(ActionDescriptor),
}

struct WorkflowRun { workflow_run_id, definition_id, definition_version, status, current_step, steps: Vec<StepRun> }
struct StepRun { step_id, status, attempt, last_error, last_execution_id, resolved_context_generation: Option<u64> }

/// Producer-agnostic：无 Workflow 身份字段（INV-054）；AI/MCP 复用同一类型
struct ActionInvocation { action: WorkflowAction, input: Value, resolved_context_generation: Option<u64>, attempt: u32 }
```

规则：`Definition ≠ Run ≠ ResolvedAction ≠ Effect`；Definition 不绑定 current context/capability grant/plugin process/filesystem state；Run 不含可直接执行的 Effect（INV-053）。

## 2. Run / Step 状态机

Run：`Created → Queued → Running → (Paused(ConfirmationRequired)) → Succeeded | Failed | Cancelled`。
**无 run 级 Retrying**——Retry 是 Step execution policy，不是 run 业务状态（消除 Run×Step 组合爆炸）。

Step：`Pending → Resolving（可重入：stale context / retry 均回到此处）→ Resolved → (WaitingForConfirmation) → Executing → Complete | Failed | Skipped`。

## 3. Reference Resolution（R1，INV-055）

由独立的 **ReferenceResolver** 负责（Runner 不得自行理解 Provider/Search 细节）：

```text
ActionReference
  ① 当前 Popup Session 结果集（同会话精确匹配 provider_id + command.id）
  ② Fresh Provider Query：按指定 provider_id 获取新 Command 集合
     （不是重新执行用户搜索词；排序/score 无关）
     → 按 (provider_id, command.id) 精确匹配
  ①②均未命中 → FailureClass::CommandNotFound（默认 Stop）
```

与 ActionResolver 职责分离：**ReferenceResolver 解决"在哪里找到 Action"；ActionResolver 解决"当前是否允许执行"**。

## 4. 双路径汇聚（R2）

```text
WorkflowAction::Reference  → ReferenceResolver → ActionDescriptor ┐
WorkflowAction::Inline     → （persisted declaration，不可信）      ┘
                             → ActionResolver → ResolvedAction → ActionEngine → Effect
```

## 5. Failure Classification（R3，冻结 8 类 + 分类映射）

| FailureClass | 来源映射 | 默认策略 |
|---|---|---|
| CapabilityDenied | Resolver | Stop |
| InvalidInput | Resolver（UnknownActionType 归一化为此，reason 保留） | Stop（policy 可 Skip） |
| StaleContext | Context generation 比对 | ReResolve |
| Timeout | PluginError::Timeout | Retry（有界 max_attempts，新 execution_id） |
| BusinessError | PluginError::ActionFailed | Policy（v0.1 默认 Stop） |
| ProtocolViolation | PluginError::Malformed / Crashed | Stop |
| PluginUnavailable | PluginError::Spawn / 未安装 | Retry（有界）→ Stop |
| CommandNotFound | ReferenceResolver | Stop |

分类由 **Failure Classifier** 从 native errors 映射，Workflow 不自行猜错误类型。`FailureAction = { Stop, Retry, Skip, ReResolve }`（不做 Fallback/Compensate/Rollback/CircuitBreaker）。默认矩阵冻结如上；`max_attempts` 是实现参数不写入协议。

## 6. Execution 语义

- **at-least-once**：Timeout ≠ Effect 未发生；Retry 是新的 attempt（新 execution_id），不保证 exactly-once。
- 三层执行链：generation check → precondition validation → actual Effect（INV-047 语义沿用）。
- id 分层：`workflow_run_id → step_id → execution_id`；retry 永远产生新 execution_id（INV-054）。

## 7. Confirmation（R4，WF-010）

`ConfirmationRequired → Run Paused(ConfirmationRequired)`；恢复只能来自**发起该 run 的 UI 会话**；resume 后 **必须重新 resolve** 再进 engine。`Paused(ConfirmationRequired)` 表达"等待一个新的外部确认事件"，不是"曾经授权过"——confirmed=true 永不持久化（INV-048）。

## 8. 无 Capability Authority（WF-008）

Workflow 可声明步骤需要什么能力，不能声明能力已授予；`authorized=true / confirmed=true / requires=[]` 均为不可信输入，授权只来自 Manifest + GrantedCapabilities + Policy。

## 9. 序列化（m4）

冻结为："WorkflowRun MUST have a stable serializable representation"。介质（SQLite/File/Memory）是实现细节；**Serializable ≠ Resumable**——崩溃自动恢复属运行时能力，v0.1 不要求。

## 10. 术语（m5）

全契约唯一术语：`resolved_context_generation`（该 ActionInvocation 最近一次成功解析所依据的 generation）。不使用 observed_/at_observation 变体。

## 11. v0.1 明确不做

DAG / 条件 / 循环 / 并行 / 变量表达式 / 子工作流 / 补偿回滚 / 分布式 / 持久队列 / exactly-once / 版本迁移 / 人工任务 / AI 生成工作流 / MCP 触发工作流。

## 12. 验收（CAT-WF-001~012）

single-step success / multi-step sequential / stale→re-resolve / capability denied→stop / timeout→retry(new execution_id) / business error→policy / protocol violation→stop / invalid input→stop|skip / confirmation pause+resume / plugin action via engine / run serialize+reload / never execute persisted ResolvedAction。

---

# Addendum 1（review 26，2026-09-05：WF-A1~A4 + 错误归类，冻结不重开）

## A1. 输入唯一来源（WF-A1）

**`WorkflowStep.input` 是 ActionInvocation 的唯一权威执行输入**；`Inline(ActionDescriptor).input` 降级为"声明/默认输入"，被 step.input 覆盖。Reference 路径的 descriptor 由 ReferenceResolver 找到，同样使用 step.input。禁止第二输入源——AI/MCP 复用 ActionInvocation 时沿用同一规则。

## A2. Inline 仅限 Host-owned（WF-A2）

`WorkflowAction::Inline` MUST 只携带 `system.*` ActionDescriptor；`plugin.*` MUST 使用 `Reference`。语义定稿：

```text
Reference = external capability reference（Provider-owned / Plugin-owned）
Inline    = Host-native declarative action（system.*）
```

这样 plugin 身份绑定问题不会被 Inline 路径重新打开（INV-047 只在 Reference 路径生效）。

## A3. Fresh Provider Query = empty-query discovery（WF-A3）

不新增 RPC。冻结语义：

```text
Fresh Provider Query := provider_id = referenced provider, text = "", limit = 有界发现上限
```

并规定：**对 Reference Resolution 而言，Provider 的 empty-text query MUST 返回其可发布 Command 集合的发现结果**（不是普通搜索）。这是 Plugin Contract 的 additive 语义澄清；现状与迁移：

- 内置 Provider（apps/files/context）已天然满足（空 query 返回全量/目录集合）。
- 外部插件当前对空 query 返回空集：未实现 discovery 的插件，其 Reference 解析得到 `CommandNotFound`（Stop）——如实降级，不粉饰；插件侧补 discovery 后自动恢复。

## A4. step_id 唯一性（WF-A4）

`step_id` MUST 在 `WorkflowDefinition` 内唯一（`StepRun.step_id` / `current_step` / id 分层均依赖它）。

## A5. 错误归类补充

Provider/Plugin 对 query/reference resolution 返回 **malformed command 响应 → ProtocolViolation**（Stop），不得归为 CommandNotFound——坏数据是契约损坏，不是"找不到"。

## 边界确认（review 26 §4）

provider 存在但无法启动插件 → `PluginUnavailable`（有界 Retry → Stop）；provider 查询成功但目标 command 不存在 → `CommandNotFound`（Stop）。二者不混。

## 恢复语义确认（review 26 §7）

`Serializable ≠ Resumable` 已挡住最大风险：reload 后不得从 `Executing` 状态自动推断"Effect 未完成 → 自动 Retry"（与 at-least-once / Timeout≠未发生 一致）；自动恢复属未来运行时能力，需专门定义。
