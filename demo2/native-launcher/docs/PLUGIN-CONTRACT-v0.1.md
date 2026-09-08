# PLUGIN-CONTRACT-v0.1.md

## Native Launcher Plugin Contract v0.1

**Status:** **FROZEN** (2026-09-04, Freeze Gate 全项通过，见 ADR-0009；后续 breaking → v0.2，additive → v0.1.x)  
**Date:** 2026-09-04  
**Normative language:** MUST / MUST NOT / SHOULD / MAY

---

# 1. Scope

本文件是 Native Launcher 插件公开 Contract 的规范定义。

它定义：

```text
Manifest
Protocol
Command
Query
Action
Context
Capability
Error
Lifecycle
Limits
Compatibility
```

它不定义具体语言 SDK，也不要求 MVP 同时实现全部 runtime。

---

# 2. Plugin Boundary

第三方插件默认运行于独立进程。

```text
launcher-core
      ↓
plugin-host
      ↓
plugin process
```

Plugin MUST NOT：

- 依赖 Launcher UI 内部实现。
- 直接调用 Slint API。
- 直接修改 Core Provider Registry。
- 绕过 Action Engine 执行受控 Effect。
- 假设 Host 永久持有 Plugin Process。

---

# 3. Transport

v0.1 transport：

```text
NDJSON / JSON-RPC 2.0 over stdin/stdout
```

每条 request/response 必须占一行。

Plugin MUST NOT 将非协议文本写入 stdout；诊断日志使用 stderr。

---

# 4. Version Model

必须同时区分：

```text
schema_version
api_version
protocol_version
plugin_version
```

规则：

- `schema_version` 控制 Manifest 数据格式。
- `api_version` 控制 Host API 兼容性。
- `protocol_version` 控制 RPC framing/semantics。
- `plugin_version` 是插件自身发布版本。

未知 optional field：Host MUST ignore。  
不兼容 major API：Host MUST reject。  
未知 method：返回 `-32601`。

---

# 5. Manifest

推荐最小结构：

```json
{
  "schema_version": 1,
  "id": "com.example.echo",
  "name": "Echo",
  "version": "1.0.0",
  "api_version": "0.1",
  "runtime": {
    "type": "process",
    "executable": "plugin.exe"
  },
  "capabilities": [],
  "commands": [],
  "limits": {
    "query_timeout_ms": 2000,
    "action_timeout_ms": 10000,
    "idle_timeout_ms": 10000,
    "max_results": 100,
    "max_result_bytes": 1048576,
    "max_stderr_bytes": 65536
  }
}
```

### Manifest requirements

`id`, `name`, `version`, `api_version`, `runtime` MUST exist。

`id` SHOULD use reverse-DNS naming.

`runtime.executable` MUST resolve inside the plugin package directory.

Host MUST reject：

- absolute executable path逃逸
- `..` traversal
- invalid package path
- invalid API version
- invalid timeout
- invalid resource limits

---

# 6. Command

Plugin 的基本扩展对象是 `Command`。

```text
Command
├── id
├── title
├── subtitle?
├── description?
├── icon?
├── keywords[]
├── category?
├── mode
├── context_requirements[]
├── arguments[]
├── preview?
└── actions[]
```

Command 表示用户可发现、选择和执行的能力或实体。

Plugin SHOULD return stable command IDs。

UI selection MUST identify a command/action by ID，而不是以显示文本作为 identity。

---

# 7. Query

Required method：

```text
query
```

Request：

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "method": "query",
  "params": {
    "query_id": "q-123",
    "text": "term",
    "context": {},
    "limit": 50
  }
}
```

Response：

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "result": {
    "query_id": "q-123",
    "commands": []
  }
}
```

### Query invariants

- Host MUST generate a unique `query_id` for each logical query session/update。
- Plugin MUST echo `query_id`。
- Host MAY supersede/cancel an older query。
- Host MUST ignore late results belonging to a superseded query。
- Query SHOULD be side-effect free。
- Query MUST complete within Host-enforced timeout。

---

# 8. Action

Plugin returns `ActionDescriptor`。

```json
{
  "id": "open-terminal",
  "title": "Open Terminal Here",
  "type": "system.open_terminal",
  "input": {
    "cwd": "C:\\Projects\\Demo"
  },
  "requires": ["process.launch"]
}
```

Action execution path MUST be：

```text
Plugin
  ↓
ActionDescriptor
  ↓
Host validation
  ↓
Capability check
  ↓
Action Engine
  ↓
Effect
```

Plugin MUST NOT directly invoke arbitrary Host/OS effects through hidden side channels。

---

# 9. Context

Host MAY provide `ContextSnapshot` according to declared capabilities。

Canonical structure：

```json
{
  "timestamp": "...",
  "foreground": {
    "process_name": "explorer.exe",
    "window_id": "..."
  },
  "location": {
    "type": "folder",
    "path": "C:\\Projects\\Demo"
  },
  "selection": {
    "items": []
  }
}
```

Context access is capability-gated：

```text
context.process.read
context.window.read
context.location.read
context.selection.read
```

Context snapshot for a Popup Session SHOULD be stable for that session unless the session explicitly requests refresh。

---

# 10. Capability Model

Three states：

```text
Requested
   ↓
Granted
   ↓
Enforced
```

Manifest declares Requested。  
User/Host policy determines Granted。  
Runtime Broker enforces capability at call time。

### v0.1 capability set

```text
context.process.read
context.window.read
context.location.read
context.selection.read

clipboard.read
clipboard.write

filesystem.read
filesystem.write

network.connect

process.launch
shell.execute

notification.send
store.read
store.write
```

High-risk capabilities SHOULD require explicit user approval/policy。

Capability declarations without runtime enforcement are NOT considered security controls。

---

# 11. Error Contract

Standard JSON-RPC errors：

```text
-32700 Parse error
-32600 Invalid Request
-32601 Method not found
-32602 Invalid params
```

Private Host/plugin errors：

```text
-32001 CapabilityDenied
-32002 Timeout
-32003 PluginCrashed
-32004 ResultTooLarge
-32005 RateLimited
-32006 PluginUnavailable
-32007 VersionMismatch
```

Error MUST be deterministic and machine-readable。

User-facing layer SHOULD translate expected errors into non-blocking feedback where appropriate。

---

# 12. Lifecycle

Required lifecycle：

```text
Discovered
  ↓
Validated
  ↓
Ready
  ↓
Spawned
  ↓
Initialized
  ↓
Running
  ↓
Idle
  ↓
Terminated
```

Failure transitions：

```text
Running
  ├── timeout → Terminated
  ├── crash → Terminated
  ├── malformed result → Terminated
  └── protocol violation → Terminated
```

默认 policy：

```text
Builtin          permanent
External process on-demand
Python            on-demand
Node              on-demand
WASM              on-demand
```

---

# 13. Process Isolation

Windows external plugin MUST be contained in a Job Object before being allowed to run normally。

Preferred launch sequence：

```text
CreateProcess(CREATE_SUSPENDED)
        ↓
AssignProcessToJobObject
        ↓
apply limits
        ↓
ResumeThread
```

Termination MUST apply to the process tree。

The Host MUST detect and report orphan processes。

---

# 14. Resource Limits

Host MUST enforce upper bounds for：

```text
request bytes
response bytes
result count
result string length
action count
UI node count
UI nesting depth
icon/image size
stderr bytes
query time
action time
idle time
```

Manifest MUST NOT be able to request unlimited resources。

---

# 15. UI Schema

Plugin UI is declarative and renderer-independent。

```text
Plugin
  ↓
UI Schema
  ↓
Host validation
  ↓
Native UI Model
  ↓
Slint
```

Plugin MUST NOT depend on `slint`, `Window`, `HWND`, UI event loop, or widget internals。

v0.1 reserved primitives：

```text
List
ListItem
Section
Text
Separator
Form
TextField
PasswordField
Dropdown
Detail
Markdown
Image
Progress
```

MVP implementation may support only a subset。

---

# 16. Preview

Preview is an optional capability of a Command。

```text
Command
  └── preview
```

Preview data is validated by Host。

Plugin MUST NOT open arbitrary launcher-owned preview windows as a substitute for Preview Contract。

Web/HTML preview, if introduced, MUST be a separate isolated subsystem and MUST NOT become the launcher main UI plugin mechanism。

---

# 17. Preferences

Preferences are manifest-described data definitions。

Minimum types：

```text
text
password
checkbox
dropdown
file
directory
```

Secrets MUST NOT be silently stored in plugin plain-text files by Host-managed APIs。

Future implementations SHOULD use OS secure storage。

---

# 18. Background / Long-running Work

Not required in v0.1 implementation。

Reserved model：

```text
search
view
no-view
background
streaming
```

Long-running work SHOULD use Host-managed `RunHandle` so progress/cancel/error state remains outside plugin UI internals。

---

# 19. Ranking

Plugin may provide：

```text
keywords
category
optional bounded score_hint
```

Plugin MUST NOT directly determine final global ranking。

Core owns：

```text
lexical relevance
exact match
type prior
frequency
recency
context
provider prior
```

Plugin score hints, if enabled, MUST be bounded。

---

# 20. Trigger / Alias

Command metadata MAY include：

```text
keywords[]
aliases[]
trigger
arguments[]
```

v0.1 trigger types：

```text
prefix
exact
keyword
```

Regex triggers remain future capability and MUST execute under host-controlled timeout/length limits if introduced。

---

# 21. SDK Rules

An SDK MUST hide protocol mechanics from the plugin developer。

Developer SHOULD write：

```text
query(context)
→ Command[]
```

rather than manually handling：

```text
stdin
stdout
request IDs
JSON-RPC envelopes
framing
```

SDKs MUST remain protocol-compatible and MUST pass the common Contract Test Kit。

---

# 22. Contract Test Requirements

Every official SDK MUST pass：

```text
initialize
query-empty
query-normal
query-superseded
malformed-response
oversized-response
timeout
crash
action-validation
capability-denied
capability-allowed
idle-shutdown
process-tree-cleanup
version-negotiation
```

---

# 23. Performance Requirements

Plugin runtime MUST NOT be a permanent idle dependency unless explicitly declared by future policy。

Benchmark MUST distinguish：

```text
cold spawn → first result
warm query → first result
shutdown
peak private bytes
final private bytes
process count
spawn count
kill count
orphan count
```

Plugin-free idle MUST have：

```text
plugin_process_count = 0
```

---

# 24. Compatibility Policy

Compatible additions：

```text
new optional manifest field
new optional command field
new non-required capability
new optional UI node
```

Breaking changes require：

```text
new major api_version
ADR
migration note
Contract Test updates
```

Existing plugins MUST NOT silently change behavior because of a host upgrade。

---

# 25. Security Invariants

```text
INV-P01 Plugin cannot crash Core.
INV-P02 Plugin cannot bypass capability enforcement.
INV-P03 Plugin cannot escape its package executable path.
INV-P04 Plugin process tree can be terminated deterministically.
INV-P05 Plugin cannot directly manipulate launcher UI internals.
INV-P06 Plugin result size is bounded.
INV-P07 Plugin query timeout is host-enforced.
INV-P08 Plugin cannot directly execute privileged/unapproved effects.
INV-P09 Protocol violations are isolated to the plugin session.
INV-P10 Secrets are not inherited or persisted implicitly.
```

---

# 26. Performance Invariants

```text
INV-PERF01 Core idle does not require plugin runtimes.
INV-PERF02 Plugin runtime starts on demand by default.
INV-PERF03 Plugin exit removes the entire process tree.
INV-PERF04 Plugin result flood cannot cause unbounded memory growth.
INV-PERF05 Plugin timeout cannot block the UI thread.
INV-PERF06 Query supersession prevents stale result publication.
```

---

# 27. Freeze Boundary

## Frozen now

```text
Command
Action
Query
Context
Capability
Manifest
JSON-RPC
Error model
Lifecycle
Process isolation
Versioning
Resource limits
Contract tests
```

## Reserved but not implemented

```text
Python SDK
Node SDK
WASM runtime
Rich UI schema
Preview subsystem
Background tasks
Tools / MCP
Deep Links
Marketplace
Signature / trust chain
```

Any change to the frozen set requires an ADR。

---

# 28. Reference

本 Contract 的设计依据详见：

`docs/PLUGIN-DESIGN-REVIEW.md`

其中记录 Wox / Flow Launcher / Raycast / uTools / Lertaro / Asyar 的逐项比较，以及各项借鉴/排除决策。

---

# 29. Addendum: 协议边界硬化（review 11-mvp2-0.7，ADR-0008，2026-09-04）

冻结评审后的边界钉死，全部已实现并有契约测试。

## 29.1 版本四元组（INV-018）

`schema_version`（plugin.json 格式）/ `version`（插件自身版本）/ `api_version`（SDK/API 契约）/ `protocol_version`（IPC wire 协议，initialize 协商）**语义互斥，禁止合并**。

## 29.2 Manifest Profile（对 §5 的补充）

- `schema_version` 缺省 → **Legacy Manifest Profile**（顶层 `executable`）。
- `schema_version = 1` → **Contract v0.1 Manifest Profile**。

## 29.3 Response Profile（对 §7 的收紧）

- Legacy Manifest Profile：`query` 结果容忍裸数组（无 echo 可言）。
- v1 Manifest Profile：结果 MUST 为 `{query_id, commands}`；裸数组 = 协议违规。兼容路径由此有界，不会永久存在。
- 测试：`legacy_profile_manifest_tolerates_bare_array` / `v1_manifest_rejects_bare_array_response` ✅

## 29.4 并发语义（对 §7 的补充）

一个 PluginHandle 同时最多 **1 个 in-flight query**；旧 query 由 Host 侧按 query_id supersede（丢弃过期结果），**不要求插件实现 cancel**，cancel RPC 留待 v0.2+。（INV-021）

## 29.5 协议状态机（对 §12 的细化）

```text
NotStarted → Spawned → Initializing → Ready → Running ⇄ Ready → ShuttingDown → Stopped
```

非法迁移（Spawned→query、Ready→initialize、Stopped→query、Initializing→shutdown）= 协议违规，终止插件会话，不影响 Core（INV-P09）。当前 Host 为同步顺序状态机，天然满足。

## 29.6 stdout 红线（对 §3 的强化，INV-019）

**stdout 是协议通道，不是日志通道。** stdout 只允许 NDJSON JSON-RPC 帧，诊断输出 MUST 走 stderr；违反即 handshake/query 失败。
测试：`stdout_noise_violates_protocol_and_fails_handshake` ✅

## 29.7 资源限制 byte 级（对 §14 的补充，INV-020）

| 限制 | 值 | 语义 |
|---|---|---|
| max result items | 100（`MAX_PLUGIN_RESULTS`） | 截断 |
| `MAX_FRAME_BYTES` | 256KB/帧 | **违规 → kill** |

`max_result_bytes` / `max_stderr_bytes` / `max_process_runtime` 为 manifest 预留字段；数字待 benchmark，概念现在冻结。
测试：`oversized_frame_is_a_violation_not_truncated` ✅

## 29.8 错误码三层（对 §11 的分类）

Transport（进程/管道级，不进 JSON-RPC 空间）／Protocol（标准码 + -32001..-32007）／Plugin（插件自有错误放 `error.data`，不得侵占私有区间）。

## 29.9 优雅关闭语义（对 §12 的澄清）

协议层只规定 `shutdown requested`；宽限期（当前默认 200ms + 10×10ms 轮询）是 **Host implementation policy**，可按 runtime 调整，不写入协议。

## 29.10 握手身份

`initialize.params.plugin_id` 为 Host→Plugin 单向声明；v0.1 的 InitializeResult 仅含 `protocol_version`，无插件回显 plugin_id 的攻击面。未来若引入回显，MUST 做 identity binding（不匹配即拒绝）。

## 29.11 进程树清理测试（对 §13）

实现不动（ADR-0005 保持现状），补真实行为测试：插件 spawn 孙进程后 Host kill，Job Object 必须回收整树。
测试：`killing_plugin_reaps_whole_process_tree` ✅

## 29.12 Canonical Reference Plugin 门禁

`apps/calculator-plugin` 为 Canonical Reference Plugin：**每次 Contract 变更必须先让 calculator 通过全量 E2E，否则不允许合入协议修改。**

## 29.14 Python 脚本运行时（ADR-0009，additive）

`runtime.type` 新增 `"python"`：`executable` 为插件目录内的脚本路径（confinement 不变，INV-013），Host 用解析出的解释器 spawn：`interpreter script.py [runtime.args]`。

**解释器解析顺序**（用户可自定义）：
1. `config.toml` 的 `python_path`（支持 `%VAR%` / `${VAR}` / `$VAR` 环境变量，如 `"%LOCALAPPDATA%\Programs\Python\python.exe"`）
2. 环境变量 `LAUNCHER_PYTHON`
3. PATH 上的 `python`

Host 对解释器与可执行路径统一做 `expand_env` 展开（plugin-host::expand_env），未知变量原样保留、由 spawn 报错。

## 29.15 shutdown 与 in-flight query

`shutdown` 到达时若 query 仍在途：**该 query 被放弃，任何迟到结果不被接受**（handle 销毁/进程退出）。纯状态机语义，不新增 RPC。

## 29.16 资源策略 Roadmap

v0.1 冻结：frame/result 条数/超时/进程树/优雅关闭。**v0.2+**：memory quota、CPU quota、query rate limit、stderr volume——概念现在记录，实现等真实需求（不提前引入 Job Object quota 复杂度）。

## 29.17 Plugin Contract Test Kit（正式命名）

`apps/example-testplugins/tests/contract.rs`（16 项）+ calculator/Python E2E 正式命名为 **Plugin Contract Test Kit**：任何新增 SDK（Python/Node/WASM）必须跑同一套 conformance；`launcher-ipc` / `launcher-plugin-api` / `launcher-plugin-host` 的修改在 calculator conformance 未通过时不得合并（AGENTS 规则）。

## 29.19 Runtime 抽象（ADR-0010）

- **`runtime.type` 是 Host 启动策略，不是编程语言**：`"python"` 意为"Host 使用 Python Runtime Strategy 启动"，插件完全可以用 `runtime.type = "process"` 启动自带解释器的 host 可执行。语义上禁止把 runtime.type 当作语言标签使用。
- **RuntimeResolver 单一扩展点**：`runtime.type → LaunchPlan{program, args}` 的映射只存在于 `launcher-plugin-host::runtime::resolve_launch`；未来 node/wasm 只新增 match 分支，spawn 与 Contract 其余部分保持 runtime-agnostic。禁止在 Host 其他位置堆 runtime 分支。
- **Runtime Discovery 策略**（每个 runtime 一致）：explicit config → environment override（如 `LAUNCHER_PYTHON`）→ system discovery（PATH）→ unavailable（局部失败，仅该插件空结果）。解析来源必须进日志（diagnostics），为未来 `launcher plugins doctor` 留数据基础。

## 29.20 作者 API / Host API 边界（ADR-0010）

- **Plugin Author API**：`launcher-plugin-api`（Rust）与 `plugins/python/launcher_plugin.py`（Python）。插件开发者只允许依赖这些。
- **Host API（内部）**：`launcher-plugin-host` / `launcher-core` / `launcher-context` / `launcher-action` 对插件作者不可见、不可依赖；SDK 不得为便利而 re-export Host 内部类型，防止 Host 实现细节绑架 SDK。

## 29.21 SDK Conformance 分层（ADR-0010）

```text
Protocol Conformance   → Plugin Contract Test Kit（Host 侧，16+ 项）
SDK Conformance        → 每个 SDK（Rust/Python/未来 Node/WASM）跑同一 Test Kit
Reference Plugins      → calculator（Rust，Canonical）+ calculator（Python）
```

新增 SDK 版本只跑 Test Kit，杜绝"理论上兼容、实际行为漂移"。

## 29.22 本 Addendum 后仍明确不做

reference-stateful plugin（store 后端未实现）、Python/Node SDK、UI Schema/Action 扩展、cancel RPC、regex trigger——见 §27 Freeze Boundary 与 review §36 P2+。

---

# 30. Addendum（ADR-0014，MVP4.0）：`execute_action` RPC（v0.1.x additive）

方法集扩展（additive，不破坏 §15/§29）：

```text
initialize / query / shutdown / execute_action   ← 新增
```

- Params：`{execution_id, action_id, input, context_generation}`；结果 MUST 回显 `execution_id`：`{"execution_id": "...", "result": ...}`。
- `execution_id`（`e-<seq>`）与 `query_id`（`q-<seq>`）空间完全分离（INV：状态管理不互相污染）。
- 仅 manifest 声明 `plugin.invoke` 的插件可被调用；action type 必须以 `plugin.<自身 manifest id>.` 开头。
- 插件级失败返回 JSON-RPC error（-32603 等）= EffectFailed，Host 不终止进程；timeout/malformed 仍按 §29.5 状态机终止。
- SDK：Rust `serve_with_actions(query, action)`；Python SDK 待 MVP4.x 同步。

Contract tests 增补：execute_action roundtrip / unknown action / identity binding（`apps/calculator-plus/tests/mvp4_acceptance.rs`）。

### 30.1 execution_id 生命周期（review 23 §6）

`execution_id` 标识**一次最终 Effect execution attempt**，不是一次用户 Action。一次用户 Enter 产生的多个 Effect（如 Workflow 计划）各自拥有独立 execution_id；上层编排对象（`workflow_run_id → step_id → execution_id`）分层保存自己的 id，不污染本协议字段。

### 30.2 context_generation 语义边界（review 23 §7）

`context_generation` 只保证"action 与解析时所见的 Context 同代"（Context validity），不构成事务锁：文件存在性、窗口存活、网络在线、插件状态属于 Execution preconditions 与 Effect failure 层，由执行链 `generation check → precondition validation → actual Effect` 分层处理。
