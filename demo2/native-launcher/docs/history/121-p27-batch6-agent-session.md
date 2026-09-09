# 121 — P2.7 Batch 6：Agent Session 状态机（C01/C02）

> 日期：2026-09-09。范围：P2.7 第六批（`P2.7 开发设计规范` §22-§24）。
> 输入基线：history/120（770 tests）。

## 交付

- `crates/launcher-ai/src/agent_session.rs`（新）：
  `AgentSession`（session_id + 状态 + turns/max_turns）+ **白名单迁移**
  状态机——Created→Observing→Planning→Executing→(Completed|Failed|
  Replanning|WaitingForConfirmation)，Replanning/Waiting 可回 Planning/
  Executing；Cancelled/BudgetExhausted 可从任意活跃态到达，终态冻结。
- **§24 步预算**：turn 消耗型迁移受 max_turns 门禁（耗尽即
  StepBudgetExhausted），非消耗型簿记迁移不受限。
- 测试 3 条：合法循环全程（含 Replanning 回环 + 终态冻结）、非法捷径
  （Created→Executing）拒绝、步预算耗尽拒绝 + 非消耗迁移仍可用。
- 过程中修正测试的 max_turns 设定（2→3，与三步消耗序列匹配）。

## Gate 结果

- `cargo test --workspace`：**773 passed / 0 failed**（770 → 773，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

C03–C07（Loop/Replanning/预算联动/取消恢复——BudgetController/ExecutionHost
已有，接入 Session 状态机）、A01/B/D/E/F/G/H 线。
