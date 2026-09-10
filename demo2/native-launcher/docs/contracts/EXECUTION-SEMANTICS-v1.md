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
2. **Retry 矩阵 v2**（§20，`retry_decision(result, effect_state, idempotent)`）：
   - `EffectState::Completed` / `Started` → Forbidden（已发生 / 在途未定）；
   - `EffectState::Unknown`（含 `Cancelled + Unknown`）→ 仅幂等操作可重试；
   - 其余（效果未落地）→ Allowed；`Succeeded` 结果无论如何 Forbidden；
   - 每次重试必须携带新 execution id。
   > P210-D07：`Cancelled` 不天然等于"安全重试"——取消发生在执行中且
   > 未确认停止时，效果可能已落地，因此由 (result, effect_state) 共同决定。
3. **Replan 安全**（H3，`replan_may_execute`）：
   - `Succeeded` = 不可变成功，重规划绝不重执行；
   - `Executing` / `Unknown` 效果未定，不得盲目重跑——需先经显式恢复；
   - 重规划的语义是**替换后缀**（从失败点继续），不是整计划重跑。
4. **Generation ≠ Identity**：generation 计数器只表示版本，不能替代
   PID/HWND 的身份校验（见 EFFECT-AUTHORITY.md 动态目标一节）。
5. **PID 身份保持 OS 原始精度**（P210-B07）：`creation_time_ft` 存
   FILETIME 原始 100ns tick，不做毫秒降采样。

## Recovery Protocol（P210-D08）

```text
NotStarted  → safe execute
Executing   → crash/timeout → Unknown → RecoveryRequired
Unknown     ├── Queryable           → DetermineResult（探测后定论）
            ├── Idempotent          → PolicyRetry（新 execution id）
            └── NonIdempotent       → ExplicitRecoveryRequired（人工/显式）
```

实现：`recovery_route(idempotent, queryable)`。

## 语义依赖图

```text
                ┌──────────────┐
                │ Command      │
                └──────┬───────┘
                       ↓
                 Validation → Policy → Approval
                       ↓
                AuthorizedEffect（engine 铸造，move-only）
                       ↓
                    Execute
                       ↓
              ┌────────┴────────┐
              ↓                 ↓
        CommandResult       EffectState
              └────────┬────────┘
                       ↓
                 Retry Decision
                       ↓
                    Replan
                       ↓
                   Recovery
```

各模块不得自行定义 success/failed/timeout/cancelled 词汇——统一引用本契约。

## 实现锚点与测试

| 规则 | 锚点 | 测试 |
|---|---|---|
| Timeout→Unknown | `timeout_outcome()` | launcher-domain `timeout_is_not_failure` |
| Retry 矩阵 | `retry_decision` | `retry_matrix` |
| Replan 跳过成功步骤 | launcher-ai `agent_loop` step_status | g_line_suite `p210_replan_skips_succeeded_steps` |
| StaleTarget | launcher-action `verify_*_identity` | `pid_reuse_is_detected` / `dead_hwnd_is_stale_target` |
