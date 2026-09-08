# UI-CONTRACT-v0.1（FROZEN · Canonical）

**Status:** FROZEN v0.1（Addendum 1：两版本合并，2026-09-05）
**Canonical:** 本文件是 UI 行为契约的唯一权威版本；外部 GPT 版已 SUPERSEDED。
**Scope:** Launcher UX Architecture
**Depends on:** ACTION-CONTRACT v0.1 / WORKFLOW-CONTRACT v0.1 / PLUGIN-CONTRACT v0.1.x / ADR-0014 / ADR-0015 / ADR-0016 / ADR-0017
**Platform:** Windows-first
**UI Technology:** Rust + Slint native UI
**Primary principle:** UI is a projection and interaction layer, not a semantic or execution authority.

> **Addendum 1（合并决议）**：本契约由仓库版（B）与外部 GPT 版（A）合并而成。吸收 A1–A7（Main 细分状态 / Workflow.Cancelled / 焦点保持规则 / execution_id / 新 SLO / 50 条细粒度验收 / 显式不定义清单）；**D1 决议：Result Row 单击 = 选中并执行 primary**（Windows launcher 惯例，修正 A 的"仅选择"表述）；保留 B1 禁止迁移清单、B2 Workflow surface 位置语义、B3 Confirmation status-line 绑定、B4 实现状态标注、B5 INV-062~065 链接。验收集替换为 UI-ACC-001~050（含 ✅/⏳ 实现状态标注）。

---

## 1. Scope / Principles

### 1.1 Scope

冻结：UI Mode / State / Presentation Model / Focus / Keyboard-Mouse Events / State Transition / Search-Result / Context / Action / Confirmation / Workflow Runtime / AI Proposal / Runtime-Failure presentation / Accessibility / Stable ID / Forbidden behaviors / Performance / Acceptance。

**显式不定义**（归各自 Domain/Runtime Contract）：ActionResolver 语义、Capability policy、Effect 语义、Workflow 调度语义、AI planning 算法、Plugin RPC、MCP 协议。

### 1.2 UI Responsibility

负责：Present / Select / Navigate / Invoke / Confirm / Observe。
不负责：Resolve Action / Authorize Capability / Execute Effect / Resolve Context / Run Workflow / Generate AI Proposal / Invoke PluginBroker。

### 1.3 Core Boundary

所有执行入口保持：Producer → Action/ActionProposal → ActionResolver → ResolvedAction → ActionEngine → Effect。UI 不得创建第二套执行路径。

### 1.4 Transient Workspace

Launcher 是围绕当前 Context 进行 Command Discovery 与 Action Execution 的瞬时工作空间（Open → Discover → Select → Execute → Close）。不得默认演化为 Dashboard / 常驻工作台 / 多页面应用。

### 1.5 Visual Complexity Budget

默认主界面信息层级 ≤3：Search / Results / Context-Action affordance。Workflow 与 Confirmation 是 transient state，不是永久信息层。

## 2. UI Modes

三个顶层 Mode：**Main / Action / Workflow**。Confirmation 是 **Action Mode 的子状态**（`Action.ConfirmationPending`），不是独立 Mode。AI / Plugin / Context / Runtime / Failure / Shortcut 是内容来源或运行状态，不是 Mode。Action 与 Main 使用同一 Launcher window，不创建第二窗口。

### 2.1 Main
SearchInput / ResultList / ContextHint / Status——搜索、发现、选择、primary 执行、Context 提示。

### 2.2 Action
选中命令的 Action 展示与选择；子状态 `Action.Normal` / `Action.ConfirmationPending`。

### 2.3 Workflow
仅 Runtime Surface：`Workflow Runtime UI = YES / Workflow Builder = NO`。不执行 Workflow 逻辑。

## 3. State Model

### 3.1 Main
`Main.Idle | Main.Searching | Main.Results | Main.Empty | Main.Error`——实现可合并呈现，但 Presentation 层必须能表达全部。

### 3.2 Action
`Action.Normal | Action.ConfirmationPending`。

### 3.3 Workflow
`Workflow.Running | Paused | Completed | Failed | Cancelled`。`Paused` 必须能表达 `Reason = ConfirmationRequired`；其他 pause reason 不在 v0.1 建立用户可操作语义。

### 3.4 Runtime State
`Ready / Disabled / Executing / Success / Failure / Unavailable / Retrying`——由 Core/Runtime 提供，UI 不自行推断。

### 3.5 State Authority
禁止：UI observes raw error / UI guesses failure class / UI infers primary from index / UI infers capability from action type。

## 4. Presentation Models

### 4.1 CommandPresentation
`command_id, title, subtitle?, icon?, selected, shortcut?, source?`。禁止暴露 capabilities / ActionKind 语义 / Effect 实现 / authorization state。

### 4.2 ActionPresentation
`action_id, title, enabled, reason?, shortcut?, is_primary`。`is_primary` 由 Core 的 `Command::primary_action()`（first Ready）计算；UI 禁止 `actions[0]` 推导。✅ 已实现（INV-035/036）。

### 4.3 WorkflowStepPresentation ⏳
`step_id, title, state, error?(UserFacingError)`。UI 不读取 ResolvedAction / Effect / CapabilityGrant / PluginHandle。

### 4.4 WorkflowPresentation ⏳
`workflow_run_id, title, state, current_step?, completed_steps, total_steps, steps[]`。

### 4.5 RuntimeStatusPresentation
统一词汇：`Ready / Running / Paused / Retrying / Succeeded / Failed / Unavailable`。内部 FailureClass 不直接作为默认 UI 文案。

## 5. Focus Model

- **5.1 Main**：SearchInput 获得逻辑焦点。
- **5.2 Result Navigation**：↑/↓ 移动 selection（按 command_id 标识），不得依赖 index 作逻辑 identity。
- **5.3 Action**：逻辑焦点 = selected action；disabled action 不得成为 focus target。
- **5.4 Confirmation**：焦点位于确认交互；只能产生 Confirm/Cancel，禁止 UI 自动确认。
- **5.5 Workflow**：逻辑焦点 = 当前可操作控件；**Running 且无用户操作时 UI MUST NOT steal focus**。
- **5.6 焦点保持**：Presentation 更新不得无条件重置用户焦点——仅 Mode transition / 选中对象失效 / 焦点目标消失时例外。

## 6. Keyboard Events

### 6.1 Main
↑ 上一命令；↓ 下一命令；Enter 执行 primary；Ctrl+↓ 打开 Action；Esc 关闭 Launcher。

### 6.2 Action
↑/↓ 在 **enabled** action 间移动；Enter 执行选中；Esc 回 Main。disabled：visible / not selectable / not executable。

### 6.3 Confirmation（Action 子状态）
Enter = Confirm；Esc = Cancel / 回 Action.Normal。Confirmation 不改变 Action identity。

### 6.4 Shortcut
`shortcut → selected command → action_id → execute_action_by_id()`；shortcut 永不直接绑定 Effect。

### 6.5 Workflow
Running：Esc 只关闭 Runtime Surface，不取消已提交 Effect。Paused(ConfirmationRequired)：Enter = Confirm/Resume（resume 后必须 re-resolve）；Esc = Keep paused / close UI。

## 7. Mouse Events

Mouse 与 Keyboard 使用相同逻辑事件、同一 stable ID 执行路径。

- **7.1 Command Row（D1 决议）**：`Click = select + execute primary`（Windows launcher 惯例：单击即执行，与 Raycast/PowerToys Run 一致；不采用"仅选择"文件管理器范式）。
- **7.2 Action Row**：Click enabled action → `execute_action_by_id()`；Click disabled → 无执行。
- **7.3 Confirmation**：Click Confirm/Cancel → 对应逻辑事件；UI 不直接调用 Effect。
- **7.4 Workflow Step**：Click = Select/Inspect；**禁止因点击 Step 而重新执行该 Step**。

## 8. State Transition Matrix

### 8.1 Main
| Current | Event | Next | Effect |
|---|---|---|---|
| Main.Idle/Results | QueryChanged | Searching | 提交查询（query generation 递增，旧结果不得覆盖新结果——INV-012 呼应） |
| Searching | Results | Results | 更新呈现 |
| Searching | Empty | Empty | 空态 |
| Results | Ctrl+↓ | Action | 打开面板 |
| Results | Enter | closed | 执行 primary |
| Results/Empty | Esc | closed | 关闭 Launcher |

### 8.2 Action
| Current | Event | Next |
|---|---|---|
| Action.Normal | ↑/↓ | Action.Normal（跳过 disabled） |
| Action.Normal | Enter / 执行成功 | closed |
| Action.Normal | 执行失败 | Action.Normal + status |
| Action.Normal | confirmation required | ConfirmationPending |
| Action.Normal | Esc | Main（并取消 pending confirmation） |
| ConfirmationPending | Enter（同 action） | 确认 → 执行 |
| ConfirmationPending | Esc / 更换选择 | Action.Normal（取消 pending） |

### 8.3 Workflow
| Current | Event | Next |
|---|---|---|
| Running | step complete | Running / Completed |
| Running | confirmation required | Paused(ConfirmationRequired) |
| Running | recoverable failure | Running（Retry 展示） |
| Running | terminal failure | Failed |
| Paused | originating UI confirm | Running（re-resolve 后执行） |
| Completed/Failed | dismiss | closed / Main |

### 8.4 Context
Context changes → Core re-resolution / updated Presentation → UI 消费呈现更新。v0.1 不自动重写当前 Action selection；stale invocation → reject / non-blocking feedback；下一次 invocation 才执行新 resolution。

### 8.5 禁止迁移（B1，保留）
Main → 直接 Executing；ConfirmationPending → 跳过 engine 直接 Complete；任何 mode → 携带旧 ResolvedAction 进入执行；UI → 直接 Effect/Broker。

## 9. Search / Result Contract

SearchInput：low latency / incremental / non-blocking / keyboard-first。Result Row 最小信息：Icon / Title / Description / Optional source / Optional shortcut；禁止默认显示内部 identity。查询带 generation，旧结果不得覆盖新结果。Selection 由 command_id 标识；禁止 selected_index / title matching / row object identity 作业务 identity。Empty state：`No results`，不得强制引导 AI。

## 10. Context Presentation

Context 默认 invisible input；仅在帮助理解结果时轻量提示（`📁 Documents`）。允许显示 location/application/window/relevant selection 的用户可理解形式；禁止 hwnd / generation / internal provider state / raw ContextSnapshot。Context generation 由 Core 管理，UI 只消费 Presentation。StaleContext 不作为默认用户错误文案（"Action updated" 或非阻断提示；具体 wording 由 Runtime Failure Presentation 提供）。

## 11. Action Presentation

顺序保持 Core 声明顺序。Primary = first Ready，经 `is_primary` 呈现，UI 不重算。Disabled：visible / dimmed / reason visible / not selectable / not executable（示例 `Execute command — Requires shell.execute`）；UI 不提供不存在的 permission management 入口。Hidden：不呈现、不可选、不可执行（unknown/invalid 不进入 ActionPresentation）。Shortcut 显示于 Action Row。所有执行经 `command_id + action_id`。

## 12. Confirmation

位置 = `Action.ConfirmationPending`（非独立 Mode）。第一阶段 Enter 只进入 Pending 不执行 Effect；第二阶段 Enter 才 Confirm → ActionEngine。Confirmation 是 transient execution state，禁止持久化 `confirmed=true` 或等价永久授权。Workflow：ConfirmationRequired → Paused，恢复仅来自 originating UI session，恢复后 re-resolve → engine。呈现绑定 status line + Action 子状态（B3）。

## 13. Workflow Runtime

只表现 Run state / Current step / Progress / Pause / Failure / Completion。Step Presentation 状态至少：Pending / Current / Complete / Skipped / Failed（映射自 runtime）。Running 时 UI 只 observe，不控制 execution order / retry / re-resolve / failure policy。Paused(ConfirmationRequired) 显示 `⏸ Waiting for your confirmation`。Failure 以 Step 为主（`Move files ⚠ Permission denied`）而非 `Workflow failed`。Retry 展示 runtime 已决定的 `Retrying... Attempt 2 of 2`，UI 不主动决定。v0.1 不向用户承诺 automatic crash recovery（Serializable ≠ Resumable）。

## 14. AI Proposal Presentation

AI 不是 UI Mode；ActionProposal 进入现有 Command/Action UX。可选来源标记 `✨ Suggested`，不得表示 AI authorized/trusted/confirmed。执行链：Proposal → ActionInvocation → ReferenceResolver → ActionResolver → ActionEngine，UI 不为 AI 建立独立 execution path。AI Planner 失败 = Producer unavailable / planning failure，不是 Action failed（除非已形成 Action 进入执行）。

## 15. Runtime / Failure Presentation

统一词汇：Ready / Running / Paused / Retrying / Succeeded / Failed / Unavailable。三级失败呈现：Inline（`⚠ Plugin unavailable`）→ Detail（step/reason/attempt）→ Diagnostic（Plugin / Execution ID / Failure Class / Native error，仅用户主动查看）。UI 消费 classified failure → UserFacingError，不通过 native exception 自行推导 FailureClass。示例：CapabilityDenied `⚠ Requires clipboard.write`；PluginUnavailable `⚠ Plugin unavailable`；BusinessError `⚠ Could not compress the selected file`（不是 "Plugin crashed"，除非实际类是 ProtocolViolation）；StaleContext 非阻断提示。普通 Action 成功 → close popup；Workflow 完成 → `✓ Completed`。

## 16. Accessibility

非颜色强制：每个语义状态至少一种非颜色通道（禁只依赖 color/opacity/border）。Disabled 必含 visible reason。Confirmation 必须提供 `Enter Confirm / Esc Cancel` 文本。Workflow 状态（Running/Paused/Completed/Failed/Current step）可经文本/语义表达。所有 P0 功能仅键盘可达。

## 17. Stable ID Rules

逻辑 identity：`command_id / action_id / step_id / workflow_run_id / execution_id`。禁止 array index / visible index / display title / localized text / row position / Slint element position。Command identity = `(provider_id, command.id)`；action.id 在 Command scope 内稳定；`workflow_run_id` 标识一次运行；`execution_id` 标识一次最终 Effect execution attempt，Retry 产生新 id。

## 18. Forbidden UI Behaviors

18.1 直接 Effect 执行（UI→OS API / UI→PluginBroker / UI→Plugin RPC）。18.2 Action 语义解释（经 ActionKind/requires/capability/effect_type 自行决定 enabled/primary/executable）。18.3 位置选择（actions[0] / results[index] 作 identity）。18.4 自主授权（grant capability / assume authorization / assume confirmation）。18.5 持久化 confirmed=true。18.6 AI 执行通道（AI→Effect/PluginBroker/OS API）。18.7 MCP 执行通道（MCP→Effect/PluginBroker）。18.8 Workflow UI 决定 retry/skip/re-resolve/effect order。18.9 第二窗口。18.10 WebView/WebView2/CEF/Electron。

## 19. Performance Constraints

服从 transient / low-memory / keyboard-first。Interaction SLO：Warm Popup P95 ≤ 10ms；Action Panel Open P95 ≤ 10ms；Action Selection P95 ≤ 5ms。Search：App P95 ≤ 10ms / File P95 ≤ 50ms。执行：Enter → Effect P95 ≤ 50ms（A5）；plugin cold/warm 单独 benchmark。UI Memory 记录（A5）：hidden idle / visible idle / Action Panel visible / Action Panel hidden / Workflow visible——主指标 Private Bytes，Working Set 辅助。UI thread 禁止 blocking IO / heavy CPU / Plugin RPC / filesystem scan / network。

## 20. Acceptance Tests（UI-ACC-001~050）

状态标注：✅ 已实现并有测试/实现锚点；⏳ 待实现（Workflow Runtime Surface / 细化呈现）；◐ 部分实现。

### 20.1 Main
- ✅ UI-ACC-001 Hotkey opens Main
- ✅ UI-ACC-002 Search input receives focus
- ✅ UI-ACC-003 Keyboard navigation selects commands
- ✅ UI-ACC-004 Enter executes primary
- ✅ UI-ACC-005 Esc closes Launcher

### 20.2 Action
- ✅ UI-ACC-006 Ctrl+↓ opens Action Mode
- ✅ UI-ACC-007 First Ready action is marked primary（`is_primary`，launcher-ui 单测）
- ✅ UI-ACC-008 Disabled action visible but not selectable
- ✅ UI-ACC-009 Unknown action is not displayed（host resolver 丢弃）
- ✅ UI-ACC-010 Execution uses command_id + action_id
- ✅ UI-ACC-011 Keyboard and mouse use the same execution path

### 20.3 Shortcut
- ✅ UI-ACC-012 Shortcut is rendered
- ✅ UI-ACC-013 Shortcut resolves to action_id
- ✅ UI-ACC-014 Shortcut never bypasses ActionEngine

### 20.4 Confirmation
- ✅ UI-ACC-015 First request enters ConfirmationPending
- ✅ UI-ACC-016 No Effect before confirmation（engine gate）
- ✅ UI-ACC-017 Second Enter confirms
- ✅ UI-ACC-018 Confirmation not persisted（Esc = Cancel 清除 pending）
- ⏳ UI-ACC-019 Workflow confirmation enters Paused（UI surface 待 P0-UI-007；runner 逻辑已 ✅）
- ⏳ UI-ACC-020 Workflow resume re-resolves（runner 逻辑已 ✅，CAT-WF-009）

### 20.5 Context
- ✅ UI-ACC-021 Context changes consumed through Presentation
- ✅ UI-ACC-022 UI does not calculate context generation
- ✅ UI-ACC-023 Stale invocation cannot silently execute（`context_is_stale` 守卫）

### 20.6 Workflow
- ⏳ UI-ACC-024~030（Workflow Runtime Surface，P0-UI-007）：Running 可见 / Current step 非颜色区分 / Paused 可见 / failure 关联 step / retry 状态呈现非 UI 发起 / Completion 呈现 / UI 不执行 workflow 逻辑

### 20.7 AI
- ◐ UI-ACC-031 AI proposals render through normal Action UX（管道 ✅；提案的 UI 呈现标记 ⏳）
- ✅ UI-ACC-032 AI has no dedicated execution path（`execute_proposals`）
- ✅ UI-ACC-033 AI proposal cannot claim authorization（结构性丢弃）

### 20.8 Runtime / Failure
- ✅ UI-ACC-034 CapabilityDenied has non-color representation（reason 文本）
- ✅ UI-ACC-035 PluginUnavailable distinguishable from BusinessError（分类映射）
- ✅ UI-ACC-036 ProtocolViolation presented as Runtime-level failure
- ✅ UI-ACC-037 StaleContext not exposed as raw internal error
- ⏳ UI-ACC-038 Failure details inspectable separately（Level 2/3 诊断呈现待实现）

### 20.9 Accessibility
- ◐ UI-ACC-039 Every P0 state has non-color representation（disabled/confirmation ✅；selected 行当前仅 accent ⏳ 需文本/焦点标记）
- ✅ UI-ACC-040 All P0 actions keyboard reachable
- ✅ UI-ACC-041 Disabled states expose reasons
- ⏳ UI-ACC-042 Workflow state semantically represented

### 20.10 Architecture
- ✅ UI-ACC-043 No direct UI → Effect executor path
- ✅ UI-ACC-044 No direct UI → PluginBroker path（broker 仅经 engine 结果路由）
- ✅ UI-ACC-045 No AI-specific execution channel
- ✅ UI-ACC-046 No MCP-specific execution channel
- ✅ UI-ACC-047 No Electron/WebView/CEF dependency
- ✅ UI-ACC-048 Stable IDs for logical selection
- ✅ UI-ACC-049 ActionPresentation.primary is Core-derived
- ⏳ UI-ACC-050 Workflow UI consumes Presentation state only

**统计：✅ 39 / ◐ 2 / ⏳ 9（全部集中于 Workflow Runtime Surface 与诊断呈现，即 P0-UI-007/008）。**

---

## Appendix A — Canonical UI Lifecycle

```text
Main → select command → Action → execute/shortcut → (Confirmation → Enter) → ActionEngine → Effect
Workflow: Definition → Run → StepRun → ActionInvocation → Reference Resolution → Resolver → Engine → Effect → Runtime State → Presentation
AI: AI Planner → ActionProposal → existing Action path
MCP: MCP Adapter → ActionProposal → existing Action path
```

## Appendix B — Mode Summary

| Mode | Purpose | Primary Focus | Can Execute? |
|---|---|---|---|
| Main | Search / discovery | Search / Result | Yes, through ActionEngine |
| Action | Action selection | Selected Action | Yes, through ActionEngine |
| Action.ConfirmationPending | Confirm pending execution | Confirmation | No Effect before confirm |
| Workflow | Runtime observation | Current actionable state | No direct execution |

## Appendix C — Frozen Boundaries

```text
UI → Presentation → Core state → ActionResolver → ActionEngine → Effect
```
禁止反向绕行：UI ─X→ Effect / PluginBroker / Capability grant / Resolver semantics；AI ─X→ Effect；MCP ─X→ Effect；Workflow UI ─X→ Effect。

## Appendix D — Contract Statement

> **The Launcher UI is a transient, keyboard-first projection and interaction layer. It presents Core-provided state, selects stable identities, requests actions, and observes results. It does not interpret Action semantics, grant authority, execute Effects, run Workflows, or create independent AI/MCP/Plugin execution paths.**
