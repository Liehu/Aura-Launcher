# Plugin Lifecycle v0.1 — Test Plan

## 1. Test Scope

覆盖：Manifest、生命周期解析、进程生命周期、Job Object 子树清理、query、execute_action、timeout、crash、protocol violation、resident compatibility、retry identity、concurrency、performance、memory、topology。

不测试：Cordis runtime、session lifetime、分布式插件。

## 2. Test Taxonomy

| ID | Category | Level |
|---|---|---|
| CAT-LIFE-001..012 | Unit / contract | 必须 |
| LIFE-PROC-001..012 | Integration | 必须 |
| LIFE-EXEC-001..010 | Action integration | 必须 |
| LIFE-SEC-001..006 | Security/authority | 必须 |
| LIFE-SOAK-001..004 | Soak | 必须 |
| LIFE-PERF-001..008 | Benchmark | 必须 |
| LIFE-ARCH-001..004 | Source/topology guards | 必须 |
| LIFE-E2E-001..004 | Real app E2E | 建议 |

## 3. Unit / Contract Tests

### CAT-LIFE-001 — default legacy

输入：manifest 无 `runtime.lifetime`。

期望：解析结果 `Resident`。

### CAT-LIFE-002 — explicit resident

输入：`lifetime=resident`。

期望：Resident。

### CAT-LIFE-003 — explicit ephemeral

输入：`lifetime=ephemeral`。

期望：Ephemeral。

### CAT-LIFE-004 — unsupported session

输入：`lifetime=session`。

期望：manifest validation error；不得降级成 resident。

### CAT-LIFE-005 — malformed type

输入：number/object/null 等。

期望：reject；无 panic。

### CAT-LIFE-006 — serialization

Enum/manifest roundtrip 保持语义。

### CAT-LIFE-007 — runtime type orthogonality

改变 `runtime.type` 不自动改变 lifetime。

### CAT-LIFE-008 — canonical executable resolution

legacy top-level executable 与 runtime.executable 语义保持既有规则。

### CAT-LIFE-009 — invalid manifest isolation

一个 manifest 失败不能阻止其他 plugin discovery。

### CAT-LIFE-010 — no lifetime leakage

PluginDescriptor/CommandDescriptor 不应暴露 lifetime 到 UI/API presentation。

### CAT-LIFE-011 — default documentation contract

示例 manifest/fixture 明确展示 lifetime。

### CAT-LIFE-012 — backward-compatible fixture set

现有 plugin fixture 不改文件即可继续通过 contract tests。

## 4. Process Integration Tests

### LIFE-PROC-001 — ephemeral query spawns

执行 1 次 query：

期望：spawn_count +1。

### LIFE-PROC-002 — ephemeral query does not reuse pid

连续执行 N 次 query（建议 N=10）：

期望：至少每次 invocation 都产生新的 process identity；不得共享 running child。

### LIFE-PROC-003 — ephemeral process exits

query 成功后等待 cleanup。

期望：

```text
plugin_process_count == 0
```

### LIFE-PROC-004 — resident reuse

N 次 query：

期望：期间保持同一 pid；只有首次 spawn。

### LIFE-PROC-005 — resident idle timeout

超过 idle timeout：

期望：进程退出，process_count=0。

### LIFE-PROC-006 — graceful shutdown

正常 completion 后发送 shutdown。

期望：不 force kill。

### LIFE-PROC-007 — shutdown timeout

构造拒绝 shutdown 的 fixture。

期望：达到 grace period 后 force kill；最终 process_count=0。

### LIFE-PROC-008 — descendant cleanup

插件创建 child + grandchild。

期望：ephemeral 完成后所有 descendants 消失。

### LIFE-PROC-009 — crash cleanup

插件异常退出。

期望：旧 process 不残留；下次 invocation 可重新 spawn。

### LIFE-PROC-010 — timeout cleanup

插件 query 超时。

期望：process tree 清理；下一次 invocation 成功。

### LIFE-PROC-011 — malformed/flood cleanup

协议违规/洪泛。

期望：与当前 contract 一致，进程被处理并清理。

### LIFE-PROC-012 — concurrent invocation isolation

并发触发多个 ephemeral invocations。

期望：process identity 分离；无 result cross-talk。

## 5. Action / Execution Tests

### LIFE-EXEC-001 — ephemeral execute_action

一次 execute：

- process spawn
- initialize
- execute_action
- result
- process exit

### LIFE-EXEC-002 — execution_id echo

plugin 必须 echo caller execution_id；host 不重铸。

### LIFE-EXEC-003 — retry creates new process and id

Timeout → Retry：

```text
e-1 / P1
↓
e-2 / P2
```

两个 id 与 pid 都不同。

### LIFE-EXEC-004 — business error

Plugin 返回 business error。

期望：按既有 FailureClass::BusinessError 处理；不产生 lifetime-specific branch。

### LIFE-EXEC-005 — protocol violation

期望：Stop/cleanup；不改变 Workflow classification。

### LIFE-EXEC-006 — PluginUnavailable

spawn fail：按现有 bounded retry policy 工作。

### LIFE-EXEC-007 — confirmation

Confirmation pause/resume 后 re-resolve，再按 lifetime 启动新的 action process。

### LIFE-EXEC-008 — stale context

stale → re-resolve → ephemeral execute。

### LIFE-EXEC-009 — no persisted process state

重新创建 invocation 时，不依赖 previous plugin process memory。

### LIFE-EXEC-010 — MCP same path

MCP reference 与 plugin reference 均不出现 lifetime-specific Workflow code。

## 6. Security Tests

### LIFE-SEC-001

manifest lifetime 无法修改 capabilities。

### LIFE-SEC-002

plugin runtime 报告的 authorized/confirmed 不被信任。

### LIFE-SEC-003

ephemeral process 重新读取/验证 manifest policy，而不继承旧实例的可变状态。

### LIFE-SEC-004

跨 plugin action/provider identity 规则保持现有 `owns_plugin_namespace` 等统一 primitive。

### LIFE-SEC-005

lifetime 不绕过 ActionEngine。

### LIFE-SEC-006

workflow source 中无 PluginBroker/lifetime authority。

## 7. Soak Tests

### LIFE-SOAK-001

10,000 ephemeral query cycles。

记录：

```text
initial
min
median
p95
peak
final
slope_bytes_per_op
last_third_growth_bytes
peak_to_final_bytes
```

### LIFE-SOAK-002

1,000 ephemeral execute cycles。

### LIFE-SOAK-003

1,000 resident spawn/idle/respawn cycles。

### LIFE-SOAK-004

混合 70% ephemeral / 30% resident 长时间运行。

失败条件：staircase trend、最终 plugin_process_count 非 0、无法回落到 baseline envelope。

## 8. Performance Tests

### LIFE-PERF-001

Ephemeral query P50/P95。

### LIFE-PERF-002

Resident query P50/P95。

### LIFE-PERF-003

Ephemeral execute P50/P95。

### LIFE-PERF-004

Resident execute P50/P95。

### LIFE-PERF-005

分解：spawn / initialize / invoke / shutdown。

### LIFE-PERF-006

10k ephemeral soak memory trend。

### LIFE-PERF-007

Launcher idle Private Bytes 对比 baseline。

### LIFE-PERF-008

回归策略：

- >5%：warning + review
- >10%：fail
- baseline update 必须有理由/环境说明

## 9. Architecture Guards

### LIFE-ARCH-001

`launcher-workflow` source 不出现：

```text
PluginLifetime
Ephemeral
Resident
PluginBroker
McpExecutor
```

其中最后两项沿用现有 architecture guard，具体允许项以当前 topology script 为准。

### LIFE-ARCH-002

lifetime model 只能从 domain/plugin-host 方向被依赖，不允许 UI/Workflow 定义自己的生命周期 enum。

### LIFE-ARCH-003

依赖图无环。

### LIFE-ARCH-004

不引入新的长期驻留 runtime。

## 10. Test Fixtures

建议新增：

```text
lifecycle-echo
lifecycle-slow
lifecycle-crash
lifecycle-badshutdown
lifecycle-childspawner
lifecycle-action
```

优先复用现有 `example-testplugins` fixtures，不重复实现已有 failure behaviors。

## 11. Required Commands

最小门禁：

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python check_topology.py
```

benchmark：

```bash
cargo run -p launcher-bench
cargo run -p launcher-bench record
cargo run -p launcher-bench check
```

具体 CI 是否执行 benchmark record 取决于现有 benchmark policy，不允许 Agent 自行改 baseline。
