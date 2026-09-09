# 109 — P2.6 Batch 5：Human Approval（C01/C02/C03/C04 部分 + D03 安全）

> 日期：2026-09-09。范围：P2.6 第五批（`P2.6 开发设计规范` §19-§21）。
> 输入基线：history/108（722 tests）。

## 交付

- **C01 Approval Contract**：`ApprovalStatus`（Pending/Approved/Rejected）、
  `ApprovalRecord`。决策是 pending → decided 的**唯一**迁移（显式决定才能
  离开 pending）；重复 request 幂等且**永不重置已决定状态**。
- **C02 Approval Store**：`approvals.db`（SQLite/WAL）——
  request（INSERT OR IGNORE 幂等）/ decide（仅 pending 可迁移）/
  decision_for（scheduler 查询）/ pending()（C03 UI/审计面）。
  Pending 跨重启存活（restart-safe 测试）。
- **Scheduler 集成**：`WorkflowNode.approval: bool`（serde default false，
  旧定义零迁移成本）+ `SchedulerStop::AwaitingApproval{node_id}` + 管线
  接线——approval 节点在执行前检查决策：Approved → 执行；Rejected →
  skip；Pending/无 → checkpoint `AwaitingApproval` + 停机 + 挂起请求。
  **gate 前 node 不执行**（C04 Resume 安全的核心：执行只发生在显式
  approve 之后，且经既有 Resolver 链）。
- 测试 3 条 e2e（真实 RunStore+ApprovalStore）：审批暂停→批准→恢复执行
  且只执行一次；拒绝→跳过、run Finished、skipped 记录；pending 跨重启
  存活→决策→恢复。

## Gate 结果

- `cargo test --workspace`：**727 passed / 0 failed**（722 → 727，+5）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.6 剩余

C05（expiry/cancel/replay 保护）、B04（parallel 执行）、D 线 Trigger、
E 线 Editor、F/G 线 QA/Release。
