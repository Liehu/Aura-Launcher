# 167 — P3.1 Batch B2/B3：跟随钉 + 桌面钉（免 hook 收口）

> 日期：2026-09-10。范围：P3.1 第二/三批。输入基线：history/166（909 tests）。

## 交付

- **B2 跟随钉（P31-021/022/023）**：
  - `win_platform::follow_set/clear/current`：跟随关系（pin hwnd →
    目标 hwnd + 标题）；**免注入实现**——进程外
    `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`（系统广播，非 DLL
    注入）+ 专用 GetMessage 线程（事件驱动，空闲 0 CPU，FOL-5）；
  - 语义：目标应用成为前台 → 插件窗 `TOPMOST` 浮于其上；目标离开 →
    非用户置顶时恢复 `NOTOPMOST`（用户置顶永不自动还原，PIN_TABLE
    区分）；目标/插件窗口销毁 → **自动解除**跟随（FOL-3/4）；
  - UIPI：对提权目标/插件的 SetWindowPos 失败即错误上抛并解除
    （明确报错，不静默）；
  - 纯谓词 `follow_should_topmost/restore` 可单测（FOL-1）；
  - 管理页「Windows」Tab：插件窗口行可选中；「运行中的窗口」目标
    选择列表（用户已确认的交互）；每行 Pin / Follow… / Unfollow /
    To desktop 操作。
- **B3 桌面钉（P31-031/032，EXPERIMENTAL）**：
  - `pin_desktop`：Progman `0x052C` → WorkerW 生成 → `EnumChildWindows`
    定位含 `SHELLDLL_DefView` 的 WorkerW → `SetParent` 把插件窗口挂到
    图标层之后；`unpin_desktop` 恢复顶层；
  - 管理页「To desktop / Unpin desktop」切换；状态由 DESKTOP 表维护；
  - **实验性声明**：框架兼容矩阵（tkinter/Electron/Qt）为手工清单
    （P3.1-B3），失败路径响亮报错并保留窗口。
- 测试 1 条 + 既有回归：FOL 纯谓词矩阵（含"用户置顶不自动还原"）。

## Gate 结果

- `cargo test --workspace`：**910 passed / 0 failed**（909 → 910，+1）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 安全自查（P3.1 红线）

- 无 `SetWindowsHookEx`、无 DLL 注入、无跨进程内存写入（代码评审）；
- 窗口操作 pid 边界：枚举/置顶/跟随仅作用于宿主 spawn 的插件窗口；
  目标窗口仅被**观察前台事件**（用户显式选择），不被修改；
- UIPI/提权：失败响亮报错；
- WinEventHook 线程事件驱动，无忙等。

## 待续（P3.1 收尾）

框架兼容矩阵实测（B3 手工清单）+ 用户体验回归脚本；随后 P3.1 验收。
