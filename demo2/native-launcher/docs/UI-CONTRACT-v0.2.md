# UI-CONTRACT-v0.2（P1-E 冻结，review 60 §67）

状态：**已实现并冻结**（P1-E.1 + P1-E.2 交付）。实现位置：`crates/launcher-ui/src/viewmodel.rs`（纯 DTO，无 Slint 类型、无 Core 运行时句柄）。

## 1. 边界

- 顶层 Mode 冻结为 `UiMode::{Main, Action, Workflow}`（§4）。Agent / MCP / Runtime 是 panel 内容，**永远不是**新顶层模式。
- UI 只能发 `UiCommand::{Select, Invoke, Confirm, Cancel, Navigate, Close, ToggleDiagnostics}`（§60）。不存在 ExecuteEffect / Authorize / Grant。
- Core→UI 通知：`UiEvent::{SearchUpdated, ResultsUpdated, ActionStateChanged, WorkflowUpdated, AgentUpdated, ConfirmationRequired, DiagnosticsUpdated}`（§61）。
- 焦点模型：`FocusTarget::{Search, Results, Actions, Confirmation, Workflow, Diagnostics}`（§30），禁止组件内部散落 bool。

## 2. 状态→UI 词汇映射（全部是全函数，INV-UI-009）

| 域 | 枚举 | 文案 |
|---|---|---|
| 空状态（§6） | `EmptyState` | Start typing... / No matching commands / No providers available / **Provider temporarily unavailable**（≠ No Results） |
| 禁用原因（§11） | `DisabledReason` | Not permitted / Requires a folder / Context changed / Input is invalid / Temporarily unavailable / Confirmation required |
| 连接状态（§24/§26） | `ProviderStatus` | Connected / Starting… / Reconnecting… / Unavailable —— Reconnecting 是 info 级，**不是 error**（§45） |
| 步骤/活动状态（§15） | `ActivityStatus` | ○ pending / ◐ running / ✓ ok / ! failed / – skipped（图标+文本双通道，禁止仅颜色） |
| Agent 状态（§21） | `AgentTerminalStatus` | Running / **Replanning…**（永不写 Retrying）/ Completed / Failed / Budget exhausted / Cancelled |
| Toast（§46/§65） | `ToastSubject` | 仅 Copied / Saved / Completed / Disconnected 允许 toast；Confirmation、SecurityDenial、WorkflowFailure 必须进入真实 UI state |

## 3. Agent 视图（§18–§23）

`AgentView`（goal、`Budget{used,total}`×2、phases、proposals、replanning、status）通过 `agent_run_view()` / `show_agent_run()` 投影到**共享的 workflow runtime panel**。Budget 行格式 `Turn 3 / 8 · Executions 5 / 16`（最后一个配额时加 `⚠`）。Proposal 一律标注 "proposed"，从不呈现为 authorized action（§20）。

## 4. Timeline（§50）

`ActivityItem { timestamp, kind, title, summary, status }` 是 UI 专用 DTO；宿主负责把 Workflow/Agent/MCP 事件投影为 ActivityItem，timeline 永不持有 Effect/proposal/运行时句柄。timestamp 由生产方固定，视觉 fixture 可确定性（§52/§53）。

## 5. 不变量（§77）

- INV-UI-001/002/003：UI 不构造 Effect、不绕过 Resolver/Engine、不授权 Capability —— 由 `tests/ui_contract.rs` 源码扫描 + `check_topology.py` UI 依赖 guard（launcher-ui ✗ mcp/plugin-host/runtime/ai/core/workflow）双通道强制。
- INV-UI-004：`UiCommand::Confirm` 是无载荷意图，UI 无法自确认。
- INV-UI-005/006/007：viewmodel 无 `pub fn set_*` 变更面（测试强制）。
- INV-UI-008：UI 只消费宿主构建的不可变快照 DTO。
- INV-UI-009：状态拼写只来自上述全函数映射。
- INV-UI-010：`Close ≠ Cancel`（值级断言）；关闭面板不等于回滚已发出的 Effect。

## 6. Theme Tokens v0.2（§40，P1-E.2）

`theme.slint` 追加：`spacing-xs..xl`、`radius-sm/md`、`state-ready/disabled/running/success/warning/error`、`panel-width-main/action/diagnostics`。**只增不改**：v0.1 token 冻结，VR-001..010 基线不因主题变化而漂移。

## 7. Visual Regression v0.2（§51）

VR-001..010 保留，新增：VR-011 Workflow.Success、VR-012 Workflow.Branch（condition-false → goto 文案 + Level-3 诊断）、VR-013 Agent.Running（workflow-style 面板 + budget 行）、VR-014 MCP.Reconnecting（info 级）、VR-015 Main.ProviderUnavailable。共 **15 个确定性基线**，`scripts/release_gate.py` Gate 7 逐字节比对。

VR 捕获 harness 修复（不改应用语义）：`PrintWindow(PW_RENDERFULLCONTENT)` 替代屏幕 BitBlt（被遮挡窗口不再捕获为黑）；显示/居中/置顶在 UI 线程执行；焦点走根 key scope（消除 caret 闪烁不确定性）；BMP 以标准 bottom-up 编码输出。
