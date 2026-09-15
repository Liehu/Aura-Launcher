# Plugin Lifetime v0.1 — Agentic Coding Design

## 1. Mission

在现有 Native Launcher 架构上实现插件生命周期策略，不进行 Plugin Runtime 大重构，不引入 Cordis，不改变 Workflow / Action / MCP contract。

目标实现：

```text
runtime.lifetime = ephemeral | resident
```

其中 legacy manifest 缺失 `lifetime` 时保持 `resident`。

## 2. Mandatory Read Before Coding

Agent 必须先读取：

1. `README.md`
2. `docs/01-design-spec-v0.1.md`
3. `docs/02-agentic-coding-development-spec-v0.1.md`
4. `docs/03-test-plan-v0.1.md`
5. 相关 ADR，至少包括 Plugin / Action / Workflow
6. `PLUGIN-CONTRACT` 当前实现与测试
7. `launcher-plugin-host`、`launcher-plugin-api`、`launcher-core` 相关代码
8. 最新 architecture/topology guard

## 3. Architecture Freeze

### MUST NOT change

- `launcher-workflow` 不能依赖 PluginHost/MCP transport。
- WorkflowRunner 不允许出现 lifetime / plugin process 分支。
- ActionEngine 继续作为唯一 Effect gateway。
- MCP 不得出现独立 lifetime execution path。
- 不新增 Core 内嵌 Python/Node runtime。
- 不把 UI 线程改成同步等待。
- 不改变 `execution_id = one execution attempt`。
- 不修改已冻结 Workflow contract 的语义。

### SHOULD change only in

优先范围：

```text
crates/launcher-domain
crates/launcher-plugin-host
crates/launcher-plugin-api
apps/example-echo-plugin
apps/example-testplugins
相关 plugin contract tests
benchmark harness
docs/adr/ADR-0016*
```

除非编译依赖真实阻塞，不应扩大到 Workflow/UI。

## 4. Delivery Strategy

### Task L1 — Inspect / Baseline

输出：

```text
Summary
Current manifest parsing path
Current PluginHandle lifecycle
Current timeout/idle shutdown path
Current spawn/kill instrumentation
Existing plugin tests
Baseline performance
Potential compatibility hazards
```

完成定义：不修改代码；确认 legacy manifest 语义为 resident。

### Task L2 — Domain Contract

新增：

```rust
PluginLifetime::{Ephemeral, Resident}
```

要求：

- serde roundtrip
- invalid value rejected
- absent field maps to Resident
- session rejected, not silently downgraded

修改范围只限 manifest/runtime model 与 validation。

测试：至少 CAT-LIFE-001..005。

### Task L3 — Launch Policy

在 PluginHost 内增加：

```rust
PluginLifetimePolicy
```

只允许 host 根据 validated manifest 选择 lifecycle。

要求：

- lifetime 解析不污染 Provider/Workflow
- no duplicated prefix / identity logic
- launch strategy 与 runtime.type 解耦

### Task L4 — Ephemeral Query

实现：

```text
spawn
initialize
query
collect response
shutdown
verify exit
```

要求：

- 一个 query 一个 process
- query 完成后不得保留 PluginHandle 的 running child
- 子进程使用现有 Job Object cleanup
- timeout/crash/malformed/flood 后仍必须清理

### Task L5 — Ephemeral Action

实现：

```text
spawn
initialize
execute_action(execution_id)
response
shutdown
exit
```

要求：

- execution_id 由调用方铸造
- registry/executor 只透传
- 每次 retry 使用新 process + 新 execution_id
- plugin business error 不自动 kill，除非 invocation lifetime 自然结束

### Task L6 — Resident Compatibility

确保原有 behavior 不变：

```text
first call -> spawn
next calls -> reuse
idle timeout -> shutdown
```

要求：

- legacy manifest regression test
- explicit resident == legacy behavior

### Task L7 — Observability

增加结构化 lifecycle metrics：

```text
spawn_count
successful_spawn_count
forced_kill_count
plugin_process_count
lifetime
operation
spawn_latency_ms
shutdown_latency_ms
```

不增加无限历史缓存。必要时只保留 benchmark counters 或 bounded recent samples。

### Task L8 — Benchmark

加入：

```text
plugin_ephemeral_query
plugin_ephemeral_execute
plugin_resident_query
plugin_resident_execute
```

至少 P50/P95。

与现有 benchmark policy 对齐：>5% warning，>10% regression fail，除非 baseline 有理由更新。

### Task L9 — Contract / Integration Tests

新增独立生命周期测试模块，不把 lifecycle case 散落到 unrelated tests。

至少：

```text
manifest
process count
pid uniqueness
shutdown
forced kill
child tree cleanup
query isolation
execution isolation
retry identity
resident compatibility
concurrency guard
```

### Task L10 — Topology / Review

运行：

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python check_topology.py
```

并用源码搜索保证：

```text
launcher-workflow
launcher-action
MCP adapter
AI planner
```

都没有 `PluginLifetime` / `Ephemeral` / `Resident` 特判。

## 5. Recommended API Shape

### Manifest

```rust
pub struct RuntimeSpec {
    pub runtime_type: RuntimeType,
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub lifetime: PluginLifetime,
}
```

兼容解析：`None => Resident`。

### Plugin Host

推荐：

```rust
pub enum ExecutionMode {
    Ephemeral,
    Resident,
}
```

如果现有 `PluginHandle` 已承载 resident state，不要强行让 ephemeral 复用同一个 persistent handle abstraction；允许引入内部 `InvocationProcess`，但不要暴露给 Core。

## 6. Error Rules

生命周期层错误：

```text
SpawnFailure
InitializeFailure
ShutdownTimeout
ProcessCleanupFailure
```

必须映射到现有 PluginError/FailureClass，不创建 Workflow 专属 lifetime errors。

建议：

```text
SpawnFailure -> PluginUnavailable
InitializeFailure -> ProtocolViolation / PluginUnavailable（按现有分类）
ShutdownTimeout -> cleanup internally; invocation result semantics unchanged
```

## 7. Agent Completion Report

每个 task 必须输出：

```text
Summary
Changed files
Tests run
Benchmark / memory impact
Architecture impact
Known risks
```

并额外报告：

```text
Lifecycle coverage
Plugin process count invariant
Child-process cleanup result
Legacy compatibility result
```

## 8. Stop Conditions

出现以下任意情况必须 STOP，不继续扩散修改：

- 需要修改 Workflow contract 才能实现
- 需要给 MCP 增加 lifetime 分支
- 需要 ActionEngine 认识 PluginLifetime
- 需要把 PluginHandle 暴露到 UI/Core
- legacy plugin 的默认语义无法保持
- 无法证明 process tree 在 ephemeral 结束后已清理
- 无法提供性能基线
- topology 出现依赖倒置

## 9. Review Checklist

### API

- [ ] 新字段是 additive
- [ ] absent lifetime 保持 resident
- [ ] unsupported session 明确拒绝
- [ ] 公开 API 有测试

### Runtime

- [ ] ephemeral 一次 invocation 一个 process
- [ ] resident 可复用
- [ ] timeout/crash/malformed 都 cleanup
- [ ] Job Object 兜底

### Security

- [ ] capability 仍由 manifest/policy 决定
- [ ] lifetime 不升权
- [ ] execution_id authority unchanged

### Architecture

- [ ] Workflow zero lifetime refs
- [ ] MCP zero lifetime refs
- [ ] Action zero lifetime refs
- [ ] no new circular dependency

### Performance

- [ ] spawn/initialize/shutdown separately measured
- [ ] P50/P95 recorded
- [ ] launcher idle Private Bytes no regression
- [ ] plugin process absent after ephemeral invocation
