# 173 — P3-UI.1 尾款：Base64 示例工具 + 弹窗入口接线

> 日期：2026-09-11。范围：P3-UI.1 收尾——Base64 示例工具 + 弹窗入口 +
> tool event 回调接线。输入基线：history/172（925 tests）。

## 交付

- **Base64 示例工具**：
  - `apps/example-testplugins/base64_tool.py`：完整工具协议——
    initialize / query（返回工具发现条目）/ tool.open（返回 UI schema:
    input + encode/decode 按钮 + output）/ tool.event（encode/decode
    操作 + 返回更新后的 UI with 结果）；
  - `base64_tool_plugin.json`：manifest 声明 `window: true` +
    `tools` 数组（interactive entry）；
  - 安装位置：`%LOCALAPPDATA%\native-launcher\plugins\com.example.base64\`。
- **弹窗入口接线**：`ToolCommandProvider`（`tool-open` 命名空间，搜索
  `base64` 或 `tool` 出现 "🔐 Open Base64 Tool" 入口）→
  `execute_action_by_id` 路由 → `tool_ui_host::open_tool(manifest_path,
  "base64", ui_weak)` → 发送 `tool.open` → 渲染 UI schema。
- **tool event 回调**：`ui.on_tool_event` → `tool_ui_host::relay_event`
  → plugin 的 `tool.event` 方法 → 返回更新后的 UI → 刷新 ToolSurface
  节点列表。
- **tool close**：Esc 或 Close → `tool_ui_host::close_session` → 发送
  `tool.close` → 关闭 ToolSurface → 清理会话。

## Gate 结果

- `cargo test --workspace`：**925 passed / 0 failed**（不变）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 手工测试步骤

1. 启动启动器（debug 或 release）
2. 搜索 `base64` 或 `tool` → 选中 "🔐 Open Base64 Tool" → Enter
3. ToolSurface 打开，显示 Input text_area + Encode/Decode 按钮 + Output text_area
4. 在 Input 输入 `hello world` → 点击 Encode → Output 显示 `aGVsbG8gd29ybGQ=`
5. 点击 Decode → Output 恢复 `hello world`
6. 按 Esc 关闭 ToolSurface，回落搜索框

## 待续（P3-UI.1 尾款）

- `tool.update` 通知接收（plugin → host 通知，非 request/response）
- Base64 fixture 的 rich result 联动（查询结果 + 详情面板 rich 页）
