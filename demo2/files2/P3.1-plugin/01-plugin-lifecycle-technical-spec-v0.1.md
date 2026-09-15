# Native Launcher 插件生命周期策略技术规范 v0.1

**状态：PROPOSED / 供 MVP4.4+ 实施**  
**目标：在不改变 Action / Workflow / MCP 语义的前提下，引入可验证的插件生命周期策略：按需启动、调用结束退出，或在必要时保持驻留。**

## 1. 背景与设计目标

当前插件已经具备外部进程隔离、按需拉起、超时/崩溃隔离、Idle 超时回收和 Job Object 子进程清理能力。本规范不重做 Plugin Protocol，而是在现有 PluginHost / PluginBroker 上增加**生命周期策略**。

当前系统的关键不变量必须继续成立：

- ActionEngine 仍是唯一 Effect gateway。
- Workflow 不获得 PluginBroker / Effect / Resolver 权限。
- MCP 与 Plugin 使用相同 WorkflowRunner / Resolver / ActionEngine 路径。
- 每个 execution attempt 只铸造一个 execution_id；Executor 不自行铸造。
- 插件进程崩溃、超时、协议违规不会污染 Core。
- UI 线程不等待插件进程。
- 插件不能因为生命周期策略而成为 Core 内嵌 Python/Node runtime。

本规范解决的问题是：**插件什么时候存在，以及什么时候必须不存在。**

## 2. 生命周期模型

支持两个实际生命周期，第三个作为保留扩展：

| Lifetime | 语义 | 进程行为 | v0.1 支持 |
|---|---|---|---|
| `ephemeral` | 调用级 | 每次 invocation 独立启动，完成后退出 | ✅ |
| `resident` | 进程级 | 首次调用启动，后续复用，空闲超时退出 | ✅，兼容现有行为 |
| `session` | Popup/session 级 | 会话开始启动，会话结束退出 | 保留，❌ |

### 2.1 核心定义

`ephemeral`：

```text
request
  -> spawn
  -> initialize
  -> query / execute_action
  -> response
  -> graceful shutdown
  -> bounded wait
  -> force kill if needed
  -> Job Object guarantees descendant cleanup
  -> plugin process absent
```

`resident`：

```text
first request
  -> spawn
  -> initialize
  -> request(s)
  -> idle timer refresh
  -> idle timeout
  -> graceful shutdown
  -> bounded wait
  -> force kill if needed
```

## 3. Manifest 扩展

在现有 `runtime` 对象中增加：

```json
{
  "runtime": {
    "type": "process",
    "executable": "example-plugin.exe",
    "args": [],
    "lifetime": "ephemeral"
  }
}
```

### 3.1 默认与兼容

为了不破坏现有插件：

- `runtime.lifetime` 缺失：解释为 `resident`（legacy compatible）。
- `runtime.lifetime = "ephemeral"`：显式进入调用级进程模型。
- `runtime.lifetime = "resident"`：显式保留现有模型。
- `session` 在 v0.1 被识别但必须拒绝执行并给出明确 manifest validation error；不得静默降级。
- `lifetime` 是生命周期策略，不改变 `runtime.type`。

后续若要把默认值从 `resident` 改成 `ephemeral`，必须进入 breaking contract / v0.2，而不是静默修改 v0.1.x。

## 4. Invocation 语义

### 4.1 Query invocation

对于 `ephemeral`：一次 `query()` = 一个完整生命周期。

```text
Query ID q-17
  └─ Process P17
       initialize
       query(q-17)
       result
       shutdown
       exit
```

下一次 query 必须使用新的进程实例。

### 4.2 Action execution invocation

对于 `ephemeral`：一次 `execute_action(execution_id)` = 一个完整生命周期。

```text
execution e-17
  └─ Process P18
       initialize
       execute_action(e-17)
       result
       shutdown
       exit
```

注意：`query` 产生的 Command/Action descriptor 是逻辑数据；不能依赖查询进程仍然存活。执行时由 Host 按现有 resolver 规则重新定位并重新启动需要的 Plugin。

### 4.3 Resident invocation

允许多个 query / action 共用同一 PluginHandle，但必须继续遵守：

- one in-flight query per PluginHandle；
- execution_id/query_id 不复用；
- stale result 丢弃；
- idle timer 在成功或规定的活动事件后刷新；
- timeout / crash / protocol violation 后销毁旧实例，下次调用重新拉起。

## 5. 状态机

### 5.1 Ephemeral

```text
Discovered
   ↓
Validated
   ↓
Starting
   ↓
Running
   ↓
Invoking
   ↓
Completed ──→ ShutdownPending ──→ Exited
   │                               ↑
   ├── Timeout ────────────────────┤
   ├── Crash ──────────────────────┤
   └── ProtocolViolation ──────────┘
```

任何终态都必须保证：

```text
plugin_process_count = 0
```
并且 Job Object 下的 descendants 也不存在。

### 5.2 Resident

```text
Discovered
  ↓
Validated
  ↓
Starting
  ↓
Running
  ↓
Idle
  ├── request → Running
  └── idle_timeout → ShutdownPending → Exited
```

## 6. 正常退出策略

统一采用两阶段退出：

1. Host 发送已有 `shutdown` RPC。
2. 等待 `shutdown_grace_ms`，默认 200ms。
3. 未退出则使用 Job Object / process handle 强制终止。
4. 等待并确认 process + descendants 清理完成。

`shutdown_grace_ms` 为 Host policy，不由插件声明；插件不能通过 manifest 无限延长退出时间。

## 7. 超时策略

`ephemeral` 与 `resident` 共用现有请求 timeout 语义。

- Query timeout：该 invocation 失败；ephemeral 直接 teardown；resident 销毁实例。
- Action timeout：返回现有 PluginError::Timeout；Workflow 按既有策略决定 bounded retry。
- Retry 必须创建新的 execution_id。
- Timeout 不表示 Effect 未发生，继续保持 at-least-once 语义。

这些执行语义不能因为 lifetime 改变。

## 8. Capability / Security

生命周期策略不拥有 Capability Authority。

以下字段依旧不可信：

```text
authorized
confirmed
g ranted_capabilities
trust_level
```

Plugin 的生命周期策略不能提升权限，也不能绕过 ActionResolver / ActionEngine。

Ephemeral 进程每次启动都必须重新执行 manifest/capability validation，不能依赖上一次进程状态。

## 9. 内存目标

定义两个不同指标：

### Launcher 常驻内存

继续沿用现有 Private Bytes 主预算。

目标保持：

```text
Idle launcher Private Bytes <= existing baseline gate
```

插件未运行时：

```text
plugin_process_count = 0
plugin Private Bytes contribution = 0
```

这里的“0”指插件进程不存在，不承诺 Windows 文件缓存等系统级缓存归零。

### 插件峰值

对 `ephemeral` 不设置全局固定峰值，因为插件类型不同；但必须记录：

```text
spawn_peak_private_bytes
post_exit_process_count
```

禁止插件进程泄漏成为 Launcher 常驻资源。

## 10. 性能策略

生命周期优化不是以“零内存”换取不可接受的交互延迟。

必须测量：

```text
spawn_ms
initialize_ms
query_ms
shutdown_ms
force_kill_count
```

至少记录：P50 / P95；P99 / Max 仅记录，不作为 v0.1 主 gate。

对于 native `process` runtime，新增 benchmark 应记录：

```text
cold_ephemeral_query
warm_resident_query
cold_ephemeral_execute
warm_resident_execute
```

不得直接用一个固定 spawn 常数猜测跨 runtime（Python/Node/WASM）的性能。

## 11. 兼容与迁移

### 11.1 现有插件

没有 `lifetime` 的现有 manifest 保持 `resident`。

### 11.2 新插件建议

文档/模板建议：

- 无状态、快速 CLI：`ephemeral`
- 有会话状态、昂贵初始化：`resident`
- session：暂不发布

### 11.3 不允许的隐式行为

以下均禁止：

- 因为 query 命中而自动把 `ephemeral` 升级为 `resident`。
- 因为 resident 超时失败而永久驻留。
- 因为 action 需要状态而绕过 lifetime 声明。
- 为了减少 spawn 次数在 Host 内保存不可见的第二份 Plugin runtime。

## 12. API / Trait 设计

建议将 lifecycle policy 封装在 PluginHost 层：

```rust
pub enum PluginLifetime {
    Ephemeral,
    Resident,
}
```

建议 Host 负责：

```rust
pub trait PluginExecutor {
    fn execute_query(...);
    fn execute_action(...);
}
```

生命周期由 executor/handle 内部实现，而不是扩散到：

```text
launcher-workflow
launcher-action
ActionResolver
WorkflowRunner
MCP adapter
AI Planner
```

核心架构必须保持 provider/executor-local semantics。

## 13. 日志与可观测性

每个 invocation 至少记录：

```text
plugin_id
runtime_type
lifetime
operation(query|execute_action)
query_id / execution_id
process_pid
spawn_at
invoke_at
shutdown_at
exit_at
exit_code
forced_kill
peak_private_bytes (if available)
```

日志不能记录 capability secrets 或 Action input 中的敏感内容；遵循现有日志脱敏政策。

## 14. 明确非目标

v0.1 不做：

- session lifetime
- persistent daemon registry
- plugin auto-pinning based on heuristics
- plugin memory sandbox quotas
- hot code reload
- plugin migration/state checkpoint
- distributed plugin runtime
- Cordis-style generic service container

## 15. 架构变更要求

该功能改变 Plugin Manifest / lifecycle contract，因此必须创建或更新 ADR；建议编号：`ADR-0016 Plugin Lifetime Policy`。

必须保持现有 crate dependency direction 无环。

## 16. 成功标准

实现完成后应满足：

```text
1. default legacy plugin behavior unchanged
2. explicit ephemeral plugin runs only for its invocation
3. after ephemeral completion, plugin process tree is absent
4. resident still reuses one process and exits on idle timeout
5. timeout/crash/protocol violation cleanup semantics unchanged
6. query_id / execution_id uniqueness preserved
7. Workflow/MCP/AI source code contains no lifetime-specific branches
8. launcher idle memory budget does not regress
9. full workspace test/build/topology gates remain green
```
