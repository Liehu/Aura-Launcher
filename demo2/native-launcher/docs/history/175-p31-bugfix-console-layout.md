# 175 — P3.1 修复：Base64 工具 Encode/Decode 不生效 + 管理窗口生命周期

> 日期：2026-09-11。范围：用户反馈的两个 bug 修复 + tool event 事件
> 传递链修复。输入基线：history/174。

## 修复 1：Base64 工具 Encode/Decode 不生效

**根因**：`TOOL_SESSION` 使用 `thread_local!` 存储——`open_tool` 在
"tool-opener" 工作线程上创建并存储 PluginHandle，但 `relay_event` 在
**UI 线程**上调用时读到的 `TOOL_SESSION` 是空的（thread_local 按线程
隔离），导致事件静默丢弃。

**修复**：
- `TOOL_SESSION` 改为 `static Mutex<Option<ToolSession>>`（跨线程共享）
- `relay_event` 在 worker 线程上执行 IPC（避免阻塞 UI），完成后通过
  `invoke_from_event_loop` 回传 UI 更新
- `close_session` 从 Mutex 取出会话、发送 `tool.close`，然后隐藏
  ToolSurface

## 修复 2：管理窗口关闭后无法再打开

**根因**：`ManagementWindow` 的 Close 按钮调用 `hide()`，而 winit 的
show/hide 循环会使窗口失效（与弹窗当年遇到的同一问题）。加上 `Weak`
不持有强引用，hide 后组件被销毁，Weak 无法 upgrade。

**修复**：
- 强引用存入 UI 线程的 `thread_local!`（Slint 句柄 !Send，不能存
  static Mutex）
- Close = `park` 移到屏外（不 hide/destroy），重新打开 = show + 居中

## 修复 3：左右布局 + 基线重生成

- `management.slint` 重写为左右布局（1/7 菜单 + 6/7 明细），
  uTools/Wox 风格
- VR 基线 dark+light 全套重新生成，逐字节一致性验证

## Gate 结果

- `cargo test --workspace`：**925 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：ok
