# Native Launcher v0.1 设计规范

状态：Draft / Architecture Baseline
版本：0.1.0
面向：Agentic Coding
平台：Windows-first

---

## 1. 项目定位

Native Launcher 是一个 **keyboard-first、本地优先、原生渲染、按需运行时** 的桌面工作台。

它的核心抽象不是“搜索结果”，而是：

```text
Trigger -> Query -> Candidate -> Command -> Action -> Effect
```

统一把应用、文件、书签、剪贴板、系统命令、插件能力、Workflow、AI Tool 视为可被发现和执行的 Command。

### 1.1 产品原则

1. 常驻轻量优先于功能堆叠。
2. 搜索响应优先于复杂动画。
3. Native UI 优先于 Web UI。
4. Runtime 按需加载，而不是常驻。
5. 插件隔离优先于插件便利性。
6. 核心能力内置；扩展能力插件化。
7. 可观测、可测试、可回归优先。
8. Agent 可以自主编码，但不能绕过架构和质量门禁。

---

## 2. 技术基线

### 2.1 MVP

| 层 | 技术 |
|---|---|
| Language | Rust stable |
| UI | Slint |
| Async | Tokio（仅服务端/IPC/IO 所需部分） |
| Serialization | serde + JSON（MVP） |
| IPC | Named Pipe / localhost IPC，具体协议单独 ADR 决定 |
| DB | SQLite |
| Search | SQLite FTS5 + 自定义轻量 ranking |
| Logging | tracing |
| Error | thiserror / anyhow 按边界使用 |
| Build | cargo |
| Test | cargo test + integration tests + benchmark |
| CI | GitHub Actions |

### 2.2 明确禁止

MVP 主程序：

- Electron
- CEF
- WebView/WebView2
- 浏览器作为 UI Runtime
- 将 Python/Node runtime 静态嵌入 Core
- 插件直接加载进 Core 动态库并依赖 Rust 私有类型

---

## 3. 总体架构

```text
                       +----------------------+
                       |      Launcher UI     |
                       |      Rust + Slint    |
                       +----------+-----------+
                                  |
                                  | IPC
                                  v
                       +----------------------+
                       |      Launcher Core    |
                       | hotkey / command /    |
                       | ranking / action      |
                       +----+-------+----------+
                            |       |
              +-------------+       +------------------+
              |                                    |
              v                                    v
   +----------------------+             +----------------------+
   |    Indexer Service   |             |    Plugin Broker     |
   | NTFS/MFT/USN/FS      |             | lifecycle/capability |
   +----------+-----------+             +-----+----------------+
              |                               |
              v                         +-----+-----+----------+
        +-----------+                    |           |          |
        | SQLite DB |                    v           v          v
        +-----------+                 WASM       Python      Node
                                      optional    on-demand   on-demand

                       +----------------------+
                       |      AI Service      |
                       | on-demand providers  |
                       +----------------------+
```

### 3.1 进程原则

至少存在以下逻辑边界：

- `launcher-ui.exe`
- `launcher-core.exe`
- `launcher-indexer.exe`
- `launcher-plugin-host.exe`（仅有插件任务时启动）
- `launcher-ai.exe`（仅有 AI 任务时启动）

MVP 可以将 UI 与 Core 暂时合并，以缩短验证周期；但模块边界必须按最终形态设计。

---

## 4. 内存设计

### 4.1 一级目标

```text
Idle Private Bytes:
  Target <= 50 MB
  Hard budget <= 80 MB

Idle CPU:
  Target < 0.1%

No plugin runtime:
  Python/Node/AI runtime = 0 MB resident
```

### 4.2 设计规则

- 所有昂贵对象延迟初始化。
- 所有大对象都有明确生命周期。
- 搜索结果限制数量，不允许无限缓存。
- 图片缩略图采用 bounded cache。
- 插件 Host 默认按需启动。
- Preview 默认按需生成。
- AI client 不得在 Core 启动时初始化。
- Indexer 不进入 UI 进程。
- 禁止全量扫描结果长期驻留内存。
- 禁止静态全局缓存无上限增长。

### 4.3 观测指标

必须能够采集：

- Private Bytes
- Working Set
- Commit
- CPU time
- Handle count
- Thread count
- UI latency
- Search latency
- IPC latency

---

## 5. Command Domain Model

核心对象：

```rust
Command {
    id
    title
    subtitle
    icon
    provider_id
    score
    keywords
    category
    actions
    preview
    context_requirements
    capabilities
}
```

### 5.1 Provider

Provider 负责发现 Command：

```text
Provider
  ├── start()
  ├── query(QueryContext)
  ├── metadata()
  └── shutdown()
```

Provider 不直接控制窗口。

### 5.2 Action

Action 是用户对 Command 的实际操作：

```text
Open
Copy
Reveal
OpenTerminalHere
Execute
RunWorkflow
AskAI
```

Action 执行必须经过 Action Engine。

---

## 6. Context Engine

上下文不是 Command 的附加字段，而是独立服务。

输入：

- 当前前台窗口
- Explorer 当前目录
- 当前选中文件
- 当前剪贴板
- 当前应用

输出：

```text
ContextSnapshot
```

示例：

```json
{
  "foreground_app": "explorer.exe",
  "current_folder": "C:\\Projects\\Launcher",
  "selected_items": [],
  "clipboard_type": "text"
}
```

Context 必须带时间戳和可信度。

---

## 7. Search Engine

### 7.1 搜索流程

```text
Raw Query
  -> Normalize
  -> Detect intent
  -> Select providers
  -> Parallel query
  -> Score
  -> Deduplicate
  -> Rank
  -> Limit
  -> Render
```

### 7.2 MVP Intent

- Application
- File
- Command
- Folder

### 7.3 Ranking

MVP 使用确定性算法，不引入模型。

建议特征：

```text
exact > prefix > token > fuzzy
frequency
recency
context bonus
provider priority
```

所有得分函数必须可测试。

---

## 8. Indexer

Windows-first MVP：

### Phase 1

- 指定目录扫描
- SQLite 元数据
- 文件名 FTS
- 修改时间
- 大小

### Phase 2

- NTFS MFT
- USN Journal
- 增量更新

Indexer API：

```text
search(query)
get_metadata(id)
watch_changes()
rebuild()
status()
```

Indexer 不负责 UI，不负责排序，不负责 Action。

---

## 9. Plugin Architecture

### 9.1 插件层级

```text
Tier 0  Built-in
Tier 1  WASM（预留）
Tier 2  Native Process Plugin
Tier 3  Python Process Plugin
Tier 4  Node Process Plugin
Tier 5  Shell / Script
```

### 9.2 MVP

MVP 只实现：

- Built-in Provider
- 一个 External Process Plugin Host
- JSON-RPC 风格请求/响应

不要在 MVP 同时实现 WASM + Python + Node。

### 9.3 生命周期

```text
Installed
  -> Discovered
  -> Validated
  -> Ready
  -> Spawned
  -> Running
  -> Idle
  -> Shutdown
```

默认：

```text
OnDemand
IdleTimeout = 10s
```

### 9.4 Capability

插件 Manifest 必须声明：

```text
filesystem.read
filesystem.write
clipboard.read
clipboard.write
network
shell.execute
process.spawn
notifications
ui.render
```

MVP 至少实现权限声明和 Host 侧检查，即使实际权限系统先只支持 allow/deny。

---

## 10. Native UI Schema

插件不可提供 HTML UI。

插件只能返回 Host 可理解的数据结构：

```json
{
  "view": "list",
  "items": [
    {
      "title": "Example",
      "subtitle": "Description",
      "icon": "app://example",
      "actions": ["open", "copy"]
    }
  ]
}
```

UI Schema 的原则：

- 声明式
- 有限组件集
- 无脚本执行
- 无任意 DOM
- Host 控制布局和生命周期

---

## 11. Workflow

MVP 暂不实现完整 DAG。

先定义抽象：

```text
Workflow
  id
  name
  trigger
  input_schema
  steps[]
  output_schema
```

最终版本可以升级为 DAG。

---

## 12. AI

AI Service 必须为独立边界：

```text
Core -> AI Gateway -> Provider
```

Provider 可包括：

- OpenAI-compatible
- Ollama
- Anthropic
- Gemini
- MCP

MVP 只定义接口，不要求完整 AI 产品能力。

---

## 13. 数据存储

MVP 使用 SQLite。

核心表：

```text
providers
commands
apps
files
file_paths
history
plugins
plugin_permissions
settings
```

要求：

- schema migration
- WAL
- prepared statement
- bounded history
- 任何索引结构必须有 benchmark

---

## 14. 错误处理

分类：

```text
Recoverable
Transient
UserActionRequired
PluginFailure
IPCFailure
Corruption
SecurityViolation
Bug
```

外部插件永远不能 panic/exit 导致 Core 崩溃。

---

## 15. 日志与可观测性

统一使用 `tracing`。

日志级别：

```text
error
warn
info
debug
trace
```

核心事件：

```text
launcher.started
launcher.hotkey
query.started
query.completed
plugin.spawned
plugin.failed
plugin.timeout
indexer.started
indexer.updated
memory.sampled
```

---

## 16. 安全基线

- 插件最小权限
- 外部进程隔离
- IPC 请求必须可识别来源
- 不信任插件输入
- 路径必须规范化
- Shell 执行必须显式 action
- 敏感能力不能通过任意参数隐式开启
- 插件崩溃不得影响 Core

---

## 17. 目录结构

```text
native-launcher/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── docs/
├── crates/
│   ├── launcher-core/
│   ├── launcher-ui/
│   ├── launcher-domain/
│   ├── launcher-ipc/
│   ├── launcher-search/
│   ├── launcher-context/
│   ├── launcher-action/
│   ├── launcher-indexer/
│   ├── launcher-plugin-api/
│   ├── launcher-plugin-host/
│   └── launcher-testkit/
├── apps/
│   ├── launcher-app/
│   ├── launcher-indexer-service/
│   └── launcher-plugin-host/
├── plugins/
│   └── examples/
├── benches/
├── tests/
└── scripts/
```

---

## 18. ADR 必须项

以下决定禁止通过“顺手改代码”解决，必须先写 ADR：

- IPC 协议
- UI/Core 是否分进程
- DB schema 大改
- Plugin manifest schema
- Plugin lifecycle
- Sandbox/runtime
- Search ranking 结构
- Indexing technology
- 跨平台 UI 架构

---

## 19. MVP 结束定义

MVP 必须能够：

```text
Hotkey
  -> Native popup
  -> Search app/file
  -> Show result
  -> Keyboard select
  -> Execute action
  -> Context-aware command
  -> External plugin response
```

同时通过：

- 单元测试
- 集成测试
- IPC 测试
- Plugin crash test
- Memory soak test
- 100-provider stress test
- 基本 UX smoke test

---

## 20. Reference Projects / Current Research Notes

This design intentionally references current implementations for architecture study rather than copying their implementations.

- Wox: current repository separates core and language-specific plugin hosts, including Node.js and Python plugin support.
- Flow Launcher: Python plugins use JSON-RPC as a local procedure call mechanism.
- Lertaro: current repository describes a Windows architecture using C# WPF, an NT service, MFT/USN-based indexing, plugin support, and process isolation.
- uTools: its plugin model uses `plugin.json`, optional `preload.js`, and exposes Node.js APIs through Electron preload.

These references are inputs to architectural analysis; they are not requirements to reproduce their technology choices.
