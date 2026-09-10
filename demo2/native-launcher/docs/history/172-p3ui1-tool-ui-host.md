# 172 — P3-UI.1 Tool UI Host：声明式 UI Schema → Slint 渲染

> 日期：2026-09-11。范围：P3-UI.1 首批——Tool UI Host 渲染层。
> 输入基线：history/171（925 tests）。

## 交付

- **`crates/launcher-ui/ui/tool-surface.slint`**（新）：`ToolSurface`
  组件——从扁平化节点列表渲染声明式 UI，按 kind 分发到对应 Slint
  控件（text/markdown→Text、text_field/text_area→LineEdit、button/
  action_button→ThemedButton、key_value→文本行、separator→分隔线、
  checkbox→勾选 + 标签、spacer→间距）；depth 编码驱动缩进；
  close-requested / tool-event 回调上抛宿主。
- **AppWindow 集成**：`tool-visible` 互斥模式 + `in-tool` 状态参与
  mode-height 计算；`tool-nodes` / `tool-title` / `tool-status` /
  `detail-lines` 属性穿透；Esc / Close 关闭回落主模式。
- **宿主 `tool_ui_host.rs`**（launcher-app）：
  - `flatten(schema)`：将 UiSchema 树扁平化为 ToolNodeItem 列表
    （depth 编码缩进）；
  - `open_tool`：发送 `tool.open` → 解析返回的 UiSchema → 校验 →
    投影到 ToolSurface；
  - `relay_event`：ToolSurface 用户操作 → `tool.event` → 解析 UI
    update → 刷新节点列表；
  - 会话生命周期管理（session_slot）。

## 后置（B2）

- `tool.update` 接收（当前 plugin → host 通知路径未接线）
- 多 session 并存
- Base64 完整示例插件

## Gate 结果

- `cargo test --workspace`：**925 passed / 0 failed**（不变）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
