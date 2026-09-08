# PLUGIN-DESIGN-REVIEW.md

## Native Launcher Plugin Architecture Review & Contract Freeze Proposal

**Version:** 0.1-draft  
**Date:** 2026-09-04  
**Status:** Architecture Review / proposed freeze  
**Scope:** Wox / Flow Launcher / Raycast / uTools / Lertaro / Asyar + current Native Launcher MVP2.1

---

## 0. Executive Decision

本文件不是“插件系统产品需求”，而是插件架构评审与 Contract 冻结前的设计依据。

### Proposed architecture

Native Launcher 插件体系采用：

```text
                           Plugin Contract
                                  │
                ┌─────────────────┼──────────────────┐
                │                 │                  │
             Manifest          Protocol          Capability
                │                 │                  │
                └─────────────────┼──────────────────┘
                                  │
                           Plugin Broker
                                  │
             ┌────────────────────┼────────────────────┐
             │                    │                    │
       Built-in Provider    External Process       Future WASM
       (trusted/native)        Plugin Host          Runtime
                                  │
                         ┌────────┼────────┐
                         ▼        ▼        ▼
                       Rust     Python     Node
                                  │
                                  ▼
                         Command / Preview
                                  │
                                  ▼
                              Action Engine
                                  │
                                  ▼
                                 Effect
```

### Frozen principles

1. **Plugin Contract 与语言 SDK 解耦。** Rust / Python / Node / WASM 都是 Contract 的实现，不是 Contract 本身。
2. **插件不直接控制 Launcher UI。** 插件产生 Command、Action、Preview/Panel Schema；Host 决定 Native UI 呈现。
3. **插件不直接执行 Host Effect。** 所有受控外部效果必须通过 Action Engine / Capability Broker。
4. **第三方插件默认 out-of-process。** 当前 MVP 的 stdio JSON-RPC 可以继续作为 v0.1 transport。
5. **Runtime 按需启动。** Python/Node 等 runtime 不是 Core 的常驻依赖。
6. **Capability 必须从“声明”走到“执行时 enforcement”。** Manifest 中声明不等于授权。
7. **协议采用 forward-compatible 版本化。** Schema、API、Capability、UI Schema 均独立版本管理。
8. **插件失败必须是局部失败。** Crash、hang、malformed response、result flood 都不得破坏 Core。
9. **Plugin API 首先面向 Query → Command → Action。** 复杂 UI、后台任务、Preview、Context 等能力作为可选 capability 扩展。
10. **MVP 不同时实现 Python/Node/WASM。** 先冻结 contract，再以 Rust reference plugin + contract tests 验证；Python 作为跨语言 compatibility test，WASM 作为后续 sandbox runtime。

---

# 1. Review Method

## 1.1 比较维度

统一从以下十个维度比较六个项目：

| 维度 | 重点问题 |
|---|---|
| Manifest | 如何发现、描述、启停、版本化插件 |
| RPC / IPC | 插件如何与 Host 通信 |
| Command | 插件如何贡献可搜索/可执行命令 |
| Action | 结果如何产生后续动作 |
| Context | 插件如何感知当前应用/文件/选择 |
| UI | 插件能否提供列表、详情、表单、面板、窗口 |
| Permission | 插件能获得哪些系统能力，如何约束 |
| Lifecycle | load/query/run/idle/update/shutdown 模型 |
| SDK | 插件开发者实际需要编写多少代码 |
| Error handling | crash、timeout、malformed data、network error 如何处理 |

## 1.2 证据层级

本文把结论区分为：

- **Source fact**：官方仓库/官方开发文档明确描述的行为。
- **Architecture inference**：基于公开结构做出的架构判断。
- **Our decision**：Native Launcher 的设计决策。

不要把第三类重新表述为某个参考项目“已经这样做”。

---

# 2. Current Native Launcher Baseline

当前 MVP2.1 已经具备外部插件边界：

```text
launcher-app
      │
      ▼
launcher-core
      │
      ▼
Plugin Provider / Plugin Broker
      │
      ▼
launcher-plugin-host
      │
      ▼
External Plugin Process
      │
      ▼
stdio JSON-RPC
```

当前插件 Manifest 已包含 `id / name / api_version / executable / capabilities / timeout_ms / idle_timeout_ms`；Host 对 Manifest 做校验，并限制 timeout 与结果数量；生命周期采用发现 → 校验 → 按需启动 → idle → kill，异常插件不得影响 Core。见当前架构文档。 

MVP2.1 的 `Context → Provider → Command → Action` 已真实闭环，这使插件 Contract 现在可以直接建立在已经验证的 Domain 模型上，而不是重新创造一套 Plugin-specific object model。

当前主要红线：

- 不在 Core 内嵌 Python/Node runtime。
- 不允许插件直接访问 UI internals。
- 不允许无界缓存。
- 不允许 UI thread 被 IO/CPU 阻塞。
- 进程边界、公开协议、插件模型、DB schema、UI 技术变更必须有 ADR。 

---

# 3. Cross-Project Comparison

## 3.1 Wox

### Source facts

当前 Wox 已明确把插件能力拆出为独立的 Node.js / Python host，并在仓库中存在 `wox.plugin.host.nodejs`、`wox.plugin.host.python`、`wox.plugin.nodejs`、`wox.plugin.python`；官方定位是 native、plugin-driven，支持 Node.js、Python 与 script plugins。 

Wox v2 的公开讨论还明确提出：V2 希望不同语言插件成为 first-class citizen，通过不同 Plugin Host 让 Node/Python/C#/Java 等语言获得统一体验；同时核心插件可随 Wox 分发，以避免基础体验依赖用户手工准备 runtime。 

### Architectural lesson

```text
Core
 ├── Node Host
 ├── Python Host
 └── future language hosts
```

最大的价值不是“支持 Python”，而是：

> **Runtime Adapter 与 Plugin Contract 分离。**

### Strengths

- 跨语言思路明确。
- Runtime Host 是独立边界。
- Built-in 与 external plugin 的层级清晰。
- 插件生态能够覆盖脚本语言与原生语言。

### Weaknesses / risks

- 每增加一种语言，就增加一个 runtime/host 维护面。
- 依赖管理与版本管理复杂。
- 如果 runtime 被长期保持运行，Launcher 常驻内存模型会迅速恶化。
- “所有语言都 first-class”在 API 层很好，但在安装、更新、debug、依赖隔离上代价很高。

### Our decision

**吸收：** Plugin Host / Runtime Adapter 分离、跨语言 Contract。  
**不吸收：** “语言越多越好”作为早期目标。  
**改进：** runtime 默认按需启动，Contract first，SDK second。

---

## 3.2 Flow Launcher

### Source facts

Flow 使用 JSON-RPC 作为本地过程调用协议，把 Flow 与 Python、JavaScript/TypeScript 等其他语言绑定起来，并建立一套 Host ↔ Plugin common API。公开 API 包括改变查询、显示/隐藏应用、消息、设置、获取插件等 Host 操作。 

Flow 另有专门的 Python JSON-RPC SDK/包，把 Python 插件接入统一 RPC；插件仓库的 manifest 又独立承载 ID、Name、Description、Author、Version、Language、Website、下载 URL、源码 URL、图标等发行信息。 

### Architectural lesson

Flow 最值得借鉴的不是某一个 API，而是：

```text
Plugin language
       │
       ▼
Language SDK
       │
       ▼
JSON-RPC Contract
       │
       ▼
Launcher API
```

### Strengths

- JSON-RPC 非常适合跨语言。
- SDK 降低开发门槛。
- Plugin manifest 与 store metadata 有明确关系。
- API 可以逐步扩展。

### Weaknesses / risks

- 如果 Host API 暴露太多“控制 Launcher 本身”的操作，插件容易从“提供能力”变成“操纵宿主”。
- JSON-RPC 本身不提供 sandbox。
- RPC API 如果不做 capability/version discipline，最终容易形成“万能 Host API”。

### Our decision

**吸收：** JSON-RPC、Language SDK、统一 common API、manifest/store metadata 分离。  
**不吸收：** 让插件直接调用 `ShowApp/HideApp/ShellRun/...` 等宿主控制 API 作为默认模型。  
**改进：** Host 能力全部按 Capability 暴露，插件只请求所需能力。

---

## 3.3 Raycast

### Source facts

Raycast 的 manifest 基于 npm `package.json` 的扩展格式，声明 extension metadata、platforms、commands、tools、AI 信息、preferences 等；command 有 name/title/description/icon/mode/keywords/arguments/preferences 等字段。`mode` 明确区分 `view`、`no-view`、`menu-bar`，并支持后台 interval。 

Raycast UI 模型把 List、List.Item、Detail、ActionPanel 等作为核心抽象。List Item 可以携带 `id`、`keywords`、`detail`、`actions` 等，而 ActionPanel 可以根据当前 selected item 提供 context-aware actions。 

Raycast 的 Security 文档描述其 extension runtime：Host 启动单独 Node child process，Extension 在自己的 V8 isolate / worker thread 中运行，使用定义的 RPC 暴露 API；公开安全说明同时明确指出它们在文件 IO、网络等 Node runtime 能力上并没有进一步 sandbox。 

### Architectural lesson

Raycast 最值得吸收的是：

```text
Command
  ├── presentation
  ├── arguments
  ├── actions
  ├── preferences
  ├── lifecycle mode
  └── optional background execution
```

以及：

```text
Selected Item
      ↓
Context-aware ActionPanel
```

这与我们的 `Command → Action` 模型高度契合。

### Strengths

- Command 是一等公民。
- Action 是 context-aware 的。
- UI abstraction 丰富但统一。
- `view / no-view / menu-bar` 直接描述运行模式。
- Preferences、Arguments、Tools 等元数据都进入 manifest。
- 官方工程对 error handling、background refresh、extension publishing 有成熟规范。

### Weaknesses / risks

- Node runtime 依然是成本中心。
- Extension 能力与 Host API 的强结合使跨 Host 迁移困难。
- 权限模型没有达到 Asyar 这种 capability-enforced sandbox。

### Our decision

**吸收：** Command / Action / Arguments / Preferences / no-view / context-aware actions。  
**强烈吸收：** “Result 是 entity，Action 是 entity 上的操作集合”。  
**不吸收：** Node runtime 作为默认 extension substrate。  
**改进：** Native UI Schema 替代 React/Web runtime；Capability enforcement 替代隐式 Node access。

---

## 3.4 uTools

### Source facts

uTools 用 `plugin.json` 定义插件入口、logo、preload 与 features；feature 可以声明 `code`、`explain`、`icon`、`cmds`、`mainPush`、`mainHide` 等，commands 既可以是普通 command，也可以是 regex match。 

uTools 的 preload 可以调用 Node.js 原生能力和 Electron renderer API；官方文档明确说明 preload 是为了让插件突破传统 Web 沙箱，访问本地文件、跨域网络、本地存储等能力。 

uTools 还提供模板插件：可以省去自定义界面而直接使用模板，以少量 API 获得较轻量的开发体验；开发流程包含创建、开发接入、调试、离线打包、市场发布。 

### Architectural lesson

uTools 非常强的是：

```text
plugin.json
   ↓
features
   ├── command
   ├── regex trigger
   ├── main push
   └── main hide
```

它把“插件是什么、插件能被什么 query 触发”描述得很直接。

### Strengths

- 极低开发门槛。
- Manifest 直观。
- feature/trigger 模型优秀。
- 模板插件降低学习成本。
- Developer Tool / package / publishing path 完整。

### Weaknesses / risks

- Electron + preload + Node 是高能力、低约束模型。
- 插件 UI 走 Web/HTML，与我们的 Native-only UI 红线冲突。
- preload 可以获得非常大的系统能力范围，因此权限边界不够细。

### Our decision

**吸收：** manifest + feature + trigger/alias + developer tooling + template。  
**不吸收：** HTML UI、Electron、preload unrestricted native access。  
**改进：** Feature → Command；native UI schema；Capability Broker。

---

## 3.5 Lertaro

### Source facts

Lertaro 当前官方 Developer Guide 把自身描述为 decoupled multi-process architecture + extensible plugin ecosystem，并提供 `Lertaro.PluginSdk`。其 SDK 扩展范围包括 custom search sources、context action menus、third-party file manager/native file dialogs、themes、preview handlers。 

公开 SDK 分类包括 `ISearchableItemProvider`、`IInstantResultProvider`、`IAliasProvider`、`IQueryTokenProvider`、`ISearchResultAction`、`IDynamicActionProvider`，以及 Active Path、File Dialog、Inline Search、Quick Navigation、Sidebar Filter、Result Column、Quick Panel Tab 等扩展点。 

Lertaro 同时把 SYSTEM-level Windows Service 与 per-user WPF app 分开，说明它把“重索引”和“交互 UI”作为不同生命周期与权限域。 

### Architectural lesson

Lertaro 最大的贡献不是 RPC，而是：

> **Plugin Surface 的颗粒度。**

它没有把插件统一定义成“一个搜索回调”，而是把：

```text
Search
Alias
Action
Context
Dialog
Inline UI
Preview
Theme
```

拆成多个 extension point。

### Strengths

- Windows 深度集成能力强。
- Provider / Action / Context / Preview 等职责拆分清楚。
- SDK 以具体扩展点描述业务能力。
- 与文件管理器和原生文件 dialog 集成非常适合 Launcher。

### Weaknesses / risks

- API 面非常大，长期兼容成本高。
- 若把所有扩展点都设计成长期稳定 public interface，SDK 会快速膨胀。
- C# assembly/plugin model 与跨语言模型不同。

### Our decision

**吸收：** capability-based extension surfaces、Context/Action/Preview/Provider 分离。  
**不吸收：** 一开始就发布几十个细粒度接口。  
**改进：** 用少量稳定 primitives + schema/capabilities 组合出更多能力。

---

## 3.6 Asyar

### Source facts

Asyar 当前采用 Rust backend + Svelte UI；extensions 使用 TypeScript/任意 web framework，并在 iframe 中 sandbox。官方仓库明确描述 installed extensions 为 isolated sandbox，扩展可以贡献 commands、live search results、rich UI panels。 

其 manifest 能定义 extension-level 与 command-level actions，支持 deep links、background scheduling、live metadata update 等。 

Asyar 的 permission model 是本次比较中最值得吸收的部分：manifest 声明权限，运行时在 frontend IPC router 与 Rust permission registry 两层检查；能力包含 clipboard、filesystem、network、shell、notifications、store、tools、run tracking 等。扩展间隔离，通信走 typed `postMessage` bridge，畸形消息会被拒绝。 

同时 Asyar 的 runtime 采取 on-demand download / verify：bun、uv、claude 等 runtime 在真正需要时才下载，并做 sha256 校验。这验证了“Runtime 是资源，不是常驻依赖”的思想。 

### Architectural lesson

Asyar 给我们的核心启发是：

```text
Manifest capability declaration
             ↓
      frontend gate
             ↓
       IPC boundary
             ↓
       Rust gate
             ↓
          Effect
```

以及：

```text
Runtime dependency
        ↓
     on-demand
```

### Strengths

- Permission/capability 设计成熟。
- Extension crash isolation 与 sandbox 思路明确。
- Command、Action、Tool、Schedule、Deep Link 都统一到 manifest。
- 开发工具链完整。
- Runtime on-demand，非常适合低资源常驻软件。

### Weaknesses / risks

- Iframe/Web UI 与 Native-only UI 的目标冲突。
- 多层 permission / typed bridge 本身有实现复杂度。
- Web sandbox 解决的是特定威胁模型，不能直接代替 Windows process isolation。

### Our decision

**吸收：** capability model、double enforcement、runtime on-demand、typed bridge、malformed message rejection。  
**不吸收：** iframe/Web extension UI。  
**改进：** Windows 下以 process isolation 为主安全边界，WASM 作为未来更轻量 sandbox；Native UI schema 作为 UI contract。

---

# 4. Consolidated Lessons

## 4.1 最值得继承的六件事

```text
Wox      → Runtime Host / language neutrality
Flow     → JSON-RPC / SDK / common API
Raycast  → Command + Action + UI abstraction + lifecycle
uTools   → Manifest + features + trigger + developer tooling
Lertaro  → Context / Preview / Action / Provider extension surfaces
Asyar    → Capability + sandbox + on-demand runtime
```

## 4.2 最需要避免的六个陷阱

```text
1. Runtime 常驻
2. Plugin 直接控制 Host UI
3. Plugin 直接获得无限系统 API
4. Query() 成为万能函数
5. 每增加一个特性就增加一个永久 public interface
6. SDK API 与具体语言 runtime 强耦合
```

---

# 5. Proposed Native Launcher Plugin Model

## 5.1 Plugin 不是一个类，而是一组声明 + 一个执行端

```text
Plugin Package
├── manifest.json
├── icon
├── runtime
├── executable / entrypoint
├── assets
└── optional signature
```

逻辑上：

```text
Plugin
 ├── Metadata
 ├── Commands
 ├── Capabilities
 ├── Preferences
 ├── Runtime Policy
 └── Entry Endpoint
```

---

# 6. Manifest Contract v0.1

建议把当前 MVP manifest 扩展成以下 forward-compatible 结构：

```json
{
  "schema_version": 1,
  "id": "com.example.github",
  "name": "GitHub",
  "version": "1.0.0",
  "api_version": "0.1",
  "description": "GitHub launcher commands",
  "author": {
    "name": "Example",
    "website": "https://example.com"
  },
  "icon": "assets/icon.png",
  "runtime": {
    "type": "process",
    "executable": "plugin.exe",
    "args": [],
    "working_directory": "plugin"
  },
  "capabilities": [
    "network",
    "store.read"
  ],
  "commands": [
    {
      "id": "issues",
      "title": "Issues",
      "description": "Search GitHub issues",
      "keywords": ["gh", "issue"],
      "mode": "search",
      "enabled": true
    }
  ],
  "preferences": [],
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

### Mandatory invariants

- `schema_version` 控制 Manifest 格式。
- `api_version` 控制 Host ↔ Plugin API compatibility。
- `version` 控制插件自身版本。
- `id` 全局唯一，推荐 reverse-DNS 风格。
- executable 必须解析到 plugin package 目录内部。
- 禁止 path traversal、绝对路径逃逸、不可控 working directory。
- 未声明 capability 的 API 调用必须失败。
- limits 必须有 host-side upper bound，插件不能通过 Manifest 请求无限值。

---

# 7. Command Contract

Command 是插件最主要的贡献对象。

```text
Command
├── id
├── title
├── subtitle
├── description
├── icon
├── keywords[]
├── category
├── score_hint?
├── mode
├── context_requirements[]
├── arguments[]
├── preview?
└── actions[]
```

### 设计原则

Command 表示：

> “用户可以找到并选择的一项能力/实体。”

而不是：

> “插件 UI 中的一个按钮。”

这继承 Raycast 的 command-first 思路，同时与现有 Native Launcher `Command` domain model 对齐。

---

# 8. Search Contract

最低协议：

```text
query(request)
   ↓
Command[]
```

Request：

```json
{
  "query_id": "8c2d...",
  "text": "term",
  "context": {
    "app": "explorer.exe",
    "folder": "C:\\Projects\\Demo"
  },
  "limit": 50
}
```

Result：

```json
{
  "query_id": "8c2d...",
  "commands": []
}
```

### Query semantics

- `query_id` 必须由 Host 生成并回传。
- Plugin 不得假设 Query 一定按顺序到达。
- Host 可以 cancel/supersede 旧 Query。
- Host 可以丢弃过时结果。
- Plugin 返回结果不得超过 Host 强制上限。

这是对当前 MVP2.1 时序问题的正式强化：UI/Action 不再依赖另一份 `last_results` 真相，而通过稳定 command/action identity 执行。

---

# 9. Action Contract

Action 不应该允许插件任意执行：

```text
Plugin → ShellExecute(...)
```

而应该：

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

例如：

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

### Important rule

Plugin 可以**提出 Action**；Host 决定：

- Action 是否合法
- 是否需要 capability
- 是否允许当前 Context
- 是否需要确认
- 是否执行

这为未来 AI / Workflow / MCP 提供同一个 Effect surface。

---

# 10. Context Contract

插件不应直接自己调用 Win32 读取所有前台状态。

Host 根据 capability 提供稳定 ContextSnapshot：

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

### Context access levels

```text
context.process.read
context.window.read
context.location.read
context.selection.read
```

插件只获得声明的部分。

这继承 Lertaro 的 ActivePath / file-manager integration 思路，同时吸收 Asyar 的 permission model。

---

# 11. UI Contract

## 11.1 Native-only principle

插件禁止：

```text
HTML → WebView → Browser DOM
```

作为 Launcher 主 UI 的默认扩展机制。

插件只返回声明式 UI Schema：

```text
Plugin
  ↓
UI Schema
  ↓
Host Validator
  ↓
Native UI Model
  ↓
Slint Renderer
```

## 11.2 UI primitives v0.1

先冻结最小集合：

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

其中 MVP 阶段只实现：

```text
List
ListItem
Text
Separator
```

其余属于 Contract reservation。

## 11.3 Plugin 不知道 Slint

插件不得依赖：

```text
slint::ComponentHandle
Window
Widget
EventLoop
```

这保证未来即使 Native Launcher 更换 UI toolkit，Plugin Contract 不需要重写。

---

# 12. Preview Contract

Preview 应成为 Command 的可选能力，而不是 Plugin 自定义 Window。

```json
{
  "type": "preview",
  "kind": "text",
  "title": "README.md",
  "content": "..."
}
```

未来允许：

```text
text
markdown
image
metadata
file
html-rendered-preview   ← future isolated subsystem, not launcher WebView contract
```

尤其注意：**Web content preview 可以是未来独立安全 subsystem，但不能反向把 WebView 引入 Launcher 主 UI。**

---

# 13. Permission / Capability Contract

## 13.1 三阶段模型

```text
Requested
    ↓
Granted
    ↓
Enforced
```

Manifest 只能声明 Requested。

用户/Host Policy 决定 Granted。

Runtime Broker 决定 Enforced。

## 13.2 v0.1 Capability taxonomy

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

### High-risk capabilities

```text
filesystem.write
process.launch
shell.execute
network.connect
```

这些不应因为安装插件自动获得。

## 13.3 Double enforcement

参考 Asyar：

```text
API call
   ↓
Broker gate
   ↓
OS/native implementation
```

任何“frontend declaration only”都不视为安全边界。

在我们的 process-based model 中，真正安全边界是 Plugin Host + OS process boundary；capability broker 再提供细粒度控制。

---

# 14. Runtime Model

## 14.1 Runtime tiers

```text
Tier 0  Built-in Native Provider
Tier 1  External Native Process
Tier 2  Script Host (Python / Node)
Tier 3  WASM Sandbox
Tier 4  Remote / Service backed plugin (future)
```

## 14.2 Default lifecycle

```text
Discovered
   ↓
Validated
   ↓
Ready
   ↓
Spawn on first demand
   ↓
Running
   ↓
Idle
   ↓
Terminate
```

### Default policy

```text
Core / builtin       permanent
External plugin     on-demand
Python runtime      on-demand
Node runtime        on-demand
WASM                on-demand / pooled
AI runtime          on-demand
```

这直接继承 Wox 的 host 分层、Asyar 的 runtime-on-demand，并服务于本项目的低内存目标。

---

# 15. IPC / RPC Contract

## 15.1 v0.1 Transport

暂定继续使用：

```text
stdin/stdout
newline-delimited JSON-RPC
```

原因：

- 已在 MVP2 中落地。
- 跨语言简单。
- Windows Named Pipe 尚未证明必要。
- 不应为了理论上的架构完美重新引入复杂 transport。

## 15.2 Methods

### Required

```text
initialize
query
shutdown
```

### Reserved

```text
execute
get_context
get_preview
get_preferences
health
ping
```

### Host → Plugin

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "method": "query",
  "params": {
    "query_id": "...",
    "text": "term",
    "context": {}
  }
}
```

### Plugin → Host response

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "result": {
    "commands": []
  }
}
```

### Error

标准 JSON-RPC error envelope：

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "error": {
    "code": -32001,
    "message": "Capability denied",
    "data": {
      "capability": "network.connect"
    }
  }
}
```

---

# 16. Error Model

插件错误必须分层：

```text
Protocol error
Schema error
Plugin error
Capability error
Timeout
Crash
Host error
Dependency error
User-facing error
```

## 16.1 Protocol errors

```text
-32700 Parse error
-32600 Invalid request
-32601 Method not found
-32602 Invalid params
```

## 16.2 Host plugin errors

建议使用 `-32000 .. -32099` 私有区间：

```text
-32001 CapabilityDenied
-32002 Timeout
-32003 PluginCrashed
-32004 ResultTooLarge
-32005 RateLimited
-32006 PluginUnavailable
-32007 VersionMismatch
```

## 16.3 User experience

插件网络错误不应该默认让整个 Launcher 进入 error page。

应优先：

```text
cache
 → stale result
 → toast/message
 → graceful empty state
```

该原则与 Raycast 官方 best-practice 对网络失败/缓存 fallback 的思路一致。 

---

# 17. Result Flood / Resource Limits

Host 必须强制限制：

```text
max_results
max_result_bytes
max_string_length
max_icon_size
max_action_count
max_ui_depth
max_request_size
max_stderr_bytes
```

尤其：

```text
100 results
```

只是 MVP 默认值，不是永久 Contract 上限；协议必须允许 Host policy 在未来调整。

---

# 18. Process Isolation

## 18.1 Windows execution boundary

建议：

```text
CreateProcess
    ↓
CREATE_SUSPENDED
    ↓
AssignProcessToJobObject
    ↓
Set limits / handles
    ↓
ResumeThread
```

这是当前 Known Issue 中已经识别的 Job Object race 的正式解决路径。

## 18.2 Job Object

至少负责：

- process-tree termination
- orphan cleanup
- optional memory limit
- optional CPU time limit
- process accounting

## 18.3 Environment isolation

默认只传递最小必要环境：

```text
PATH        explicit policy
TEMP        plugin-scoped temp if needed
HOME/profile not implicit unless required
Proxy       explicit policy
Secrets     never inherited silently
```

---

# 19. Preferences / Secrets

参考 Raycast：preferences 应进入 manifest，而不是每个插件自行设计一套 config UI。Raycast 当前支持 text/password/checkbox/dropdown/appPicker/file/directory 等 preference types。 

Native Launcher v0.1 建议：

```text
Preference
├── id
├── title
├── description
├── type
├── required
├── default
└── secret
```

Secret value：

```text
Plugin filesystem
    ✗
plain config.toml
    ✗

Host secure store
    ✓ future
```

插件不应该自己决定敏感值存在哪里。

---

# 20. Context-Aware Actions

这是 Native Launcher 与普通 Wox/Flow plugin model 拉开差异的重要能力。

例如：

```text
Explorer
  ↓
C:\Projects\Demo
  ↓
Plugin result: Git Repository
  ↓
Actions:
  ├── Open repository
  ├── Open terminal here
  ├── Copy remote URL
  └── Git status
```

插件不需要重新查询 Explorer。

Host 提供：

```text
ContextSnapshot
```

Plugin 生成：

```text
Command + Actions
```

这直接复用了 MVP2.1 已验证的 Context → Command → Action 链。

---

# 21. Background / Long-running Tasks

参考 Raycast `no-view` / background refresh 和 Asyar run tracking，但不在 MVP 实现全部能力。

未来定义：

```text
Command mode
├── search
├── view
├── no-view
├── background
└── streaming
```

Long-running task：

```text
Plugin
  ↓
RunHandle
  ↓
Host Run Registry
  ├── started
  ├── progress
  ├── completed
  ├── failed
  └── cancelled
```

这样插件不会自己创建独立 Progress Window。

---

# 22. Settings and Dynamic Command Registration

uTools 的 feature declaration、Raycast 的 preferences/commands、Asyar 的 manifest-driven commands 都说明：

> 用户可见的扩展能力必须尽可能由 manifest 静态声明。

动态命令仍允许，但应满足：

```text
Static metadata
    ↓
Dynamic results
```

而不是：

```text
Plugin starts
    ↓
mutates arbitrary host registry
```

这样可以：

- 在插件未启动时显示命令。
- 不启动 runtime 也能做搜索 routing。
- 支持 disabled-by-default。
- 支持 command-level preferences。

---

# 23. SDK Architecture

SDK 不应该只有一个 `launcher-plugin-api` crate。

建议拆成：

```text
plugin-contract
plugin-protocol
plugin-sdk-core
plugin-sdk-rust
plugin-sdk-python    future
plugin-sdk-node      future
plugin-sdk-wasm      future
plugin-dev-cli
plugin-test-kit
```

## 23.1 Contract layer

定义：

```text
Command
Action
Context
Manifest
Capability
Error
UI Schema
```

## 23.2 Protocol layer

定义：

```text
JSON-RPC framing
serialization
request/response
handshake
version negotiation
```

## 23.3 SDK layer

提供：

```rust
Plugin::initialize()
Plugin::query()
Plugin::shutdown()
```

同时隐藏：

```text
stdin
stdout
JSON serialization
request IDs
error envelopes
```

---

# 24. Plugin Developer Experience

参考 Flow / uTools / Asyar 的 developer tooling，最终目标：

```text
launcher plugin init github
launcher plugin validate
launcher plugin dev
launcher plugin test
launcher plugin package
launcher plugin install
```

项目结构：

```text
my-plugin/
├── plugin.json
├── README.md
├── src/
├── tests/
├── assets/
└── dist/
```

Rust template：

```text
src/
└── main.rs
```

Example：

```rust
fn query(ctx: QueryContext) -> Vec<Command> {
    ...
}
```

开发者不应该直接处理 JSON-RPC。

---

# 25. Contract Test Kit

每种 SDK 必须通过统一 Contract Test。

```text
contract-tests/
├── initialize
├── query-empty
├── query-normal
├── query-cancel
├── malformed-response
├── oversized-response
├── timeout
├── crash
├── action-validation
├── capability-denied
├── capability-allowed
├── idle-shutdown
├── process-tree-cleanup
└── version-negotiation
```

因此：

```text
Rust SDK ─┐
Python SDK ├── same protocol compliance suite
Node SDK ──┤
WASM SDK ──┘
```

这比单独测试每个 SDK 更可靠。

---

# 26. Compatibility / Versioning

必须同时存在：

```text
Manifest schema version
API version
Protocol version
Plugin version
```

例如：

```text
schema_version = 1
api_version    = 0.1
protocol       = 1
plugin         = 2.3.4
```

### Compatibility rules

```text
Major API mismatch → reject
Minor compatible extension → accept
Unknown optional field → ignore
Unknown required capability → deny / negotiate
Unknown result field → ignore
Unknown method → -32601
```

禁止直接依赖：

```text
launcher build version == plugin target version
```

而应该使用明确 API compatibility。

---

# 27. Security Threat Model

## Trusted

```text
Builtin Rust provider
```

## Semi-trusted

```text
Signed first-party plugin
```

## Untrusted

```text
Community external plugin
```

默认假设：

> 第三方插件代码可能是恶意的、buggy 的、被供应链污染的，或者只是资源失控。

因此：

```text
Crash isolation
Timeout
Result limits
Job Object
Capability enforcement
Package path validation
Version validation
Optional signature
```

都是基础设施，而不是高级安全功能。

---

# 28. Plugin UI Security

UI Schema 必须：

- 有最大递归深度。
- 有最大节点数。
- 有最大文本长度。
- 有最大图片尺寸。
- 禁止任意 native handle。
- 禁止 plugin 直接取得 `HWND`。
- 禁止 plugin 注入 Slint event loop。

以后扩展 Web preview 时必须另设 sandbox boundary，不能通过 UI Schema 直接获得 WebView。

---

# 29. Performance Contract

插件体系必须服从 Native Launcher 的性能契约。

### Idle

```text
plugin process count = 0
```

### Cold

```text
spawn
→ handshake
→ query
→ first result
```

### Warm

```text
running
→ query
→ first result
```

### Shutdown

```text
idle timeout
→ process tree terminated
→ no orphan process
```

### Memory

插件必须分别记录：

```text
plugin_peak_private_bytes
plugin_final_private_bytes
plugin_process_count
plugin_spawn_count
plugin_kill_count
orphan_process_count
```

这直接对齐项目 PERFORMANCE-METHODOLOGY v2。

---

# 30. Search / Ranking Integration

Plugin 不应该拥有最终 ranking 权。

插件提供：

```text
textual relevance
keywords
category
optional score_hint
```

Core 负责：

```text
lexical
exact match
type prior
frequency
recency
context
provider prior
```

建议最终模型：

```text
score =
    lexical_score
  + exact_match_bonus
  + type_prior
  + frequency_score
  + recency_score
  + context_score
  + provider_prior
  + bounded_plugin_hint
```

插件不得通过巨大 score 直接劫持全局 ranking。

---

# 31. Trigger / Alias Model

吸收 uTools 的 feature command 与 Flow 的 action keywords，但统一为：

```text
Command
├── keywords[]
├── aliases[]
├── trigger?
└── argument schema
```

Trigger 类型 v0.1：

```text
prefix
exact
keyword
```

未来：

```text
regex
url
file-type
context
```

Regex trigger 必须：

- 有 timeout。
- 有长度上限。
- 防 ReDoS。
- 最好在 Host controlled engine 中执行。

---

# 32. What We Explicitly Do NOT Copy

## From Wox

不把“支持很多 runtime”作为早期成功指标。

## From Flow

不让插件通过越来越多 Host control APIs 反向控制 Launcher。

## From Raycast

不把 Node.js runtime 当作插件默认执行环境。

## From uTools

不采用 HTML/plugin window + preload unrestricted native access。

## From Lertaro

不在第一版发布大量细粒度 public interfaces。

## From Asyar

不采用 iframe/WebView 作为 Native Launcher 主 UI plugin surface。

---

# 33. What We Explicitly Copy

```text
Wox
  Plugin Host abstraction

Flow
  JSON-RPC
  language SDK
  common API

Raycast
  Command-first model
  ActionPanel concept
  preferences
  arguments
  no-view/background lifecycle

uTools
  manifest simplicity
  feature/trigger concept
  developer scaffolding
  offline package concept

Lertaro
  Context / Search / Action / Preview extension surfaces
  deep Windows integration

Asyar
  capability declarations
  capability enforcement
  runtime on-demand
  typed bridge discipline
```

---

# 34. Final Plugin Contract v0.1

## Mandatory primitives

```text
Manifest
Command
Query
Action
ContextSnapshot
Capability
Error
Lifecycle
```

## Optional primitives

```text
Preview
Preferences
Arguments
LongRunningTask
BackgroundSchedule
Tool
DeepLink
UI Schema
```

## Transport

```text
NDJSON JSON-RPC over stdio
```

## Security boundary

```text
External process + Job Object
+
Capability Broker
```

## UI boundary

```text
Plugin → Declarative Schema → Native UI
```

## Runtime boundary

```text
Default: on-demand
```

## Action boundary

```text
Plugin proposes Action
→ Host validates
→ Action Engine executes Effect
```

---

# 35. Proposed Package Layout

```text
crates/
├── launcher-domain
├── launcher-ipc
├── launcher-plugin-api
├── launcher-plugin-host
├── launcher-plugin-sdk
└── launcher-plugin-testkit

apps/
├── launcher-app
├── example-echo-plugin
└── example-testplugins

future/
├── plugin-sdk-python
├── plugin-sdk-node
└── plugin-runtime-wasm

docs/
├── PLUGIN-DESIGN-REVIEW.md
├── PLUGIN-CONTRACT.md
├── PLUGIN-DEVELOPMENT-GUIDE.md
└── adr/
    ├── ADR-0001-plugin-rpc.md
    ├── ADR-0005-plugin-job-object.md
    ├── ADR-0006-plugin-capability-model.md
    ├── ADR-0007-plugin-ui-schema.md
    └── ADR-0008-plugin-versioning.md
```

---

# 36. Recommended Implementation Order

## Phase P0 — Contract freeze

```text
PLUGIN-001  Manifest schema
PLUGIN-002  Command / Action schema
PLUGIN-003  Query request / response
PLUGIN-004  Error model
PLUGIN-005  Lifecycle model
PLUGIN-006  Capability taxonomy
PLUGIN-007  Version negotiation
```

## Phase P1 — Harden existing MVP

```text
PLUGIN-008  query_id + supersession
PLUGIN-009  Job Object hardening
PLUGIN-010  process-tree cleanup
PLUGIN-011  result/resource limits
PLUGIN-012  Contract Test Kit
```

## Phase P2 — SDK

```text
PLUGIN-013  Rust SDK
PLUGIN-014  Rust reference plugin
PLUGIN-015  plugin CLI
PLUGIN-016  developer template
```

## Phase P3 — Cross-language validation

```text
PLUGIN-017  Python SDK
PLUGIN-018  Python reference plugin
PLUGIN-019  Node SDK
PLUGIN-020  Node reference plugin
```

## Phase P4 — Advanced runtime

```text
PLUGIN-021  UI Schema
PLUGIN-022  Preview
PLUGIN-023  Long-running tasks
PLUGIN-024  WASM runtime
PLUGIN-025  capability hardening
```

Marketplace、签名分发、自动更新、AI/MCP plugin integration 不应阻塞 Contract v0.1。

---

# 37. Architecture Acceptance Criteria

Plugin Contract v0.1 只有满足以下条件才能宣布冻结：

### Correctness

- Rust reference plugin passes all contract tests。
- malformed JSON never crashes Core。
- plugin crash does not terminate Core。
- timeout reliably terminates plugin process tree。
- result flood is bounded。
- old query cannot overwrite newer query results。

### Security

- undeclared capability is denied。
- path traversal manifest is rejected。
- executable outside package root is rejected。
- process tree cannot escape Job Object cleanup。

### Performance

- plugin-free idle process count remains zero。
- cold/warm plugin latency is separately measurable。
- plugin process memory is separately measurable。
- repeated plugin cycles show no staircase leak。

### Compatibility

- unknown optional fields are ignored。
- API mismatch produces deterministic error。
- protocol version negotiation is deterministic。

### Developer experience

A developer who understands Rust/Python should be able to implement a minimal query plugin without knowing the internal launcher architecture, Slint, Core Provider Registry, or Action Engine implementation details.

---

# 38. Final Architectural Position

本项目最终不应该变成：

```text
Wox + Python
+ Flow + JSONRPC
+ Raycast + React
+ uTools + Electron
+ Lertaro + Interfaces
+ Asyar + iframe
```

而应该形成一个更小的共同核心：

```text
                    Plugin
                       │
                 Manifest/Contract
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
      Query          Context       Capability
        │              │              │
        └──────────────┼──────────────┘
                       ▼
                    Command
                       │
                  Preview/UI
                       │
                    Action
                       │
                  Action Engine
                       │
                     Effect
```

**Plugin 是能力提供者，不是第二个 Launcher。**

这是本设计最重要的边界。

---

# 39. Source Register

以下为本次评审使用的一手资料入口；具体断言应以对应项目当前版本文档为准。

### Wox
- https://github.com/Wox-launcher/Wox
- https://github.com/Wox-launcher/Wox/releases
- https://github.com/Wox-launcher/Wox/discussions/3937

### Flow Launcher
- https://github.com/Flow-Launcher/docs/blob/main/json-rpc.md
- https://github.com/Flow-Launcher/Flow.Launcher.JsonRPC.Python
- https://github.com/Flow-Launcher/Flow.Launcher.PluginsManifest

### Raycast
- https://developers.raycast.com/information/manifest
- https://developers.raycast.com/api-reference/user-interface/list
- https://developers.raycast.com/api-reference/user-interface/actions
- https://developers.raycast.com/api-reference/user-interface/action-panel
- https://developers.raycast.com/information/security
- https://developers.raycast.com/information/lifecycle/background-refresh
- https://developers.raycast.com/information/best-practices

### uTools
- https://www.u-tools.cn/docs/developer/information/plugin-json.html
- https://www.u-tools.cn/docs/developer/information/preload.html
- https://www.u-tools.cn/docs/developer/information/window-exports.html
- https://www.u-tools.cn/docs/developer/docs.html

### Lertaro
- https://github.com/Lertaro/Lertaro
- https://lertaro.github.io/dev-guide/

### Asyar
- https://github.com/Xoshbin/asyar

---

# 40. Review Outcome

**Recommendation: APPROVE WITH CONDITIONS**

允许进入 Plugin Contract implementation，但在正式宣布 API stable 前，必须先完成：

```text
Manifest
Command
Action
Query ID / Cancellation
Capability
Error
Versioning
Job Object
Contract Tests
Rust Reference Plugin
```

不要求现在实现：

```text
Python
Node
WASM
Marketplace
AI/MCP
Background scheduling
Rich UI
```

这些属于后续 Contract consumer/runtime，而不是 Contract 本身。
