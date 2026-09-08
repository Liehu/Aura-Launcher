# Native Launcher — Post-1.0 Roadmap

`Launcher 1.0` 完成后，开发模式应该从 **“完成核心产品”** 转为 **“能力演进 + 平台化”**。

这一阶段不应该简单继续编号 `P2.4 / P2.5 / P2.6` 堆功能，而应该进入新的产品路线：

```text
Launcher 1.0
     │
     ▼
══════════════════════════════════
          2.x Capability
══════════════════════════════════
     │
     ├── Search Intelligence
     ├── Plugin Ecosystem
     ├── Workflow 2.0
     ├── AI / Agent
     ├── Advanced Index
     └── Windows Integration
                │
                ▼
══════════════════════════════════
          3.x Platform
══════════════════════════════════
     │
     ├── Plugin Marketplace
     ├── Cloud Sync
     ├── Durable State
     ├── Multi-device
     └── Enterprise
```

---

# 一、先建立新的完成度模型

1.0 之后继续保留之前的三个状态：

| 状态              | 含义                    |
| --------------- | --------------------- |
| ✅ Closed        | 产品能力完整、测试和 Gate 完成    |
| ⚠️ Core Slice   | 核心能力已有，但产品体验/生命周期尚未完整 |
| 🧪 Experimental | 已有技术验证，不属于正式产品能力      |
| 📌 Planned      | 已确定但尚未实现              |
| ⏸ Deferred      | 有意推迟                  |
| ❌ Rejected      | 明确不做                  |

这样后续不会再出现：

```text
“代码写了”
=
“产品完成”
```

---

# 二、1.0 之后首先处理的不是 AI

我建议把 1.0 后的第一阶段定义为：

# P2.4 — Post-1.0 Foundation Closure

目标：

> 把 1.0 中为了控制范围而留下的基础能力补齐。

这不是新功能大版本，而是 **2.x 的地基**。

---

## P2.4-A — Persistent Application Catalog 2.0

当前已经有：

```text
Discovery
   ↓
Catalog DB
```

但仍需要完善：

```text
Catalog Coordinator
incremental reconciliation
stale/broken state
package ↔ application
AUMID
App Paths
portable app
icon association
```

最终：

```text
Application Sources
 ├── PackageManager
 ├── Start Menu
 ├── Registry
 ├── App Paths
 └── Portable
          ↓
   ApplicationDiscovery
          ↓
 ApplicationCatalogCoordinator
          ↓
     Persistent Catalog
          ↓
       Search
```

这是 2.x Search Intelligence 的基础。

---

# 三、P2.4-B — Advanced Index Maintenance

目前 File Index 已经成熟，但还可以继续补：

```text
Incremental reconciliation
Root disappearance/reappearance
Watcher re-registration
Queue overflow recovery
Generation cutover
Index compaction
Periodic health check
Index statistics
```

特别是：

```text
ReadDirectoryChangesW
        ↓
queue overflow
        ↓
dirty root
        ↓
bounded rescan
```

形成长期稳定的 self-healing index。

---

# 四、P2.4-C — Plugin Registry Secure Recovery

1.0 虽然已经完成，但这个能力之前属于 GA 尾项，因此 2.x 第一批仍建议正式产品化：

```text
plugins.db
   ↓
backup
   ↓
corruption
   ↓
restore
```

必须保留：

```text
enabled
disabled
quarantined
capability approval
```

并区分：

```text
Known Plugin
Trusted Plugin
Disabled Plugin
Quarantined Plugin
Unknown Plugin
```

这一层完成后，Plugin Control Plane 才真正成熟。

---

# 五、P2.4-D — Plugin SDK / CLI

这是 2.x 非常值得投入的一项。

目前：

```text
Plugin Runtime
Plugin Host
Manifest
Registry
```

都有了。

下一步应该让第三方真正能够开发插件。

建议：

```text
launcher-plugin-sdk
launcher-plugin-cli
```

提供：

```text
init
build
validate
package
install
run
debug
logs
inspect
```

目标：

```text
plugin init my-plugin
        ↓
plugin build
        ↓
plugin package
        ↓
Launcher install
```

---

# 六、P2.4-E — Plugin Development / Debugging

SDK 后继续：

```text
Plugin
 ↓
Development Mode
 ↓
Live Logs
 ↓
Test Input
 ↓
RPC Inspector
 ↓
Crash Diagnostics
```

尤其可以把目前 Plugin/MCP 已经拥有的：

```text
ExecutionId
RuntimeId
request
response
failure
```

直接用于诊断工具。

这样你的插件生态才真正从：

> “系统可以加载插件”

升级为：

> “开发者可以高效开发插件”。

---

# 七、P2.5 — Search Intelligence

基础设施稳定后进入真正的搜索升级。

这是 **2.x 最大主线之一**。

---

## P2.5-A — Query Intent

当前：

```text
Query
 ↓
Providers
```

升级：

```text
Query
 ↓
Intent Detection
 ↓
Search Strategy
 ↓
Providers
```

例如：

```text
chrome
→ Application

download
→ Folder/File

compress xxx.pdf
→ Action

restart computer
→ System Action
```

---

# 八、P2.5-B — Context Intelligence

你已经有：

```text
ForegroundApp
CurrentFolder
ContextGeneration
```

下一阶段可以扩展：

```text
Time
Recent Activity
Recent Files
Current Window
Current Selection
Session State
```

但继续保持：

> Context 影响 Ranking，不拥有执行权限。

---

# 九、P2.5-C — Personal Ranking

建立真正的：

```text
UserState
   ↓
Usage Features
   ↓
Ranking Features
   ↓
Personal Ranker
```

考虑：

```text
frequency
recency
application association
time-of-day
context
manual favorite
manual pin
```

然后让：

```text
RankingGeneration
```

真正从目前的：

```text
0
```

变成动态代际。

---

# 十、P2.5-D — Ranking Explainability

这是很适合 Launcher 的高级能力。

用户可以知道：

```text
Why this result?
```

例如：

```text
Chrome
+20 Favorite
+14 Recent Use
+8 Foreground Association
+100 Exact Match
```

内部可以存在：

```text
RankingBreakdown
```

但 UI 只展示用户可理解的结果。

---

# 十一、P2.5-E — Candidate Merge v2

之前的 Merge 是最小可用版。

这里正式演进：

```text
Provider Candidate
       ↓
Canonical Candidate
       ├── Identity
       ├── Sources
       ├── Metadata
       ├── Actions
       ├── Provenance
       └── Ranking features
```

完整解决：

```text
same application
same file
same folder
same command
```

多 Provider 共存的问题。

这应该成为 Search Contract v2 的基础。

---

# 十二、P2.5-F — Search Contract v2

补之前有意 Deferred 的：

```text
SearchProvider
SearchRequest
SearchContextSnapshot
SearchCandidate
SearchBatch
SearchEvents
SearchDiagnostics
SearchSnapshot
```

并进一步引入：

```text
typed SearchRequestId
provider timeout isolation
partial results
query cache improvements
SingleFlight
```

这一步完成以后，Search Core 才真正进入成熟平台状态。

---

# 十三、P2.6 — Workflow 2.0

你现在的 Workflow 0.2 已经非常适合作为基础。

下一代不再主要是语法，而是**运行时能力**。

---

## P2.6-A — Workflow Editor

```text
Workflow
   ↓
Visual Editor
```

支持：

```text
Step
Branch
Variable
Condition
Output
Failure Policy
```

---

## P2.6-B — Parallel / Join

从：

```text
A → B → C
```

升级：

```text
       ┌→ B ─┐
A ─────┤     ├→ D
       └→ C ─┘
```

---

## P2.6-C — Subworkflow

```text
Workflow A
    ↓
Subworkflow B
    ↓
return output
```

---

## P2.6-D — Human Approval

让 Workflow 正式支持：

```text
Step
 ↓
Approval
 ↓
Pause
 ↓
User confirms
 ↓
Resume
```

这里重新利用已经建立的：

```text
Confirmation
stale protection
re-resolve
```

---

# 十四、P2.6-E — Durable Workflow

这是一个明显的大能力。

从：

```text
in-memory WorkflowRun
```

变成：

```text
persistent WorkflowRun
```

支持：

```text
pause
restart Launcher
resume
retry
history
recovery
```

也就是：

```text
Workflow
 ↓
Durable State
 ↓
Crash
 ↓
Restart
 ↓
Resume
```

这一能力建议单独作为 2.x 中后期项目，不要早做。

---

# 十五、P2.7 — AI Foundation

AI 不应该直接从“Agent”开始。

先做：

# P2.7-A — AI Query Understanding

```text
Query
 ↓
LLM
 ↓
Intent / Entities
 ↓
Search
```

例如：

```text
“找一下我昨天下载的 PDF”
```

变成：

```json
{
  "intent": "file_search",
  "filters": {
    "extension": "pdf",
    "relative_time": "yesterday"
  }
}
```

AI 只负责理解。

---

# 十六、P2.7-B — AI Action Proposal

随后：

```text
Natural Language
 ↓
AI
 ↓
ActionProposal
 ↓
ReferenceResolver
 ↓
ActionResolver
 ↓
ActionEngine
```

严格保持现有安全链。

AI：

```text
❌ Effect
❌ Capability grant
❌ Direct execution
```

只能：

```text
✅ Proposal
```

---

# 十七、P2.7-C — Agent Productization

你已经有一个：

```text
Minimal Agent Runtime
```

但当前没有正式产品入口。

2.x 才真正做：

```text
Agent
 ↓
Observe
 ↓
Plan
 ↓
Propose
 ↓
Execute
 ↓
Observe
 ↓
Replan
```

加上：

```text
Confirmation
Budget
History
Cancellation
Visualization
```

---

# 十八、P2.7-D — Agent Confirmation / Resume

正式产品化 Agent 后：

```text
Proposal
 ↓
Risk classification
 ↓
Confirmation
 ↓
Execute
```

以及：

```text
WaitingForConfirmation
        ↓
Launcher restart
        ↓
recover policy
```

这时候可以把 Agent 与 Durable Workflow 建立合理连接。

---

# 十九、P2.7-E — Agent + Workflow

最终：

```text
Agent
  ↓
Plan
  ↓
Workflow Definition
  ↓
Workflow Engine
```

但必须保留：

```text
Agent ≠ Executor
Workflow ≠ Authority
```

Agent 生成的是**计划/编排意图**，最终仍然：

```text
Resolver
→ ActionEngine
```

---

# 二十、P2.8 — Advanced Windows Integration

这是一条独立的 Windows 主线。

可以逐步加入：

```text
Windows Search
AUMID
Jump List
App Paths
URI schemes
File associations
Shell verbs
Startup apps
Clipboard
Window management
Virtual desktops
PowerToys-like system commands
```

尤其：

```text
Application Catalog
```

已经有了基础，这时可以继续完善 Windows application identity。

---

# 二十一、P2.9 — Advanced File Intelligence

对于你的 Launcher，文件能力会是很大的差异化方向。

可以加入：

```text
Recent files
File type intelligence
Archive awareness
Document metadata
PDF metadata
EXIF
Git repository awareness
Project detection
Folder semantics
```

但：

> 不建议马上做全盘全文索引。

先建立：

```text
File
→ Metadata
→ Context
→ Ranking
```

再决定是否进入 Content Index。

---

# 二十二、P3 — Plugin Platform

等 Plugin SDK 稳定以后进入：

```text
P3-A Plugin Repository
P3-B Plugin Marketplace
P3-C Signing / Trust
P3-D Automatic Update
P3-E Compatibility Management
```

架构：

```text
Developer
   ↓
Plugin SDK
   ↓
Package
   ↓
Sign
   ↓
Repository
   ↓
Launcher
   ↓
Verify
   ↓
Install
```

---

# 二十三、P3 — Cloud / Multi-device

这是明确的 3.x 能力。

不要在 2.x 核心产品中提前加入。

可以同步：

```text
Settings
Favorites
Plugins
Workflows
Ranking profile
AI preferences
```

但是：

```text
Local file index
Runtime state
Execution history
```

需要单独设计，不能直接上传。

---

# 二十四、P3 — Durable User State

进一步：

```text
Local DB
   ↓
Event / Change Log
   ↓
Sync Engine
   ↓
Remote
```

需要处理：

```text
conflict
version
generation
offline
merge
rollback
```

这个时候之前设计的 Generation 体系会继续派上用场。

---

# 二十五、P4 — Enterprise

只有商业需求出现时再做：

```text
Enterprise Policy
Central Configuration
RBAC
Managed Plugins
Audit
Fleet Management
Organization
SSO
```

---

# 二十六、最终路线图

我建议冻结成：

```text
═══════════════════════════════════════
             Launcher 1.0
═══════════════════════════════════════
                    │
                    ▼
             P2.4 Foundation
                    │
        ┌───────────┼────────────┐
        ▼           ▼            ▼
   Catalog 2.0   Index 2.0   Plugin Control
        │           │            │
        └───────────┼────────────┘
                    ▼
          Plugin SDK / CLI
                    │
                    ▼
═══════════════════════════════════════
             Launcher 2.x
═══════════════════════════════════════
                    │
       ┌────────────┼─────────────┐
       ▼            ▼             ▼
 Search Intel   Workflow 2.0   Windows Intel
       │            │             │
       └────────────┼─────────────┘
                    ▼
               AI Foundation
                    │
                    ▼
             Agent Productization
                    │
                    ▼
            Durable Workflow
═══════════════════════════════════════
             Launcher 3.x
═══════════════════════════════════════
       │              │             │
       ▼              ▼             ▼
 Marketplace      Cloud Sync    Multi-device
       │              │             │
       └──────────────┼─────────────┘
                      ▼
              Platform Ecosystem
                      │
                      ▼
═══════════════════════════════════════
             Launcher 4.x
═══════════════════════════════════════
                Enterprise
```

---

# 二十七、建议的具体 Batch 编号

最终我建议不要让后续编号继续杂乱，直接冻结：

| Batch    | 主题                                    | 优先级 |
| -------- | ------------------------------------- | --- |
| **P2.4** | Post-1.0 Foundation Closure           | P0  |
| P2.4-A   | Persistent Catalog 2.0                | 高   |
| P2.4-B   | Advanced Index Maintenance            | 高   |
| P2.4-C   | Plugin Secure Recovery                | 高   |
| P2.4-D   | Plugin SDK / CLI                      | 高   |
| P2.4-E   | Plugin Debugging                      | 中   |
| **P2.5** | Search Intelligence                   | P0  |
| P2.5-A   | Query Intent                          | 高   |
| P2.5-B   | Context Intelligence                  | 高   |
| P2.5-C   | Personal Ranking                      | 高   |
| P2.5-D   | Ranking Explainability                | 中   |
| P2.5-E   | Candidate Merge v2                    | 高   |
| P2.5-F   | Search Contract v2                    | 高   |
| **P2.6** | Workflow 2.0                          | 中   |
| P2.6-A   | Workflow Editor                       | 中   |
| P2.6-B   | Parallel / Join                       | 中   |
| P2.6-C   | Subworkflow                           | 中   |
| P2.6-D   | Human Approval                        | 中   |
| P2.6-E   | Durable Workflow                      | 中   |
| **P2.7** | AI / Agent                            | 高   |
| P2.7-A   | AI Query Understanding                | 高   |
| P2.7-B   | Action Proposal                       | 高   |
| P2.7-C   | Agent Productization                  | 中   |
| P2.7-D   | Agent Confirmation                    | 中   |
| P2.7-E   | Agent + Workflow                      | 中   |
| **P2.8** | Advanced Windows Integration          | 中   |
| **P2.9** | Advanced File Intelligence            | 中   |
| **P3**   | Plugin Platform / Cloud / Marketplace | 后置  |
| **P4**   | Enterprise                            | 最后  |

---

# 二十八、开发优先级

如果只看**产品价值 / 技术依赖 / 风险**，我会给出：

```text
                     优先级
                        │
             ┌──────────┴──────────┐
             │                     │
       Search Intelligence     Plugin Platform
             │                     │
             ▼                     ▼
       Query Intent           SDK / CLI
       Context Ranking        Debugging
       Personalization        Secure Registry
             │                     │
             └──────────┬──────────┘
                        ▼
                 Workflow 2.0
                        │
                        ▼
                  AI Foundation
                        │
                        ▼
                  Agent Runtime
                        │
                        ▼
               Durable Workflow
                        │
                        ▼
              Marketplace / Cloud
```

## 我的建议是：**2.x 第一主线做 Search Intelligence**

原因很简单：

你现在已经把：

```text
Index
Catalog
Context
Cache
Generation
Candidate Merge
Ranking
History
Favorite
Action
```

这些基础全部铺好了。

因此下一步最自然的收益不是“再增加一个功能模块”，而是让 Launcher 从：

> **一个速度很快的本地搜索/执行器**

升级成：

> **真正理解用户意图、上下文和习惯的本地 Launcher。**

而 Plugin SDK 则作为第二主线同步建设，因为它可以把你现有的 Plugin Runtime 真正转化成生态能力。

---

# 二十九、一个重要的路线原则

以后每个大版本都保持：

```text
Foundation
   ↓
Capability
   ↓
Productization
   ↓
Ecosystem
```

不要再出现：

```text
AI
 ↓
发现缺 Plugin SDK
 ↓
发现缺 Durable State
 ↓
发现缺 Search Contract
 ↓
回头补基础设施
```

所以：

**Launcher 1.0 = Core Product**

**Launcher 2.0 = Intelligent Launcher**

**Launcher 3.0 = Launcher Platform**

**Launcher 4.0 = Ecosystem / Enterprise**

这个分层与目前已经完成的架构非常匹配，也能继续利用你已经冻结的 `ActionResolver → launcher-action(ActionEngine) → Effect Executor`、Generation Matrix、Runtime/Session/Execution 三层身份和 Plugin/MCP 隔离模型，而不需要在进入 AI/生态阶段重新推翻底层。
