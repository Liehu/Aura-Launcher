# ADR-0019: Plugin Lifetime Policy

**状态：ACCEPTED（MVP4.4 / Plugin Lifetime Contract Gate）**
**来源：`P3.1-plugin/01-plugin-lifecycle-technical-spec-v0.1.md`（技术规范 v0.1）
及配套的 Agentic Coding 设计、测试计划与验收标准。**

> 注：规范建议编号 ADR-0016，但 0016 已被 AI Planner 占用；本 ADR 顺延为
> 0019。内容与规范一一对应。

## 背景与问题

插件已经具备外部进程隔离（Job Object，ADR-0005）、按需拉起、超时/崩溃隔离
与两阶段退出（contract SS12）。缺失的是**生命周期策略**：插件什么时候存在、
什么时候必须不存在。所有插件目前实际只有一种隐式行为——首次调用 spawn 后
由 Provider 持有进程直到失败/禁用（无 idle 回收）。

## 决策

### 1. Manifest 契约（additive）

`runtime` 对象新增 `lifetime` 字段：

```json
{ "runtime": { "type": "process", "executable": "x.exe", "lifetime": "ephemeral" } }
```

- `launcher_domain::PluginLifetime::{Ephemeral, Resident}`（serde 小写）。
- **缺失 → Resident**（INV-LIFE-001，legacy 兼容；含完全没有 `runtime`
  对象的 legacy manifest）。改默认值必须走 breaking contract。
- `session` 与非法值在反序列化层**直接拒绝**，不静默降级（CAT-LIFE-004/005）。
- `lifetime` 与 `runtime.type` 正交（CAT-LIFE-007）。

### 2. 策略只存在于 PluginHost

`launcher-plugin-host::lifetime::PluginLifetimePolicy` 是唯一从**已验证
manifest** 推导执行模式的位置。`PluginHandle`：

- `query` / `execute_action` 对 ephemeral 插件构成**一次完整生命周期**：
  调用结束后发送 `shutdown`，等待 host 固定 grace（200ms，不可被 manifest
  声明延长），未退出则 Job Object 强杀并 reap；`finish_invocation` 校验进程
  不复存在（INV-LIFE-003）。业务错误（ActionFailed）同样结束 ephemeral
  invocation，但结果语义不变（仍是 BusinessError）。
- 暴露进程状态原语：`poll_exit` / `process_running` / `idle_expired` /
  `pid`。**调用方不得导入 `PluginLifetime`**。
- Resident 行为不变：复用、失败后销毁重建、无后台驻留线程。

### 3. Provider 只消费原语

`launcher-core::PluginProvider::ensure_running` 在使用边界回收：

- 进程已退出（ephemeral 结束或崩溃）→ 丢弃 handle，下次 respawn；
- resident 且 `idle_expired()` → 优雅回收后下次 respawn（**惰性回收**，
  无后台 reaper——UI 线程永不等待插件）；
- `running_plugin()` 只报告存活进程（ephemeral 空档不误报给窗口管理）。

window 插件（P3.1）不参与 idle 回收：其进程生命周期归窗口管理，避免 legacy
行为回退。

### 4. 不变量保持

- execution_id 仍由调用方铸造，host 只透传+校验 echo（INV-LIFE-007）。
- Timeout/Crash/ProtocolViolation/BusinessError 的分类与清理语义不变。
- lifetime 不升权、不绕过 ActionEngine/ActionResolver（INV-LIFE-009）。
- Workflow / Action / MCP / AI Planner 源码零 lifetime 分支（INV-LIFE-008，
  由源码搜索 guard 验证）。

### 5. 可观测性

- 每次生命周期事件输出结构化 tracing
  （`lifecycle.invocation_closed`：plugin/lifetime/operation/invocation_id/
  pid/forced_kill/exit_code/shutdown_ms）。
- 有界全局计数器 `lifecycle_metrics()`：spawn_count /
  successful_spawn_count / forced_kill_count / ephemeral_invocations。
  无历史缓存。
- `launcher-bench plugin-lifetime`：cold ephemeral 与 warm resident 的
  query/execute P50/P95/P99/Max，写入 `benchmarks/plugin-lifetime.json`。

## 后果

- 新字段 additive；现有 fixture 不改文件继续通过（CAT-LIFE-012）。
- Resident idle 回收是**新增**能力（此前无回收），限定在 headless 插件 +
  使用边界，属保守收紧而非行为回退。
- v0.1 已知限制：resident idle 回收为惰性（下次交互时执行），不满足"无交互
  时进程也必须消失"的强读法；如需后台 reaper 须单独 ADR。

## 测试与证据

- 单元：`launcher-domain`（CAT-LIFE-001..007）、`launcher-plugin-host::lifetime`。
- 集成：`launcher-plugin-host/tests/plugin_lifecycle.rs`（PROC-001..010/012、
  EXEC-001..004/009、legacy 兼容；含子/孙进程 Job Object 清理与 force-kill
  证据，python fixture，LAUNCHER_PYTHON 门控）。
- Provider 级：`launcher-core/tests/plugin_lifecycle_provider.rs`。
- Benchmark：`cargo run -p launcher-bench -- plugin-lifetime`。

## 明确非目标

见规范 §14：session lifetime、persistent daemon registry、auto-pinning、
内存配额、热重载、状态迁移、分布式 runtime 等均不做。
