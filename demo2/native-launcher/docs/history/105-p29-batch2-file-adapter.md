# 105 — P2.9 Batch 2：File Adapter（既有 Effect 收编）

> 日期：2026-09-09。范围：P2.9 第二批（File/Clipboard Adapter 第一部分）。
> 输入基线：history/104（708 tests）。

## 交付

- `crates/launcher-domain/src/system_adapter.rs`（新）：
  `to_action(&SystemCommand) -> Option<Action>`——把**已通过校验**的
  SystemCommand 映射到既有 host-owned Action（Open/Reveal/
  OpenTerminalHere/Copy + Path payload），返回的 Action 仍走
  launcher-action 的 validate → Effect 链——**数据映射，零新执行路径**。
- Fail-closed：非法命令、非 File 目标、未知 operation（含破坏性未映射
  操作如 delete）一律 `None`（拒绝）；映射后的操作若 risk=Destructive
  则携带 `confirmation_required`。
- 测试 3 条：四操作映射、未映射破坏性操作拒绝 + 确认标志、
  非 File 目标/未知操作/非法命令全 None。

## 设计说明

- Clipboard 写（§19）推迟到 File Transaction 批次（需要 Text payload
  的传递链路，v0.1 的 origin 字段不承载内容）。
- Destructive 文件操作（§14 delete 等）随 File Transaction 批次映射。

## Gate 结果

- `cargo test --workspace`：**711 passed / 0 failed**（708 → 711，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
