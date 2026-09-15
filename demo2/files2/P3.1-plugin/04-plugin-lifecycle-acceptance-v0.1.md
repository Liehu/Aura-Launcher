# Plugin Lifecycle v0.1 — Acceptance Specification

## 1. Acceptance Level

本规范把插件生命周期定义为 **MVP4.4 / Plugin Lifetime Contract Gate**。

验收不是“代码实现了 lifetime 字段”，而是证明：

```text
未使用的 ephemeral plugin 不持续存在
resident plugin 的旧语义不被破坏
生命周期差异不会进入 Workflow / Action / MCP 语义层
```

## 2. Golden Invariants

### INV-LIFE-001 — Legacy Compatibility

`runtime.lifetime` 缺失时，行为必须等价于现有 resident plugin 行为。

### INV-LIFE-002 — Ephemeral Isolation

一个 ephemeral invocation 必须对应一个独立 plugin process instance。

### INV-LIFE-003 — Ephemeral Exit

ephemeral invocation 完成后：

```text
plugin_process_count == 0
```

并且由 Job Object 管理的 descendant process tree 也不存在。

### INV-LIFE-004 — No Hidden Residency

ephemeral invocation 后 Host 不得保留：

- running child process
- background thread 专门服务该 plugin
- hidden persistent runtime
- unbounded plugin state cache

### INV-LIFE-005 — Resident Reuse

resident plugin 在 idle timeout 前允许复用同一进程实例。

### INV-LIFE-006 — Resident Idle Reclaim

resident idle timeout 到达后，进程及 descendants 必须被回收。

### INV-LIFE-007 — Attempt Identity

一次 execution attempt 只能产生一个 execution_id；retry 必须使用新的 execution_id。

### INV-LIFE-008 — Lifetime Is Executor-local

Workflow / ActionResolver / ActionEngine / MCP / AI Planner 不得判断或分支于 PluginLifetime。

### INV-LIFE-009 — Capability Unchanged

lifetime 不增加、减少或绕过 capability policy。

### INV-LIFE-010 — Failure Semantics Unchanged

Timeout / BusinessError / ProtocolViolation / PluginUnavailable 的 Workflow 分类与策略保持现有 contract。

## 3. Must-Pass Acceptance Cases

| ID | Given | When | Then |
|---|---|---|---|
| ACC-LIFE-001 | legacy plugin | query | 正常执行并保持原 resident 行为 |
| ACC-LIFE-002 | ephemeral plugin | query | query 后 plugin process 为 0 |
| ACC-LIFE-003 | ephemeral plugin | repeat query | 每次 invocation 新 process，不能复用 pid |
| ACC-LIFE-004 | ephemeral plugin with child | query complete | child/grandchild 全部清理 |
| ACC-LIFE-005 | resident plugin | repeat query before timeout | 复用同一 pid |
| ACC-LIFE-006 | resident plugin | idle timeout | process tree 为 0 |
| ACC-LIFE-007 | ephemeral slow plugin | timeout | process tree 清理，下次可重新执行 |
| ACC-LIFE-008 | ephemeral crash plugin | execute | 无残留，下次可重新 spawn |
| ACC-LIFE-009 | ephemeral malformed plugin | query | protocol failure 分类不变，进程退出 |
| ACC-LIFE-010 | ephemeral action | execute_action | execution_id 原样回显且 process 完成后退出 |
| ACC-LIFE-011 | timeout | workflow retry | e-1 → e-2，且对应新 process |
| ACC-LIFE-012 | confirmation | resume | 必须 re-resolve，然后新 invocation |
| ACC-LIFE-013 | stale context | retry path | 无 lifetime-specific branch |
| ACC-LIFE-014 | MCP reference | workflow execute | 与 plugin reference 相同 Runner/Resolver 路径 |
| ACC-LIFE-015 | forged capability metadata | execute | CapabilityDenied/现有拒绝语义不变 |
| ACC-LIFE-016 | 10k ephemeral cycles | soak | 无 staircase leak，final process count 0 |
| ACC-LIFE-017 | build | workspace build | 0 warnings |
| ACC-LIFE-018 | topology | check | pass |

## 4. Hard Gates

### Gate A — Build

```bash
cargo build --workspace
```

**必须：0 warnings。**

### Gate B — Test

```bash
cargo test --workspace
```

**必须：100% pass。**

任何 test ignored/skipped 都必须显式说明原因，不能用 skip 规避失败。

### Gate C — Topology

```bash
python check_topology.py
```

**必须 pass。**

### Gate D — Lifecycle cleanup

对：

```text
successful ephemeral
failed ephemeral
timeout ephemeral
crash ephemeral
malformed ephemeral
childspawner ephemeral
```

全部必须最终达到：

```text
plugin_process_count == 0
```

### Gate E — Architecture cleanliness

以下文件/模块不得引入 lifetime branching：

```text
launcher-workflow
launcher-action
MCP adapter
AI planner
UI presentation
```

允许的 lifecycle branching 位置仅限 PluginHost/runtime domain/test/bench。

### Gate F — Performance

执行 benchmark 后：

- P50/P95 有记录。
- 与 baseline 差异 >5% 必须 warning + review。
- >10% 自动 fail，除非显式更新 baseline 并记录理由。
- Launcher idle Private Bytes 不得超出现有预算。

### Gate G — Soak

10,000 ephemeral invocation：

```text
plugin_process_count final = 0
last_third_growth_bytes ≈ 0 / within baseline envelope
no staircase pattern
```

不能仅凭平均值通过。

## 5. Rejection Conditions

出现任何一项直接 REJECT：

1. ephemeral plugin 在 invocation 后持续存活。
2. 子进程/孙进程残留。
3. legacy plugin 行为发生未声明变化。
4. workflow 为支持 lifecycle 增加 MCP/plugin 分支。
5. ActionEngine 不再是唯一 Effect path。
6. executor/registry 重新铸造 execution_id。
7. Agent 增加隐式常驻 host 来“优化”ephemeral。
8. 通过修改 benchmark baseline 掩盖 >10% 回归。
9. topology 出现环或越权依赖。
10. 用线程 sleep 等待退出且无进程句柄/超时控制。

## 6. Evidence Package

Agent 完成后必须提供：

```text
1. Changed files
2. Contract diff / manifest schema diff
3. Test command + result
4. Lifecycle process-count evidence
5. Child-tree cleanup evidence
6. P50/P95 benchmark
7. Memory / soak trend summary
8. topology result
9. Architecture impact
10. Known risks
```

## 7. Recommended Demo Script

验收演示可以固定为：

```text
A. install ephemeral calculator plugin
B. Ctrl+Space
C. invoke query
D. verify result
E. inspect process tree -> no calculator/plugin process
F. invoke action
G. verify action result
H. inspect process tree -> no plugin process
I. switch same plugin to resident
J. invoke twice -> same PID
K. wait idle timeout -> process gone
L. run workflow timeout -> retry -> new execution_id
M. run MCP workflow -> same Runner path
```

## 8. Release Decision

### PASS

同时满足：

```text
All Must-Pass cases
+ All hard gates
+ No rejection condition
```

### CONDITIONAL PASS

仅允许文档/日志质量类非阻断项，不允许：

- lifecycle correctness defect
- process leak
- architecture violation
- security regression
- build/test/topology failure

### REJECT

任一 Hard Gate 失败或任一 Rejection Condition 命中。

## 9. Post-Acceptance Compatibility Rule

v0.1.x 后续版本只能新增 lifetime capability，不能改变：

```text
legacy absent -> resident
```

也不能在 Workflow/Action contract 中加入 lifecycle-specific semantics。

若以后需要：

```text
session
prewarm
adaptive pinning
auto promotion
memory quota
```

必须单独建立 ADR/contract revision，不得把它们作为隐式优化加入。
