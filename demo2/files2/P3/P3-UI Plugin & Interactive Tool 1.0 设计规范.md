# P3-UI Plugin & Interactive Tool 1.0

**项目：** Native Launcher / Aura Launcher  
**阶段：** P3-UI  
**版本：** 1.0  
**状态：** FROZEN DESIGN SPEC  
**目标：** 为插件提供统一的 Rich Result、Interactive Tool、Declarative UI、Host Capability、IPC Session 与 Workflow Integration 能力。  
**前置依赖：**

- Plugin Contract v0.1.x
- Plugin Control Plane 2.0
- Action Contract
- Action Engine
- Capability Model
- ContextSnapshot
- Search / Candidate Merge / Result Projection
- Workflow Contract v0.1
- P2.10 Authority / Execution Semantics

---

# 1. Design Position

## 1.1 本阶段解决的问题

Aura Launcher 目前已经具备：

```text
Search
Plugin
Action
Workflow
Capability
Context
Plugin Lifecycle
Plugin Runtime
```

但 Plugin Contract v0.1 中的：

```text
Tool
UI Schema
Preview
```

仍属于 reserved capability，尚未形成完整的产品化 UI 模型。

因此 P3-UI 不继续向：

```text
AI
Agent
Memory
Prediction
Adaptive Intelligence
```

扩展，而是解决：

```text
Plugin
  ↓
Rich Result
  ↓
Interactive Tool
  ↓
Declarative UI
  ↓
Host Capability
  ↓
Workflow Integration
```

---

# 2. Product Goal

P3-UI 1.0 必须允许第三方开发：

```text
Codec
Formatter
Converter
Generator
Inspector
Mini Tool
Utility
```

例如：

```text
Base64
URL Encode / Decode
Hex
JWT Decoder
JSON Formatter
JSON/YAML Converter
Timestamp Converter
UUID Generator
Regex Tester
Hash Calculator
Text Diff
```

同时插件不能因此获得：

```text
直接 UI 权限
直接 Windows API 权限
直接 Effect Authority
直接 Workflow Authority
直接 Capability Grant
```

---

# 3. Core Architecture

冻结后的总体架构：

```text
                         ┌───────────────────────┐
                         │      Launcher UI      │
                         └───────────┬───────────┘
                                     │
                              Search / Session
                                     │
                                     ▼
                         ┌───────────────────────┐
                         │    Search Engine      │
                         └───────────┬───────────┘
                                     │
                              SearchCandidate
                                     │
                                     ▼
                         ┌───────────────────────┐
                         │   ResultProjection    │
                         └───────────┬───────────┘
                                     │
                                     ▼
                              RichResult
                                     │
                     ┌───────────────┼────────────────┐
                     │               │                │
                     ▼               ▼                ▼
                  Command          Action            Tool
                     │               │                │
                     │               │                ▼
                     │               │          Tool Session
                     │               │                │
                     │               │                ▼
                     │               │           Tool UI Model
                     │               │                │
                     ▼               ▼                ▼
                ActionResolver   ActionResolver   UI Host
                     │               │                │
                     ▼               ▼                ▼
                 ActionEngine     Effect          Capability Broker
```

原则：

```text
SearchCandidate ≠ RichResult
RichResult ≠ Action
Tool ≠ Action
Tool UI ≠ Launcher UI
Tool ≠ Effect Authority
```

---

# 4. Four Primary Objects

P3-UI 1.0 正式冻结四种对象：

```text
Plugin
Tool
RichResult
ToolSession
```

关系：

```text
Plugin
 ├── Commands
 ├── Actions
 └── Tools
       │
       └── ToolSession

SearchCandidate
       ↓
ResultProjection
       ↓
RichResult
       ↓
OpenTool
       ↓
ToolSession
```

---

# 5. Plugin Model

## 5.1 Plugin

Plugin 是安装、信任、Capability、生命周期和运行时管理单位。

现有 Plugin 状态必须继续保持分层：

```text
Plugin Identity
Installation Revision
Trust State
Capability Decision
Lifecycle State
Runtime State
```

Runtime State 仍然是 ephemeral；Registry State 是 persistent。

---

# 6. Plugin Manifest 1.0 Extension

## 6.1 Compatibility

现有 Manifest 基础字段继续有效：

```json
{
  "schema_version": 1,
  "id": "com.example.codec",
  "name": "Codec Toolbox",
  "version": "1.0.0",
  "api_version": "0.1",
  "runtime": {
    "type": "process",
    "executable": "codec.exe"
  }
}
```

原 Contract 已要求：

```text
id
name
version
api_version
runtime
```

且 executable 必须位于 package root 内。

P3-UI 只增加 optional fields：

```json
{
  "ui": {
    "enabled": true,
    "theme": "host",
    "entry": "tool"
  },
  "tools": [
    "base64",
    "url"
  ]
}
```

---

# 7. Tool Manifest

## 7.1 Tool Definition

Tool 是 Plugin 内可独立打开和运行的交互式能力。

```json
{
  "id": "base64",
  "name": "Base64",
  "description": "Encode and decode Base64",
  "category": "codec",
  "icon": "assets/base64.svg",

  "entry": {
    "type": "interactive"
  },

  "capabilities": [
    "clipboard.read",
    "clipboard.write"
  ],

  "input": {
    "modes": [
      "text",
      "clipboard"
    ]
  },

  "ui": {
    "schema_version": 1
  },

  "actions": [
    {
      "id": "encode",
      "title": "Encode"
    },
    {
      "id": "decode",
      "title": "Decode"
    }
  ]
}
```

---

# 8. Tool Identity

Tool ID 必须在 Plugin 内稳定。

Canonical ID：

```text
plugin_id + tool_id
```

例如：

```text
com.example.codec/base64
```

禁止使用：

```text
display_name
title
localized_text
UI position
```

作为 identity。

这一原则延续现有 Plugin Contract：UI selection 必须依据稳定 ID，而不是显示文本。

---

# 9. Tool Types

P3-UI 1.0 冻结以下 Tool Types：

```text
interactive
```

Reserved：

```text
background
long_running
embedded
wasm
remote
```

1.0 不实现：

```text
background
long_running
embedded
wasm
remote
```

避免把 Tool Host 同时变成任务调度系统。

---

# 10. Interactive Tool Definition

Interactive Tool 的核心模型：

```rust
struct ToolDefinition {
    id: ToolId,
    name: String,
    description: Option<String>,
    icon: Option<IconRef>,
    category: Option<String>,
    capabilities: Vec<Capability>,
    input: ToolInputSpec,
    ui: ToolUiSpec,
    actions: Vec<ToolActionDescriptor>,
}
```

---

# 11. Tool Session

打开 Tool 后，不直接创建永久 UI。

必须创建：

```text
ToolSession
```

```rust
struct ToolSession {
    session_id,
    plugin_id,
    tool_id,
    created_at,
    context_generation,
    state,
}
```

状态：

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

异常：

```text
Starting → Failed
Active   → Failed
Active   → Crashed
Active   → Timeout
```

---

# 12. Tool Session Principle

Tool Session 不代表：

```text
Capability Grant
Effect Authorization
Workflow Authority
```

它只是：

```text
UI interaction session
```

因此：

```text
ToolSession ≠ Authorization
ToolSession ≠ Effect Token
```

---

# 13. UI Schema

现有 Plugin Contract 已经冻结：

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

并明确禁止 Plugin 直接依赖：

```text
slint
Window
HWND
UI event loop
widget internals
```



P3-UI 1.0 正式启用这一模型。

---

# 14. Declarative UI

Plugin 只能描述：

```text
WHAT
```

不能控制：

```text
HOW
```

即 Plugin 描述：

```text
TextField
Button
Form
List
Detail
Progress
```

而不是：

```text
HWND
position
raw event loop
native widget pointer
Slint object
```

---

# 15. UI Primitive Set

P3-UI 1.0 冻结基础节点：

```text
Container
Section
Text
Markdown
Icon
Image

TextField
PasswordField
TextArea
Dropdown
Checkbox
Radio
Toggle
NumberInput

Button
ActionButton

List
ListItem
Detail
KeyValue

Progress
Status
Badge

Separator
Spacer
```

现有 v0.1 已经预留：

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

P3-UI 1.0 在此基础上扩展，但保持 declarative / renderer-independent。

---

# 16. UI Tree

示例：

```json
{
  "type": "section",
  "id": "main",
  "children": [
    {
      "type": "text_area",
      "id": "input"
    },
    {
      "type": "row",
      "children": [
        {
          "type": "button",
          "id": "encode",
          "title": "Encode"
        },
        {
          "type": "button",
          "id": "decode",
          "title": "Decode"
        }
      ]
    },
    {
      "type": "text_area",
      "id": "output",
      "readonly": true
    }
  ]
}
```

---

# 17. UI Node Rules

Host MUST enforce:

```text
max_ui_nodes
max_ui_depth
max_text_length
max_image_size
max_children
max_form_fields
```

现有 Plugin Contract 已要求 Host 对：

```text
UI node count
UI nesting depth
icon/image size
```

实施上限。

---

# 18. UI State

UI State 必须由：

```text
ToolSession
```

拥有。

插件不得把：

```text
HWND
window object
Slint component
```

作为状态 identity。

状态模型：

```text
Session
 └── UI State
      ├── field values
      ├── selection
      ├── focus
      └── transient status
```

---

# 19. UI Events

UI Event 只描述用户行为：

```text
click
change
submit
select
focus
blur
close
refresh
copy
paste
```

示例：

```json
{
  "method": "ui.event",
  "params": {
    "session_id": "s-123",
    "node_id": "encode",
    "event": "click"
  }
}
```

---

# 20. IPC Architecture

底层继续使用：

```text
NDJSON
JSON-RPC 2.0
stdin/stdout
```

这是 Plugin Contract v0.1 的既有冻结项。

P3-UI 不另造 WebSocket / HTTP / custom socket。

---

# 21. IPC Message Categories

正式分成：

```text
Control
Query
Tool
UI
Capability
Action
Lifecycle
Diagnostic
```

---

# 22. Required RPC Methods

P3-UI 1.0：

```text
initialize
query
tool.list
tool.open
tool.close
tool.event
tool.update
tool.refresh
action.execute
shutdown
```

Optional：

```text
tool.state.save
tool.state.restore
tool.focus
tool.resize
```

---

# 23. initialize

Host：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocol_version": "0.1",
    "ui_schema_version": 1,
    "capabilities": [
      "clipboard.read"
    ]
  }
}
```

Plugin 返回：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "plugin_id": "com.example.codec",
    "api_version": "0.1",
    "ui_schema_version": 1,
    "tools": [
      "base64"
    ]
  }
}
```

---

# 24. tool.list

用于 Host 获取 Plugin Tool Catalog。

```text
tool.list
    ↓
ToolDefinition[]
```

该调用必须：

```text
side-effect free
bounded
deterministic
```

---

# 25. tool.open

```json
{
  "method": "tool.open",
  "params": {
    "session_id": "s-100",
    "tool_id": "base64",
    "context": {},
    "initial_input": {
      "text": "hello"
    }
  }
}
```

返回：

```text
ToolSession + UI Schema + Initial State
```

---

# 26. tool.event

Host：

```json
{
  "method": "tool.event",
  "params": {
    "session_id": "s-100",
    "node_id": "encode",
    "event": "click"
  }
}
```

Plugin 可以返回：

```text
state update
UI update
result update
action proposals
error
```

但不能返回：

```text
already-authorized Effect
capability grant
trusted executable object
```

---

# 27. UI Update

Plugin 通过：

```text
tool.update
```

向 Host 提交增量 UI Model。

推荐：

```text
replace subtree
patch node
update property
set state
```

不允许 Plugin 直接操纵 Host widget tree。

---

# 28. Capability Boundary

现有 capability model 延续：

```text
Requested
    ↓
Granted
    ↓
Enforced
```

Manifest：

```text
requested
```

Registry / Policy：

```text
granted
```

Runtime Broker：

```text
enforced
```



---

# 29. UI Capability

P3-UI 不把：

```text
ui.render
```

作为传统 OS capability。

原因：

Plugin UI 本身是 Host controlled rendering。

因此：

```text
tool.open
tool.update
```

属于 Plugin Runtime Protocol，而不是：

```text
OS Effect Capability
```

---

# 30. Capability Taxonomy

1.0 沿用：

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



新增 UI Tool 场景不扩大默认 Capability。

---

# 31. Capability Grant Rule

绝对禁止：

```text
Tool Manifest
    ↓
capability grant
```

必须：

```text
Manifest request
    ↓
Registry / User Policy
    ↓
Grant
    ↓
Runtime enforcement
```

因此：

```text
Manifest = Request
Tool UI = Presentation
Plugin = Proposal
Host = Authority
ActionEngine = Effect Boundary
```

---

# 32. Context

Tool 打开时可以收到：

```text
ContextSnapshot
```

但必须经过 capability check。

现有 context capability：

```text
context.process.read
context.window.read
context.location.read
context.selection.read
```

并且 Popup Session 的 ContextSnapshot 原则上保持稳定，除非显式请求 refresh。

因此：

```text
ToolSession
   ↓
ContextSnapshot
```

默认：

```text
snapshot = stable
```

---

# 33. Initial Context

例如用户在 Explorer 中选中了：

```text
test.txt
```

启动：

```text
Hash Calculator
```

Host 可以发送：

```json
{
  "selection": {
    "items": [
      {
        "type": "file",
        "path": "C:\\test.txt"
      }
    ]
  }
}
```

但只有：

```text
context.selection.read
```

被允许时才可以提供。

---

# 34. RichResult

RichResult 不是新的 Domain Entity。

定义：

```text
Canonical SearchCandidate
        ↓
ResultProjection
        ↓
RichResult
```

即：

```text
SearchCandidate = semantic object
RichResult = presentation projection
```

---

# 35. RichResult Structure

```rust
struct RichResult {
    result_id,
    kind,
    title,
    subtitle,
    icon,
    badge,
    status,
    description,
    metadata,
    primary_action,
    secondary_actions,
    open_target,
    detail,
    provenance,
}
```

---

# 36. ResultKind

P3-UI 1.0：

```text
Application
File
Folder
Command
Plugin
Tool
Workflow
SystemTarget
```

Reserved：

```text
AIAnswer
Suggestion
Agent
```

暂不实现 Adaptive Intelligence。

---

# 37. Tool Rich Result

例如：

```text
输入：
base64
```

RichResult：

```text
┌──────────────────────────────┐
│ 🔐 Base64                   │
│ Encode / Decode              │
│ Codec Toolbox                │
│                              │
│ [Open Tool]                  │
└──────────────────────────────┘
```

对应：

```json
{
  "kind": "Tool",
  "id": "com.example.codec/base64",
  "title": "Base64",
  "subtitle": "Encode / Decode",
  "plugin_id": "com.example.codec",
  "tool_id": "base64",
  "primary_action": {
    "id": "open_tool"
  }
}
```

---

# 38. RichResult Detail

Detail 采用：

```text
Overview
Actions
Metadata
Capabilities
Plugin
Diagnostics
```

其中 Plugin Detail 可使用：

```text
Overview
Actions
Capabilities
Dependencies
Runtime
Trust
Audit
Diagnostics
```

但 Detail 页面仍然只是 UI Projection。

---

# 39. RichResult Action

RichResult 中出现的：

```text
Open
Run
Install
Enable
Disable
Delete
Copy
Reveal
Run Workflow
```

都必须明确区分：

```text
presentation command
```

和：

```text
effectful action
```

凡是产生 Effect 的 action：

```text
RichResult
  ↓
Action
  ↓
Existing Resolver
  ↓
Policy / Approval
  ↓
ActionEngine
  ↓
Effect
```

不得：

```text
RichResult
  ↓
Plugin
  ↓
Direct Effect
```

这与现有 Plugin Action Contract 完全一致。

---

# 40. Tool Action

Tool 内部操作分两类。

## 40.1 Pure Tool Operation

例如：

```text
Base64 Encode
JSON Format
JWT Decode
Timestamp Convert
```

可以：

```text
UI Event
   ↓
Tool Compute
   ↓
UI Update
```

不产生 Effect。

---

## 40.2 Effectful Action

例如：

```text
Copy to Clipboard
Save File
Open Terminal
Launch Process
Send Notification
```

必须：

```text
Tool
 ↓
ActionDescriptor
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

# 41. Why Tool ≠ Action

这是本设计最重要的一个边界。

例如：

```text
Base64 Encode
```

本身只是计算：

```text
String → String
```

如果强制把它建模成 Action：

```text
Base64 Encode
 ↓
Action Engine
 ↓
Effect
```

会让所有纯计算工具都被错误地纳入 Effect 系统。

正确模型：

```text
Tool Function
        ↓
      Result
```

只有：

```text
Copy
Save
Open
Launch
Send
```

才进入 Action / Effect 系统。

---

# 42. Tool Execution Model

Tool 内部计算：

```text
UI Event
  ↓
Tool Runtime
  ↓
Pure Compute
  ↓
State Update
  ↓
Render
```

Effect：

```text
UI Event
  ↓
Tool Runtime
  ↓
Action Proposal
  ↓
Host
  ↓
Resolver
  ↓
Policy
  ↓
ActionEngine
  ↓
Effect
```

---

# 43. Workflow Integration

Tool 能力必须能够被 Workflow 调用。

但 Workflow 不直接操作：

```text
ToolSession
UI State
Widget ID
```

Workflow 只引用：

```text
Logical Tool Function / Action
```

---

# 44. Workflow Tool Adapter

定义：

```text
Tool Function
       ↓
Workflow Action Reference
```

例如：

```text
com.example.codec/base64.encode
```

Workflow：

```json
{
  "step_id": "decode",
  "action": {
    "reference": {
      "provider_id": "com.example.codec",
      "command_id": "base64",
      "action_id": "decode"
    }
  },
  "input": {
    "text": "${clipboard.text}"
  }
}
```

这里继续遵守 Workflow Contract：

```text
Definition
≠
ResolvedAction
≠
Effect
```

并且每次执行前重新 Resolve。

---

# 45. Tool Function Contract

为了让 Tool 能被 Workflow 调用，Tool 必须可声明：

```text
Function
├── id
├── input_schema
├── output_schema
├── deterministic
├── side_effect
├── required_capabilities
```

例如：

```json
{
  "id": "base64.decode",
  "input_schema": {
    "type": "object",
    "required": ["text"]
  },
  "output_schema": {
    "type": "object",
    "properties": {
      "text": {
        "type": "string"
      }
    }
  },
  "deterministic": true,
  "side_effect": false,
  "required_capabilities": []
}
```

---

# 46. Pure Tool Function

纯函数：

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

可以直接作为 Workflow Step。

---

# 47. Effectful Tool Function

例如：

```text
clipboard.write
filesystem.write
process.launch
```

必须：

```text
side_effect = true
```

并声明：

```text
required_capabilities
```

例如：

```json
{
  "id": "clipboard.write",
  "side_effect": true,
  "required_capabilities": [
    "clipboard.write"
  ]
}
```

最终仍由 ActionEngine 管理 Effect Authority。

---

# 48. Workflow UI vs Tool UI

二者职责不同。

## Workflow UI

负责：

```text
Definition
Step List
Inputs
Conditions
Variables
Run
Pause
Resume
Failure
History
```

P2.6 已明确 Workflow 是 orchestration；其 Definition 持久化的是逻辑 Action reference/proposal，而非 executable object。

## Tool UI

负责：

```text
Input
Transform
Result
Interactive State
Tool-specific controls
```

二者不应共享一个“万能 UI Runtime”。

---

# 49. Tool Can Launch Workflow

允许：

```text
Tool
 ↓
Action
 ↓
RunWorkflow
```

但 Tool 不能创建：

```text
Effect
```

只能提出：

```text
RunWorkflow Action
```

然后：

```text
ActionResolver
 ↓
Workflow Service
```

---

# 50. Workflow Can Invoke Tool

允许：

```text
Workflow
 ↓
Tool Function
```

但不允许：

```text
Workflow
 ↓
Tool UI
 ↓
simulate click
```

Workflow 应调用：

```text
logical function
```

而不是：

```text
UI node
```

---

# 51. Interactive Tool and Workflow Separation

禁止这种设计：

```text
Workflow step:
click("button-123")
```

允许：

```text
Workflow step:
base64.decode(input)
```

这是为了确保 Tool UI 变化不会破坏 Workflow。

---

# 52. Input / Output Schema

Tool Function 应提供 JSON Schema 风格：

```text
Input
Output
```

例如：

```text
base64.decode

Input:
{
    text: string
}

Output:
{
    text: string,
    valid: boolean
}
```

这样 Workflow 可以进行：

```text
Tool A
  ↓
Output
  ↓
Tool B Input
```

---

# 53. Tool Composition

P3-UI 1.0 支持：

```text
Tool A
 ↓
Tool Function
 ↓
Tool B
```

例如：

```text
Clipboard
 ↓
URL Decode
 ↓
JSON Parse
 ↓
JSON Format
 ↓
Clipboard Write
```

但每一个节点都必须遵守：

```text
Resolver
Policy
Capability
ActionEngine
```

相应边界。

---

# 54. Clipboard Example

完整流程：

```text
User selects text
      ↓
Launcher Context
      ↓
Open Base64 Tool
      ↓
clipboard.read
      ↓
Tool Input
      ↓
Base64 Decode
      ↓
Output
      ↓
[Copy]
      ↓
ActionDescriptor
      ↓
clipboard.write
      ↓
ActionEngine
```

其中：

```text
Base64 Decode
```

不是 Effect。

而：

```text
Clipboard Write
```

是 Effect。

---

# 55. Plugin UI Host

Host 必须负责：

```text
Window
Focus
Keyboard
Theme
DPI
Layout
Rendering
Input dispatch
Lifecycle
Error fallback
```

Plugin 只负责：

```text
Tool semantics
UI Schema
UI State
Function execution
Action proposals
```

---

# 56. Host Window Model

默认：

```text
One Launcher Window
One active Tool Session
```

P3-UI 1.0 不实现：

```text
multiple arbitrary plugin windows
popup HWND ownership
plugin-controlled native windows
```

Tool UI 应嵌入：

```text
Launcher Host Surface
```

---

# 57. Tool Navigation

推荐：

```text
Launcher
   ↓
Search
   ↓
Tool Result
   ↓
Enter
   ↓
Tool View
   ↓
Esc
   ↓
Launcher
```

Tool 不能绕开 Launcher Host 建立第二套全局窗口体系。

---

# 58. Keyboard Contract

Host 保留：

```text
Esc
Enter
Arrow Keys
Tab
Shift+Tab
Ctrl+C
Ctrl+V
```

Tool 可以申请：

```text
tool-local shortcut
```

但不能劫持全局 Launcher shortcut。

---

# 59. Focus Contract

Host 必须维护：

```text
focused_node_id
```

Plugin 可以：

```text
request_focus(node_id)
```

但不能：

```text
直接访问 native focus handle
```

---

# 60. Theme Contract

Tool 默认：

```text
theme = host
```

Host 提供：

```text
light
dark
system
```

Tool 不得强制：

```text
global theme change
```

允许：

```text
semantic appearance hint
```

例如：

```text
danger
warning
success
muted
```

最终颜色由 Host Theme Resolver 决定。

---

# 61. Plugin State Persistence

Tool State 分成：

```text
Session State
User Preference
Persistent Data
```

---

## 61.1 Session State

例如：

```text
输入框内容
当前 tab
当前 selection
```

生命周期：

```text
ToolSession
```

---

## 61.2 User Preference

例如：

```text
默认 Base64 模式
默认换行设置
```

通过：

```text
store.read
store.write
```

受 Capability 控制。

---

## 61.3 Persistent Tool Data

例如：

```text
历史
词典
数据库
```

必须明确：

```text
storage namespace
```

禁止直接写任意 Host path。

---

# 62. Storage Namespace

推荐：

```text
plugin:<plugin_id>
tool:<tool_id>
```

例如：

```text
plugin:com.example.codec
tool:base64
```

Host 负责隔离：

```text
plugin A ≠ plugin B
```

---

# 63. Security Invariants

P3-UI 必须增加以下 invariant。

```text
INV-UI01
Plugin cannot access native UI internals.

INV-UI02
Tool UI cannot grant capability.

INV-UI03
Tool UI cannot create Effect authority.

INV-UI04
Workflow cannot execute UI events.

INV-UI05
Workflow references logical Tool Functions, not UI nodes.

INV-UI06
RichResult is presentation-only and cannot become authority.

INV-UI07
Tool Action must re-enter existing Resolver / Policy / ActionEngine path.

INV-UI08
Tool Session is not an authorization token.

INV-UI09
Plugin cannot escape its package/runtime boundary.

INV-UI10
UI schema cannot create unbounded resource consumption.

INV-UI11
Plugin crash cannot terminate Launcher Core.

INV-UI12
Plugin timeout cannot block UI thread.

INV-UI13
Undeclared capability is denied.

INV-UI14
Manifest cannot self-promote trust.

INV-UI15
Diagnostic path cannot execute Effect.
```

这些原则继承 P2.4 已冻结的：

```text
Plugin cannot bypass capability enforcement
Plugin cannot manipulate Launcher UI internals
Plugin result size is bounded
Plugin timeout is host-enforced
Plugin cannot directly execute privileged/unapproved effects
```



---

# 64. Trust

Tool 与 Plugin 共用 Plugin Trust。

不单独建立：

```text
ToolTrusted
ToolUntrusted
```

Trust hierarchy：

```text
Plugin Trust
    ↓
Plugin lifecycle
    ↓
Tool availability
```

---

# 65. Quarantine

Plugin 被：

```text
Quarantined
```

时：

```text
Tool discovery = disabled
Tool launch = denied
Tool execution = denied
```

但历史记录和 diagnostics 必须保留。

现有 Plugin Control Plane 已冻结 Quarantine 为 persistent state，并阻止 discovery / execution。

---

# 66. Error Model

继续使用：

```text
-32001 CapabilityDenied
-32002 Timeout
-32003 PluginCrashed
-32004 ResultTooLarge
-32005 RateLimited
-32006 PluginUnavailable
-32007 VersionMismatch
```



P3-UI 增加：

```text
-32008 ToolNotFound
-32009 ToolSessionExpired
-32010 InvalidUiSchema
-32011 UiLimitExceeded
-32012 InvalidToolInput
-32013 ToolFunctionNotFound
-32014 UnsupportedUiSchema
```

---

# 67. UI Error Handling

Tool error 必须结构化：

```json
{
  "code": "InvalidToolInput",
  "message": "Invalid Base64 input",
  "field": "input",
  "retryable": false
}
```

Host 负责：

```text
inline error
status message
error page
```

而不是要求 Plugin 自己创建原生 MessageBox。

---

# 68. Timeout Model

必须区别：

```text
Query Timeout
Tool Event Timeout
Action Timeout
Idle Timeout
```

不能统一成：

```text
plugin timeout
```

例如：

```text
Query = 2s
UI Event = 5s
Action = existing Action policy
Idle = 10s
```

最终以 Host Policy 为准。

---

# 69. Long Operation

P3-UI 1.0 原则上：

```text
Tool Function = bounded
```

不提供完整 LongRunningTask 系统。

如果以后需要：

```text
LongRunningTool
```

应作为独立 Extension 设计。

---

# 70. Cancellation

Host 必须支持：

```text
tool.cancel
```

当用户：

```text
Esc
close
switch result
```

时，可以取消当前 Tool Event。

插件收到：

```text
cancel
```

之后必须尽快结束计算。

---

# 71. Late Result

继续使用现有 Query Supersession 原则。

Host MUST ignore：

```text
stale session result
stale event response
closed session response
```

现有 Plugin Contract 已要求旧 Query 结果不能覆盖新 Query。

P3-UI 扩展同样要求：

```text
session_id
event_id
generation
```

进行关联。

---

# 72. Generation

Tool Session 需要自己的：

```text
ui_generation
```

规则：

```text
accepted update → +1
rejected update → unchanged
closed session → no further generation
```

用于防止：

```text
late update
out-of-order patch
stale UI state
```

---

# 73. Resource Limits

Host MUST enforce：

```text
max_message_bytes
max_ui_nodes
max_ui_depth
max_text_length
max_image_bytes
max_image_dimensions
max_update_rate
max_event_rate
max_output_bytes
max_tool_sessions
```

原 Plugin Contract 已冻结：

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

必须继续执行。

---

# 74. Process Isolation

继续使用现有：

```text
External Process
      +
Job Object
```

Windows 启动建议保持：

```text
CreateProcess(CREATE_SUSPENDED)
        ↓
AssignProcessToJobObject
        ↓
Apply Limits
        ↓
ResumeThread
```

Plugin 终止必须清理整个 process tree。

---

# 75. Package Layout

推荐：

```text
codec-toolbox/
├── plugin.json
├── codec.exe
├── assets/
│   ├── icon.svg
│   └── base64.svg
├── tools/
│   ├── base64.json
│   ├── url.json
│   └── jwt.json
└── schemas/
    ├── base64-input.json
    └── base64-output.json
```

---

# 76. Plugin SDK API

SDK 层不暴露 Host internals。

开发者看到：

```rust
Plugin
Tool
ToolContext
ToolSession
UiNode
UiEvent
ToolFunction
ActionProposal
Capability
```

而不是：

```rust
PluginHost
ActionEngine
Slint
HWND
RuntimeBroker
```

现有 SDK Boundary 已明确：插件代码不得依赖 `launcher-plugin-host`、`launcher-core`、`launcher-action` 和内部 runtime types。

---

# 77. Rust SDK Example

目标体验：

```rust
tool("base64")
    .ui(build_ui)
    .on_event(handle_event)
    .function("encode", encode)
    .function("decode", decode)
```

开发者不需要处理：

```text
stdin
stdout
JSON-RPC id
framing
session routing
```

这符合现有 SDK 设计：SDK 应隐藏协议 mechanics，开发者应直接实现 query/command，而非手动处理 JSON-RPC。

---

# 78. Example: Base64 Tool

## UI

```text
Base64
────────────────────────

Input
┌──────────────────────────┐
│ hello world              │
└──────────────────────────┘

[ Encode ] [ Decode ]

Output
┌──────────────────────────┐
│ aGVsbG8gd29ybGQ=         │
└──────────────────────────┘

[ Copy ]
```

## Architecture

```text
Search
 ↓
RichResult(Base64)
 ↓
ToolSession
 ↓
UI
 ↓
ToolFunction(base64.encode)
 ↓
Output
 ↓
Action(copy)
 ↓
Resolver
 ↓
ActionEngine
 ↓
Clipboard Effect
```

---

# 79. Example: JSON Formatter

```text
Search:
json format

     ↓

RichResult:
JSON Formatter

     ↓

ToolSession

     ↓

Input
{
    "a":1
}

     ↓

ToolFunction:
json.format

     ↓

Output
{
    "a": 1
}
```

完全可以：

```text
side_effect = false
capabilities = []
```

所以这个 Tool 不需要系统权限。

---

# 80. Example: JWT Decoder

JWT Decoder 可以：

```text
Input
 ↓
Split
 ↓
Base64URL Decode
 ↓
JSON Parse
 ↓
UI Detail
```

如果只读取 token：

```text
capability = []
```

若要求：

```text
fetch JWKS
```

才需要：

```text
network.connect
```

并通过 Capability Broker。

---

# 81. Search Integration

Plugin Tool 必须可被 Search Provider 发现。

推荐：

```text
query:
base64
```

返回：

```text
Tool
 ├── title
 ├── subtitle
 ├── plugin_id
 ├── tool_id
 ├── keywords
 ├── icon
 ├── ranking metadata
 └── primary action
```

---

# 82. Provider Boundary

Search Provider 负责：

```text
Discovery
Metadata
Candidate
Ranking input
```

不负责：

```text
Tool execution
UI rendering
Capability Grant
Effect
```

---

# 83. Result Projection

冻结：

```text
SearchCandidate
        ↓
ResultProjection
        ↓
RichResult
```

不要：

```text
SearchCandidate
 ↓
RichResult
 ↓
modify domain object
```

RichResult 是 presentation model。

---

# 84. Plugin Result Projection

Plugin Candidate：

```text
Plugin
 ↓
Plugin Metadata
 ↓
Tool Metadata
 ↓
SearchCandidate
 ↓
RichResult
```

因此：

```text
Plugin Core State
```

不应直接暴露给 UI。

---

# 85. Detail Loading

RichResult 默认只包含：

```text
summary
```

Detail 可以：

```text
lazy load
```

例如：

```text
Search
 ↓
small result
 ↓
Enter
 ↓
get detail
```

避免搜索阶段加载：

```text
entire Tool UI schema
plugin runtime
large assets
```

---

# 86. Runtime Loading Policy

默认：

```text
Search
    → no plugin process required unless current provider architecture requires it

Open Tool
    → spawn on demand

Tool closed
    → idle timeout

Idle timeout
    → terminate
```

现有 Plugin Contract 已要求：

```text
External Process = on-demand
plugin-free idle = plugin_process_count 0
```



---

# 87. Tool Catalog Caching

Host 可以缓存：

```text
ToolDefinition
UI metadata
icons
```

但必须定义：

```text
key
max entries
max bytes
TTL / invalidation
owning generation
```

不得出现：

```text
unbounded cache
```

这延续 P2.4 的明确约束。

---

# 88. Diagnostics

每次 Tool failure 至少记录：

```text
plugin_id
tool_id
session_id
runtime_id
protocol_session_id
operation
elapsed
result_size
classification
```

现有 Plugin Diagnostics 已要求：

```text
classification
plugin id
query_id
runtime_id
protocol_session_id
elapsed
bounded payload
```



---

# 89. Audit

以下必须进入现有 audit：

```text
tool.open
tool.close
capability.request
capability.denied
action.proposed
action.executed
tool.failure
plugin.crash
plugin.quarantine
```

但：

```text
普通 UI Event
```

默认不进入长期 audit，避免噪声。

---

# 90. Secret Handling

禁止自动把：

```text
password
credentials
tokens
cookies
private clipboard data
```

写入：

```text
persistent storage
diagnostic logs
telemetry
audit
```

Plugin Contract 已明确：

```text
Secrets are not inherited or persisted implicitly.
```



---

# 91. Prompt / Untrusted Content Boundary

虽然本阶段暂停 AI，但未来 Tool 可能处理：

```text
web content
file content
clipboard content
user input
plugin metadata
```

统一视为：

```text
Untrusted Data
```

不得因内容中出现：

```text
"run this"
"ignore policy"
"grant permission"
```

而改变 Host authority。

---

# 92. Deep Link

P3-UI 1.0 暂不冻结完整 Deep Link。

但保留：

```text
aura://tool/<plugin>/<tool>
```

作为未来兼容预留。

当前实现不得依赖 Deep Link。

---

# 93. Versioning

继续区分：

```text
schema_version
api_version
protocol_version
plugin_version
ui_schema_version
```

现有 Plugin Contract 已明确这几类版本不能混淆。

新增：

```text
ui_schema_version = 1
```

兼容规则：

```text
unknown optional field → ignore
unsupported required UI feature → reject/open fallback
incompatible major UI schema → Tool unavailable
```

---

# 94. Backward Compatibility

旧 Plugin：

```text
PLUGIN-CONTRACT v0.1
```

必须继续工作：

```text
query
command
action
context
```

如果没有：

```text
tools
ui
```

则：

```text
legacy plugin
```

仍作为传统 Command Plugin 使用。

---

# 95. UI Compatibility

Plugin 可以声明：

```json
{
  "ui": {
    "min_schema_version": 1,
    "max_schema_version": 1
  }
}
```

Host：

```text
compatible → open
incompatible → fallback / reject
```

不能让旧 Host：

```text
 silently misrender
```

---

# 96. Contract Extension Policy

以下变更需要 ADR：

```text
Tool identity
UI schema semantics
IPC method semantics
Capability semantics
Action boundary
Workflow integration semantics
Session lifecycle
resource limits
```

现有项目已经要求 Plugin Contract、Command/Action、Process Boundary、Capability、Package Format 等变化必须有 ADR。

---

# 97. Testing Strategy

测试层级：

```text
Unit
 ↓
Schema Validation
 ↓
IPC Contract
 ↓
UI Contract
 ↓
Tool Runtime
 ↓
Capability
 ↓
Action Integration
 ↓
Workflow Integration
 ↓
Fault Injection
 ↓
Performance
 ↓
Soak
 ↓
Release Gate
```

---

# 98. Mandatory Contract Tests

必须覆盖：

```text
initialize
tool.list
tool.open
tool.event
tool.update
tool.close

invalid tool
invalid ui schema
oversized ui
oversized result
timeout
crash
cancel
stale event
closed session event
capability denied
capability allowed
action proposal
workflow invocation
```

---

# 99. Security Tests

至少：

```text
undeclared capability → denied
manifest forged capability → denied
tool UI attempts native access → rejected
malicious UI depth → rejected
UI flood → bounded
tool result flood → bounded
plugin crash → Core survives
plugin timeout → process cleanup
closed session update → ignored
workflow UI click attempt → rejected
ToolAction bypass Resolver → impossible
```

---

# 100. Integration Test

完整 E2E：

```text
Install Plugin
 ↓
Validate Manifest
 ↓
Register Plugin
 ↓
Search Tool
 ↓
RichResult
 ↓
Open Tool
 ↓
Create Session
 ↓
Render UI
 ↓
Input
 ↓
Tool Function
 ↓
Result
 ↓
Copy
 ↓
ActionResolver
 ↓
Capability Check
 ↓
ActionEngine
 ↓
Clipboard Effect
 ↓
Close Session
 ↓
Idle Timeout
 ↓
Process Exit
```

---

# 101. Workflow E2E

测试：

```text
Clipboard
 ↓
Base64 Decode
 ↓
JSON Format
 ↓
Clipboard Write
```

验证：

```text
Tool Function reference is logical
WorkflowDefinition does not contain executable Effect
each step re-resolves before execution
capability is checked at execution
```

这与 Workflow Contract 中：

```text
Every ActionInvocation is re-resolved immediately before execution
```

保持一致。

---

# 102. Fault Injection

必须覆盖：

```text
plugin crash
plugin hang
malformed JSON
oversized UI schema
oversized result
event flood
update flood
invalid tool ID
invalid node ID
duplicate session ID
stale session
late result
capability denial
process tree leak
orphan process
```

---

# 103. Performance Baseline

记录：

```text
Search → RichResult
RichResult → Tool Open
Tool cold start
Tool warm event
UI render
UI update
Tool close
Plugin shutdown
Private Bytes
Process count
```

继续遵守现有 Plugin performance baseline：

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

# 104. UI Performance Rules

明确禁止：

```text
blocking UI thread
large synchronous IPC
unbounded UI update loop
busy polling
```

要求：

```text
event-driven
bounded
cancellable
host controlled
```

---

# 105. MVP Scope

P3-UI 1.0 第一实现只需要：

```text
Plugin Manifest Extension
Tool Manifest
Tool Session
UI Schema
UI Host
Tool IPC
Capability enforcement
RichResult
Tool Action
Workflow Tool Function
```

---

# 106. Explicitly Deferred

以下不属于 P3-UI 1.0：

```text
Python SDK
Node SDK
WASM runtime
WebView plugin
Marketplace UI
Cloud Plugin Sync
Signature / Trust Chain 2.0
Background Task Framework
Long Running Task Framework
Multi-window Plugin
Plugin-owned Native HWND
AI Tool Planning
Agent Tool Planning
Tool Memory
Tool Prediction
Adaptive Tool Recommendation
```

其中 Python / Node / WASM 本身已经属于此前 Plugin Contract 的 deferred runtime；本规范不重新开启。

---

# 107. Implementation Order

```text
P3-UI.0 Contract Foundation
    ↓
P3-UI.1 RichResult Projection
    ↓
P3-UI.2 Tool Runtime / Session
    ↓
P3-UI.3 UI Schema / Native Host
    ↓
P3-UI.4 Capability Broker Integration
    ↓
P3-UI.5 Tool Action Integration
    ↓
P3-UI.6 Workflow Tool Functions
    ↓
P3-UI.7 Example Tool
    ↓
P3-UI.8 E2E / Fault / Performance
    ↓
P3-UI.9 UX Polish
```

---

# 108. Recommended First Official Plugin

第一官方参考插件：

```text
codec-toolbox
```

包含：

```text
Base64
URL Encode / Decode
Hex
JWT Decode
Timestamp
UUID
JSON Format
```

原因：

```text
纯计算能力多
Effect 能力少
UI 丰富度适中
Workflow 可复用
Capability 场景清晰
```

它非常适合验证整个 P3-UI Contract。

---

# 109. Reference Architecture

最终冻结架构：

```text
                         ┌─────────────────────────────┐
                         │        Search Engine        │
                         └──────────────┬──────────────┘
                                        │
                                 SearchCandidate
                                        │
                                        ▼
                             ┌────────────────────┐
                             │  ResultProjection  │
                             └─────────┬──────────┘
                                       │
                                   RichResult
                                       │
                    ┌──────────────────┼───────────────────┐
                    │                  │                   │
                    ▼                  ▼                   ▼
                 Command            Action                Tool
                                                           │
                                                           ▼
                                                     ToolSession
                                                           │
                                                           ▼
                                                     Tool Runtime
                                                           │
                                      ┌────────────────────┼────────────────┐
                                      │                    │                │
                                      ▼                    ▼                ▼
                                   UI Model          Pure Function      Action Proposal
                                      │                    │                │
                                      ▼                    ▼                ▼
                                  UI Host              Result          ActionResolver
                                      │                                     │
                                      ▼                                     ▼
                                    Slint                                Policy
                                                                            │
                                                                            ▼
                                                                       ActionEngine
                                                                            │
                                                                            ▼
                                                                          Effect


Workflow
   │
   ▼
Logical Tool Function
   │
   ▼
ActionReference
   │
   ▼
Fresh Resolve
   │
   ▼
Policy / Capability
   │
   ▼
ActionEngine / Tool Runtime
```

---

# 110. Frozen Invariants Summary

最终只记住以下边界：

```text
1. Plugin is not UI.
2. Tool is not Action.
3. RichResult is not Domain Authority.
4. ToolSession is not Authorization.
5. UI Schema is not Native UI.
6. UI Event is not Effect.
7. Tool Function is not UI Event.
8. Workflow references Functions, not Widgets.
9. Effect always re-enters ActionResolver / Policy / ActionEngine.
10. Manifest declares capability; it never grants capability.
11. Trust never directly grants Effect authority.
12. Plugin crash/timeout cannot affect Core.
13. All UI/resource/input/result paths are bounded.
14. RichResult is a projection, not a persistent execution object.
15. P3-UI does not depend on AI / Agent / Adaptive Intelligence.
```

---

# 111. Definition of Done

P3-UI Plugin & Interactive Tool 1.0 只有在以下条件全部成立后才能宣布冻结实现：

```text
[ ] Existing v0.1 plugins still work
[ ] Plugin Manifest extension validates deterministically
[ ] Tool Manifest validates deterministically
[ ] Tool Session lifecycle works
[ ] Declarative UI renders through Host
[ ] Plugin cannot access native UI internals
[ ] Capability enforcement works at runtime
[ ] Pure Tool Function works
[ ] Effectful Tool Action re-enters ActionEngine
[ ] RichResult works for Tool
[ ] Plugin Detail works
[ ] Workflow can invoke Tool Function
[ ] Workflow cannot invoke Widget Event
[ ] stale session update is rejected/ignored
[ ] plugin crash does not crash Core
[ ] plugin timeout terminates process tree
[ ] resource limits are enforced
[ ] diagnostics are complete
[ ] audit is complete
[ ] contract tests pass
[ ] integration tests pass
[ ] fault injection passes
[ ] performance baseline passes
[ ] workspace build has zero warnings
[ ] documentation and ADR index are consistent
```

---

# 112. Final Product Boundary

P3-UI 1.0 完成后，Aura Launcher 的插件能力正式从：

```text
Plugin
 └── Command
      └── Action
```

升级为：

```text
Plugin
│
├── Command
│    └── Action
│
├── Tool
│    ├── UI
│    ├── Function
│    └── Action
│
└── Metadata
```

而整个系统形成：

```text
                 Search
                   │
                   ▼
              Rich Result
                   │
        ┌──────────┴──────────┐
        ▼                     ▼
      Action                Tool
        │                     │
        ▼                     ▼
   ActionEngine          Tool Runtime
        │                     │
        ▼              ┌──────┴──────┐
      Effect           ▼             ▼
                    Pure           Action
                   Function          │
                       │             ▼
                       │        ActionEngine
                       │             │
                       └──────┬──────┘
                              ▼
                             Effect

Workflow
   │
   └── references both logical Actions and logical Tool Functions
```

这套模型足以覆盖：

```text
uTools 编解码插件
JSON 工具
JWT 工具
文本工具
开发者工具
文件工具
转换器
小型生产力工具
```

同时不需要把 Aura Launcher 变成：

```text
Electron Plugin Runtime
Web Browser
Widget OS
第二套 Action Engine
第二套 Permission System
```

**P3-UI 的核心不是“给插件做一个 UI”，而是正式冻结 `Tool = 可交互能力` 这个一级产品抽象。**