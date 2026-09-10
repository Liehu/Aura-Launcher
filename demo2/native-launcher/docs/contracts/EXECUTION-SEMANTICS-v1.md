# EXECUTION-SEMANTICS-v1（P210-002/005，spec §17-§20）

> 状态：CONTRACT（当前有效语义）。跨 P2.6–P2.9 的统一执行语义冻结。

## CommandResult（统一结果词汇）

```text
Succeeded | Failed | Rejected | Cancelled | Timeout | Unknown
```

实现：`launcher_domain::execution_semantics::CommandResult`

## EffectState（效果确定性）

```text
NotStarted | Started | Completed | Failed | Unknown
```

## StepStatus（§15 replan 安全词汇）

```text
Pending | Executing | Succeeded | Failed | Cancelled | Unknown | Skipped
```

实现：`launcher_domain::execution_semantics::StepExecutionRecord`

## 核心规则

1. **Timeout ≠ Failed**（§19）：effect 已开始 + 调用方超时
   ⇒ `CommandResult::Timeout` + `EffectState::Unknown`。
2. **Retry 矩阵**（§20，`retry_decision(result, idempotent)`）：
   - `Succeeded` → Forbidden（效果已发生，绝不重复）；
   - `Unknown` / `Timeout` → 仅幂等操作可重试；
   - `Failed` / `Cancelled` / `Rejected` → Allowed；
   - 每次重试必须携带新 execution id。
3. **Replan 安全**（H3，`replan_may_execute`）：
   - `Succeeded` = 不可变成功，重规划绝不重执行；
   - `Executing` / `Unknown` 效果未定，不得盲目重跑——需先经显式恢复；
   - 重规划的语义是**替换后缀**（从失败点继续），不是整计划重跑。
4. **Generation ≠ Identity**：generation 计数器只表示版本，不能替代
   PID/HWND 的身份校验（见 EFFECT-AUTHORITY.md 动态目标一节）。

## 实现锚点与测试

| 规则 | 锚点 | 测试 |
|---|---|---|
| Timeout→Unknown | `timeout_outcome()` | launcher-domain `timeout_is_not_failure` |
| Retry 矩阵 | `retry_decision` | `retry_matrix` |
| Replan 跳过成功步骤 | launcher-ai `agent_loop` step_status | g_line_suite `p210_replan_skips_succeeded_steps` |
| StaleTarget | launcher-action `verify_*_identity` | `pid_reuse_is_detected` / `dead_hwnd_is_stale_target` |
