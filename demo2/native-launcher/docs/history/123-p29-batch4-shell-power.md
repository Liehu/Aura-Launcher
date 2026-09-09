# 123 — P2.9 Batch 4：Shell/URI/Notification/Power 分类扩展

> 日期：2026-09-09。范围：P2.9 第四批（`P2.9 — System Integration &
> Automation 1.0 技术设计规范.md` §18/§24/§25）。输入基线：history/122
> （776 tests）。

## 交付

- `crates/launcher-domain/src/system_process_window.rs` 扩展：
  - `shell_risk()`：open_uri/notify=Info、run_command=Destructive；
  - `power_risk()`：lock=Reversible、sleep/restart/shutdown=Destructive；
  - `uri_command()`：scheme 词法校验（小写字母数字）后构建 Info 风险命令；
  - `power_command()`：fail-closed 构建（未知操作 = None）。
- 测试 3 条：taxonomy 对齐、scheme 校验、power fail-closed。

## Gate 结果

- `cargo test --workspace`：**779 passed / 0 failed**（776 → 779，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.9 剩余

Batch 5（System Policy/Confirmation/Origin 传播——Resolver 骨架已就绪）、
Batch 6（Race/Security/Soak/Fault 收口）。Windows Adapter（真实 Win32）
按规格在全部命令分类冻结后统一接入。
