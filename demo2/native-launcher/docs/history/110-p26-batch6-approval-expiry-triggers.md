# 110 — P2.6 Batch 6：审批过期/取消（C05）+ Trigger Framework 基座（D01/D02）

> 日期：2026-09-09。范围：P2.6 第六批（`P2.6 开发设计规范` §21-§24）。
> 输入基线：history/109（727 tests）。

## Task P26-C05 — Approval Expiry / Cancel

- `approvals` 表增加 `expires_ms` 列（建表即含，向后兼容）；
- `request_with_expiry(..., expires_ms)`：带截止的挂起请求；
- `expire_before(now)`：批量将过期 pending 迁移为 **rejected（fail-closed）**
  ——过期永不放行；已决定行绝不被触碰；
- `cancel(approval_id)`：用户显式取消 = 同 rejected。
- 测试：过期迁移恰好 1 行、已决定行不受影响、显式取消生效。

## Task P26-D01/D02 — Trigger Contract + Durable Run Queue

- `crates/launcher-workflow/src/triggers.rs`（新）：`TriggerKind`
  （hotkey/plugin/schedule/ai_mcp——§22 四类触发源）、`TriggerEvent`
  （仅元数据，无凭证/敏感 payload）、`TriggerQueue`（SQLite FIFO，
  consume-once 语义，pending() 深度供健康检查）。
- 测试：FIFO 顺序 + consume-once + 深度计数。

## P2.6 剩余

D03–D06（四类触发源的宿主接线——hotkey 托盘已有雏形）、B04 parallel、
E 线 Visual Editor、F/G QA+Release。Durable 核心闭环
（Store/Checkpoint/Scheduler/Approval/Expiry/Queue）已全部就绪。

## Gate 结果

- `cargo test --workspace`：**729 passed / 0 failed**（727 → 729，+2）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
