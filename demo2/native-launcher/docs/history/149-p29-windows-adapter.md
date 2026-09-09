# 149 — P2.9 Windows Adapter：SystemCommand → 真 Win32 执行

> 日期：2026-09-09。范围：P2.9 后置批次（`P2.9 — System Integration &
> Automation 1.0 技术设计规范.md` §21/§31，122-125 号冻结管线的消费）。
> 输入基线：history/148（832 tests）。

## 交付

- `crates/launcher-action/src/system_adapter.rs`（新，cfg(windows)，非
  Windows 为 Unsupported stub）——P2.9 规划中后置的 Windows Adapter 正式
  落地：`execute_system_command(&SystemCommand, confirmed)` 是唯一把
  已验证 SystemCommand 变成真实 Win32 调用的地方：
  - **Window**：focus（restore-if-iconic + SetForegroundWindow + SetFocus）、
    minimize/restore（ShowWindow）、close（WM_CLOSE 投递）；
  - **Process**：kill/terminate（OpenProcess(PROCESS_TERMINATE) →
    TerminateProcess → CloseHandle；pid 过期 = 干净报错，绝不放宽权限重试）；
  - **Power**：lock（LockWorkStation）、sleep（SetSuspendState）、
    restart/shutdown（ExitWindowsEx EWX_REBOOT/EWX_SHUTDOWN）；
  - **Uri/Shell**：open_uri（ShellExecuteW；scheme 严格校验
    `[alnum+-.]` ≤32 且首字符字母数字，否则拒绝）。
- **门禁不变量**（红线逐条保持）：
  - 冻结契约先行：`SystemCommand::validate()` 失败 = Unsupported；
  - 确认门（INV-041 镜像）：Destructive/Privileged 必须 `confirmed=true`
    —— adapter 在 Policy 下游，自己不是 policy 权威；未确认在任何
    OS 调用之前拒绝；
  - fail-closed：未知操作、`list`/`enum` 观测类（非 effect，归宿主枚举
    provider）、以及刻意不接线的 `shell.run_command`/`notify` 一律
    Unsupported；hwnd=0 / 超指针宽度 / pid=0 拒绝。
- `Effect::SystemApplied(String)`（新 variant，标注所应用操作）；
  现有 match 的 `Ok(_)` 捕获臂保持兼容。
- workspace windows 依赖增加 `Win32_System_Power`/`Win32_System_Shutdown`
  两个 feature（仅此）。
- 测试 3 条：未确认的 destructive 一律 ConfirmationRequired（在任何
  OS 调用前）、未知/非法命令与非法 scheme fail-closed、观测类操作
  不经 adapter 执行。

## Gate 结果

- `cargo test --workspace`：**835 passed / 0 failed**（832 → 835，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 待续（更新后优先级）

1. P2.6 E02–E05 Slint 画布 VIEW（EditorSurface 投影层已就绪）
2. P2.7 D 线（Approval UI/交互式 clarify §18/plan 编辑 D04——B02
   PlanDocument 已备消费）
3. P2.7 E/F/G/H（Memory/Privacy、Product UX、QA、Release）
4. MSIX 签名（等外部证书）；Pinyin 完整拼音表（优化项）
