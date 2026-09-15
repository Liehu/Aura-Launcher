# Aura-Launcher P3 — UI/UX Design Specification v1.0

> Status: FROZEN  
> Baseline: P2.4 Foundation  
> Platform: Windows-first  
> Development Mode: Agentic Coding  
> Primary UX Reference: Flow Launcher  
> Objective: Launcher UX / Interaction Architecture Closure

---

# 1. Purpose

P3 的目标不是简单改善视觉效果，而是建立 Aura-Launcher 的完整 UI/UX 信息架构、状态模型、搜索交互模型、插件交互模型和系统级入口。

P3 必须解决当前 MVP 中的核心问题：

1. Launcher、Plugin UI、Settings、Plugin Management 状态混杂。
2. 搜索结果缺乏明确的优先级和分组。
3. 内置应用 / 核心插件无法稳定优先于低价值结果。
4. 插件选择后的交互路径复杂。
5. 插件管理缺少独立的信息架构。
6. System Tray 缺少完整的产品入口。
7. 右键 / 上下文操作不统一。
8. 鼠标与键盘交互模型没有形成统一 Action Model。
9. 用户无法明确判断“我现在是在搜索、执行、插件上下文还是管理界面”。
10. UI 层没有形成可以持续支持未来 AI / Workflow / MCP 的稳定 UX 基础。

---

# 2. Product UX Principle

## 2.1 Launcher 的核心任务

Launcher 的唯一核心任务：

```text
Invoke
  ↓
Type
  ↓
Discover
  ↓
Select
  ↓
Execute
```

任何不直接服务于上述流程的复杂 UI，都不得进入默认 Launcher 主界面。

---

# 3. Three UX Domains

Aura 必须建立三个明确隔离的 UX Domain：

```text
┌─────────────────────────────────────┐
│ Launcher                            │
│ Search / Result / Context / Action  │
└─────────────────────────────────────┘
                 │
                 ├──────────────┐
                 ↓              ↓
        ┌──────────────┐  ┌──────────────┐
        │ Plugin Center│  │ Settings     │
        │ Plugin Mgmt  │  │ Configuration│
        └──────────────┘  └──────────────┘
                 ↑              ↑
                 └──────┬───────┘
                        ↓
                  System Tray
```

## 3.1 Launcher

负责：

- 搜索
- 结果展示
- 结果选择
- Plugin Context
- Action
- Preview
- Context Menu

不负责：

- Plugin 安装管理
- Plugin Store
- 大规模配置
- Diagnostics
- Advanced Settings

---

## 3.2 Plugin Center

负责：

- Installed Plugins
- Plugin Store
- Install
- Uninstall
- Enable / Disable
- Configure
- Update
- Permissions
- Trust
- Diagnostics

---

## 3.3 Settings

负责：

- General
- Search
- Appearance
- Hotkeys
- Sources
- Plugins
- Advanced
- Diagnostics

---

# 4. UI State Machine

必须建立显式 UI State Machine。

```text
LauncherClosed
      │
      │ hotkey
      ↓
LauncherSearch
      │
      ├── select normal result ──→ ActionExecution
      │
      ├── select plugin ─────────→ PluginContext
      │
      ├── F1 ────────────────────→ Preview
      │
      ├── right click ───────────→ ContextMenu
      │
      ├── open plugins ──────────→ PluginCenter
      │
      └── open settings ─────────→ Settings

PluginContext
      │
      ├── query
      ├── select
      ├── execute
      └── Esc → LauncherSearch

PluginCenter
      │
      └── close/Esc → previous state

Settings
      │
      └── close/Esc → previous state
```

## 4.1 Hard Rule

**Launcher、Plugin Center、Settings 必须是不同 UX State。**

禁止通过：

```text
改变 Launcher 内部布局
+
塞入另一套完整 UI
```

来模拟 Plugin Center / Settings。

---

# 5. Launcher Shell

Launcher 必须保持稳定的 Shell。

```text
┌────────────────────────────────────────────┐
│  🔍  Search...                             │
├────────────────────────────────────────────┤
│                                            │
│  Applications                             │
│  ┌────────────────────────────────────────┐│
│  │ ◎ Visual Studio Code                   ││
│  │   Microsoft                            ││
│  └────────────────────────────────────────┘│
│                                            │
│  Commands                                  │
│  ┌────────────────────────────────────────┐│
│  │ > GitHub                               ││
│  │   Plugin                               ││
│  └────────────────────────────────────────┘│
│                                            │
│  Files                                     │
│  ┌────────────────────────────────────────┐│
│  │ 📄 README.md                           ││
│  │   D:\Projects\...                      ││
│  └────────────────────────────────────────┘│
├────────────────────────────────────────────┤
│ Enter Open   ↑↓ Select   → Actions   Esc   │
└────────────────────────────────────────────┘
```

## 5.1 Launcher 不允许出现

默认情况下禁止：

- 左侧 Plugin Sidebar
- Plugin Store
- Settings navigation
- Diagnostics panel
- Plugin installation UI
- 大型 configuration forms

---

# 6. Search Result Contract

所有搜索结果必须进入统一的 `SearchResult` 模型。

```rust
struct SearchResult {
    id: ResultId,
    title: String,
    subtitle: Option<String>,
    icon: IconRef,

    source: ResultSource,
    kind: ResultKind,

    score: f64,

    ranking: RankingSignals,

    actions: Vec<ActionDescriptor>,

    metadata: ResultMetadata,
}
```

## 6.1 ResultKind

至少支持：

```text
Application
Command
Plugin
File
Folder
Web
Setting
SystemCommand
Other
```

## 6.2 ResultSource

至少支持：

```text
ApplicationCatalog
FileIndex
BuiltinPlugin
ExternalPlugin
Web
Settings
MCP
AI
```

---

# 7. Search Ranking

搜索必须采用**综合评分模型**，而不是简单按照 Plugin / File / Application 的固定列表拼接。

基础评分：

```text
FinalScore =
    TextMatch
  + ExactMatch
  + PrefixMatch
  + UsageScore
  + RecencyScore
  + SourcePriority
  + PluginPriority
  + FavoriteBoost
```

建议优先级：

```text
Exact Application
        ↓
Exact Command
        ↓
Prefix Application
        ↓
Frequently Used Application
        ↓
Builtin Command / Plugin
        ↓
User Plugin
        ↓
Files
        ↓
Web / Other
```

但以上仅作为初始权重，不允许实现成绝对硬编码顺序。

---

# 8. Result Grouping

搜索结果必须具有视觉层级。

推荐：

```text
Applications
Commands
Files
Folders
Plugins
Web
Other
```

每个 Group：

- 有明确标题
- 有稳定排序
- 支持 keyboard navigation
- 不得让 Group Header 成为不可选择的噪声元素

当结果数量较少时允许自动隐藏 Group Header。

---

# 9. Built-in Priority

核心内置能力必须拥有明确的 `source_priority`。

例如：

```text
Builtin Application
Builtin Command
Builtin Plugin
Installed User Plugin
File
Web
AI
```

默认情况下：

```text
Windows Calculator
Notepad
Explorer
Settings
System Commands
```

等核心能力不能因为第三方 Plugin 返回大量结果而被淹没。

---

# 10. Plugin Context

插件不能打开第二套 Launcher UI。

例如：

```text
User:
    github
```

结果：

```text
GitHub
GitHub Issues
GitHub Repositories
```

用户选择：

```text
GitHub
```

进入：

```text
┌────────────────────────────────────────────┐
│ ← GitHub                                   │
├────────────────────────────────────────────┤
│ Search GitHub...                            │
├────────────────────────────────────────────┤
│ Repositories                               │
│ Issues                                      │
│ Pull Requests                               │
└────────────────────────────────────────────┘
```

这是：

```text
LauncherContext = PluginContext(GitHub)
```

而不是：

```text
NewWindow(GitHub)
```

## 10.1 Exit

```text
Esc
```

返回：

```text
LauncherSearch
```

而不是退出整个 Launcher。

---

# 11. Unified Action Model

所有 Result 必须拥有统一 Action Model。

## Application

```text
Open
Run as Administrator
Open Location
Pin
Properties
Uninstall
```

## File

```text
Open
Open With
Copy Path
Open Location
Rename
Delete
Properties
```

## Folder

```text
Open
Open in Terminal
Copy Path
Properties
```

## Plugin

```text
Open
Configure
Enable
Disable
Reload
Uninstall
Permissions
Diagnostics
```

Action 必须经过：

```text
ActionProposal
      ↓
ActionResolver
      ↓
validate()
      ↓
execute()
      ↓
Effect
```

P3 不得绕过 P2.4 已冻结的 Action execution authority。 

---

# 12. Context Menu

所有主要 Result 必须支持 Context Menu。

```text
┌────────────────────────────┐
│ Open                       │
│ Open Location              │
│ Run as Administrator       │
├────────────────────────────┤
│ Pin                        │
│ Add Favorite               │
├────────────────────────────┤
│ Properties                 │
└────────────────────────────┘
```

不同 ResultKind 使用不同 Action 集。

Context Menu 必须与：

```text
→ Actions
```

使用同一个 Action Model。

禁止维护两套行为逻辑。

---

# 13. Keyboard UX

Launcher 必须 keyboard-first。

最低要求：

| Key | Action |
|---|---|
| Enter | Execute |
| ↑ / ↓ | Navigate |
| Esc | Close / Back |
| → | Open actions |
| ← | Back |
| Tab | Next area |
| Shift+Tab | Previous area |
| F1 | Preview |
| Ctrl+O | Open location / alternate action |
| Shift+Enter | Alternate execution |

所有主要操作必须存在 keyboard path。

---

# 14. Mouse UX

鼠标支持：

- Single click → Select
- Double click → Execute
- Right click → Context Menu
- Hover → optional metadata/preview
- Wheel → Scroll

鼠标操作不得引入键盘模式不存在的核心功能。

---

# 15. System Tray

System Tray 是产品级入口，不是简单的生命周期图标。

左键：

```text
Open Launcher
```

右键：

```text
┌─────────────────────────────┐
│ Aura Launcher               │
├─────────────────────────────┤
│ Open                        │
│ Plugins                     │
│ Settings                    │
│ Reindex                     │
├─────────────────────────────┤
│ Pause Hotkey                │
│ Check for Updates           │
├─────────────────────────────┤
│ About                       │
│ Exit                        │
└─────────────────────────────┘
```

必须保证：

```text
Tray → Plugin Center
Tray → Settings
Tray → Launcher
```

均为一跳操作。

---

# 16. Plugin Center

Plugin Center 采用 Master/Detail 布局。

```text
┌────────────────────────────────────────────────────┐
│ Plugins                              Search...     │
├──────────────┬───────────────────────┬─────────────┤
│ Categories   │ Plugin List           │ Detail      │
│              │                       │             │
│ Installed    │ GitHub                │ GitHub      │
│ Store        │ Calculator            │ v1.4.2      │
│ Updates      │ Explorer              │             │
│ Disabled     │ Browser               │ Enabled     │
│              │                       │ Configure   │
│              │                       │ Disable     │
└──────────────┴───────────────────────┴─────────────┘
```

Plugin Detail 至少：

```text
Name
Icon
Description
Version
Author
Status
Trust
Permissions
Capabilities
Actions
Diagnostics
```

---

# 17. Settings Information Architecture

Settings：

```text
General
Search
Appearance
Hotkeys
Sources
    Applications
    Files
Plugins
    Installed
    Store
    Permissions
Advanced
    Index
    Catalog
    Diagnostics
    Developer
About
```

禁止：

```text
Settings
 └── Plugins
      └── Plugin
           └── Plugin-specific UI
```

导致 Settings navigation 与 Plugin Center 重复。

Plugin-specific configuration 应通过统一 Plugin Configuration surface 进入。

---

# 18. Visual Design System

P3 不追求复杂视觉效果。

原则：

```text
Minimal
Dense
Keyboard-first
Readable
Consistent
Windows-native
```

统一定义：

```text
Spacing
Typography
Corner Radius
Icon Size
Row Height
Border
Shadow
Focus Ring
Selected State
Disabled State
Error State
```

推荐 Launcher：

```text
Search box
    48–56px

Result row
    44–52px

Section spacing
    8–16px

Launcher width
    adaptive

Launcher height
    content-driven
```

不得通过大量卡片、渐变、动画掩盖信息架构问题。

---

# 19. Accessibility

必须支持：

- keyboard navigation
- visible focus
- high contrast
- readable font sizes
- screen-reader semantic labels where framework supports them
- disabled state
- error state
- loading state

Focus state 不得仅依赖颜色。

---

# 20. UX Performance

Launcher 打开后：

```text
UI Shell
   ↓
Immediate input
   ↓
Incremental results
```

禁止：

```text
Open Launcher
   ↓
Discover applications
   ↓
Scan filesystem
   ↓
Load plugins
   ↓
Render
```

搜索必须使用已经建立的：

```text
Catalog
File Index
Plugin Registry
```

而不是在 UI 线程同步执行 Discovery。

P2.4 已规定应用搜索通过 Catalog Provider 获取数据，搜索不得同步重新发现应用。 

---

# 21. Loading / Empty / Error States

所有主要 UI 必须定义：

```text
Loading
Empty
Error
Disabled
Unavailable
Recovering
```

例如 Plugin：

```text
Plugin unavailable
Reason:
  Timeout / Crash / Quarantined / Disabled

Actions:
  Retry
  Open Diagnostics
  Enable
```

错误信息不得直接暴露内部 Rust stack trace。

---

# 22. Plugin Failure UX

Plugin 状态：

```text
Healthy
Starting
Busy
Failed
Timeout
Quarantined
Disabled
```

Quarantined Plugin：

```text
GitHub
⚠ Quarantined

Reason:
Protocol failure detected

[View Diagnostics]
[Recover]
```

UI 不得自行改变 Plugin Trust / Capability 状态。

所有安全敏感状态必须通过 Control Plane。

---

# 23. Search Query Lifecycle

```text
User Input
    ↓
Query State
    ↓
Search Provider
    ↓
Candidate Results
    ↓
Ranking
    ↓
Grouping
    ↓
Presentation
    ↓
Selection
    ↓
ActionResolver
    ↓
Execution
```

明确禁止：

```text
UI
 ↓
Plugin executable
 ↓
execute directly
```

---

# 24. UX Telemetry / Learning Boundary

P3 可以记录：

```text
query
result selected
result executed
result kind
execution success/failure
timestamp
```

但不得：

- 让 UI 自己修改 ranking model
- 让 Plugin 修改全局 ranking
- 让 AI 直接改变 execution authority

Ranking Learning 属于后续独立能力。

---

# 25. Future Compatibility

P3 必须为未来能力保留扩展点：

```text
AI Query Understanding
Semantic Search
Workflow
MCP
Automation
Agent
```

但 P3 不实现这些能力。

未来：

```text
AI
 ↓
Query Intent
 ↓
Search / Action Proposal
 ↓
ActionResolver
```

仍然必须遵守现有 execution authority。

---

# 26. Explicit Non-Goals

P3 不实现：

- AI Search
- Semantic Search
- Workflow Editor
- DAG
- Agent
- MCP orchestration
- Cloud Sync
- Marketplace backend
- Multi-device Sync
- Enterprise Management

这些能力只能使用 P3 提供的 UX extension points。

---

# 27. P3 Priority

## P0 — Correctness

必须首先完成：

1. UI State Machine
2. Launcher Shell
3. Search Result Contract
4. Ranking
5. Grouping
6. Unified Action Model
7. Context Menu
8. Tray

## P1 — Product UX

然后完成：

9. Plugin Context
10. Plugin Center
11. Settings IA
12. Keyboard UX
13. Preview
14. Loading/Error states

## P2 — Polish

最后：

15. Visual Design System
16. Animation
17. Accessibility
18. UX telemetry
19. Fine-grained ranking tuning

---

# 28. Definition of Done

P3 完成必须满足：

### Launcher

- [ ] Search immediately available
- [ ] Applications rank correctly
- [ ] Built-in capabilities receive priority
- [ ] Results are grouped
- [ ] Keyboard navigation complete
- [ ] Mouse navigation complete
- [ ] Context Menu complete
- [ ] Action Model unified

### Plugin

- [ ] Plugin selection enters PluginContext
- [ ] PluginContext does not replace Launcher architecture
- [ ] Esc returns correctly
- [ ] Plugin Center is independent
- [ ] Install/uninstall/enable/disable are available
- [ ] Trust/capability states are visible
- [ ] Quarantine is represented

### System

- [ ] Tray exists
- [ ] Tray menu provides Launcher / Plugins / Settings
- [ ] Exit works
- [ ] Hotkey lifecycle works

### Settings

- [ ] Information architecture frozen
- [ ] Plugin configuration isolated
- [ ] Search settings isolated
- [ ] Advanced diagnostics isolated

### Architecture

- [ ] UI never bypasses ActionResolver
- [ ] UI never executes Plugin directly
- [ ] UI does not rediscover applications
- [ ] UI does not own Plugin trust
- [ ] UI does not own Plugin capability decisions

---

# 29. P3 Agent Task Map

```text
P3-A  UI State Architecture
P3-B  Launcher Shell
P3-C  Search Result + Ranking
P3-D  Result Grouping + Navigation
P3-E  Unified Action + Context Menu
P3-F  Plugin Context
P3-G  Plugin Center
P3-H  Settings + System Tray
P3-I  Visual System + Accessibility
P3-J  UX Integration + Regression Tests
```

Dependency:

```text
              P3-A
                │
        ┌───────┼────────┐
        ↓       ↓        ↓
      P3-B     P3-C     P3-E
        │       │        │
        └───┬───┘        │
            ↓            │
          P3-D ──────────┘
            │
      ┌─────┴─────┐
      ↓           ↓
    P3-F         P3-G
      │           │
      └─────┬─────┘
            ↓
          P3-H
            ↓
          P3-I
            ↓
          P3-J
```

P3-A、P3-B、P3-C、P3-E 可在接口冻结后部分并行。

---

# 30. Agent Execution Rules

每个 Agent Task 必须：

1. 先读取 P3 Specification。
2. 检查当前代码状态。
3. 找到现有实现。
4. 不假设目标架构不存在。
5. 不重写已有 P2.4 infrastructure。
6. 只修改 Allowed Changes。
7. 不修改 Forbidden Changes。
8. 运行指定 Tests。
9. 输出 Completion Report。
10. 若发现规范冲突，停止并报告，不自行修改 Frozen Contract。

Completion Report：

```text
Task:
Status:
Files Changed:
Architecture Changes:
Tests Run:
Test Result:
Acceptance Criteria:
Known Limitations:
Deferred Issues:
```

---

# 31. Frozen Architecture Boundary

最终架构：

```text
┌──────────────────────────────────────────────┐
│                 UI Layer                     │
│                                              │
│ Launcher Shell                              │
│ Plugin Context                              │
│ Plugin Center                               │
│ Settings                                    │
│ Tray                                        │
└──────────────────────┬───────────────────────┘
                       │
                 Search / UX Model
                       │
┌──────────────────────┴───────────────────────┐
│                  Core                        │
│                                              │
│ CatalogProvider                              │
│ File Index                                   │
│ Plugin Control Plane                         │
│ Search                                       │
│ Ranking                                      │
└──────────────────────┬───────────────────────┘
                       │
                  ActionResolver
                       │
               launcher-action
                       │
                    Effect
```

UI 是 Presentation / Interaction Layer。

UI 不拥有：

- execution authority
- plugin trust authority
- capability authority
- catalog authority
- filesystem indexing authority

---

# 32. Final Product Principle

Aura 的目标不是：

> “做一个更漂亮的 Flow Launcher。”

而是：

> **Build a modern Windows-first launcher with a clean interaction architecture, plugin-native extensibility, deterministic search behavior, and a strict separation between discovery, presentation, management and execution.**

P3 的成功标准不是“截图更好看”。

而是用户可以自然完成：

```text
Alt+Space
   ↓
Type
   ↓
看到正确结果
   ↓
Enter
   ↓
完成操作
```

并且：

```text
管理插件
设置
诊断
系统托盘
```

都不会污染上述核心路径。