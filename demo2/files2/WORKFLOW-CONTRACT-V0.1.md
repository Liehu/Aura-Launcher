下面可以直接作为 **`WORKFLOW-CONTRACT-v0.1` + Workflow State Machine + Failure Policy Matrix** 的设计评审稿。核心原则沿用 MVP4.0，不引入新的 Effect Gateway。

# WORKFLOW-CONTRACT-v0.1

**Status:** Proposed
**Scope:** MVP4.1 Workflow Design Review
**Depends on:** ADR-0014 / Action Contract v0.1 / Plugin Contract v0.1.x
**Primary invariant:** Workflow is an orchestration layer, not an Effect execution layer.

---

# 1. Design Principle

Workflow 只回答：

> **“按什么顺序请求哪些 Action？”**

Workflow 不回答：

> “这个 Action 现在是否有权限执行？”
> “这个 Action 是否仍然适用于当前 Context？”
> “具体 Effect 怎么执行？”

后三者分别属于：

```text
ActionResolver
    ↓
ActionEngine
```

因此统一执行链保持：

```text
Workflow
   ↓
ActionInvocation
   ↓
ActionResolver
   ↓
ResolvedAction
   ↓
ActionEngine
   ↓
Effect
```

Workflow 不得直接：

```text
system.*
plugin.*
PluginBroker
OS API
Plugin RPC
```

---

# 2. Five Questions

## 2.1 WorkflowDefinition — 编排什么？

`WorkflowDefinition` 描述**逻辑工作流**，不包含任何已经解析完成的执行对象。

它定义：

```text
Workflow identity
Workflow metadata
Step graph
Action references / proposals
Step-local input
Failure policy
```

核心原则：

```text
Definition ≠ Run
Definition ≠ ResolvedAction
Definition ≠ Effect
```

Definition 可以长期保存。

它不绑定：

```text
current context
current capability grant
current plugin process
current window
current filesystem state
```

---

## 2.2 WorkflowRun — 这一轮运行到哪里？

`WorkflowRun` 是某个 `WorkflowDefinition` 的一次执行实例。

它负责记录：

```text
workflow_run_id
definition_id
status
started_at
finished_at
current_step
step states
failure state
context baseline / latest observation
```

例如：

```text
WorkflowDefinition: wf.organize-downloads

Run:
wr-1001
```

同一个 Definition 可以拥有：

```text
wr-1001
wr-1002
wr-1003
```

互不覆盖。

---

## 2.3 ActionInvocation — 当前步骤请求执行什么？

`ActionInvocation` 是 Workflow 对 Action 的一次**逻辑执行请求**。

它保存：

```text
step_id
provider_id
command_id
action_id
input
context_generation_at_observation
attempt
execution reference
```

但：

> `ActionInvocation` 保存的是 Action Reference / Proposal，而不是 `ResolvedAction`。

这是 INV-049/050 的直接落点。

---

## 2.4 Failure Policy — 步骤失败以后怎么办？

Failure Policy 决定：

```text
当前 step 如何处理
是否 retry
是否 re-resolve
是否 skip
是否继续下一个 step
是否终止 WorkflowRun
```

它只决定**编排行为**。

它不能绕过 ActionResolver，也不能授权 Action。

---

## 2.5 Re-resolution — 为什么此刻仍然允许执行？

每个 `ActionInvocation` 在进入 Effect 执行前，都必须重新经过：

```text
current capability
current policy
current context
current confirmation policy
current provider availability
```

解析：

```text
ActionReference
      ↓
ActionResolver
      ↓
ResolvedAction
      ↓
ActionEngine
```

Workflow 永远不能直接执行历史 `ResolvedAction`。

---

# 3. Core Objects

## 3.1 WorkflowDefinition

概念模型：

```rust
struct WorkflowDefinition {
    id: WorkflowId,
    version: u32,
    name: String,
    steps: Vec<WorkflowStep>,
    failure_policy: FailurePolicy,
}
```

第一版 Step 不要求支持任意 DAG。

建议 MVP4.1 初期先采用：

```text
ordered steps
```

即：

```text
Step1 → Step2 → Step3
```

分支、循环、并行暂不进入 v0.1。

这样可以先验证 Workflow 的核心语义，而不是提前建设通用编排语言。

---

## 3.2 WorkflowStep

```rust
struct WorkflowStep {
    step_id: StepId,
    action: ActionReference,
    input: Value,
    failure_policy: StepFailurePolicy,
}
```

Action Reference：

```rust
struct ActionReference {
    provider_id: ProviderId,
    command_id: CommandId,
    action_id: ActionId,
}
```

不得携带：

```text
ResolvedAction
Effect
granted_capabilities
confirmation approval
trusted context snapshot
```

---

# 4. WorkflowRun

```rust
struct WorkflowRun {
    workflow_run_id: WorkflowRunId,
    definition_id: WorkflowDefinitionId,
    definition_version: u32,
    status: WorkflowRunStatus,
    current_step: Option<StepId>,
    steps: Vec<StepRun>,
}
```

状态：

```text
Created
Queued
Running
WaitingForConfirmation
Retrying
Paused
Succeeded
Failed
Cancelled
```

v0.1 中是否支持 `Paused` 可以由实现阶段决定；契约层允许，但不是强制 UI 能力。

---

# 5. StepRun

`WorkflowStep` 是逻辑定义。

`StepRun` 是这次运行中该 Step 的状态。

```rust
struct StepRun {
    step_id: StepId,
    status: StepRunStatus,
    attempt: u32,

    last_error: Option<WorkflowError>,
    last_execution_id: Option<ExecutionId>,

    resolved_context_generation: Option<u64>,
}
```

这样可以表达：

```text
step-2
    attempt=1
    execution=e-102
    timeout

    attempt=2
    execution=e-103
    success
```

这与 MVP4.0 已冻结的 `execution_id` 语义兼容。

---

# 6. ActionInvocation

ActionInvocation 是真正进入 Core 执行流程的对象。

```rust
struct ActionInvocation {
    workflow_run_id: WorkflowRunId,
    step_id: StepId,

    action: ActionReference,
    input: Value,

    observed_context_generation: Option<u64>,
    attempt: u32,
}
```

执行时：

```text
ActionInvocation
      ↓
resolve_descriptor / resolve reference
      ↓
ResolvedAction
      ↓
ActionEngine
```

每次 retry 都产生新的 execution attempt。

---

# 7. Workflow State Machine

## 7.1 WorkflowRun 状态

```text
              ┌──────────┐
              │ Created  │
              └────┬─────┘
                   ↓
              ┌──────────┐
              │ Queued   │
              └────┬─────┘
                   ↓
              ┌──────────┐
         ┌─── │ Running  │ ───┐
         │    └────┬─────┘    │
         │         │           │
         │         ↓           │
         │   Step finished     │
         │         │           │
         │         ↓           │
         │     next step       │
         │         │           │
         │         └──────┐    │
         │                │    │
         │                ↓    │
         │             Succeeded
         │
         ├── retry ─────→ Retrying
         │                    │
         │                    ↓
         │                  Running
         │
         ├── confirmation → WaitingForConfirmation
         │                         │
         │                         ↓
         │                      Running
         │
         ├── pause ──────→ Paused
         │                    │
         │                    ↓
         │                  Running
         │
         └── terminal ───→ Failed / Cancelled
```

---

# 8. Step State Machine

一个 Step 的生命周期：

```text
Pending
   │
   ▼
Resolving
   │
   ├── stale context ─────► Resolving
   │
   ├── capability denied ─► Failed
   ├── invalid input ─────► Failed / Skipped
   │
   ▼
Resolved
   │
   ├── confirmation ──────► WaitingForConfirmation
   │                              │
   │                              ▼
   │                           Executing
   │
   └───────────────────────► Executing
                                  │
                        ┌─────────┼─────────┐
                        ▼         ▼         ▼
                     Success    Retry      Failed
                        │         │
                        ▼         ▼
                     Complete   Resolving
```

关键点：

> **`Resolving` 可以重复出现。**

因此 re-resolution 不是异常路径，而是 Workflow 正常生命周期的一部分。

---

# 9. Re-resolution Contract

Step 执行时禁止：

```text
stored ResolvedAction
        ↓
ActionEngine
```

必须：

```text
stored ActionReference
        ↓
current state
        ↓
ActionResolver
        ↓
new ResolvedAction
        ↓
ActionEngine
```

Resolver 每次必须重新检查：

```text
1. Provider identity
2. Action existence
3. Capability
4. Policy
5. Context applicability
6. Confirmation policy
7. Action input validity
8. Provider/runtime availability
```

---

# 10. Context Generation Semantics

Workflow 可以记录：

```text
observed_context_generation
```

但这不是永久授权。

执行阶段：

```text
invocation.observed_generation
             ↓
        current generation
             │
        ┌────┴────┐
        │         │
       same     changed
        │         │
        ▼         ▼
     continue   re-resolve
```

如果重新解析后 Action 仍然有效：

```text
changed generation
      ↓
re-resolve
      ↓
valid
      ↓
execute
```

如果新 Context 下 Action 已经不成立：

```text
changed generation
      ↓
re-resolve
      ↓
not applicable
      ↓
StaleContext / InvalidInput
```

因此：

```text
generation changed
```

不必然意味着 Workflow 失败。

---

# 11. Failure Taxonomy

v0.1 至少区分以下六类：

| Failure             | 默认策略        | 原因                  |
| ------------------- | ----------- | ------------------- |
| `CapabilityDenied`  | Stop        | 当前明确无权限             |
| `StaleContext`      | Re-resolve  | 原执行依据已经过时           |
| `Timeout`           | Retry       | Effect 可能只是暂时失败     |
| `BusinessError`     | Policy      | 业务语义由 Workflow 决定   |
| `ProtocolViolation` | Stop        | Plugin/Runtime 契约损坏 |
| `InvalidInput`      | Stop / Skip | 请求本身无法执行            |

---

# 12. Failure Policy Matrix

## 12.1 CapabilityDenied

```text
ActionResolver
      ↓
CapabilityDenied
      ↓
Workflow Failure Policy
```

默认：

```text
STOP
```

原因：

CapabilityDenied 不是暂时故障。

Workflow 不应该：

```text
retry capability
```

更不能：

```text
request extra capability
```

然后自动继续。

---

## 12.2 StaleContext

默认：

```text
RE-RESOLVE
```

流程：

```text
StaleContext
    ↓
refresh/read current context
    ↓
ActionResolver
    ↓
ResolvedAction
    ↓
continue
```

如果重新解析仍然失败，则进入新的 failure classification。

禁止：

```text
stale → blindly execute old resolution
```

---

## 12.3 Timeout

默认：

```text
RETRY
```

但必须限制：

```text
max_attempts
backoff
```

每次 retry：

```text
new execution_id
```

例如：

```text
attempt 1
execution=e-102
timeout

attempt 2
execution=e-103
success
```

不能复用 `e-102`。

---

## 12.4 BusinessError

例如 Plugin：

```text
PluginError::ActionFailed
```

插件进程保持存活。

Workflow 默认不替它决定：

```text
stop
retry
skip
```

而交给：

```text
StepFailurePolicy
```

例如：

```text
on business error:
    retry = 0
```

或：

```text
on business error:
    skip = true
```

---

## 12.5 ProtocolViolation

默认：

```text
STOP
```

原因：

这意味着 Runtime 契约本身已经不可信。

例如：

```text
malformed response
invalid envelope
execution_id mismatch
frame violation
unsupported protocol behavior
```

Workflow 不应该简单：

```text
retry immediately
```

否则会掩盖 Runtime 崩坏。

---

## 12.6 InvalidInput

默认：

```text
STOP
```

但允许显式：

```text
SKIP
```

例如批处理：

```text
file #1 valid
file #2 invalid
file #3 valid
```

Workflow 可以声明：

```text
InvalidInput → Skip
```

但这必须是 Workflow policy，而不是 Resolver 擅自决定。

---

# 13. Failure Policy Model

建议 v0.1 使用非常小的策略集合：

```rust
enum FailureAction {
    Stop,
    Retry,
    Skip,
    ReResolve,
}
```

不要现在加入：

```text
Fallback
Compensate
Rollback
ParallelRecovery
CircuitBreaker
```

这些属于后续 Workflow Runtime 能力。

Step policy 可以：

```rust
struct StepFailurePolicy {
    capability_denied: FailureAction,
    stale_context: FailureAction,
    timeout: RetryPolicy,
    business_error: FailureAction,
    protocol_violation: FailureAction,
    invalid_input: FailureAction,
}
```

但建议为 v0.1 提供默认策略，而不是要求每个 Workflow 都声明完整矩阵。

---

# 14. Default Failure Policy

建议冻结默认值：

```text
CapabilityDenied   → Stop
StaleContext       → ReResolve
Timeout            → Retry
BusinessError      → Stop
ProtocolViolation  → Stop
InvalidInput       → Stop
```

其中：

```text
Timeout → Retry
```

必须伴随：

```text
max_attempts = bounded
```

例如默认：

```text
max_attempts = 2
```

具体数值属于实现参数，不必写死进 Protocol Contract。

---

# 15. Confirmation in Workflow

Workflow 不应该绕过 MVP3.2 已经建立的 Confirmation 语义。

因此：

```text
Workflow
   ↓
ActionInvocation
   ↓
Resolver
   ↓
ConfirmationRequired
   ↓
WorkflowRun = WaitingForConfirmation
```

确认之后：

```text
Confirmed
   ↓
重新 Resolve
   ↓
ActionEngine
```

而不是：

```text
第一次 Resolve
   ↓
保存“confirmed=true”
   ↓
以后直接 execute
```

尤其不能把：

```text
confirmed=true
```

持久化成永久授权。

这与 MVP4.0 INV-048 保持一致。

---

# 16. Workflow 不持有 Capability Authority

Workflow 可以声明：

```text
“这个步骤需要什么能力”
```

但不能决定：

```text
“这些能力已经被授予”
```

最终仍然：

```text
Manifest
  +
GrantedCapabilities
  +
Policy
  ↓
ActionResolver
```

Workflow 提供的任何：

```text
authorized=true
confirmed=true
```

都必须视为不可信输入。

---

# 17. Workflow 与 Plugin Action

Plugin Action 完全按照普通 Action 对待：

```text
Workflow
   ↓
ActionReference:
    provider = com.example.calculator.plus
    command = ...
    action = echo
   ↓
Resolver
   ↓
plugin identity binding
   ↓
plugin.invoke
   ↓
ActionEngine
   ↓
PluginBroker
```

Workflow 不需要：

```text
if action starts_with("plugin.")
```

然后自己调用 PluginBroker。

这是 MVP4.0 架构边界最直接的复用。

---

# 18. Workflow Run 与 execution_id

三层关系冻结为：

```text
workflow_run_id
      │
      └── step_id
             │
             ├── attempt 1 → execution_id=e-102
             └── attempt 2 → execution_id=e-103
```

含义：

```text
workflow_run_id
    = 一次完整 Workflow 运行

step_id
    = WorkflowDefinition 中的逻辑步骤

execution_id
    = 一次最终 Effect execution attempt
```

三者不混用。

---

# 19. Idempotency Consideration

v0.1 不实现通用事务/回滚，但 Workflow 必须意识到：

```text
Timeout
```

不代表：

```text
Effect definitely did not happen
```

例如：

```text
Plugin
   ↓
Effect completed
   ↓
response lost
   ↓
Host sees timeout
   ↓
Retry
```

可能产生：

```text
Effect #1 actually succeeded
Effect #2 executes again
```

因此：

> `Retry` 是重新执行 attempt，不是保证 exactly-once。

Workflow Contract v0.1 建议明确声明：

```text
Execution semantics:
at-least-once for retried effects unless the Effect provider
offers stronger idempotency semantics.
```

这条对未来文件移动、上传、发送消息尤其重要。

---

# 20. 为什么现在不实现 Rollback

例如：

```text
Step1 → move file
Step2 → compress
Step3 → upload
```

Step3 失败。

不能默认：

```text
rollback Step2
rollback Step1
```

因为：

```text
Effect 不一定可逆
Plugin 不一定支持 compensate
External system 不一定支持 transaction
```

因此 v0.1：

```text
Failure Policy
```

只负责：

```text
Stop / Retry / Skip / ReResolve
```

不提供：

```text
transaction rollback
```

---

# 21. Workflow Definition v0.1 建议保持线性

第一版：

```text
WorkflowDefinition
    ↓
Step 1
    ↓
Step 2
    ↓
Step 3
```

而不要现在就做：

```text
                    ┌── Step2
Step1 ──────────────┤
                    └── Step3
                         │
                    condition
                         │
                    parallel
                         │
                      loop
```

原因不是这些能力没有价值，而是它们会立即引入：

```text
graph semantics
condition evaluation
join semantics
parallel execution
cancellation
child runs
variable scopes
```

这会把 MVP4.1 从“验证 Action 编排”膨胀成完整 Workflow Engine。

---

# 22. MVP4.1 的第一版执行模型

最简单且完整的模型是：

```text
load Definition
      ↓
create WorkflowRun
      ↓
select next Pending Step
      ↓
create ActionInvocation
      ↓
resolve
      ↓
validate
      ↓
confirmation if needed
      ↓
ActionEngine
      ↓
Effect Result
      ↓
classify failure
      ↓
Failure Policy
      ↓
next Step / Retry / ReResolve / Skip / Stop
```

这已经足够验证整个架构。

---

# 23. 五个核心对象的最终职责

| 对象                   | 负责               | 不负责           |
| -------------------- | ---------------- | ------------- |
| `WorkflowDefinition` | 定义步骤及编排关系        | 权限、Effect     |
| `WorkflowRun`        | 保存一次运行状态         | 解析 Action     |
| `ActionInvocation`   | 表示一次逻辑 Action 请求 | 直接执行          |
| `FailurePolicy`      | 决定失败后的编排行为       | 授权            |
| `ActionResolver`     | 每次执行前重新判断可执行性    | 编排            |
| `ActionEngine`       | 执行 Effect        | Workflow 状态管理 |

---

# 24. 最终边界

整个 MVP4.1 可以冻结成：

```text
                 WorkflowDefinition
                         │
                         ▼
                    WorkflowRun
                         │
                         ▼
                 ActionInvocation
                         │
                         ▼
                ┌─────────────────┐
                │ ActionResolver  │
                │                 │
                │ Context         │
                │ Capability      │
                │ Policy          │
                │ Confirmation    │
                │ Identity        │
                └────────┬────────┘
                         │
                         ▼
                  ResolvedAction
                         │
                         ▼
                  ActionEngine
                         │
                         ▼
                       Effect
                         │
              ┌──────────┴──────────┐
              ▼                     ▼
           system.*             plugin.*
```

失败从 Effect 返回：

```text
EffectResult
    ↓
Failure Classification
    ↓
Failure Policy
    │
    ├── Stop
    ├── Retry
    ├── Skip
    └── ReResolve
```

而不是：

```text
Failure
   ↓
Workflow bypass ActionEngine
```

---

# 25. MVP4.1 Design Freeze Candidates

在开始编码前，建议冻结以下项目：

```text
WF-001
WorkflowDefinition 与 WorkflowRun 分离。

WF-002
WorkflowDefinition 不持有 ResolvedAction。

WF-003
WorkflowRun 不持有可直接执行的 Effect。

WF-004
ActionInvocation 只持有 ActionReference / Proposal。

WF-005
每次 ActionInvocation 执行前必须重新 Resolution。

WF-006
Retry 产生新的 execution_id。

WF-007
StaleContext 默认触发 ReResolve。

WF-008
Workflow 不拥有 Capability Authority。

WF-009
Workflow 不拥有 Effect Gateway。

WF-010
Plugin Action 与 System Action 通过同一个 ActionEngine 执行。

WF-011
v0.1 默认采用线性 Step execution。

WF-012
v0.1 不提供 rollback / transaction semantics。

WF-013
Timeout retry 不保证 exactly-once。

WF-014
Failure Policy 只能决定编排行为，不得绕过 Resolver/Engine。

WF-015
External authorized / confirmed state 永不作为可信状态。
```

---

# 26. MVP4.1 暂不定义

以下内容应明确留给后续版本：

```text
Parallel execution
Conditional branches
Loops
Variables / expressions
Sub-workflows
Compensation / rollback
Distributed execution
Persistent queue
Exactly-once semantics
Workflow version migration
Human task / approval service
AI-generated workflows
MCP-triggered workflows
```

---

# 27. MVP4.1 的最小验收闭环

最终只需要证明这一条链：

```text
WorkflowDefinition
      ↓
WorkflowRun
      ↓
ActionInvocation
      ↓
Re-resolve
      ↓
Confirmation (if required)
      ↓
ActionEngine
      ↓
Effect
      ↓
Result
      ↓
Failure Policy
      ↓
next step / retry / re-resolve / stop
```

至少覆盖：

```text
CAT-WF-001
single-step success

CAT-WF-002
multi-step sequential execution

CAT-WF-003
stale context → re-resolve

CAT-WF-004
capability denied → stop

CAT-WF-005
timeout → retry with new execution_id

CAT-WF-006
business error → policy

CAT-WF-007
protocol violation → stop

CAT-WF-008
invalid input → stop/skip

CAT-WF-009
confirmation-required action

CAT-WF-010
plugin action through ActionEngine

CAT-WF-011
persist/reload WorkflowRun

CAT-WF-012
never execute persisted ResolvedAction
```

---

# 28. Design Review Conclusion

MVP4.1 的核心不是创造一个“强大的 Workflow Engine”，而是证明：

> **Workflow 可以在不获得 Effect 权限、不拥有 Resolver、不拥有 PluginBroker 的情况下，安全地编排多个 Action。**

因此最终分工固定为：

```text
Workflow
    = Orchestration

ActionResolver
    = Authorization + Applicability + Freshness

ActionEngine
    = Effect Gateway

EffectProvider / PluginBroker
    = Effect Executor
```

这使未来的 AI 和 MCP 可以严格复用同一个模型：

```text
AI
 ↓
ActionProposal
 ↓
ActionResolver
 ↓
ActionEngine

MCP
 ↓
ActionProposal
 ↓
ActionResolver
 ↓
ActionEngine
```

而不需要为 AI / MCP 再造执行体系。

### 评审时我会重点盯住 3 个点

**第一，线性 Workflow 是否足够。**我倾向于 v0.1 就坚持线性 Step。先证明 `Invocation → Re-resolve → Engine → Failure Policy`，不要提前建设 DAG/条件/循环。

**第二，Timeout Retry 的语义必须明确是 at-least-once。**这会直接影响以后文件操作、上传、Plugin RPC 等 Effect 是否需要 idempotency key；不能把“新的 execution_id”误解成“不会重复副作用”。

**第三，`ActionInvocation` 与 `ResolvedAction` 必须从类型层面尽可能分开。**如果 Rust 结构上两者很容易互相转换或被 Workflow 持久化，INV-049/050 很容易在后续实现中被无意违反。最理想的是让持久化模型根本没有 `ResolvedAction` 字段。

这份设计如果通过评审，下一步就可以直接冻结 **`WORKFLOW-CONTRACT-v0.1`、状态枚举和 Failure Policy 默认矩阵**，然后再拆 MVP4.1 的代码任务。
