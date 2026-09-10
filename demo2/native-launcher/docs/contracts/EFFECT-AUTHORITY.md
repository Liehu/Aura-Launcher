# EFFECT-AUTHORITY（P210-003，spec §6-§8）

> 状态：CONTRACT（当前有效语义）。本文件记录权威链的冻结约束；历史与
> 临时状态见 `docs/history/`。

## 权威链

```text
Producer (Search / Workflow / AI / Plugin / UI)
    ↓  Proposal / SystemCommand（DATA，非授权）
Resolver
    ↓
Validator（frozen contract validate()）
    ↓
Policy（origin 允许列表 / 风险上限）
    ↓
Approval（run-scoped / step-scoped / event-scoped，单次使用）
    ↓
ActionEngine —— 唯一授权铸造点
    ↓  AuthorizedEffect（不透明 token）
Adapter
    ↓
OS Effect
    ↓
Audit
```

## 禁止的绕过

```text
UI        ─X→ Effect
AI        ─X→ Effect
Plugin    ─X→ Effect
Workflow  ─X→ Effect
Resolver  ─X→ Effect
Adapter   ─X→ Authorization（adapter 不铸造授权，也不接收 authority 参数）
```

## 已落地的实现锚点

| 约束 | 锚点 | 测试 |
|---|---|---|
| confirmed: bool 不得出现在 adapter 边界 | `launcher_action::authorize_system_command` → `SystemAuthorization`（私有字段，跨 crate 不可构造）→ `system_adapter::execute_authorized` | launcher-action `p210_tests::gate_refuses_unconfirmed` |
| 破坏性/特权操作需确认，且在任何 OS 调用之前 | engine gate 内 `requires_confirmation()` 检查 | 同上 + `destructive_requires_confirmation` |
| 动态目标身份在 effect 前校验（PID/HWND reuse） | `verify_process_identity` / `verify_window_identity` → `ActionError::StaleTarget` | `pid_reuse_is_detected`、`dead_hwnd_is_stale_target` |
| AI Proposal 无执行权威 | proposal 只能经 `CoreAgentHost` → 冻结 Resolver→Engine 链 | launcher-ai g_line_suite |

## 规则

1. `Command ≠ ResolvedCommand ≠ AuthorizedEffect ≠ OS Effect`。
2. `AuthorizedEffect` 只能由 ActionEngine（launcher-action 的授权门）生成。
3. Adapter 收到的是 token，不是命令 + 布尔。
4. 授权是**一次一命令**的；不存在"批准后全部放行"。
5. **INV-EFFECT-104**：铸造是 engine 内部函数（`pub(crate)`）——外部
   crate 唯一的公共入口是 `execute_system_effect(resolver, cmd,
   confirmed)`，它在铸 token 之前先跑 Resolver policy（origin 允许表/
   风险上限/deny 表）。`confirmed` 在这条边界上是 host 的**批准输入**，
   不是 adapter 凭证。
6. **INV-EFFECT-105**：token 是 move-only（无 Clone/Copy），
   `execute_authorized(auth)` 按值消费——一次性由编译器强制。
7. **Token ↔ Command 绑定**（P210-C07）：token 携带完整被授权命令 +
   审计元数据（origin/risk/policy 决定/铸造时刻）；adapter 执行的就是
   token 里的命令，不接收第二个命令参数。
