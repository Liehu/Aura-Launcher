# 166 — P3.1 Batch B1：插件窗口发现与置顶（免 hook）

> 日期：2026-09-10。范围：P3.1 第二批。输入基线：history/165（908 tests）。

## 交付

- **pid 访问链（P31-011）**：`ProcessSession::pid()`（launcher-runtime，
  child.id() 直通）→ `PluginHandle::pid()`（plugin-host）。
- **manifest `window` 声明（P31-012）**：`PluginManifest.window: bool`
  （serde default false，rename `"window"`）——插件自声明"我开 UI 顶层
  窗口"；**只有声明的插件被枚举**（未声明插件的终端/隐藏窗口不误伤）。
- **Provider 发现通道（P31-013）**：`Provider` trait 增补默认方法
  `running_plugin() -> Option<(id, pid, window_ui)>`（默认 None），
  仅 PluginProvider 覆盖；`Core::running_plugins()` 聚合——
  pid 边界硬限制：只可能发现宿主自己 spawn 的进程。
- **窗口枚举（P31-013）**：`win_platform::visible_windows_of_pid(pid)`
  ——`EnumWindows` + 可见 + 有标题 + 属主 pid 过滤。
- **置顶（P31-014）**：`win_platform::set_topmost(hwnd, bool)`——
  `SetWindowPos HWND_TOPMOST/NOTOPMOST`，UIPI 拒绝时错误上抛
  （"UIPI/set_window_pos: …"）。
- **管理页「Windows」Tab**：发现的插件窗口列表（标题 + 所属插件 +
  Pin on top / Unpin），pin 状态由宿主 `PINNED` 表维护，Unpin 恢复
  `NOTOPMOST`；Win32 失败明确报错。
- 测试 3 条：pid 暴露（python 门控 + window 声明解析）、启停即时/
  持久（MGT-3/4 已在 165 号）。

## Gate 结果

- `cargo test --workspace`：**909 passed / 0 failed**（908 → 909，+1）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## B1 验收清单对照（P3.1 验收标准 §2）

| # | 结果 |
|---|---|
| B1-H1 未声明 window 的插件不被枚举 | ✅（running_plugins 携带 window_ui，枚举侧过滤） |
| B1-H2 枚举仅限宿主 spawn 的 pid | ✅（running_plugins 只聚合本宿主 provider 句柄） |
| B1-H3 置顶生效可取消 | ✅ set_topmost + PINNED 表（手工验证记入批次文档） |
| B1-H4 提权目标明确报错 | ✅（SetWindowPos 失败上抛 UIPI 错误串） |
| B1-H5 plugin-host 测试全绿 | ✅ |

## 待续（B2/B3）

B2 跟随钉：`SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` 监听线程 +
管理页目标窗口选择（从运行中窗口选，用户已确认）+ 目标销毁自动解除；
B3 桌面钉 WorkerW 实验（tkinter/Electron/Qt 矩阵）。
