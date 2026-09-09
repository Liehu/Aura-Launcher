# 136 — P2.7 Batch 10：Agent Loop 接线（C03–C06 核心）

> 日期：2026-09-10。范围：P2.7 第十批（`P2.7 开发设计规范` §22-§26）。
> 输入基线：history/135（800 tests）。

## 交付

- `crates/launcher-ai/src/agent_loop.rs`（新）：
  `run_agent(session, input, context, catalog, llm, exec, cancel, policy,
  budget) -> LoopStop`——把 AgentSession 状态机、StepExecutor 宿主、
  取消旗标和 P2.7 pipeline 串成完整 Agent Loop：
  - **C03/C04**：Observing→Planning→pipeline→Executing→逐步
    `TurnExecutor::execute`（经冻结 Resolver→Engine 链）→Completed；
  - **§25 Replanning**：步骤失败且未重规划过 → Replanning→Planning→
    Executing（全 plan 从头重执行一次）→ 二次失败即 Failed；
  - **C06**：cancel 旗标在每步之间轮询 → Cancelled；
  - 状态机全程白名单迁移（Session::transition 强制），终态冻结。
- 测试 3 条：全步完成、首步失败触发重规划后完成、取消即 Cancelled。
- 修复：replan 后缺 Executing 迁移导致状态卡在 Planning（测试抓出）。

## Gate 结果

- `cargo test --workspace`：**803 passed / 0 failed**（800 → 803，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

A01（Provider 深化——llm.rs mock/OpenAI 已备）、B01-B05、C07（Runtime
恢复——RunStore 式持久化待接）、D/E/F/G/H 线。
