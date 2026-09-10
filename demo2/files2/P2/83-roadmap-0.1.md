这部分全部完成后，我建议**不要直接继续堆 P2.4、P2.5 功能**。此时产品应该进入一个明确的 **1.0 Release Closure → 2.x 能力演进**阶段。

我会把路线调整为下面这样：

```text
P1 收口
├─ Candidate Merge 完整版
├─ Icon UI
└─ Release 性能基线
        ↓
P2.1-D.1
└─ Persistent Application Catalog
        ↓
P2.2-D
└─ Plugin Ecosystem Management
        ↓
P2.2-E
└─ Installer / Upgrade / Recovery
        ↓
P2.3
└─ Launcher 1.0 Release Closure
        ↓
══════════════════════════════
       Launcher 1.0
══════════════════════════════
        ↓
P3.x 产品能力演进
```

## 一、P2.3 不应该再是“大功能批”

我建议把 **P2.3 定义为 1.0 Release Closure**，核心任务是收口，而不是继续增加能力。

### P2.3 应完成

| 领域          | 收口内容                                           |
| ----------- | ---------------------------------------------- |
| Contract    | 全部冻结契约重新审计                                     |
| Search      | Search Core 全链路 E2E                            |
| Action      | Resolver → ActionEngine 全链路审计                  |
| Plugin      | Runtime + Ecosystem 联调                         |
| MCP         | Production profile / failure recovery          |
| Workflow    | 当前支持范围正式定义                                     |
| Persistence | DB migration / recovery / backup               |
| Installer   | 安装、升级、卸载、回滚                                    |
| Crash       | 崩溃恢复、startup recovery                          |
| Performance | Release benchmark                              |
| Memory      | Idle / active / long-run soak                  |
| UI          | DPI / keyboard / VR                            |
| Security    | Capability / identity / execution lineage      |
| CI          | Release Gate 全部自动化                             |
| Docs        | Architecture / SDK / migration / operator docs |

尤其要做一次类似你之前 **P2.2-F** 的：

> **Cross-Cutting Invariant Audit**

但这次不是找“断点”，而是验证：

```text
Producer
  ↓
Resolver
  ↓
ActionEngine
  ↓
Effect Executor
  ↓
OS / Plugin / MCP
```

每条执行链有没有绕过冻结边界。

---

# 二、P2.3 完成后，我建议正式打 `Launcher 1.0`

这个节点非常重要。

不要出现：

```text
P2.3
P2.4
P2.5
P2.6
...
```

一直无限开发。

而是：

```text
Launcher 1.0
    │
    ├── Core
    ├── Plugin
    ├── MCP
    ├── Workflow
    ├── Windows Integration
    ├── Installer
    └── Recovery
```

形成一个稳定产品基线。

然后进入：

# 三、P3：Product Evolution

这时才开始做之前明确延期的高级能力。

我建议拆成 **四条主线**，而不是按功能想到什么做什么。

---

## P3-A：Search Intelligence

这是 1.0 之后最值得投入的方向。

当前搜索：

```text
Query
 ↓
Providers
 ↓
Candidate
 ↓
Rank
```

P3 可以演进成：

```text
Query
 ↓
Intent
 ↓
Context
 ↓
Candidate Generation
 ↓
Semantic / Behavioral Ranking
 ↓
Explanation
```

主要包括：

### A1 Query Intent

识别：

```text
"chrome"
→ Application

"open downloads"
→ Folder

"compress xxx.pdf"
→ Action

"restart computer"
→ System Action

"translate this"
→ Workflow / AI
```

### A2 Context-aware ranking

你已经有：

```text
ForegroundApp
CurrentFolder
ContextGeneration
```

后续可以加入：

```text
TimeOfDay
RecentAction
RecentFiles
UsagePattern
SessionContext
```

但仍然坚持：

> Context 是 ranking input，不是 authority。

### A3 Personalization

逐渐形成：

```text
UserState
    ↓
Usage History
    ↓
Ranking Features
    ↓
Personalized Ranking
```

这时候 `RankingGeneration` 就真正开始有意义。

---

# 四、P3-B：AI / Agent

你的现有 Agent Runtime 已经给未来留下接口，但**不要把 Agent 当成 1.0 核心能力继续硬塞进去**。

P3 再把它产品化。

建议路线：

```text
P3-B1 AI Query Understanding
        ↓
P3-B2 Action Proposal
        ↓
P3-B3 Confirmation UX
        ↓
P3-B4 Agent Runtime
        ↓
P3-B5 Replan
        ↓
P3-B6 Multi-step Agent
```

关键仍然坚持你已经冻结的权限链：

```text
AI / Agent
   ↓
Proposal
   ↓
ReferenceResolver
   ↓
ActionResolver
   ↓
ActionEngine
   ↓
Effect
```

而不是：

```text
AI
 ↓
Effect ❌
```

这一点不要因为进入 AI 阶段而改变。

---

# 五、P3-C：Workflow 2.0

你现在 Workflow 0.2 已经有：

* Variables
* Conditions
* Branch
* Output Binding
* Retry
* Failure Policy
* Reference Resolver

后面可以进入真正的 Workflow 产品化：

```text
Workflow Editor
       ↓
Workflow Definition
       ↓
Validation
       ↓
Workflow Runner
       ↓
Observability
```

然后再考虑：

```text
Workflow
  ↓
Agent
```

而不是让 Agent 自己随意创建不可控 Workflow。

### P3-C 可以加入

```text
Subworkflow
Parallel
Join
Timeout
Compensation
Human Approval
Persistent Run
Resume after Restart
Workflow History
```

其中 **Durable Workflow** 是真正的大版本能力，不建议在 1.0 前做。

---

# 六、P3-D：生态与云能力

Plugin Ecosystem 做完以后，下一层自然就是：

```text
Local Plugin
    ↓
Plugin SDK
    ↓
Plugin CLI
    ↓
Plugin Debug
    ↓
Plugin Repository
    ↓
Plugin Marketplace
```

再往后才是：

```text
Cloud Sync
Cloud Backup
Multi-device
Shared Workflow
Account
```

这属于明显的 **2.x / SaaS 化路线**，不要污染 1.0。

---

# 七、再往后是 Enterprise / Advanced

可以独立成 P4：

```text
P4
├─ Enterprise Policy
├─ Central Configuration
├─ Managed Plugins
├─ Audit
├─ RBAC
├─ Organization
└─ Fleet Management
```

如果产品定位一直是个人 Windows Launcher，这一层甚至可以长期不做。

---

# 最终路线我建议冻结成这样

```text
P1
│
├─ Core Closure
│
P2.1
│
├─ File Index
├─ Application Discovery
├─ Application Catalog
│
P2.2
│
├─ Favorites / Cache / Context
├─ Plugin Ecosystem
├─ Installer / Recovery
│
P2.3
│
└─ 1.0 Release Closure
       │
       ▼
════════════════════════════
        LAUNCHER 1.0
════════════════════════════
       │
       ├───────────────┐
       ▼               ▼
    P3-A             P3-B
 Search Intelligence   AI / Agent
       │               │
       └───────┬───────┘
               ▼
             P3-C
         Workflow 2.0
               │
               ▼
             P3-D
      Plugin Ecosystem 2.0
               │
               ▼
              P4
       Cloud / Enterprise
```

## 我尤其建议你把 P2.3 的定位钉死

**P2.3 = “1.0 产品化与发布收口”**

而不是：

**P2.3 = “再补一批功能”**

这样你的整个项目会形成非常清晰的边界：

> **P0/P1：架构与基础设施**
> **P2：核心 Launcher 能力**
> **P2.3：1.0 产品闭环**
> **P3：智能化与高级编排**
> **P4：平台化、云化、企业化**

这样做的最大好处是：**到了 P2.3，你实际上已经拥有一个可以独立发布、长期维护的 Launcher，而不是一个“永远还差最后几个功能”的半成品。**
