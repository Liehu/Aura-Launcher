# P3-UI.0 Contract Foundation

**Project:** Aura Launcher / Native Launcher  
**Phase:** P3-UI  
**Batch:** P3-UI.0  
**Version:** 1.0  
**Status:** FROZEN  
**Purpose:** Freeze the contract foundation for Plugin Tool, Interactive Tool Session, Declarative UI, Rich Result and Workflow Tool Function integration.

---

# 0. Document Set

P3-UI.0 不作为一个单独的超大实现任务，而是冻结以下四个文档：

```text
docs/
├── P3-UI.0-AGENTIC-CODING.md
├── P3-UI.0-STATE-MODEL.md
├── P3-UI.0-SCHEMAS.md
└── P3-UI.0-IPC-TEST-MATRIX.md
```

并增加：

```text
P3-UI.0-ACCEPTANCE.md
```

作为最终 Release Gate。

---

# 1. Scope

P3-UI.0 只负责：

```text
Plugin Manifest Extension
Tool Manifest
Tool Identity
Tool Session
UI Schema Contract
IPC Extension
Capability Binding
RichResult Projection Contract
Workflow Tool Function Contract
Schema Validation
Contract Test Foundation
```

P3-UI.0 不实现：

```text
完整 UI Host
Slint 组件
Tool Runtime
Plugin Manager UI
Workflow UI
Marketplace
AI
Agent
Adaptive Intelligence
Python SDK
Node SDK
WASM
WebView
```

---

# 2. Existing Frozen Contracts

以下内容属于已有契约，不重新设计：

```text
Plugin Identity
Plugin Installation
Plugin Trust
Plugin Capability Decision
Plugin Lifecycle
Plugin Runtime
NDJSON / JSON-RPC 2.0
Command
Query
Action
ContextSnapshot
Error Model
Resource Limits
Process Isolation
Workflow Definition
WorkflowRun
ActionReference
ActionResolver
ActionEngine
```

Plugin 默认运行于独立进程，不依赖 Launcher UI/Core，也不能绕过 Action Engine。

Capability 继续采用：

```text
Requested
    ↓
Granted
    ↓
Enforced
```

Manifest 声明本身永远不等于授权。

---

# 3. New Frozen Contracts

P3-UI.0 新增：

```text
Tool
ToolDefinition
ToolFunction
ToolSession
UiSchema
UiNode
UiEvent
RichResult
ToolReference
```

关系：

```text
Plugin
 ├── Command
 ├── Action
 └── Tool
      ├── ToolFunction
      └── ToolSession

SearchCandidate
      ↓
ResultProjection
      ↓
RichResult
      ↓
ToolReference
      ↓
ToolSession
```

---

# 4. Core Architectural Invariants

```text
P3UI-001
Tool is an interactive capability, not an Effect.

P3UI-002
ToolFunction is a logical callable capability, not an authorization object.

P3UI-003
ToolSession is UI/session state, not authorization.

P3UI-004
RichResult is a presentation projection, not a domain authority.

P3UI-005
UiEvent is input, not Effect.

P3UI-006
UI node identity is session-scoped and never a Workflow identity.

P3UI-007
Workflow references ToolFunction, never Widget/UiNode.

P3UI-008
Effectful Tool actions must re-enter ActionResolver / Policy / ActionEngine.

P3UI-009
Manifest capability declarations never grant capability.

P3UI-010
Plugin cannot access native UI internals.

P3UI-011
Plugin failure cannot terminate Core.

P3UI-012
Plugin timeout cannot block the UI thread.

P3UI-013
Every externally supplied UI object is validated before rendering.

P3UI-014
Every Tool Session has bounded lifetime and bounded resources.

P3UI-015
All P3-UI caches are bounded and generation-aware.
```

---

# 5. Agentic Coding Spec

## 5.1 Agent Task Model

所有 P3-UI.0 Agent Task 必须遵循：

```text
Inspect
  ↓
Read frozen contract
  ↓
Map existing abstractions
  ↓
Mini-plan
  ↓
Implement smallest slice
  ↓
Targeted tests
  ↓
Contract tests
  ↓
Workspace tests
  ↓
Review diff
  ↓
Report
```

这与项目现有 Agentic Coding 模式一致：先读取 contract，再最小切片实现，再 targeted/conformance/workspace gate。

---

# 6. P3-UI.0 Batch Decomposition

```text
P3-UI.0
│
├── A Contract Types
│
├── B Manifest / Tool Schema
│
├── C State Machine
│
├── D IPC Methods
│
├── E UI Schema Validation
│
├── F RichResult Projection
│
├── G Workflow Tool Reference
│
├── H Contract Test Kit
│
└── I Acceptance / Documentation
```

---

# 7. Task P3UI-A — Contract Domain Types

## Goal

在 `launcher-domain` 中增加最小 P3-UI Domain Types。

## Allowed

```text
ToolId
ToolFunctionId
ToolReference
ToolDefinition
ToolFunction
ToolSessionId
UiNodeId
UiSchema
UiNode
UiEvent
RichResult
```

## Forbidden

```text
Slint type
Window
HWND
PluginHost
ActionEngine
Capability Broker
SQLite implementation
```

## Required Rule

Domain 层必须保持 pure model。

不得在这些类型中加入：

```text
filesystem IO
process spawn
UI rendering
database access
effect execution
```

---

# 8. Task P3UI-B — Manifest / Schema

## Goal

冻结 Tool Manifest JSON Schema。

## Required

```text
plugin.json
tool.json
tool function schema
ui schema
```

## Requirements

Schema validation 必须：

```text
deterministic
fail closed
bounded
version-aware
```

Manifest 原有字段继续使用，P3-UI 只增加兼容扩展。现有 Manifest 已要求 `id/name/version/api_version/runtime`，并要求 executable 位于 package 内。

---

# 9. Task P3UI-C — State Model

冻结：

```text
Plugin lifecycle
Tool lifecycle
ToolSession lifecycle
UI generation
```

不得把：

```text
Trust
Capability
Lifecycle
Runtime
Session
```

合成一个 enum。

P2.4 已明确要求 Installation / Lifecycle / Trust / Capability / Runtime 分离。

---

# 10. Task P3UI-D — IPC

继续使用：

```text
NDJSON
JSON-RPC 2.0
stdin/stdout
```

不引入第二传输层。

现有 Contract 已冻结逐行 JSON-RPC、stdout purity 和 stderr diagnostics。

新增：

```text
tool.list
tool.open
tool.close
tool.event
tool.update
tool.cancel
```

---

# 11. Task P3UI-E — UI Schema Validation

Host 在 UI Schema 进入 renderer 前完成：

```text
schema validation
identity validation
node limit validation
depth validation
text limit validation
asset validation
event binding validation
```

失败时：

```text
reject schema
preserve plugin process
emit diagnostic
```

不得执行 Effect。

---

# 12. Task P3UI-F — RichResult

建立：

```text
SearchCandidate
   ↓
ResultProjection
   ↓
RichResult
```

禁止修改 SearchCandidate 的 domain semantics。

RichResult 只做：

```text
presentation
navigation
detail projection
action projection
tool opening
```

---

# 13. Task P3UI-G — Workflow Tool Function

Tool Function 可被 Workflow 引用。

Workflow 保存：

```text
ToolReference
```

而不是：

```text
ToolSession
UiNode
ResolvedAction
Effect
```

Workflow 原有原则要求 Definition 持有 logical Action reference/proposal，而不是 ResolvedAction；执行前重新 Resolve。

---

# 14. Task P3UI-H — Contract Test Kit

必须为：

```text
Manifest
Tool
UI Schema
IPC
Session
Capability
Workflow
RichResult
```

建立可复用 contract fixtures。

至少：

```text
valid
malformed
unknown optional
missing required
oversized
wrong version
stale session
duplicate id
invalid reference
capability denied
```

---

# 15. Task P3UI-I — Documentation

必须同步：

```text
P3-UI.0-AGENTIC-CODING.md
P3-UI.0-STATE-MODEL.md
P3-UI.0-SCHEMAS.md
P3-UI.0-IPC-TEST-MATRIX.md
P3-UI.0-ACCEPTANCE.md
ADR index
```

任何 Frozen Contract 变化必须先写 ADR。

---

# 16. Allowed Repository Areas

Agent 默认只允许修改：

```text
crates/launcher-domain/
crates/launcher-ipc/
crates/launcher-plugin-api/
crates/launcher-plugin-testkit/
crates/launcher-search/
docs/
schemas/
tests/
```

除非任务明确授权，不允许修改：

```text
crates/launcher-action/
crates/launcher-core/
crates/launcher-plugin-host/
```

如果必须跨越这些边界：

```text
STOP
Report contract impact
Require explicit task approval
```

---

# 17. Dependency Direction

必须保持：

```text
launcher-domain
      ↑
launcher-ipc
      ↑
launcher-plugin-api
      ↑
launcher-plugin-testkit

launcher-search
      ↓
ResultProjection

plugin-host
      ↓
ipc
      ↓
domain
```

禁止：

```text
domain → UI
domain → host
domain → Slint
domain → ActionEngine
```

---

# 18. SQLite / State Model

P3-UI.0 原则上**不新建独立 SQLite 数据库**。

已有 Plugin Registry 是 authority for persistent Plugin state。

P3-UI 的持久化只增加最小 schema，用于：

```text
Tool metadata
Tool Function metadata
UI compatibility metadata
session history/optional state
```

但：

```text
ToolSession
UI generation
active runtime state
```

默认为 ephemeral。

---

# 19. SQLite Ownership

推荐继续：

```text
plugins.db
```

增加：

```text
plugin_tools
tool_functions
plugin_ui_contracts
```

不要新增：

```text
tools.db
ui.db
sessions.db
```

避免 P3-UI 再引入一个持久化 authority。

---

# 20. SQLite ERD

```text
plugins
  │
  │ 1:N
  ▼
plugin_tools
  │
  ├───────────┐
  │           │
  │ 1:N       │ 1:1
  ▼           ▼
tool_functions  plugin_ui_contracts
```

---

# 21. Table: plugin_tools

```sql
CREATE TABLE plugin_tools (
    plugin_id          TEXT NOT NULL,
    tool_id             TEXT NOT NULL,
    version              TEXT NOT NULL,
    name                 TEXT NOT NULL,
    description          TEXT,
    category             TEXT,
    icon_ref             TEXT,
    entry_type           TEXT NOT NULL,
    enabled              INTEGER NOT NULL DEFAULT 1,
    schema_version       INTEGER NOT NULL,
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,

    PRIMARY KEY (plugin_id, tool_id),

    FOREIGN KEY (plugin_id)
        REFERENCES plugins(plugin_id)
);
```

约束：

```text
entry_type ∈ {interactive}
enabled ∈ {0,1}
schema_version >= 1
```

---

# 22. Table: tool_functions

```sql
CREATE TABLE tool_functions (
    plugin_id             TEXT NOT NULL,
    tool_id               TEXT NOT NULL,
    function_id            TEXT NOT NULL,
    input_schema_json      TEXT NOT NULL,
    output_schema_json     TEXT NOT NULL,
    deterministic          INTEGER NOT NULL,
    side_effect            INTEGER NOT NULL,
    required_capabilities  TEXT NOT NULL DEFAULT '[]',

    PRIMARY KEY (
        plugin_id,
        tool_id,
        function_id
    ),

    FOREIGN KEY (
        plugin_id,
        tool_id
    )
    REFERENCES plugin_tools(
        plugin_id,
        tool_id
    )
);
```

规则：

```text
deterministic ∈ {0,1}
side_effect ∈ {0,1}
required_capabilities = JSON array
```

---

# 23. Table: plugin_ui_contracts

```sql
CREATE TABLE plugin_ui_contracts (
    plugin_id            TEXT NOT NULL,
    tool_id              TEXT NOT NULL,
    ui_schema_version    INTEGER NOT NULL,
    min_host_version     TEXT,
    max_host_version     TEXT,
    max_nodes            INTEGER NOT NULL,
    max_depth            INTEGER NOT NULL,
    max_text_bytes       INTEGER NOT NULL,
    max_asset_bytes      INTEGER NOT NULL,

    PRIMARY KEY (
        plugin_id,
        tool_id
    ),

    FOREIGN KEY (
        plugin_id,
        tool_id
    )
    REFERENCES plugin_tools(
        plugin_id,
        tool_id
    )
);
```

---

# 24. What NOT to Persist

以下默认禁止写入 SQLite：

```text
active UI node tree
active ToolSession object
focused node
current pointer
native window handle
Slint object
raw clipboard content
password field value
authorization token
Effect
ResolvedAction
```

---

# 25. ToolSession State

```text
Created
   ↓
Starting
   ↓
Ready
   ↓
Active
   ↓
Idle
   ↓
Closing
   ↓
Closed
```

Failure:

```text
Starting → Failed

Active → Failed
Active → Crashed
Active → Timeout
```

Cancellation：

```text
Active → Closing → Closed
```

---

# 26. Tool State Rules

```text
Created
```

只能：

```text
Starting
```

```text
Starting
```

可以：

```text
Ready
Failed
Crashed
Timeout
```

```text
Ready
```

必须：

```text
Active
```

```text
Closed
```

是 terminal state。

禁止：

```text
Closed → Active
Failed → Active
```

除非创建全新 Session。

---

# 27. UI Generation

每个 ToolSession：

```rust
ui_generation: u64
```

规则：

```text
initial state = 0

accepted update
    → +1

rejected update
    → unchanged

stale update
    → ignored

closed session
    → no more updates
```

---

# 28. Event Identity

每一个 Tool Event：

```text
session_id
event_id
node_id
event_type
```

唯一性：

```text
(session_id, event_id)
```

Host 必须拒绝：

```text
duplicate event_id
event for closed session
event for unknown node
```

---

# 29. Session Identity

Canonical:

```text
ToolSessionId
```

由 Host 生成。

Plugin：

```text
MUST NOT choose authoritative session identity
```

---

# 30. UI Node Identity

Canonical:

```text
UiNodeId
```

范围：

```text
within ToolSession
```

因此：

```text
session A / node "input"
≠
session B / node "input"
```

Workflow 不得保存这个 ID。

---

# 31. JSON Schema Package

推荐：

```text
schemas/p3-ui/
├── plugin-ui-extension.schema.json
├── tool.schema.json
├── tool-function.schema.json
├── ui-schema.schema.json
├── ui-node.schema.json
├── ui-event.schema.json
├── rich-result.schema.json
└── tool-reference.schema.json
```

---

# 32. Plugin UI Extension Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "aura://schemas/p3-ui/plugin-ui-extension.schema.json",
  "type": "object",
  "properties": {
    "ui": {
      "$ref": "ui-extension.schema.json"
    },
    "tools": {
      "type": "array",
      "items": {
        "$ref": "tool.schema.json"
      }
    }
  },
  "additionalProperties": true
}
```

规则：

```text
unknown optional fields → ignored
unknown required semantic → rejected
```

---

# 33. Tool Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "aura://schemas/p3-ui/tool.schema.json",

  "type": "object",

  "required": [
    "id",
    "name",
    "entry",
    "schema_version"
  ],

  "properties": {
    "id": {
      "type": "string",
      "pattern": "^[a-zA-Z0-9._-]+$",
      "maxLength": 128
    },

    "name": {
      "type": "string",
      "minLength": 1,
      "maxLength": 256
    },

    "description": {
      "type": "string",
      "maxLength": 4096
    },

    "category": {
      "type": "string",
      "maxLength": 128
    },

    "icon": {
      "type": "string",
      "maxLength": 512
    },

    "schema_version": {
      "type": "integer",
      "const": 1
    },

    "entry": {
      "type": "object",
      "required": ["type"],
      "properties": {
        "type": {
          "const": "interactive"
        }
      },
      "additionalProperties": false
    }
  },

  "additionalProperties": false
}
```

---

# 34. Tool Function Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",

  "type": "object",

  "required": [
    "id",
    "input_schema",
    "output_schema",
    "deterministic",
    "side_effect",
    "required_capabilities"
  ],

  "properties": {
    "id": {
      "type": "string",
      "maxLength": 128
    },

    "input_schema": {
      "type": "object"
    },

    "output_schema": {
      "type": "object"
    },

    "deterministic": {
      "type": "boolean"
    },

    "side_effect": {
      "type": "boolean"
    },

    "required_capabilities": {
      "type": "array",
      "items": {
        "type": "string",
        "maxLength": 128
      },
      "uniqueItems": true
    }
  },

  "additionalProperties": false
}
```

---

# 35. UI Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",

  "type": "object",

  "required": [
    "schema_version",
    "root"
  ],

  "properties": {
    "schema_version": {
      "type": "integer",
      "const": 1
    },

    "root": {
      "$ref": "ui-node.schema.json"
    }
  },

  "additionalProperties": false
}
```

---

# 36. UI Node Schema

```json
{
  "type": "object",

  "required": [
    "type",
    "id"
  ],

  "properties": {
    "type": {
      "type": "string",
      "enum": [
        "container",
        "section",
        "text",
        "markdown",
        "icon",
        "image",

        "text_field",
        "password_field",
        "text_area",
        "dropdown",
        "checkbox",
        "radio",
        "toggle",
        "number_input",

        "button",
        "action_button",

        "list",
        "list_item",
        "detail",
        "key_value",

        "progress",
        "status",
        "badge",

        "separator",
        "spacer"
      ]
    },

    "id": {
      "type": "string",
      "maxLength": 128
    },

    "children": {
      "type": "array",
      "items": {
        "$ref": "ui-node.schema.json"
      }
    }
  },

  "additionalProperties": true
}
```

Schema validation之外仍必须执行 runtime resource limits。

JSON Schema 本身不是资源安全控制。

---

# 37. UI Event Schema

```json
{
  "type": "object",

  "required": [
    "session_id",
    "event_id",
    "node_id",
    "event"
  ],

  "properties": {
    "session_id": {
      "type": "string",
      "maxLength": 128
    },

    "event_id": {
      "type": "string",
      "maxLength": 128
    },

    "node_id": {
      "type": "string",
      "maxLength": 128
    },

    "event": {
      "type": "string",
      "enum": [
        "click",
        "change",
        "submit",
        "select",
        "focus",
        "blur",
        "refresh",
        "copy",
        "paste",
        "close"
      ]
    },

    "value": {}
  },

  "additionalProperties": false
}
```

---

# 38. RichResult Schema

```json
{
  "type": "object",

  "required": [
    "id",
    "kind",
    "title"
  ],

  "properties": {
    "id": {
      "type": "string",
      "maxLength": 256
    },

    "kind": {
      "type": "string",
      "enum": [
        "Application",
        "File",
        "Folder",
        "Command",
        "Plugin",
        "Tool",
        "Workflow",
        "SystemTarget"
      ]
    },

    "title": {
      "type": "string",
      "maxLength": 256
    },

    "subtitle": {
      "type": "string",
      "maxLength": 512
    },

    "description": {
      "type": "string",
      "maxLength": 4096
    },

    "icon": {
      "type": "string",
      "maxLength": 512
    }
  },

  "additionalProperties": true
}
```

---

# 39. ToolReference

Workflow / Host 使用：

```json
{
  "plugin_id": "com.example.codec",
  "tool_id": "base64",
  "function_id": "decode"
}
```

Canonical identity：

```text
com.example.codec/base64/decode
```

---

# 40. ToolReference Schema

```json
{
  "type": "object",

  "required": [
    "plugin_id",
    "tool_id",
    "function_id"
  ],

  "properties": {
    "plugin_id": {
      "type": "string",
      "maxLength": 128
    },

    "tool_id": {
      "type": "string",
      "maxLength": 128
    },

    "function_id": {
      "type": "string",
      "maxLength": 128
    }
  },

  "additionalProperties": false
}
```

---

# 41. IPC Extension

Existing Plugin IPC：

```text
initialize
query
action
shutdown
```

P3-UI adds:

```text
tool.list
tool.open
tool.close
tool.event
tool.update
tool.cancel
```

---

# 42. tool.list

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "tool.list",
  "params": {
    "request_id": "tr-001"
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "result": {
    "request_id": "tr-001",
    "tools": []
  }
}
```

---

# 43. tool.open

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "tool.open",
  "params": {
    "session_id": "ts-001",
    "tool_id": "base64",
    "context": {},
    "initial_input": {
      "text": "hello"
    }
  }
}
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "result": {
    "session_id": "ts-001",
    "ui_generation": 0,
    "ui": {}
  }
}
```

---

# 44. tool.event

Request:

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "tool.event",
  "params": {
    "session_id": "ts-001",
    "event_id": "ev-001",
    "node_id": "encode",
    "event": "click"
  }
}
```

Response may contain:

```text
ui update
state update
result
action proposal
error
```

---

# 45. tool.update

Plugin → Host:

```json
{
  "jsonrpc": "2.0",
  "method": "tool.update",
  "params": {
    "session_id": "ts-001",
    "base_generation": 0,
    "update": {}
  }
}
```

Host MUST reject:

```text
base_generation != current_generation
```

unless update is explicitly defined as commutative.

P3-UI.0 默认：

```text
no implicit merge
```

---

# 46. tool.close

```json
{
  "jsonrpc": "2.0",
  "id": 13,
  "method": "tool.close",
  "params": {
    "session_id": "ts-001"
  }
}
```

成功后：

```text
Session = Closed
```

后续任何 UI update：

```text
ignored/rejected
```

---

# 47. tool.cancel

```json
{
  "jsonrpc": "2.0",
  "id": 14,
  "method": "tool.cancel",
  "params": {
    "session_id": "ts-001",
    "event_id": "ev-001"
  }
}
```

用于取消：

```text
current Tool Event
```

不是：

```text
authorization cancellation
Effect cancellation
Workflow cancellation
```

这些仍由各自 Contract 管理。

---

# 48. IPC Error Extension

Existing errors remain:

```text
-32001 CapabilityDenied
-32002 Timeout
-32003 PluginCrashed
-32004 ResultTooLarge
-32005 RateLimited
-32006 PluginUnavailable
-32007 VersionMismatch
```



P3-UI adds:

```text
-32008 ToolNotFound
-32009 ToolSessionExpired
-32010 InvalidUiSchema
-32011 UiLimitExceeded
-32012 InvalidToolInput
-32013 ToolFunctionNotFound
-32014 UnsupportedUiSchema
-32015 StaleUiGeneration
-32016 InvalidUiEvent
```

---

# 49. IPC Contract Invariants

```text
IPC-001
Every request carries JSON-RPC id.

IPC-002
Every Tool Session is Host-created.

IPC-003
Every Event is bound to one session.

IPC-004
Every Event is uniquely identified within a session.

IPC-005
Closed sessions accept no further events.

IPC-006
Late responses do not modify newer state.

IPC-007
UI generation is monotonic.

IPC-008
Stale UI update cannot overwrite newer state.

IPC-009
Tool timeout cannot block Host UI.

IPC-010
Plugin process failure does not terminate Core.

IPC-011
stdout remains protocol-only.

IPC-012
All oversized frames are rejected before allocation growth becomes unbounded.
```

现有 Plugin Contract 已经要求 query supersession、bounded result 和 timeout enforcement。

---

# 50. Capability Mapping

P3-UI 不创建新的：

```text
tool.read
tool.execute
tool.ui
```

默认 Capability。

Tool UI 本身属于 Host-controlled protocol。

需要系统能力时继续使用已有：

```text
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



---

# 51. Pure Tool Function

定义：

```text
side_effect = false
```

例如：

```text
base64.encode
base64.decode
url.encode
url.decode
json.format
timestamp.convert
uuid.generate
```

这些可以：

```text
Tool Runtime
 ↓
Function
 ↓
Result
```

无需 Effect。

---

# 52. Effectful Tool Function

定义：

```text
side_effect = true
```

例如：

```text
clipboard.write
filesystem.write
process.launch
```

执行必须：

```text
Tool
 ↓
Function
 ↓
Action Proposal
 ↓
Host validation
 ↓
Capability / Policy
 ↓
ActionResolver
 ↓
ActionEngine
 ↓
Effect
```

---

# 53. Workflow Integration

Workflow Step 不允许：

```text
session_id
node_id
ui.event
ui.click
```

允许：

```text
ToolReference
```

例如：

```json
{
  "reference": {
    "plugin_id": "com.example.codec",
    "tool_id": "base64",
    "function_id": "decode"
  },
  "input": {
    "text": "${input}"
  }
}
```

---

# 54. Workflow Execution Boundary

完整路径：

```text
WorkflowDefinition
      ↓
ToolReference
      ↓
Fresh Resolve
      ↓
Tool Function
      ↓
Capability / Policy
      ↓
Execution
```

Workflow 不获得：

```text
Tool Runtime Handle
ToolSession
Plugin Process Handle
UI Node Handle
Effect Token
```

这保持 Workflow “orchestration only”的冻结原则。

---

# 55. RichResult Integration

推荐：

```text
SearchCandidate
      ↓
ResultProjection
      ↓
RichResult
```

Tool Result：

```json
{
  "id": "com.example.codec/base64",
  "kind": "Tool",
  "title": "Base64",
  "subtitle": "Encode / Decode",
  "primary_action": {
    "type": "open_tool",
    "tool": {
      "plugin_id": "com.example.codec",
      "tool_id": "base64"
    }
  }
}
```

---

# 56. RichResult Authority Rule

以下字段：

```text
title
subtitle
icon
description
badge
metadata
```

仅用于呈现。

以下字段：

```text
open_tool
run
copy
save
launch
```

如果会产生 Effect：

```text
must become logical Action
```

不能直接成为 Effect。

---

# 57. Contract Test Matrix

## 57.1 Manifest

| ID | Test | Expected |
|---|---|---|
| M01 | valid tool | PASS |
| M02 | missing id | REJECT |
| M03 | missing name | REJECT |
| M04 | invalid tool id | REJECT |
| M05 | duplicate tool id | REJECT |
| M06 | unsupported schema version | REJECT |
| M07 | unknown optional field | IGNORE |
| M08 | oversized metadata | REJECT |
| M09 | invalid entry type | REJECT |
| M10 | forged capability grant | NO GRANT |

---

# 58. Tool Function Tests

| ID | Test | Expected |
|---|---|---|
| F01 | valid pure function | PASS |
| F02 | invalid input schema | REJECT |
| F03 | invalid output schema | REJECT |
| F04 | duplicate function ID | REJECT |
| F05 | unknown capability | REJECT |
| F06 | side_effect=false | no Effect |
| F07 | side_effect=true | Action path |
| F08 | function missing | ToolFunctionNotFound |
| F09 | function timeout | Timeout |
| F10 | oversized output | ResultTooLarge |

---

# 59. UI Schema Tests

| ID | Test | Expected |
|---|---|---|
| U01 | valid UI tree | PASS |
| U02 | unknown node type | REJECT |
| U03 | duplicate node id | REJECT |
| U04 | excessive depth | REJECT |
| U05 | excessive node count | REJECT |
| U06 | oversized text | REJECT |
| U07 | oversized image | REJECT |
| U08 | cyclic structure | REJECT |
| U09 | invalid event binding | REJECT |
| U10 | unsupported schema version | REJECT |

---

# 60. Session Tests

| ID | Test | Expected |
|---|---|---|
| S01 | create session | Created |
| S02 | open valid tool | Ready |
| S03 | close active session | Closed |
| S04 | event on closed session | REJECT |
| S05 | unknown session | ToolSessionExpired / NotFound |
| S06 | duplicate event | REJECT |
| S07 | stale update | StaleUiGeneration |
| S08 | generation increment | +1 |
| S09 | rejected update | unchanged |
| S10 | cancel active event | cancelled |

---

# 61. IPC Tests

| ID | Test | Expected |
|---|---|---|
| I01 | initialize | PASS |
| I02 | tool.list | PASS |
| I03 | tool.open | PASS |
| I04 | tool.event | PASS |
| I05 | tool.update | PASS |
| I06 | tool.close | PASS |
| I07 | tool.cancel | PASS |
| I08 | malformed JSON | reject, Core survives |
| I09 | unknown method | -32601 |
| I10 | invalid params | -32602 |
| I11 | timeout | -32002 |
| I12 | plugin crash | -32003 |
| I13 | oversized response | -32004 |
| I14 | event flood | bounded |
| I15 | update flood | bounded |
| I16 | late response | ignored |
| I17 | stale generation | rejected |
| I18 | closed-session response | ignored |

---

# 62. Capability Tests

| ID | Test | Expected |
|---|---|---|
| C01 | declared + granted | allowed |
| C02 | declared + denied | denied |
| C03 | undeclared + requested dynamically | denied |
| C04 | forged manifest grant | denied |
| C05 | capability changed while Session active | next protected operation rechecked |
| C06 | capability metadata from Tool | never authoritative |

现有测试已经明确：`unset → denied`、manifest change 不能静默升级 capability decision。

---

# 63. Action Boundary Tests

| ID | Test | Expected |
|---|---|---|
| A01 | pure Tool function | no ActionEngine call |
| A02 | copy result | Action path |
| A03 | save output | Action path |
| A04 | launch process | Action path |
| A05 | Tool attempts direct Effect | impossible/rejected |
| A06 | Plugin ActionDescriptor | Resolver |
| A07 | cached Action | re-resolve |
| A08 | Tool Session token used as auth | reject |

---

# 64. Workflow Tests

| ID | Test | Expected |
|---|---|---|
| W01 | Workflow references pure function | PASS |
| W02 | Workflow references effectful function | PASS via Action path |
| W03 | Workflow stores UI node | REJECT |
| W04 | Workflow stores session id | REJECT |
| W05 | Workflow stores Effect | REJECT |
| W06 | Tool function missing | Command/FunctionNotFound |
| W07 | capability denied at execution | denied |
| W08 | re-resolve after context change | PASS |
| W09 | retry | new execution_id |
| W10 | UI click in workflow | impossible |

---

# 65. RichResult Tests

| ID | Test | Expected |
|---|---|---|
| R01 | Tool Candidate → RichResult | PASS |
| R02 | stable result ID | PASS |
| R03 | duplicate semantic Tool | dedup |
| R04 | open_tool action | ToolSession |
| R05 | effectful result action | ActionResolver |
| R06 | malformed presentation metadata | reject/fallback |
| R07 | RichResult cannot grant capability | PASS |
| R08 | RichResult cannot contain Effect | PASS |

现有 Candidate/Identity 原则要求同一语义对象归并为一个 canonical candidate，metadata/action merge 必须 deterministic。

---

# 66. Fault Injection Matrix

```text
plugin crash
plugin hang
malformed JSON
frame flood
result flood
UI node flood
depth explosion
oversized asset
duplicate event
stale generation
closed session update
invalid capability
invalid tool reference
invalid function reference
plugin restart during session
plugin quarantine during session
DB recovery
```

要求：

```text
Core survives
No authority escalation
No stale UI publication
No process leak
No unbounded memory growth
```

P2.4 已将 plugin hang/crash/flood/malformed/package tampering 列为正式 fault injection。

---

# 67. Performance Tests

至少记录：

```text
Tool metadata load
Tool cold open
Tool warm open
First UI render
First event round-trip
UI update latency
Tool close
Plugin shutdown
Peak Private Bytes
Final Private Bytes
Process Count
```

并继续使用已有 Plugin baseline：

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



---

# 68. Performance Regression Rule

默认：

```text
<= 5%   acceptable
> 5%    warning/review
> 10%   fail
```

除非：

```text
intentional baseline change
ADR exists
benchmark evidence exists
```

这一规则与 P2.4 的既有 regression rule 保持一致。

---

# 69. Acceptance Gates

P3-UI.0：

```text
G01 Domain Contract
G02 Manifest Schema
G03 State Model
G04 IPC Contract
G05 UI Schema Validation
G06 Capability Boundary
G07 Action Boundary
G08 Workflow Reference
G09 RichResult Projection
G10 Contract Test Kit
G11 Fault Injection
G12 Performance
G13 Documentation
G14 Workspace Build
```

任何以下问题：

```text
security failure
authority bypass
contract inconsistency
state corruption
zero-warning failure
```

均为 blocking。

---

# 70. Required Commands

默认 Gate：

```bash
cargo fmt --check
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Contract fixtures：

```bash
cargo test -p launcher-plugin-testkit
cargo test -p launcher-ipc
cargo test -p launcher-domain
```

如项目已有 schema validator：

```bash
<schema-validator> schemas/p3-ui/
```

不得新增不必要的全局工具链。

---

# 71. Agent Completion Report

每个 Agent 必须报告：

```text
Task:
Goal:
Contract touched:
Existing abstraction reused:
Changed files:
Tests:
Security impact:
State impact:
IPC impact:
Performance:
Documentation:
Gate:
```

性能影响必须写：

```text
unchanged
delta measured
no-impact rationale
```

不得使用：

```text
"no obvious impact"
```

替代测量或明确 rationale。

---

# 72. Forbidden Shortcuts

Agent MUST NOT：

```text
rewrite Plugin Contract wholesale
introduce second IPC protocol
introduce WebView merely for convenience
directly expose Slint
use HWND as identity
store ToolSession as authority
store Effect in SQLite
let manifest grant capability
let Workflow call UI event
bypass ActionEngine
put plugin internals into launcher-domain
disable resource limits to pass tests
increase limits without ADR
```

---

# 73. First MVP Acceptance Scenario

官方 Reference Tool：

```text
codec.toolbox
```

包含：

```text
base64
url
json
```

最小 Demo：

```text
Search
 ↓
Base64 RichResult
 ↓
Enter
 ↓
ToolSession
 ↓
TextArea
 ↓
Encode Button
 ↓
ToolFunction(base64.encode)
 ↓
Output
 ↓
Copy
 ↓
ActionResolver
 ↓
ActionEngine
 ↓
Clipboard
```

---

# 74. Final Frozen Architecture

```text
                    Search
                       │
                       ▼
               SearchCandidate
                       │
                       ▼
              ResultProjection
                       │
                       ▼
                  RichResult
                       │
                       ▼
                 ToolReference
                       │
                       ▼
                 ToolSession
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
       UI Schema   ToolFunction   Action
          │            │            │
          ▼            ▼            ▼
       UI Host      Compute       Resolver
                                    │
                                    ▼
                                  Policy
                                    │
                                    ▼
                              ActionEngine
                                    │
                                    ▼
                                  Effect


Workflow
   │
   ▼
ToolReference
   │
   ▼
Fresh Resolve
   │
   ├── Pure Function
   │
   └── Effectful Function
             │
             ▼
        ActionResolver
```

---

# 75. Definition of Done

P3-UI.0 只有在以下全部成立后才能关闭：

```text
[ ] Existing Plugin Contract unchanged
[ ] Existing plugins continue to work
[ ] Tool schema is frozen
[ ] Tool Function schema is frozen
[ ] UI schema is frozen
[ ] Session state machine is frozen
[ ] IPC methods are frozen
[ ] Error codes are frozen
[ ] Resource limits are enforced
[ ] Capability boundary is preserved
[ ] Action boundary is preserved
[ ] Workflow reference is frozen
[ ] RichResult projection is frozen
[ ] SQLite schema migration passes
[ ] Contract Test Kit passes
[ ] Fault injection passes
[ ] Performance baseline passes
[ ] Workspace build zero warnings
[ ] Documentation consistent
[ ] ADR index updated
```

---

# 76. P3-UI.0 Exit Boundary

P3-UI.0 完成意味着：

```text
CONTRACT FOUNDATION = FROZEN
```

不意味着：

```text
TOOL UI = COMPLETE
```

下一阶段才是：

```text
P3-UI.1
RichResult Projection
```

然后：

```text
P3-UI.2
Tool Runtime / Session

P3-UI.3
Native UI Host

P3-UI.4
Capability Integration

P3-UI.5
Tool Action Integration

P3-UI.6
Workflow Tool Functions

P3-UI.7
Reference Codec Plugin

P3-UI.8
E2E / Fault / Performance

P3-UI.9
UX Polish
```

因此 P3-UI.0 的成功标准不是“能打开一个编解码界面”，而是：

> **把未来所有 Interactive Tool 的身份、状态、UI、IPC、能力、安全、Rich Result 和 Workflow 接口一次冻结，使后续 Agent 可以在不修改底层语义的前提下并行开发。**