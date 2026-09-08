> **SUPERSEDED（2026-09-05）**：本文件已与仓库 canonical 版本合并，权威版本见 `demo2/native-launcher/docs/UI-CONTRACT-v0.1.md`（Addendum 1 合并决议：吸收 A1–A7；D1 决议 Result Row 单击 = 选中并执行 primary）。本文件仅存档。

# UI-CONTRACT-v0.1

**Status:** FROZEN v0.1
**Scope:** Launcher UX Architecture
**Depends on:** ACTION-CONTRACT v0.1 / WORKFLOW-CONTRACT v0.1 / PLUGIN-CONTRACT v0.1.x / ADR-0014 / ADR-0015 / ADR-0016
**Platform:** Windows-first
**UI Technology:** Rust + Slint native UI
**Primary principle:** UI is a projection and interaction layer, not a semantic or execution authority.

---

# 1. Scope / Principles

## 1.1 Scope

本契约冻结 Launcher UI 的：

* UI Mode
* State
* Presentation Model
* Focus
* Keyboard / Mouse Event
* State Transition
* Search / Result presentation
* Context presentation
* Action presentation
* Confirmation
* Workflow Runtime
* AI Proposal presentation
* Runtime / Failure presentation
* Accessibility
* Stable ID
* Forbidden behaviors
* Performance constraints
* Acceptance criteria

本契约不定义：

```text
ActionResolver semantics
Capability policy
Effect semantics
Workflow scheduling semantics
AI planning algorithm
Plugin RPC protocol
MCP protocol
```

这些由各自的 Domain / Runtime Contract 定义。

---

## 1.2 UI Responsibility

UI 只负责：

```text
Present
Select
Navigate
Invoke
Confirm
Observe
```

UI 不负责：

```text
Resolve Action
Authorize Capability
Execute Effect
Resolve Context
Run Workflow
Generate AI Proposal
Invoke PluginBroker
```

---

## 1.3 Core Boundary

所有执行入口最终保持：

```text
Producer
    ↓
Action / ActionProposal
    ↓
ActionResolver
    ↓
ResolvedAction
    ↓
ActionEngine
    ↓
Effect
```

UI 不得创建第二套执行路径。

---

## 1.4 Transient Workspace

Launcher 是：

> 一个围绕当前 Context 进行 Command Discovery 和 Action Execution 的瞬时工作空间。

默认生命周期：

```text
Open
  ↓
Search / Discover
  ↓
Select
  ↓
Execute
  ↓
Close
```

UI 不得默认演化为 Dashboard、常驻工作台或多页面应用。

---

## 1.5 Visual Complexity Budget

默认主界面信息层级不超过三层：

```text
1. Search
2. Results
3. Context / Action affordance
```

Workflow / Confirmation 属于 transient state，不作为主界面的永久信息层。

---

# 2. UI Modes

v0.1 只定义三个顶层 Mode：

```text
Main
Action
Workflow
```

`Confirmation` 是 **Action Mode 的子状态**，不是独立顶层 Mode。

---

## 2.1 Main

用途：

```text
Search
Command Discovery
Command Selection
Context indication
Primary Action execution
```

核心 UI：

```text
SearchInput
ResultList
ContextHint
Status
```

---

## 2.2 Action

用途：

```text
Selected Command 的 Action 展示与选择
```

包含两个子状态：

```text
Action.Normal
Action.ConfirmationPending
```

Action Mode 与 Main Mode 使用同一 Launcher window。

不创建第二窗口。

---

## 2.3 Workflow

用途：

```text
展示 WorkflowRun 的运行状态
```

仅作为 Runtime Surface。

v0.1：

```text
Workflow Runtime UI = YES
Workflow Builder = NO
```

Workflow UI 不执行 Workflow 逻辑。

---

## 2.4 非 Mode 概念

以下不是 UI Mode：

```text
AI
Plugin
Context
Runtime
Failure
Shortcut
```

它们是内容来源、能力来源或运行状态。

---

# 3. State Model

## 3.1 Main State

```text
Main.Idle
Main.Searching
Main.Results
Main.Empty
Main.Error
```

UI 可以将实际实现合并，但 Presentation 层必须能够表达对应状态。

---

## 3.2 Action State

```text
Action.Normal
Action.ConfirmationPending
```

---

## 3.3 Workflow State

```text
Workflow.Running
Workflow.Paused
Workflow.Completed
Workflow.Failed
Workflow.Cancelled
```

其中：

```text
Workflow.Paused
```

必须能够表达：

```text
Reason = ConfirmationRequired
```

其他 Pause reason 不在 v0.1 中建立用户可操作语义。

---

## 3.4 Runtime State

执行相关状态：

```text
Ready
Disabled
Executing
Success
Failure
Unavailable
Retrying
```

Runtime 状态由 Core/Runtime 提供。

UI 不自行推断。

---

## 3.5 State Authority

UI State 必须来源于 Core 提供的 Presentation State。

禁止：

```text
UI observes raw error
UI guesses failure class
UI infers primary action from index
UI infers capability from action type
```

---

# 4. Presentation Models

UI 只消费 Presentation Model。

---

## 4.1 CommandPresentation

建议最小字段：

```rust
struct CommandPresentation {
    command_id: CommandId,
    title: String,
    subtitle: Option<String>,
    icon: Option<Icon>,
    selected: bool,
    shortcut: Option<String>,
    source: Option<String>,
}
```

不得暴露：

```text
capabilities
ActionKind semantics
Effect implementation
authorization state
```

---

## 4.2 ActionPresentation

Action Presentation 至少包含：

```rust
struct ActionPresentation {
    id: ActionId,
    title: String,
    enabled: bool,
    reason: Option<String>,
    shortcut: Option<String>,
    is_primary: bool,
}
```

`is_primary` 由 Core 根据：

```text
Command::primary_action()
```

计算。

UI 禁止通过：

```text
actions[0]
```

推断 primary。

---

## 4.3 WorkflowStepPresentation

建议：

```rust
struct WorkflowStepPresentation {
    step_id: StepId,
    title: String,
    state: StepPresentationState,
    error: Option<UserFacingError>,
}
```

UI 不直接读取：

```text
ResolvedAction
Effect
CapabilityGrant
PluginHandle
```

---

## 4.4 WorkflowPresentation

```rust
struct WorkflowPresentation {
    workflow_run_id: WorkflowRunId,
    title: String,
    state: WorkflowPresentationState,
    current_step: Option<StepId>,
    completed_steps: usize,
    total_steps: usize,
    steps: Vec<WorkflowStepPresentation>,
}
```

---

## 4.5 RuntimeStatusPresentation

所有运行时状态使用统一 Presentation vocabulary：

```text
Ready
Running
Paused
Retrying
Succeeded
Failed
Unavailable
```

内部 FailureClass 不直接作为默认 UI 文案。

---

# 5. Focus Model

## 5.1 Main Focus

进入 Main：

```text
SearchInput
```

获得逻辑焦点。

---

## 5.2 Result Navigation

↑ / ↓：

```text
SearchInput
   ↓
ResultList selection
```

selection 使用：

```text
command_id
```

不得依赖 index 作为逻辑 identity。

---

## 5.3 Action Focus

进入 Action Mode：

```text
selected action
```

成为逻辑 focus。

disabled action 不得成为 focus target。

---

## 5.4 Confirmation Focus

进入：

```text
Action.ConfirmationPending
```

焦点必须位于当前 Confirmation interaction。

确认只能产生：

```text
Confirm
Cancel
```

不允许 UI 自动确认。

---

## 5.5 Workflow Focus

Workflow UI 中：

```text
current actionable control
```

获得逻辑焦点。

如果 Workflow 处于 Running 且没有用户操作：

```text
UI MUST NOT steal focus
```

---

## 5.6 Presentation Update

Presentation 更新不得无条件重置用户当前焦点。

除非：

```text
Mode transition
selected object became invalid
current focus target disappeared
```

---

# 6. Keyboard Events

## 6.1 Main

| Key    | Behavior         |
| ------ | ---------------- |
| ↑      | Previous command |
| ↓      | Next command     |
| Enter  | Execute primary  |
| Ctrl+↓ | Open Action      |
| Esc    | Close Launcher   |

---

## 6.2 Action

| Key   | Behavior                |
| ----- | ----------------------- |
| ↑     | Previous enabled action |
| ↓     | Next enabled action     |
| Enter | Execute selected action |
| Esc   | Return to Main          |

disabled action：

```text
visible
not selectable
not executable
```

---

## 6.3 Confirmation

| Key   | Behavior                  |
| ----- | ------------------------- |
| Enter | Confirm                   |
| Esc   | Cancel / return to Action |

Confirmation 不改变 Action identity。

---

## 6.4 Shortcut

Action shortcut：

```text
Shortcut
  ↓
selected command
  ↓
action_id
  ↓
execute_action_by_id()
```

Shortcut 不直接绑定 Effect。

---

## 6.5 Workflow

Workflow Running：

```text
Esc
```

只关闭 UI / Runtime Surface，不取消已经提交的 Effect。

Workflow Paused(ConfirmationRequired)：

```text
Enter → Confirm / Resume
Esc   → Keep paused / Close UI
```

resume 后必须重新 resolve。

---

# 7. Mouse Events

Mouse 与 Keyboard 使用相同的逻辑事件。

---

## 7.1 Command Row

```text
Click → select command
Double Click → not required
```

---

## 7.2 Action Row

```text
Click enabled action
    ↓
execute_action_by_id()
```

disabled action：

```text
Click
    ↓
no execution
```

---

## 7.3 Confirmation

```text
Click Confirm → Confirm
Click Cancel → Cancel
```

UI 不直接调用 Effect。

---

## 7.4 Workflow

Workflow Step：

```text
Click
    ↓
Select / Inspect
```

不得因为点击 Step 而重新执行该 Step。

---

# 8. State Transition Matrix

## 8.1 Main

| Current   | Event        | Next          | Effect              |
| --------- | ------------ | ------------- | ------------------- |
| Main      | QueryChanged | Searching     | submit query        |
| Searching | Results      | Results       | update presentation |
| Searching | Empty        | Empty         | show empty          |
| Results   | Ctrl+↓       | Action        | open Action         |
| Results   | Enter        | Main / closed | execute primary     |
| Results   | Esc          | closed        | close Launcher      |
| Empty     | Esc          | closed        | close Launcher      |

---

## 8.2 Action

| Current             | Event                   | Next                      |
| ------------------- | ----------------------- | ------------------------- |
| Action.Normal       | ↑/↓                     | Action.Normal             |
| Action.Normal       | Enter / execute success | closed                    |
| Action.Normal       | execution failure       | Action.Normal + status    |
| Action.Normal       | confirmation required   | ConfirmationPending       |
| Action.Normal       | Esc                     | Main                      |
| ConfirmationPending | Enter                   | Action.Normal → execution |
| ConfirmationPending | Esc                     | Action.Normal             |

---

## 8.3 Workflow

| Current            | Event                             | Next                |
| ------------------ | --------------------------------- | ------------------- |
| Workflow.Running   | step complete                     | Running / Completed |
| Workflow.Running   | confirmation required             | Paused              |
| Workflow.Running   | recoverable failure               | Running             |
| Workflow.Running   | terminal failure                  | Failed              |
| Workflow.Paused    | valid originating UI confirmation | Running             |
| Workflow.Completed | dismiss                           | closed / Main       |
| Workflow.Failed    | dismiss                           | closed / Main       |

---

## 8.4 Context

```text
Context changes
    ↓
Core re-resolution / updated Presentation
    ↓
UI consumes presentation update
```

v0.1 不自动重写当前 Action selection。

若当前 invocation 已经 stale：

```text
invocation
    ↓
reject / non-blocking feedback
```

下一次 invocation 才执行新的 resolution。

---

# 9. Search / Result Contract

## 9.1 Search Input

SearchInput 是 Main Mode 的主要交互入口。

要求：

```text
low latency
incremental results
non-blocking
keyboard-first
```

---

## 9.2 Search Results

Result Row 最小信息：

```text
Icon
Title
Description
Optional source
Optional shortcut
```

禁止默认显示内部 identity。

---

## 9.3 Search Update

Search update：

```text
QueryChanged
    ↓
new query generation
    ↓
results update
```

旧结果不得覆盖新结果。

---

## 9.4 Selection

Selected command 由：

```text
command_id
```

标识。

禁止：

```text
selected_index
title matching
row object identity
```

作为业务 identity。

---

## 9.5 Empty State

Empty state 使用简单反馈：

```text
No results
```

不得强制将用户引导到 AI。

---

# 10. Context Presentation

Context 默认是：

> invisible input

只在对用户理解结果确有帮助时显示。

示例：

```text
📁 Documents
```

而不是内部 ContextSnapshot。

---

## 10.1 Context Hint

允许显示：

```text
location
application
window
relevant selection
```

但必须是用户可理解的 Presentation。

禁止显示：

```text
hwnd
generation
internal provider state
raw ContextSnapshot
```

---

## 10.2 Context Change

Context generation 由 Core 管理。

UI：

```text
Context update
    ↓
consume Presentation
```

UI 不自行读取/计算 generation。

---

## 10.3 StaleContext

v0.1 不把：

```text
StaleContext
```

作为默认用户错误文案。

用户看到的可以是：

```text
Action updated
```

或非阻断状态提示。

具体 failure wording 由 Runtime Failure Presentation 提供。

---

# 11. Action Presentation

## 11.1 Action Ordering

Secondary Action 顺序必须保持 Core 提供的声明顺序。

---

## 11.2 Primary

Primary：

```text
first Ready action
```

通过：

```text
is_primary = true
```

呈现。

UI 不得自行重新计算。

---

## 11.3 Disabled

Disabled action：

```text
visible
dimmed
reason visible
not selectable
not executable
```

示例：

```text
Execute command
Requires shell.execute
```

UI 不提供不存在的 permission management entry point。

---

## 11.4 Hidden

Hidden Action：

```text
not presented
not selectable
not executable
```

Unknown / invalid Action 不进入 ActionPresentation。

---

## 11.5 Shortcut

Shortcut 显示在 Action Row。

例如：

```text
Copy result                   Ctrl+Shift+C
```

---

## 11.6 Action ID

所有执行通过：

```text
command_id + action_id
```

不得通过：

```text
row index
```

---

# 12. Confirmation

## 12.1 Position

Confirmation 是：

```text
Action.ConfirmationPending
```

而非独立 UI Mode。

---

## 12.2 Behavior

第一阶段：

```text
Enter
 ↓
ConfirmationPending
```

不得执行 Effect。

第二阶段：

```text
Enter
 ↓
Confirm
 ↓
ActionEngine
```

---

## 12.3 Confirmation State

Confirmation 是 transient execution state。

不得持久化：

```text
confirmed = true
```

或任何等价的永久授权状态。

---

## 12.4 Workflow Confirmation

Workflow：

```text
ConfirmationRequired
    ↓
Workflow.Paused
```

恢复只能来自：

```text
originating UI session
```

恢复后：

```text
Re-resolve
    ↓
ActionEngine
```

---

# 13. Workflow Runtime

## 13.1 Scope

Workflow UI 只表现：

```text
Run state
Current step
Progress
Pause
Failure
Completion
```

---

## 13.2 Example

```text
Organize Downloads

✓ Scan files
✓ Classify files
● Move files
○ Generate report

Running · 3 / 4
```

---

## 13.3 Step States

Presentation 至少支持：

```text
Pending
Current
Complete
Skipped
Failed
```

映射来自 Workflow runtime。

---

## 13.4 Running

Running 时：

```text
UI observes state
```

而不控制：

```text
execution order
retry
re-resolve
failure policy
```

---

## 13.5 Paused

最重要的 v0.1 Pause：

```text
Paused(ConfirmationRequired)
```

显示：

```text
⏸ Waiting for your confirmation
```

---

## 13.6 Failure

Workflow failure 以 Step 为主：

```text
Move files
⚠ Permission denied
```

而不是只有：

```text
Workflow failed
```

---

## 13.7 Retry

UI 展示 Runtime 已经决定的：

```text
Retrying...
Attempt 2 of 2
```

UI 不主动决定 Retry。

---

## 13.8 Crash / Recovery

v0.1 不向用户承诺：

```text
automatic crash recovery
```

因为：

```text
Serializable ≠ Resumable
```

---

# 14. AI Proposal Presentation

## 14.1 AI Is Not a UI Mode

AI Planner 的输出：

```text
ActionProposal
```

进入现有 Command / Action UX。

---

## 14.2 Optional Source Hint

可以显示：

```text
✨ Suggested
```

表示 proposal source。

不得表示：

```text
AI authorized
AI trusted
AI confirmed
```

---

## 14.3 Execution

AI Proposal：

```text
Proposal
  ↓
ActionInvocation
  ↓
ReferenceResolver
  ↓
ActionResolver
  ↓
ActionEngine
```

UI 不为 AI 建立独立 execution path。

---

## 14.4 AI Failure

AI Planner 失败属于：

```text
Producer unavailable / planning failure
```

而不是：

```text
Action failed
```

除非已经形成 Action 并进入 Action execution。

---

# 15. Runtime / Failure Presentation

## 15.1 Presentation Vocabulary

统一状态：

```text
Ready
Running
Paused
Retrying
Succeeded
Failed
Unavailable
```

---

## 15.2 Three-Level Failure Presentation

### Level 1 — Inline

```text
⚠ Plugin unavailable
```

### Level 2 — Detail

```text
Step: Compress PDF
Reason: Plugin unavailable
Attempt: 1 / 2
```

### Level 3 — Diagnostic

用户主动请求时才展示：

```text
Plugin
Execution ID
Failure Class
Native error
```

---

## 15.3 Failure Mapping

UI 消费：

```text
classified failure
    ↓
UserFacingError
```

UI 不通过 native exception 自己推导 FailureClass。

---

## 15.4 Failure Examples

### CapabilityDenied

```text
⚠ Requires clipboard.write
```

### PluginUnavailable

```text
⚠ Plugin unavailable
```

### Plugin Business Error

```text
⚠ Could not compress the selected file
```

### ProtocolViolation

显示 Runtime-level error。

### StaleContext

优先非阻断提示，不默认暴露内部术语。

---

## 15.5 Success

普通 Action：

```text
Success
    ↓
close popup
```

Workflow：

```text
✓ Completed
```

---

# 16. Accessibility

## 16.1 Non-Color Requirement

每个有语义的 UI 状态至少必须存在一种非颜色通道。

禁止只依赖：

```text
color
opacity
border color
```

---

## 16.2 Disabled

必须至少包含：

```text
visible reason
```

例如：

```text
Requires shell.execute
```

---

## 16.3 Confirmation

必须明确提供：

```text
Enter Confirm
Esc Cancel
```

---

## 16.4 Workflow

必须能够通过文本/语义表达：

```text
Running
Paused
Completed
Failed
Current step
```

---

## 16.5 Keyboard Access

所有 P0 功能必须可以仅使用键盘完成。

---

# 17. Stable ID Rules

UI 逻辑 identity 使用：

```text
command_id
action_id
step_id
workflow_run_id
execution_id
```

---

## 17.1 Forbidden Identity

不得使用：

```text
array index
visible index
display title
localized text
row position
Slint element position
```

---

## 17.2 Command Identity

```text
(provider_id, command.id)
```

保持现有 Command Contract。

---

## 17.3 Action Identity

```text
action.id
```

在 Command scope 内稳定。

---

## 17.4 Workflow Identity

```text
workflow_run_id
```

标识一次运行。

---

## 17.5 Execution Identity

```text
execution_id
```

标识一次最终 Effect execution attempt。

Retry 必须产生新的 execution_id。

---

# 18. Forbidden UI Behaviors

以下行为违反本契约。

## 18.1 Direct Effect Execution

```text
UI → OS API
UI → PluginBroker
UI → Plugin RPC
```

禁止。

---

## 18.2 Action Interpretation

UI 不得通过：

```text
ActionKind
requires
capability
effect_type
```

自行决定：

```text
enabled
primary
executable
```

---

## 18.3 Positional Selection

禁止：

```text
actions[0]
results[index]
```

作为逻辑 identity。

---

## 18.4 Autonomous Authorization

禁止 UI 自行：

```text
grant capability
assume authorization
assume confirmation
```

---

## 18.5 Persisted Confirmation

禁止持久化：

```text
confirmed=true
```

作为授权事实。

---

## 18.6 AI Execution Channel

禁止：

```text
AI → Effect
AI → PluginBroker
AI → OS API
```

---

## 18.7 MCP Execution Channel

禁止建立：

```text
MCP → Effect
MCP → PluginBroker
```

独立执行路径。

---

## 18.8 Workflow Execution Logic

Workflow UI 不得决定：

```text
retry
skip
re-resolve
effect order
```

---

## 18.9 Second Window

Action / Confirmation / Workflow Runtime 在 v0.x 不要求第二执行窗口。

---

## 18.10 Embedded Browser UI

Launcher UI 不得引入：

```text
WebView
WebView2
CEF
Electron
```

---

# 19. Performance Constraints

## 19.1 General Principle

UI 性能必须服从：

> transient, low-memory, keyboard-first launcher

不得通过常驻复杂 UI runtime 换取功能。

---

## 19.2 Interaction SLO

沿用当前 Launcher 性能体系：

```text
Warm Popup P95       ≤ 10 ms
Action Panel Open P95 ≤ 10 ms
Action Selection P95 ≤ 5 ms
```

---

## 19.3 Search

目标：

```text
App Search P95  ≤ 10 ms
File Search P95 ≤ 50 ms
```

---

## 19.4 Execution

```text
Enter → Effect P95 ≤ 50 ms
```

Plugin cold/warm execution 单独 benchmark。

---

## 19.5 UI Memory

至少记录：

```text
hidden idle
visible idle
Action Panel visible
Action Panel hidden
Workflow visible
```

主要 Memory metric：

```text
Private Bytes
```

Working Set 作为辅助用户可见指标。

---

## 19.6 Non-Blocking UI

UI thread 不得直接执行：

```text
blocking IO
heavy CPU
Plugin RPC
filesystem scan
network operation
```

---

# 20. Acceptance Tests

## 20.1 Main

```text
UI-ACC-001
Hotkey opens Main.

UI-ACC-002
Search input receives focus.

UI-ACC-003
Keyboard navigation selects commands.

UI-ACC-004
Enter executes primary.

UI-ACC-005
Esc closes Launcher.
```

---

## 20.2 Action

```text
UI-ACC-006
Ctrl+↓ opens Action Mode.

UI-ACC-007
First Ready action is marked primary.

UI-ACC-008
Disabled action is visible but not selectable.

UI-ACC-009
Unknown action is not displayed.

UI-ACC-010
Action execution uses command_id + action_id.

UI-ACC-011
Keyboard and mouse use the same execution path.
```

---

## 20.3 Shortcut

```text
UI-ACC-012
Action shortcut is rendered.

UI-ACC-013
Shortcut resolves to action_id.

UI-ACC-014
Shortcut never bypasses ActionEngine.
```

---

## 20.4 Confirmation

```text
UI-ACC-015
First execution request enters ConfirmationPending.

UI-ACC-016
No Effect occurs before confirmation.

UI-ACC-017
Second Enter confirms.

UI-ACC-018
Confirmation is not persisted as authorization.

UI-ACC-019
Workflow confirmation enters Paused state.

UI-ACC-020
Workflow resume re-resolves before execution.
```

---

## 20.5 Context

```text
UI-ACC-021
Context changes are consumed through Presentation.

UI-ACC-022
UI does not calculate context generation.

UI-ACC-023
Stale invocation cannot silently execute.
```

---

## 20.6 Workflow

```text
UI-ACC-024
Workflow Running is represented visibly.

UI-ACC-025
Current Step is distinguishable without color alone.

UI-ACC-026
Workflow Paused(ConfirmationRequired) is visible.

UI-ACC-027
Workflow failure is associated with the failing Step.

UI-ACC-028
Workflow retry state is presented but not initiated by UI logic.

UI-ACC-029
Completion state is presented.

UI-ACC-030
Workflow UI does not execute Workflow logic.
```

---

## 20.7 AI

```text
UI-ACC-031
AI ActionProposal renders through normal Action UX.

UI-ACC-032
AI has no dedicated execution path.

UI-ACC-033
AI proposal cannot display/claim authorization state.
```

---

## 20.8 Runtime / Failure

```text
UI-ACC-034
CapabilityDenied has non-color representation.

UI-ACC-035
PluginUnavailable is distinguishable from BusinessError.

UI-ACC-036
ProtocolViolation is presented as Runtime-level failure.

UI-ACC-037
StaleContext is not exposed as raw internal error by default.

UI-ACC-038
Failure details can be inspected separately from inline status.
```

---

## 20.9 Accessibility

```text
UI-ACC-039
Every P0 state has a non-color representation.

UI-ACC-040
All P0 actions are keyboard reachable.

UI-ACC-041
Disabled states expose reasons.

UI-ACC-042
Workflow state is semantically represented.
```

---

## 20.10 Architecture

```text
UI-ACC-043
No direct UI → Effect executor path.

UI-ACC-044
No direct UI → PluginBroker path.

UI-ACC-045
No AI-specific execution channel.

UI-ACC-046
No MCP-specific execution channel.

UI-ACC-047
No Electron/WebView/CEF dependency.

UI-ACC-048
Stable IDs are used for logical selection.

UI-ACC-049
ActionPresentation.primary is Core-derived.

UI-ACC-050
Workflow UI consumes Presentation state only.
```

---

# Appendix A — Canonical UI Lifecycle

```text
                      ┌─────────────┐
                      │    Main     │
                      └──────┬──────┘
                             │
                       select command
                             │
                             ▼
                      ┌─────────────┐
                      │   Action    │
                      └──────┬──────┘
                             │
                    execute / shortcut
                             │
                    ┌────────┴────────┐
                    │                 │
                    ▼                 ▼
                 Execute        Confirmation
                    │                 │
                    │               Enter
                    │                 │
                    │                 ▼
                    │              Execute
                    │                 │
                    └────────┬────────┘
                             ▼
                       ActionEngine
                             │
                             ▼
                           Effect
```

Workflow：

```text
WorkflowDefinition
       ↓
WorkflowRun
       ↓
StepRun
       ↓
ActionInvocation
       ↓
Reference Resolution
       ↓
ActionResolver
       ↓
ActionEngine
       ↓
Effect
       ↓
Workflow Runtime State
       ↓
Workflow Presentation
```

AI：

```text
AI Planner
    ↓
ActionProposal
    ↓
existing Action path
```

MCP：

```text
MCP Adapter
    ↓
ActionProposal
    ↓
existing Action path
```

---

# Appendix B — Mode Summary

| Mode                       | Purpose                    | Primary Focus            | Can Execute?              |
| -------------------------- | -------------------------- | ------------------------ | ------------------------- |
| Main                       | Search / Command discovery | Search / Result          | Yes, through ActionEngine |
| Action                     | Action selection           | Selected Action          | Yes, through ActionEngine |
| Action.ConfirmationPending | Confirm pending execution  | Confirmation             | No Effect before confirm  |
| Workflow                   | Runtime observation        | Current actionable state | No direct execution       |

---

# Appendix C — Frozen Boundaries

```text
UI
  ↓
Presentation

Presentation
  ↓
Core command/action/workflow state

Core
  ↓
ActionResolver
  ↓
ActionEngine

ActionEngine
  ↓
Effect
```

禁止任何反向绕行：

```text
UI ─X→ Effect
UI ─X→ PluginBroker
UI ─X→ Capability grant
UI ─X→ Resolver semantics

AI ─X→ Effect
MCP ─X→ Effect
Workflow UI ─X→ Effect
```

---

# Appendix D — Contract Statement

> **The Launcher UI is a transient, keyboard-first projection and interaction layer. It presents Core-provided state, selects stable identities, requests actions, and observes results. It does not interpret Action semantics, grant authority, execute Effects, run Workflows, or create independent AI/MCP/Plugin execution paths.**
