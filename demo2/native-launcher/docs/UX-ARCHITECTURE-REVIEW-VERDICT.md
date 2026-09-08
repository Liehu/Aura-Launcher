# UX Architecture Review（review 29-mvp6-0.1）— 内部评审意见

**Date:** 2026-09-05
**Reviewed:** `demo2/files2/29-mvp6-0.1.md`（"Review 25 — Launcher UX Architecture Review" 草案，57 节 + UI-001~015 + UI-INV-001~010 + P0-UI-001~012）
**对照:** 当前已实现 UI（`crates/launcher-ui`、`apps/launcher-app`、`app.slint`）与冻结语义（INV-035/036/037、MVP3.1/3.2、WORKFLOW/AI 契约）

---

## 评审结论：APPROVE WITH REVISIONS

> **状态更新（2026-09-05）**：R1 已实现（is_primary）；R2–R5 已被 review 30 确认并全部吸收进 **`UI-CONTRACT-v0.1.md`（FROZEN，ADR-0017）**。Review 25 正式关闭。

草案的核心判断全部正确且与已实现架构一致：瞬时工作空间定位（§3）、UI 职责六词（Present/Select/Invoke/Confirm/Observe/Navigate）、四个 UI mode（§47 Main/Action/Confirmation/Workflow）、AI/MCP/Context 不是 UI mode（§48/§49）、Workflow UI 是 Runtime Surface 不是 Builder（§50）、无第二窗口（UI-013）、失败分级三层（§17）、StaleContext 不暴露为错误文案（§30）、CapabilityDenied 不伪造设置入口（§31）、同窗口 Confirmation（§11/12）、Esc 不取消已提交 Effect（§26）、视觉复杂度预算 ≤3 层（§46）。**建议冻结 UI-001~015 并进入 UI-CONTRACT-v0.1 收敛**——与草案自己的结论一致。

以下 5 个修订点，其中 R1 是评审过程发现并**已修复**的实现 bug。

---

## R1（已修复）Primary 身份的定位推导违反冻结语义

草案 §9 要求"UI 不自己推导 first action = primary，应消费 Core 已解析的 presentation"。对照实现发现 UI 正是用 `ai == 0`（行位置）渲染 Enter 徽标——而 host 会保留 declaration 顺序中的 Disabled action，`actions[0]` 可能是 Disabled，真正的 primary（first Ready）在后面。

**修复（已落地）：** `ActionPresentation/ActionItem` 新增 `is_primary`（由 `Command::primary_action()`——first Ready 语义的唯一权威——计算），Slint 徽标与 accent 高亮改用 `a.is-primary`；单测断言"first Ready = primary、Disabled 永非 primary"。

## R2（契约措辞修订）StaleContext UX 与现实现矛盾

草案 §30 说 StaleContext 应显示"Action updated"或静默刷新。但当前冻结语义是：**Popup Session 内 Context 冻结（INV-011），执行时 generation 不匹配 → 拒绝执行 + 提示**。session 冻结模型下不存在"session 内静默刷新"的时机。

**修订：** §30 改为——"session 内：stale = 拒绝执行 + 非阻断提示（现状，正确）；下一次 invocation 天然携带新 Context（即静默刷新）。'Action updated 自动刷新'属于 v0.2 context-refresh 能力（对应 MVP3.2-C 预留），v0.1 不承诺"。

## R3（消歧）Confirmation 是 mode 还是 status？

§11 说 Confirmation 是"transient state（状态栏内完成）"，§47 又把 Confirmation 列为四个 UI mode 之一。**修订：** Confirmation 是 **Action mode 的子状态**（confirmation-pending 标志 + 状态行呈现），不是独立顶层 mode。四 mode 定稿为 **Main / Action(main+confirmation 子状态) / Workflow**——或者保留四 mode 命名但在 UI-CONTRACT 中明确 Confirmation 无独立焦点/键位表（继承 Action mode 的 Enter/Esc）。建议后者，避免键位表重复定义。

## R4（编号去重）UI-INV-001~010 与现有 INV 大量重叠

UI-INV-001≈INV-035、002≈INV-036、003≈INV-037/004、004≈INV-048、006≈INV-047、010≈MVP3.1 冻结语义。**修订：** 不新增重复 invariant；UI-INV 中真正新的只有 UI-INV-005（Workflow UI 只表现状态）/007（MCP 无独立执行路径）/008（Context 变更经 Core Presentation 消费）/009（失败呈现消费分类结果）→ 收编为 **INV-062~065**；其余在 UI-CONTRACT 中引用现有 INV 编号。

## R5（Accessibility 落地粒度）§44 需要可验证条款

当前 disabled 行有 reason 文本（非纯视觉 ✓），但 selected 状态仅用 accent 颜色表达。UI-CONTRACT 中应冻结：每个状态（selected/disabled/confirmation）至少有一个非颜色通道（文本标记、焦点环、或 Slint accessibility API）；作为 P0-UI-010 的验收项。

---

## 确认项（冻结，无修订）

§4 搜索增量更新不做 loading screen；§5/§12 Context 隐式化 + 轻量提示（"📁 Documents"）；§7 Result→Actions 认知层级（"先决定做什么，再决定怎么做"）；§10 shortcut 只是展示、执行仍走 id；§13 confirmed 不持久；§20/21/22 AI 提案 = 普通 Action + "✨ Suggested" 轻量来源标记、无 AI confirmation 特例；§23 搜索框不做 "Ask AI anything"；§25 Empty state 不推 AI；§27 所有入口（键/鼠/快捷键/未来语音）收敛到同一 stable ID 执行路径；§28 无全屏 spinner；§34 "AI unavailable"是 Producer unavailable 不是 Action failed；§35 Workflow 后台运行 + ConfirmationRequired 时 launcher 重开；§37 UI 不承诺崩溃恢复；§45 性能约束（复用窗口与组件，禁第二窗口/嵌入式浏览器/每插件 UI 进程）；§49 MCP 无 UI。

## UI-CONTRACT-v0.1 收敛清单（下一步）

按草案 §56 的 P0-UI-001~012 顺序，UI-CONTRACT 需对四个 mode 各自冻结：状态集合、事件（键/鼠）、焦点规则、Presentation Model、允许/禁止迁移。已有实现可直接作为基线输入：Main（search+results+context hint+status line）、Action（panel+shortcut+disabled 呈现+is_primary 徽标）、Confirmation（status-line 子状态）、Workflow（未实现，P0-UI-007）。
