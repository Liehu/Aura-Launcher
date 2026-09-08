# ADR-0012: Native Action UI Boundary

日期：2026-09-04　状态：Accepted　来源：review 18-mvp3.1-0.1（实现）+ review 19-mvp3.2（冻结确认）

## Decision（冻结的一句话）

> **ActionPanel 只负责 ActionPresentation 的呈现与选择；ActionResolver 负责把不可信 Action 声明转换为可执行对象；ActionEngine 是所有 Effect 的唯一执行入口。**

## Invariants（本 ADR 固化的 UI 层边界）

- INV-035 UI 是 projection 不是真相源：只持有 query 文本、selected command/action id、展示状态；选择永远按稳定 id，禁止位置索引/显示文本。
- INV-036 UI 不解释 Action 语义：只见 `ActionPresentation{id,title,enabled,reason}`，不接触 ActionKind/capability/ContextSnapshot。
- INV-037 UI 不直接执行 Effect：一切执行经 ActionEngine；其 `validate` 拒绝 Disabled action，是唯一执行闸门。
- INV-038 Hidden action 不可执行：unknown/invalid 在 host resolver 层丢弃，永不进入 Command/UI。
- （INV-039 = INV-031 已有：secondary 失效不连坐 Command，UI 层同此约束。）

## 实现（MVP3.1 P0）

同窗口双模式（main ↔ action），无第二窗口；Ctrl+↓ 开面板、↑/↓ 导航（跳过 disabled）、Enter 执行、Esc 关面板；失败以状态行呈现，已提交 effect 不因 Esc 取消。Performance gate（launcher-bench `action_panel_us`，10k iterations，50 actions 半 disabled）：panel_open P95 = 4µs（gate ≤10ms）、selection ≈ 0µs（gate ≤5ms）、execute_bridge P95 = 1µs。

## Addendum（2026-09-04，真实环境复现）

winit 的 hide()/show() 循环在本机复现出致命缺陷：`hide()` 后窗口进入 iconic 状态且渲染面失效——`IsIconic=true`、重 show 后 OS 报 visible+foreground 但内容永不重绘（用户视角"热键无效"）。`SW_RESTORE + TOPMOST + request_redraw + recenter` 均不能可靠救回。

**最终方案：窗口常驻映射，永不调用 hide()。**"隐藏"改为 `park()`——把窗口移到屏幕外（-32000,-32000）并释放键盘焦点（`SetFocus(None)`）；"显示"即 recenter + repaint（对活跃窗口只是 move + move，无状态机）。代价：parked 窗口仍在 Alt-Tab 列表、仍占用一份 surface 内存。收益：彻底消除 show/hide 状态机这类后端缺陷面。

## Consequences

后续 UI Schema、plugin.* RPC、Workflow、AI、MCP 一律从此边界向外扩展——它们都是新的 Command Producer 或呈现面，不得把 action 语义解释塞回 UI。
