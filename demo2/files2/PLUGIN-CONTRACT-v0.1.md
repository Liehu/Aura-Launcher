# PLUGIN-CONTRACT-v0.1.md

## Native Launcher Plugin Contract v0.1

**Status:** Proposed Freeze  
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
