基于已确认的融合方向，我建议把这一版正式定义为 **`VISUAL-DESIGN-SPEC-v0.1`**。这次冻结的是“视觉语言 + 交互视觉规则 + Slint 映射”，而不是具体页面代码；因此后续 Slint 实现应以本规范为视觉基线，不再自行发明颜色、间距、状态样式。

# VISUAL-DESIGN-SPEC-v0.1

**Status:** FROZEN v0.1
**Design Direction:** Native Dark Minimal Launcher
**Basis:** Visual Fusion — 方案 3 产品外观 + 方案 2 Design System + 方案 1 Workflow 状态表达
**UI Contract:** `UI-CONTRACT-v0.1`
**Platform:** Windows 10 / 11
**UI Runtime:** Rust + Slint
**Primary Renderer:** Software Renderer
**Scope:** Visual language, design tokens, component appearance, focus, motion, four UI modes, Slint mapping

---

# 1. Design Direction

## 1.1 Product Character

Launcher 的视觉气质：

```text
Native
Dark
Minimal
Focused
Quiet
Keyboard-first
High-contrast
Low-chrome
Fast
```

核心感受：

> **打开即用，看到重点，完成操作，立即消失。**

---

## 1.2 Visual Priorities

优先级固定：

```text
1. Focus
2. Readability
3. Actionability
4. Hierarchy
5. Density
6. Decoration
```

装饰不得牺牲：

```text
Search readability
Result scanning
Keyboard focus
Action discoverability
```

---

## 1.3 Visual Complexity Budget

默认 Main Surface：

```text
Level 1: Search
Level 2: Results
Level 3: Context / Action affordance
```

不允许通过增加：

```text
cards
toolbars
sidebars
tabs
dashboards
```

来填充空间。

---

# 2. Color Tokens

默认主题：

> Dark Theme

所有颜色必须通过 Token 使用。

禁止 Slint 页面直接散落 hex literal。

---

## 2.1 Surface

| Token                 | Value     | Usage                            |
| --------------------- | --------- | -------------------------------- |
| `bg.canvas`           | `#0B1017` | Launcher 主背景                     |
| `bg.surface`          | `#111822` | 主内容面                             |
| `bg.surface_elevated` | `#16202C` | Focus / Hover / Elevated surface |
| `bg.input`            | `#0F1721` | Search Box                       |
| `bg.selected`         | `#173F73` | Selected row                     |
| `bg.selected_hover`   | `#1B4A86` | Selected + hover                 |
| `bg.disabled`         | `#10161E` | Disabled row                     |

---

## 2.2 Border

| Token            | Value     | Usage          |
| ---------------- | --------- | -------------- |
| `border.default` | `#26323F` | 普通边界           |
| `border.subtle`  | `#1E2833` | Divider        |
| `border.focus`   | `#4DA3FF` | Keyboard focus |
| `border.warning` | `#FFB547` | Confirmation   |
| `border.error`   | `#FF5C5C` | Error          |

---

## 2.3 Text

| Token            | Value     | Usage                  |
| ---------------- | --------- | ---------------------- |
| `text.primary`   | `#E6ECF2` | 标题 / 主文本               |
| `text.secondary` | `#A6B0BF` | 描述                     |
| `text.tertiary`  | `#778395` | 次要信息                   |
| `text.disabled`  | `#5A6470` | disabled               |
| `text.inverse`   | `#081019` | Accent background 上的文字 |

---

## 2.4 Accent / Semantic

| Token                  | Value     | Usage                       |
| ---------------------- | --------- | --------------------------- |
| `accent.primary`       | `#4DA3FF` | Focus / Selection / Primary |
| `accent.primary_hover` | `#63B0FF` | Hover                       |
| `semantic.success`     | `#36C77A` | Success / Complete          |
| `semantic.warning`     | `#FFB547` | Confirmation / Warning      |
| `semantic.error`       | `#FF5C5C` | Error / Failed              |
| `semantic.info`        | `#5AA9FF` | Informational               |
| `semantic.paused`      | `#FFB547` | Paused                      |
| `semantic.running`     | `#4DA3FF` | Running                     |

---

## 2.5 Color Rules

禁止：

```text
仅使用颜色区分 Selected
仅使用颜色区分 Disabled
仅使用颜色区分 Workflow state
```

必须配合：

```text
shape
symbol
text
focus indicator
```

---

# 3. Typography

## 3.1 Font

Primary UI font：

```text
Segoe UI
```

Fallback：

```text
Microsoft YaHei UI
Segoe UI Symbol
```

Monospace：

```text
Cascadia Mono
```

仅用于：

```text
diagnostic
execution_id
technical details
```

---

## 3.2 Scale

| Token                | Size | Weight   | Usage                   |
| -------------------- | ---: | -------- | ----------------------- |
| `type.title`         | 16px | Semibold | Window / Workflow title |
| `type.body`          | 14px | Regular  | Main content            |
| `type.body_emphasis` | 14px | Semibold | Selected / important    |
| `type.secondary`     | 13px | Regular  | Description             |
| `type.caption`       | 12px | Regular  | Status / metadata       |
| `type.shortcut`      | 12px | Medium   | Shortcut                |
| `type.diagnostic`    | 11px | Regular  | Diagnostic              |

---

## 3.3 Line Height

```text
Title       22px
Body        20px
Secondary   18px
Caption     16px
Diagnostic  15px
```

---

## 3.4 Typography Rules

禁止：

```text
大量粗体
全部大写
过多颜色文字
过多字号变化
```

层级主要通过：

```text
size
weight
spacing
contrast
```

形成。

---

# 4. Spacing

基础单位：

```text
4px
```

推荐 Scale：

```text
4 / 8 / 12 / 16 / 20 / 24 / 32
```

---

## 4.1 Component Spacing

| Context                 |       Spacing |
| ----------------------- | ------------: |
| Icon ↔ Text             |          12px |
| Title ↔ Subtitle        |         2–4px |
| Row ↔ Row               | 0–1px divider |
| Search internal         |          12px |
| Panel padding           |          12px |
| Main horizontal padding |          12px |
| Context bar             |        8–12px |
| Section separation      |          16px |

---

## 4.2 Row Height

Main Result Row：

```text
44px
```

Action Row：

```text
40px
```

Context Hint：

```text
28px
```

Status line：

```text
28px
```

Workflow Step：

```text
36px
```

Confirmation content：

```text
auto
```

---

# 5. Radius

| Token                 | Value | Usage                |
| --------------------- | ----: | -------------------- |
| `radius.small`        |   4px | Badge / shortcut     |
| `radius.medium`       |   6px | Search / row         |
| `radius.large`        |   8px | Launcher surface     |
| `radius.confirmation` |   8px | Confirmation surface |

默认 Launcher window：

```text
8px
```

避免大圆角。

禁止：

```text
16px+
```

作为默认 Launcher UI radius。

---

# 6. Elevation

Launcher 使用非常克制的 elevation。

## Level 0

```text
background
```

## Level 1

```text
surface
border
```

## Level 2

```text
surface_elevated
```

## Level 3

只允许用于：

```text
Confirmation emphasis
```

默认阴影：

```text
offset: 0 8px
blur: 32px
opacity: 45%
```

不要使用多重巨大阴影。

---

# 7. Window

默认：

```text
width  = 600px
height = 420px
```

允许：

```text
min-width  = 480px
min-height = 320px

max-width  = 960px
max-height = 680px
```

---

## 7.1 Window Behavior

```text
Always-on-top
No traditional title bar
Native resize when configured
High-DPI aware
Multi-monitor aware
```

Launcher window 不显示：

```text
browser chrome
traditional application toolbar
sidebar
```

---

# 8. Icon System

## 8.1 Style

采用：

```text
Monoline / outline
1.5px visual stroke
16–20px default size
```

图标整体保持：

```text
simple
recognizable
non-decorative
```

---

## 8.2 Core Icons

```text
Search
Application
Plugin
File
Folder
Action
Copy
Paste
Open
Reveal
Terminal
Settings
Warning
Error
Info
Check
Pause
Pending
Skip
```

---

## 8.3 Workflow Symbols

冻结：

```text
✓  Complete
●  Running
○  Pending
⚠  Failed
⏸  Paused
⊘  Skipped
```

符号必须配合文本，不得只显示 symbol。

---

# 9. Focus System

Focus 是本产品最重要的视觉元素之一。

## 9.1 Selected Row

Selected row：

```text
background = bg.selected
left marker = ▸
text = text.primary
```

不依赖颜色。

---

## 9.2 Keyboard Focus

Focus Ring：

```text
1px accent.primary
```

需要强调时：

```text
2px outer ring
```

建议：

```text
inner padding 2px
outer gap 1–2px
```

---

## 9.3 Focus Priority

视觉优先级：

```text
Focused
  >
Selected
  >
Hovered
  >
Normal
```

但 Selected 与 Focus 在同一对象上时不重复增加过多边框。

---

# 10. Hover

Hover 是辅助视觉反馈，不是主要状态。

默认：

```text
background subtle elevation
```

不要让 Hover 比 Selection 更醒目。

触控板 / Mouse interaction 可以增强 Hover，但 Keyboard focus 永远优先。

---

# 11. Disabled

Disabled：

```text
opacity ≈ 50%
text.disabled
no selection marker
no focus
no activation
```

必须同时显示 reason：

```text
Requires clipboard.write
```

不得只有颜色。

---

# 12. Shortcut

Shortcut 显示为 compact token：

```text
Ctrl+Shift+C
```

视觉：

```text
12px medium
secondary text
surface elevated
4px radius
```

Shortcut 永远位于 Action Row / Result Row 的右侧。

---

# 13. Badge

只用于真正需要分类的信息：

```text
App
Plugin
Primary
Suggested
```

不要把所有状态都 Badge 化。

---

# 14. Divider

默认 divider：

```text
1px border.subtle
```

尽量依靠：

```text
spacing
alignment
surface contrast
```

而不是大量线条。

---

# 15. Main Mode

## 15.1 Structure

```text
┌─────────────────────────────────────┐
│ Search                              │
├─────────────────────────────────────┤
│ Result                              │
│ Result                              │
│ Result                              │
│ Result                              │
├─────────────────────────────────────┤
│ Context              Ctrl+↓ Actions │
└─────────────────────────────────────┘
```

---

## 15.2 Search Box

```text
height = 36px
radius = 6px
horizontal padding = 12px
icon = 16px
```

Search focus：

```text
border.focus
```

Placeholder：

```text
text.tertiary
```

---

## 15.3 Result Row

```text
height = 44px
left icon = 16px
title = 14px
subtitle = 13px
```

Selected：

```text
▸ + bg.selected
```

Selected 不允许依赖背景颜色 alone。

---

# 16. Main Search States

## Searching

```text
⌛ Searching
```

采用：

```text
caption
info
```

不得阻断输入。

---

## Empty

```text
No results
```

视觉保持安静。

---

## Error

```text
⚠ Search failed: Calculator Plus — Timeout
```

Error 必须区别于 Empty。

---

# 17. Action Mode

Action Mode 是：

> 当前 Command 的第二层能力选择。

---

## 17.1 Structure

```text
┌─────────────────────────────────────┐
│ ← Calculator Plus                  │
├─────────────────────────────────────┤
│ ▸ Copy                     Ctrl+... │
│   Paste                             │
│   Open History                      │
│   Execute                           │
└─────────────────────────────────────┘
```

---

## 17.2 Primary Action

显示：

```text
Enter
```

在右侧作为 shortcut affordance。

Primary 同时由：

```text
is_primary
```

决定。

UI 不重新推算。

---

## 17.3 Secondary

按 Core 声明顺序。

---

## 17.4 Disabled

示例：

```text
Execute command
Requires shell.execute
```

显示但不可选。

---

# 18. Confirmation Substate

不是独立 Mode。

结构：

```text
Action
  ↓
ConfirmationPending
```

---

## 18.1 Visual

推荐：

```text
warning accent
small warning symbol
clear title
short explanation
Confirm / Cancel
```

Example:

```text
┌─────────────────────────────────────┐
│ ⚠ Confirm action                    │
│                                     │
│ Copy “6 × 7 = 42” to clipboard     │
│                                     │
│ This action modifies the clipboard. │
│                                     │
│ [Enter Confirm]   [Esc Cancel]      │
└─────────────────────────────────────┘
```

---

## 18.2 Confirmation Rules

第一 Enter：

```text
→ ConfirmationPending
→ no Effect
```

第二 Enter：

```text
→ Confirm
→ Re-resolve
→ ActionEngine
```

Esc：

```text
→ Action.Normal
```

---

# 19. Workflow Mode

Workflow UI 是 Runtime Surface。

不是 Builder。

---

## 19.1 Structure

```text
┌─────────────────────────────────────┐
│ Organize Downloads             Paused│
├─────────────────────────────────────┤
│ ✓ Scan files                 Done   │
│ ✓ Classify files             Done   │
│ ⏸ Move files                 Confirm│
│ ○ Generate report            Pending│
├─────────────────────────────────────┤
│ Enter Confirm     Esc Keep paused   │
└─────────────────────────────────────┘
```

---

## 19.2 Step Visual Language

| State    | Symbol | Accent    |
| -------- | ------ | --------- |
| Complete | `✓`    | Success   |
| Running  | `●`    | Primary   |
| Pending  | `○`    | Secondary |
| Failed   | `⚠`    | Error     |
| Paused   | `⏸`    | Warning   |
| Skipped  | `⊘`    | Disabled  |

颜色不是唯一表达。

---

## 19.3 Current Step

Current step：

```text
background = bg.selected
left marker = ▸
state symbol visible
```

---

## 19.4 Workflow Status

顶部显示：

```text
Running
Paused
Completed
Failed
```

不要同时显示多个 redundant status badges。

---

# 20. Workflow Confirmation

Workflow confirmation 继续使用 Action Confirmation 的视觉语言。

例如：

```text
⏸ Waiting for your confirmation

Move 27 files?

Enter Confirm
Esc Keep paused
```

不得重新设计第二套 Confirmation UI。

---

# 21. Workflow Error

Level 2：

```text
⚠ Move files
  Permission denied
```

优先显示：

```text
symbol
step title
user-facing error
```

Level 3：

```text
execution_id
failure_class
native error
```

隐藏于 Diagnostic。

---

# 22. AI Proposal

AI Proposal 使用普通 Command / Action presentation。

可以增加轻量：

```text
✨ Suggested
```

但不使用：

```text
AI Mode
AI Card
AI Chat Panel
```

---

## 22.1 AI Suggested Action

例如：

```text
✨ Copy result
   Copy “6 × 7 = 42”
                         Ctrl+Shift+C
```

视觉上仍然是普通 Action。

---

# 23. Runtime / Status

统一状态视觉：

```text
● Ready
⌛ Searching
● Running
⏸ Waiting
✓ Complete
⚠ Failed
⊘ Skipped
```

状态应使用：

```text
symbol + text
```

而不是仅颜色。

---

# 24. Failure Presentation

## Level 1

```text
⚠ Search failed
```

## Level 2

```text
⚠ Compress PDF
  Plugin unavailable
```

## Level 3

Diagnostic：

```text
Plugin: ...
Execution: e-103
Failure: PluginUnavailable
Native error: ...
```

Level 3 默认不出现。

---

# 25. Motion

整体原则：

> Motion supports feedback, not decoration.

---

## 25.1 Window

Open：

```text
fade + scale
120ms
ease-out
0.97 → 1.0
```

Close：

```text
100ms
```

---

## 25.2 Mode Transition

Main → Action：

```text
80ms
```

Main → Workflow：

```text
100ms
```

Confirmation：

```text
80ms
```

不允许大幅 slide。

---

## 25.3 Result Update

Search result update：

```text
20ms
```

避免每次结果变化重新播放完整 animation。

---

## 25.4 State Change

Success / Failed：

```text
80–120ms
```

Workflow step transition：

```text
100ms
```

---

## 25.5 Reduced Motion

如果系统要求 reduced motion：

```text
disable scale
disable decorative animation
keep state transition instantaneous
```

---

# 26. Interaction Feedback

## Selection

```text
▸
+ highlighted background
+ focus ring when keyboard focused
```

## Pressed

```text
surface compression ≈ 8%
```

禁止明显缩放。

## Disabled

```text
50% visual attenuation
reason text
no pressed state
```

---

# 27. Context Hint

默认：

```text
📁 Documents
```

或：

```text
✦ Calculator Plus
```

右侧：

```text
Ctrl+↓ Actions
```

---

## Context Hint Rules

Context Hint：

```text
informational
compact
low contrast
```

不能成为一个大型 Context Card。

Action / Workflow 打开时隐藏。

---

# 28. Status Line

Status line 位于底部。

结构：

```text
[Context]                    [Status]
```

例如：

```text
📁 Documents       3 results   ● Ready
```

但禁止显示太多指标。

---

# 29. Button System

只使用两级：

```text
Primary
Secondary
```

Primary：

```text
accent.primary
text.inverse
```

Secondary：

```text
bg.surface_elevated
text.primary
border.default
```

Danger button 不作为长期独立 Button 类型；危险性主要由 Confirmation state 表达。

---

# 30. Component Inventory

P0：

```text
LauncherWindow
SearchBox
ResultList
ResultRow
ContextHint
StatusLine

ActionPanel
ActionRow
ShortcutBadge
ConfirmationView

WorkflowRuntime
WorkflowStepRow
WorkflowStatus

RuntimeMessage
DiagnosticDetails
```

P1/P2：

```text
Tooltip
ContextInspector
WorkflowHistory
WorkflowBuilder
AI Chat Surface
```

暂不实现。

---

# 31. Slint Component Mapping

推荐组件目录：

```text
launcher-ui/
└── ui/
    ├── app.slint
    ├── launcher-window.slint
    ├── search-box.slint
    ├── result-list.slint
    ├── result-row.slint
    ├── context-hint.slint
    ├── status-line.slint
    ├── action-panel.slint
    ├── action-row.slint
    ├── confirmation-view.slint
    ├── workflow-runtime.slint
    ├── workflow-step-row.slint
    ├── runtime-message.slint
    └── theme.slint
```

---

# 32. Theme Mapping

所有视觉 Token 集中在：

```text
theme.slint
```

例如概念映射：

```slint
export global Theme {
    property <color> bg_canvas;
    property <color> bg_surface;
    property <color> bg_surface_elevated;
    property <color> bg_selected;

    property <color> text_primary;
    property <color> text_secondary;
    property <color> text_tertiary;

    property <color> accent_primary;
    property <color> success;
    property <color> warning;
    property <color> error;

    property <length> radius_small;
    property <length> radius_medium;
    property <length> radius_large;
}
```

具体颜色只能从 Theme 使用。

---

# 33. Presentation Mapping

UI 不直接消费 Domain。

推荐：

```text
Domain
   ↓
Presentation Model
   ↓
Slint properties
```

---

## Command

```text
CommandPresentation
    ↓
ResultRow
```

## Action

```text
ActionPresentation
    ↓
ActionRow
```

## Workflow

```text
WorkflowRunView
    ↓
WorkflowRuntime
```

## Runtime

```text
RuntimeStatusPresentation
    ↓
StatusLine / RuntimeMessage
```

---

# 34. Selection Mapping

UI Selection：

```text
command_id
action_id
step_id
```

视觉状态：

```text
selected
focused
disabled
is_primary
```

禁止 UI：

```text
index → semantic state
```

---

# 35. Main Mode Slint Tree

```text
LauncherWindow
└── MainView
    ├── SearchBox
    ├── ResultList
    │   └── ResultRow*
    ├── ContextHint
    └── StatusLine
```

---

# 36. Action Mode Slint Tree

```text
LauncherWindow
└── ActionView
    ├── ActionHeader
    ├── ActionList
    │   └── ActionRow*
    └── StatusLine
```

Confirmation：

```text
ActionView
└── ConfirmationView
```

而不是第二个 Window。

---

# 37. Workflow Mode Slint Tree

```text
LauncherWindow
└── WorkflowRuntime
    ├── WorkflowHeader
    ├── StepList
    │   └── WorkflowStepRow*
    ├── RuntimeMessage
    └── WorkflowControls
```

---

# 38. Window State Composition

最终状态：

```text
Launcher
│
├── Main
│
├── Action
│   └── ConfirmationPending
│
└── Workflow
```

禁止：

```text
Main + Action + Workflow simultaneously visible
```

除非未来专门设计多-pane mode。

---

# 39. Responsive Rules

虽然 Launcher 主要是固定尺寸，但允许：

```text
480–960px
```

---

## Small

```text
480–560px
```

隐藏部分：

```text
long subtitles
secondary source hints
```

保留：

```text
title
primary state
shortcut
```

---

## Medium

```text
560–720px
```

默认布局。

---

## Large

```text
720px+
```

允许增加：

```text
description
context
```

但不要增加新的导航层。

---

# 40. High DPI

目标：

```text
100%
125%
150%
175%
200%
```

所有尺寸使用 Slint logical pixels。

禁止针对 DPI 硬编码：

```text
different absolute layouts
```

---

# 41. Light Theme

v0.1 不作为第一实现重点，但 Token 必须保留语义化命名。

即：

```text
bg.surface
```

而不是：

```text
darkGray1
```

这样以后可以：

```text
Dark Theme
Light Theme
High Contrast Theme
```

共享 Component。

---

# 42. High Contrast

不能假设 accent color 永远可用。

High Contrast 下：

```text
focus
selected
disabled
state
```

仍必须通过：

```text
shape
text
icon
outline
```

表达。

---

# 43. Accessibility

至少保证：

```text
Selected
Focused
Disabled
Running
Paused
Failed
Complete
```

都有非颜色表达。

---

# 44. Keyboard-First Visual Hierarchy

快捷键不是隐藏功能。

用户应该始终能看到：

```text
Enter
Ctrl+↓
Esc
Ctrl+Shift+C
```

但不应该出现大型 keyboard help panel。

---

# 45. Animation Constraints

任何动画：

```text
≤ 200ms
```

默认：

```text
80–120ms
```

禁止：

```text
springy
bounce
large parallax
continuous background animation
```

---

# 46. Visual Performance

禁止：

```text
blur-heavy background
large translucent surfaces
continuous shadows
animated gradients
per-frame complex effects
```

优先：

```text
flat surface
simple border
simple accent
simple opacity
```

这与低内存、低 CPU 的 Launcher 定位一致。

---

# 47. Forbidden Visual Drift

后续实现不得自行加入：

```text
Glassmorphism
Large gradients
Neumorphism
Dashboard cards
Persistent sidebar
AI chat panel
MCP tab
Large toolbar
Complex breadcrumbs
Decorative illustrations
Persistent notification center
```

除非另开 Design Review。

---

# 48. Design Tokens → Slint Mapping

| Design Layer | Source of Truth             |
| ------------ | --------------------------- |
| Color        | `theme.slint`               |
| Typography   | `theme.slint` / font config |
| Spacing      | shared constants            |
| Radius       | `theme.slint`               |
| Elevation    | shared surface styles       |
| Icon         | icon component              |
| Focus        | shared focus component      |
| Motion       | shared animation constants  |
| Mode         | `app.slint` state           |
| State symbol | Workflow/status mapping     |
| Text         | Presentation Model          |

---

# 49. Implementation Rules

### Rule 1

禁止组件自定义新的颜色。

### Rule 2

禁止组件直接读取 Domain semantic enum。

### Rule 3

禁止组件通过 row index 推断 identity。

### Rule 4

禁止组件直接调用 Effect。

### Rule 5

禁止 Action / Workflow 创建新的 window。

### Rule 6

任何新的视觉状态必须先映射到 Presentation Model。

### Rule 7

任何新增 Design Token 必须在 `VISUAL-DESIGN-SPEC` 中登记。

---

# 50. Visual Acceptance Checklist

## Global

```text
[ ] Dark Minimal visual language
[ ] ≤3 default hierarchy
[ ] No dashboard drift
[ ] No second window
[ ] No browser UI
```

## Main

```text
[ ] Search visually dominant
[ ] Result scanning < 1 glance
[ ] Selected has ▸ marker
[ ] Selected not color-only
[ ] Context compact
```

## Action

```text
[ ] Primary clearly visible
[ ] Shortcut right aligned
[ ] Disabled visibly disabled
[ ] Disabled reason readable
[ ] No hidden semantic logic in UI
```

## Confirmation

```text
[ ] Warning hierarchy obvious
[ ] Confirm / Cancel visible
[ ] First Enter does not execute
[ ] No persistent authorization visual
```

## Workflow

```text
[ ] Step states distinguishable
[ ] Current step obvious
[ ] Failure attached to step
[ ] Paused distinct
[ ] Progress understandable
[ ] No Workflow Builder
```

## AI

```text
[ ] Suggested is lightweight
[ ] No AI mode
[ ] No AI execution affordance bypass
```

## Accessibility

```text
[ ] Non-color state representation
[ ] Keyboard complete
[ ] Focus visible
[ ] Disabled reason visible
[ ] Workflow status text available
```

---

# 51. Visual Baseline

后续所有 UI Screenshot / visual regression 以以下基准为准：

```text
Theme:
    Dark

Window:
    600 × 420 logical px

Corner:
    8px

Base spacing:
    4px

Primary row:
    44px

Action row:
    40px

Context:
    28px

Primary accent:
    #4DA3FF

Surface:
    #111822

Background:
    #0B1017

Focus:
    1–2px accent outline

Motion:
    80–120ms default
```

---

# 52. Frozen Design Statement

> **Launcher uses a native dark minimal visual system centered on focus, readability, and keyboard-first interaction. Scheme 3 defines the product surface; Scheme 2 defines the reusable design system; Scheme 1 defines Workflow and runtime state expression. The UI remains low-chrome, transient, and information-light by default, while complex lifecycle states appear only when required.**

---

# 53. Relationship to UI-CONTRACT

```text
UI-CONTRACT-v0.1
        │
        │ defines
        ▼
Behavior / State / Interaction
        │
        ▼
VISUAL-DESIGN-SPEC-v0.1
        │
        │ defines
        ▼
Visual / Layout / Motion / Component appearance
        │
        ▼
Slint Implementation
```

因此：

```text
UI Contract
    = What happens

Visual Design Spec
    = What it looks like

Slint
    = How it is rendered
```

视觉规范不能反向改变 UI 行为契约。

---

# 54. Freeze Boundary

本规范冻结：

```text
✅ Color tokens
✅ Typography
✅ Spacing
✅ Radius
✅ Elevation
✅ Icon style
✅ Focus treatment
✅ Hover / Disabled
✅ Motion
✅ Main visual layout
✅ Action visual layout
✅ Confirmation visual state
✅ Workflow Runtime visual language
✅ AI Proposal visual treatment
✅ Runtime / Error visual language
✅ Accessibility visual requirements
✅ Slint component mapping
```

不冻结：

```text
❌ Workflow Builder
❌ AI Chat
❌ MCP UI
❌ Advanced Settings UI
❌ Plugin UI Schema
❌ Dashboard
❌ Full Theme Editor
```

这些进入后续独立 Design Review。

### 当前建议的实现顺序

```text
VISUAL-DESIGN-SPEC-v0.1
          ↓
Theme / Tokens
          ↓
Shared primitives
          ↓
Main
          ↓
Action
          ↓
Confirmation
          ↓
Workflow Runtime
          ↓
Runtime / Error details
          ↓
Visual regression
```

这里有一个比较重要的工程决策：**不要先把整个 `app.slint` 按最终设计重写。**先把 `theme.slint + shared primitives + ResultRow/ActionRow/StatusLine` 建起来，再逐步替换现有 UI。这样可以保持你目前已经达到 Fully Conformant 的行为基线，同时只改变视觉层。
