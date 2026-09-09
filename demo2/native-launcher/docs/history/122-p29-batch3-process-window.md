# 122 — P2.9 Batch 3：Process/Window 能力分类与命令构建

> 日期：2026-09-09。范围：P2.9 第三批（`P2.9 — System Integration &
> Automation 1.0 技术设计规范.md` §4-§6/§21）。输入基线：history/121
> （773 tests）。

## 交付

- `crates/launcher-domain/src/system_process_window.rs`（新）：
  - `process_risk()/window_risk()`：操作→风险分类（§6）——list=Info、
    focus/minimize/restore=Reversible、kill/terminate/close=Destructive；
    **未知操作 = None（fail-closed，永不猜测）**；
  - `process_command()/window_command()`：构建通过冻结校验的
    SystemCommand（origin 审计随行）——Windows Adapter 批次的消费入口。
- 测试 3 条：分类对齐 taxonomy、构建命令通过校验且携带风险与 origin、
  未知操作不构建任何命令。

## Gate 结果

- `cargo test --workspace`：**776 passed / 0 failed**（773 → 776，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.9 剩余

Batch 4（Shell/URI/Notification/Hotkey/Power 分类扩展）、Batch 5
（Policy/Confirmation/Origin 传播——Resolver 已就绪）、Batch 6
（Race/Security/Soak/Fault 收口）。Windows Adapter（真实 Win32 调用）
按规格在分类与命令全部冻结后接入。
