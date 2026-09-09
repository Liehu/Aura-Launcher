# 153 — P2.7 Batch 20：F 线 Product UX + §37/§38 可观测性

> 日期：2026-09-10。范围：P2.7 第二十批（`P2.7 开发设计规范` §37/§38 +
> F 线收口，152 号交接优先级 1）。输入基线：history/152（850 tests）。

## 交付

- `crates/launcher-ai/src/telemetry.rs`（新）：
  - **§37 事件环**：`AgentEvent` 13 种（session_created … run_cancelled，
    name() 与规范逐字对齐），`EventLog` 有界 ring（128 条，FIFO），detail
    仅限 step/action 标识（≤128 字符）——**敏感 prompt 永不入库**（§37
    红线）；
  - **§38 指标**：`AgentMetrics` 全字段（llm/planning/approval_wait/run
    延迟、tool_selection/replan/budget_exceeded/model_error 计数）+
    `accumulate` 汇总（release-gate rollup）。
- **run_agent 全埋点**：签名增加 `telemetry: &mut Telemetry`；LLM 调用经
  包装闭包计时（llm_latency / model_error_count）；planning latency、
  approval wait、逐步 StepStarted/Completed/Failed、Replanned、
  BudgetExceeded、终止事件（RunSucceeded/Failed/Cancelled）+ 总时延——
  所有退出路径无遗漏。
- **F 线 UX 收口（launcher-app）**：
  - **F01 AI 搜索面 / F02 Command Surface**：已在 147 号落位（`agent `
    前缀命令 + popup surface），本批确认；
  - **F03 Plan Preview**：已在 151 号落位（D02 审批面逐条展示 plan），
    本批确认；
  - **F04 Run Progress**：`CoreTurnExecutor` 每步执行前把
    `executing <action_ref>` 推上 runtime surface 状态行（此前只有
    静态两行）；
  - **F05 Error/Retry/Explain**：失败 status 行本就携带
    step_id + error（Explain）；新增 `agent retry` —— 重跑最近目标
    （LAST_GOAL 静态；无历史目标时给出明确提示）；run 结束把事件环与
    指标汇总写日志（agent.event / agent.metrics）。
- 测试 3 条：§37 事件名全覆盖且唯一、事件环有界 + detail 截断、
  指标 accumulate 汇总。

## Gate 结果

- `cargo test --workspace`：**853 passed / 0 failed**（850 → 853，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 待续

1. P2.7 G 线 QA 收口（G01 provider stress / G02 planner determinism /
   G03 agent state / G04 approval security / G05 injection——核心用例已
   随各批内建，剩余为补强矩阵）+ H 线 Release（MSIX 签名等外部证书）
2. Pinyin 完整拼音表（优化项）
