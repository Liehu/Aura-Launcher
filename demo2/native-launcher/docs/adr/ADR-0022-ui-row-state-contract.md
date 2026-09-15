# ADR-0022: UI Row State Contract（Selected ≠ Focused，冻结）

**状态：ACCEPTED（冻结——后续 Agent 不得破坏）**
**适用：launcher-ui 全部行级组件（ResultRow 及未来的任何行类组件）**

## 契约

行状态是**四个独立属性**，不是一枚枚举：

```text
ResultRow
├── selected          // semantic result state（当前语义选中）
├── keyboard-focused  // input/navigation focus state（键盘焦点）
├── hovered           // pointer state（指针悬停）
└── pressed           // transient interaction state（按压瞬态）
```

**禁止**实现为 `state = selected | focused | hovered | pressed` 单字段枚举。
枚举形状短期简单，长期会把触摸、远程输入、Screen Reader、UI Automation
全部耦合进互斥假设——这些输入源完全可以同时成立（指针悬停 + 键盘焦点 +
语义选中并存是常态）。

## 视觉语义（token 级）

| 属性 | 视觉 | token |
|---|---|---|
| selected | 选中 pill 底色 | `Theme.bg-selected`（hover 叠加 `bg-selected-hover`） |
| keyboard-focused | **焦点描边环**（`border-focus`），永远不是第二层蓝底 | `Theme.border-focus` |
| hovered | 行底色微变 | `Theme.bg-hover-row` |
| pressed | 瞬态反馈（透明度） | 就地 opacity |

## 附加冻结项（同轮）

1. **SectionHeader 是独立组件**（`result-row.slint::SectionHeader`）：
   无 TouchArea、无 hover/selected/execute，小号大写标签 + 发丝线；禁止
   用"disabled ResultRow"模拟 header——cursor、鼠标、a11y 树、自动化都
   必须把它视为结构而非候选。
2. **键盘 cursor 空间 = 结果空间**：遍历解析在 Rust
   （`launcher_ui::result_nav` / `row_of_result` / `cursor_step`），slint
   只上报意图并渲染返回的行号。header 在 cursor 空间中由构造不存在
   （P3-D 契约的实现载体）。

## 验证

- 单测：`launcher-ui` lib（p3d_* / p3i_* 系列，28 项）。
- 静态：`scripts/check_feature_presence.py` 登记 `keyboard-focused`、
  `SectionHeader`、`bg-hover-row`、`result_nav`、`ResultListRow`。
