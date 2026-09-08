我把仓库当前 `main`、1.0 冻结文档、原始设计规范、测试方案、Reliability 记录，以及你上传的 Post-1.0 Roadmap 交叉看了一遍。

先给结论：

> **Aura-Launcher 现在已经可以认为“1.0 核心架构/核心产品完成”，但还不宜把它描述成“所有原始验收项 100% 完成的最终 1.0”。**
>
> 更准确的状态是：**1.0 RC1 / Architecture & Core Product Complete，存在少量 Release/QA/文档一致性尾项。**
>
> 我会给当前版本：
> **核心产品完成度 92~95%**
> **严格对照最初 v0.1 Test Plan：约 80~85%**
> **面向 2.x 演进的架构成熟度：明显高于普通 MVP。**

当前 `main` 的最新提交本身已经明确标记为 **Native Launcher 1.0.0 RC1**，提交说明写的是 17 crates + 8 apps、Plugin/Command/Action/Workflow/UI 契约冻结，以及 623 tests green、zero warnings、topology OK。

---

# 1. 先看：现在这个项目到底是什么状态

我认为现在已经不能再按 README 的 `v0.1 MVP` 来理解项目。

仓库实际已经进入：

```text
v0.1 MVP
   ↓
MVP2.x
   ↓
MVP3.x
   ↓
MVP4.x
   ↓
P2.3 Reliability
   ↓
Launcher 1.0 RC1
```

而且已经出现了非常完整的横向契约体系：

```text
Command
Action
Workflow
Plugin
UI
MCP
Identity
Generation
Authority
Lifecycle
Recovery
```

同时有：

```text
launcher-domain
launcher-search
launcher-indexer
launcher-context
launcher-action
launcher-plugin-host
launcher-runtime
launcher-core
launcher-workflow
launcher-ai
launcher-mcp
launcher-providers
launcher-ui
```

这已经不是“一个 launcher demo”，而是一个比较明确的 **Launcher Platform Core**。

原始设计里最核心的抽象：

```text
Trigger
  ↓
Query
  ↓
Candidate
  ↓
Command
  ↓
Action
  ↓
Effect
```

仍然是现在系统的核心，而且 ActionResolver → ActionEngine 被明确作为唯一执行权边界。原始设计也明确要求 Provider 负责发现、Action Engine 负责执行、插件不能直接进入 Core。

现在的架构实际上已经把这条链做得更完整：

```text
Producer
(UI / Workflow / AI / Plugin / MCP)
        ↓
ActionProposal / ActionDescriptor
        ↓
ActionResolver
        ↓
launcher-action::validate()
        ↓
launcher-action::execute()
        ↓
Effect
```

而横向契约文档明确规定 Search / Cache / Favorites / Context / Icon 都只能提供信号，不能产生执行 authority。

这一点我认为是这个项目目前**最有价值的架构成果之一**。

---

# 2. 原始 1.0 目标，实际上完成得怎么样？

## 2.1 Launcher Core —— ✅ 基本完成

原始 MVP 的最低闭环是：

```text
Hotkey
 → Native popup
 → Search app/file
 → Show result
 → Keyboard select
 → Execute action
 → Context-aware command
 → External plugin response
```

这个垂直闭环已经存在，而且真实 GUI E2E 做过验证。

现在的 `launcher-app` 已经包含：

```text
Global Hotkey
Popup
Search
Keyboard navigation
Enter
Esc
Tray
Context
Action
Plugin
Workflow
MCP
```

并且 UI 不做 IO、后台执行再回事件循环，这符合原始架构要求。

**结论：✅**

---

# 3. File Index —— ✅ 已经超过最初 MVP，但还有明显技术债

这块已经不是单纯 Phase 1 了。

当前实际上已经有：

```text
SQLite
WAL
Generation
Incremental Batch
Watcher
DirtyRoot
Bounded Queue
Overflow Recovery
Subtree Rescan
Reparse Point 防护
```

源码里已经存在：

```rust
pub mod coordinator;
pub mod incremental;
pub mod watcher;
```

并且 `apply_batch()` 按 commit 成功后递增 generation，`rescan_root()` 做有界子树恢复。

同时 P2.3 Reliability 已经记录：

```text
index.db
→ corrupt
→ delete/rebuild
```

并且已经建立多层 persistence recovery。

所以如果按照原始设计：

> Phase 1：目录扫描 + SQLite

已经完成。

而实际还做到了：

> Incremental Index + Watcher + Recovery

**结论：✅**

---

# 4. 但这里有一个文档/实现不一致，应该马上修

原始设计写的是：

```text
Search = SQLite FTS5 + 自定义 ranking
```

但现在 `launcher-indexer` 的实现实际是：

```sql
lower(name) LIKE ?
OR lower(path) LIKE ?
```

然后排序。

也就是说：

> **当前实现不是 FTS5。**

源码就是直接 `LIKE` 搜索。

这不一定是技术错误。

对 Launcher 来说：

```text
214k files
→ LIKE
→ P95 1.17ms
```

如果实测真的稳定，其实完全可以作为 1.0 实现。

但是现在文档不能继续写：

> SQLite FTS5 已经是实际架构。

应该改成：

```text
1.0:
SQLite metadata + bounded LIKE search + deterministic ranking

2.x:
FTS5 / richer text index / content index
```

否则 Agentic Coding 后面很容易产生：

```text
“设计要求 FTS5”
→ agent 重新实现 Indexer
→ 无意义架构 churn
```

这是现在比较典型的**文档漂移点**。

---

# 5. Application Catalog —— ⚠️ “持久化完成”，但还没有真正成为 Search 的 authoritative source

这是我认为当前 1.0 最值得注意的地方。

你们已经有：

```text
AppRegistryProvider
    +
Packaged
    +
Portable
        ↓
Merged Application Entries
        ↓
catalog.db
```

`launcher-app` 中已经明确调用：

```rust
CatalogStore::open(...)
store.reconcile(...)
```

并写入 `catalog.db`。

`CatalogStore` 也已经具备：

```text
catalog_meta
application
generation
reconcile()
list()
```

所以：

### “Persistent Application Catalog”

**已经实现。**

但是现在 Search 的实际来源仍然是：

```text
AppRegistryProvider
↓
in-memory AppEntry
↓
Core
```

而不是：

```text
catalog.db
↓
ApplicationCatalogProvider
↓
Core
```

甚至 `CatalogStore` 自己的注释就明确说：

> persistent catalog 不是 search-time dependency。

所以当前状态应该标成：

```text
Catalog persistence   ✅
Catalog generation    ✅
Catalog recovery      ✅
Catalog as Search SoT ⚠️
```

这正好说明你提供的 Post-1.0 Roadmap 中：

> **P2.4-A Persistent Application Catalog 2.0**

为什么合理。Roadmap 希望最终变成：

```text
Application Sources
      ↓
ApplicationDiscovery
      ↓
ApplicationCatalogCoordinator
      ↓
Persistent Catalog
      ↓
Search
```



我的判断是：

> **P2.4-A 不是“1.0 没做完”，而是“1.0 做出了 persistence layer，2.x 要把它升级成真正 authoritative catalog”。**

这是一个很重要的区别。

---

# 6. Search —— ✅ 基础架构完成，2.x 才真正值得升级

目前 Search 已经做得比普通 Launcher MVP 强很多。

已经有：

```text
Normalization
Exact
Prefix
Contains
Subsequence
Multi-token
Type prior
Usage boost
Recency
Favorites
Context
Identity dedup
Candidate merge
Cache
Generation
```

例如：

```text
Application > Folder > Command > Plugin > File
```

还有：

```text
Exact > Prefix > Contains > Fuzzy
```

以及：

```text
frequency
recency
favorite
context
```

源码中的 `RankingWeights` 已经把这些变成显式权重。

而且做了一个非常正确的约束：

> boost 只能重新排序已有候选，不能凭空制造候选。

这对 Launcher 的搜索质量非常重要。

---

## 但 Search 现在仍然不是“Search Intelligence”

现在更接近：

```text
Query
 ↓
All providers
 ↓
Deterministic lexical scoring
 ↓
Merge
 ↓
Rank
```

而不是：

```text
Query
 ↓
Intent
 ↓
Strategy
 ↓
Candidate selection
 ↓
Context
 ↓
Personal ranking
 ↓
Result
```

所以你上传的路线图把：

```text
P2.5 Search Intelligence
```

放在 Foundation 后面，我完全赞同。

---

# 7. Search 的另一个问题：当前 Provider fan-out 仍然是 sequential

`Core::search()` 目前是：

```rust
for p in self.providers.iter_mut() {
    all.extend(p.query(&q));
}
```

也就是 provider 顺序执行。

注释也明确写了：

> Sequential keeps results deterministic.

这在现在的 Provider 数量和本地 IO 模型下是可以接受的。

但到了：

```text
100+ plugin
MCP
HTTP
AI
cloud
```

以后就绝对不够。

所以 P2.5-F：

```text
SearchRequestId
provider timeout isolation
partial results
SingleFlight
query cache
```

是正确方向。

---

# 8. Context —— ✅ MVP 完成

现在已经实际做到了：

```text
Foreground App
Explorer Current Folder
ContextSnapshot
ContextGeneration
Quick Switch
Open Terminal Here
Copy Folder Path
```

并且 Context 只影响 ranking / candidate selection，不直接产生执行权。

当前缺：

```text
selected items
非 Explorer foreground context
clipboard context
更丰富 session context
```

这和 roadmap 的：

```text
Time
Recent Activity
Recent Files
Current Window
Current Selection
Session State
```

是自然的下一步。

**结论：✅ 1.0**

---

# 9. Plugin —— ✅ 这是目前完成度最高的模块之一

这一部分已经远远超过最初 MVP。

当前已有：

```text
Plugin Manifest
Version negotiation
JSON-RPC
query_id
ExecutionId
RuntimeId
Capability declaration
Timeout
Bounded IO
Frame limit
Flood truncation
Crash isolation
Process tree isolation
Idle shutdown
Quarantine
Persistent registry
Backup / Restore
Python SDK
Rust SDK
Reference Plugin
Contract Test Kit
```

Plugin Provider 甚至已经有：

```text
protocol_failures
quarantined
disabled
registry
spawn cooldown
execute_action
```

Plugin Registry 又已经是独立 persistence layer：

```text
plugins.db
enabled
quarantined
failures
capabilities
backup
restore
corrupt quarantine
```

这基本已经符合：

> “Plugin Control Plane”

而不是简单的“插件加载器”。

**结论：✅**

---

# 10. Plugin SDK —— ✅ 基础已经有，生态化还没完成

这里要区分：

### SDK 技术能力

已经有：

```text
Rust Reference Plugin
Python SDK
Contract Test Kit
E2E
```

所以：

**SDK runtime layer = ✅**

但 roadmap 所说：

```text
plugin init
plugin build
plugin package
plugin install
plugin debug
plugin logs
plugin inspect
```

这样的完整开发体验，目前还没有看到同等成熟度的独立 CLI 产品。

所以我会评为：

```text
Plugin API         ✅
Plugin SDK         ✅
Reference Plugin   ✅
Conformance Kit    ✅
Developer CLI      ⚠️
Developer UX       ⚠️
Marketplace        ❌
```

这正好对应你 Roadmap 的：

```text
P2.4-D
P2.4-E
```



---

# 11. Action / Security —— ✅ 架构已经基本冻结

这里我认为已经可以停止大规模设计。

现在的 authority chain 已经非常清晰：

```text
Producer
 ↓
Resolver
 ↓
validate
 ↓
execute
 ↓
Effect
```

并且：

```text
AI ≠ authority
Plugin metadata ≠ authority
MCP metadata ≠ authority
Cache ≠ authority
Context ≠ authority
```

这个设计在你们的 cross-cutting contract 里已经明确冻结。

所以后面：

> **不要再重构 Authority Model。**

最多只扩展：

```text
ActionType
Capability
Resolver
Effect
```

---

# 12. Workflow —— ⚠️ 技术基础已经非常强，但产品化还没有完成

Workflow 目前实际上已经进入：

```text
Definition
↓
Runner
↓
Step
↓
Failure classification
↓
Resolver
↓
Confirmation
↓
Pause
↓
Resume
↓
Runtime Surface
```

而且已经做了 Workflow + AI Proposal + Runtime Surface。

当前 release gate 甚至已经覆盖 MCP / Workflow / AI E2E。

但是还没有到你 Roadmap 中真正的：

```text
Visual Editor
Parallel / Join
Subworkflow
Human Approval
Durable Workflow
```

所以：

```text
Workflow Core       ✅
Workflow Contract   ✅
Runtime              ✅
UI Surface           ✅
Visual Editor        ❌
DAG                  ❌
Durable Execution    ❌
```

这正是：

> **P2.6**

而不是 1.0 bug。



---

# 13. AI —— 目前已经不是“没有 AI”，而是“AI Foundation 已经搭好”

这一点和 README 的叙述其实有点脱节。

当前已经存在：

```text
launcher-ai
LlmProvider
LLMPlanner
ActionProposal
catalog projection
proposal validation
fake authority field rejection
```

README 也明确写了 LLM planner 是：

> proposal-only，不能获得 executor/authority。

而 `launcher-core/catalog.rs` 已经可以把 MCP tool projection 成统一 ActionCatalog。

所以现在实际上是：

```text
AI Foundation             ✅
AI Query Understanding    ⚠️
AI Action Proposal        ✅
AI Agent product          ❌
```

这与 Roadmap：

```text
P2.7-A
P2.7-B
P2.7-C
P2.7-D
P2.7-E
```

基本完全吻合。

---

# 14. MCP —— 甚至已经开始超过 1.0 必需范围

当前已经：

```text
stdio
streamable HTTP
protocol profile
Tool Catalog
McpProvider
McpExecutor
security transport
compatibility matrix
E2E
workflow integration
AI integration
```

这也是为什么当前 release gate 已经专门存在：

```text
MCP compatibility
MCP E2E
AI-MCP E2E
MCP soak
```

所以我不会把 MCP 当成“1.0 还欠的基础能力”。

---

# 15. Reliability —— ✅ 很强，但测试计划还没有完全兑现

这里是当前最明显的“代码完成度 > 测试完成度”。

P2.3-C 已经实现：

```text
index recovery
favorites recovery
catalog recovery
plugin recovery
config recovery
icon recovery
startup crash loop
degraded boot
```

这已经相当不错。

但原始测试计划要求的内容更多：

```text
1000 UI loops
100,000 random queries
1000 plugin spawn/query/shutdown
1000 mock providers
100 concurrent queries
DB lock
DB corruption
Indexer crash restart
Explorer disappears
permission denied
```

原始测试方案明确写了这些验收要求。

而当前 `TESTING.md` 自己也承认仍有：

```text
hotkey latency instrumentation
10,000 show/hide soak
1000 UI loop
IME composition
DB concurrent read/write
DB corruption injection
Indexer crash restart automation
Pinyin
```

未完整覆盖。

所以：

> **代码层已经接近 1.0；QA 层没有 100% 关闭最初 Test Plan。**

这是我最主要的保留意见。

---

# 16. 当前最大的真实问题，其实不是代码，而是“文档状态漂移”

这是我认为现在最需要处理的事情。

## 16.1 README 仍叫 v0.1 MVP

README 开头写：

```text
Native Launcher (v0.1 MVP)
```

而同一个 HEAD 已经是：

```text
Native Launcher 1.0.0 RC1
```

这在 Agentic Coding 项目里是危险的。

Agent 会产生这种判断：

```text
README = v0.1
architecture = v0.1
release commit = 1.0
roadmap = 2.x

到底哪个是真的？
```

---

## 16.2 测试数量也出现明显不一致

不同文档分别出现：

```text
62/62
355
582
623
```

例如：

* README：62/62

* TESTING：开头仍写 62/62

* P2.3-C：582

* 最新 RC1 commit：623

这说明：

> **Documentation Consistency Gate 自己没有真正做到“全项目 single source of truth”。**

这是 1.0 我认为必须修掉的尾项。

---

# 17. 我会怎样重新定义当前 1.0

不要再把它定义为：

> “所有东西都完成了。”

应该冻结成：

## Launcher 1.0 — Core Product Complete

### Closed

```text
✅ Native Launcher Core
✅ Global Hotkey
✅ Native UI
✅ Application Discovery
✅ Persistent Application Catalog
✅ Incremental File Index
✅ Search + Ranking
✅ Favorites
✅ History
✅ Context Engine
✅ Action Engine
✅ External Plugin Runtime
✅ Plugin Registry
✅ Plugin Recovery
✅ Workflow Runtime Foundation
✅ AI Proposal Foundation
✅ MCP Adapter Foundation
✅ Performance Baseline
✅ Security Boundary
✅ Recovery Framework
```

### Core Slice / Deferred

```text
⚠️ Catalog authoritative read path
⚠️ Hotkey latency measurement
⚠️ Long UI soak
⚠️ Advanced search semantics
⚠️ Plugin developer tooling
⚠️ Workflow editor
⚠️ Durable workflow
⚠️ Agent runtime productization
⚠️ Advanced Windows integration
⚠️ Full file intelligence
```

### Experimental

```text
🧪 AI Agent
🧪 Advanced MCP orchestration
```

### Planned

```text
📌 Search Intelligence
📌 Plugin Ecosystem
📌 Agent
📌 Durable Workflow
📌 Marketplace
📌 Cloud
```

这其实与你上传的 Post-1.0 roadmap 非常一致。Roadmap 已经明确把 1.0 之后定义为 **Capability + Platform evolution**，而不是继续堆 P2.4/P2.5 功能。

---

# 18. 所以后续开发，我不建议直接进入 P2.5

我会稍微修改你现有 Roadmap 的顺序。

你原来的：

```text
1.0
 ↓
P2.4 Foundation
 ↓
P2.5 Search
 ↓
P2.6 Workflow
 ↓
P2.7 AI
```

方向没问题。

但结合现在真实代码，我建议实际执行：

```text
1.0 RC1
  │
  ▼
1.0 GA Closure
  │
  ├── Documentation Freeze
  ├── Release/QA Closure
  ├── Hotkey benchmark
  ├── UI soak
  ├── Indexer restart fault injection
  └── packaging/signing
  │
  ▼
P2.4 Foundation
  │
  ├── Catalog 2.0
  ├── Search Contract v2
  ├── Plugin SDK/CLI
  └── Plugin Dev Diagnostics
  │
  ▼
P2.5 Search Intelligence
  │
  ├── Intent
  ├── Context
  ├── Personal Ranking
  ├── Candidate Merge v2
  └── Explainability
  │
  ▼
P2.6 Workflow 2.0
  │
  ├── Editor
  ├── DAG
  ├── Approval
  └── Durable Runtime
  │
  ▼
P2.7 AI / Agent
  │
  ├── Query Understanding
  ├── Action Proposal
  ├── Agent
  └── Agent ↔ Workflow
  │
  ▼
P2.8 Windows Intelligence
  │
  ▼
P2.9 File Intelligence
  │
  ▼
3.x Platform
```

---

# 19. 其中最重要的一条：2.x 不要先做 AI

这一点我和你上传的 Roadmap 的判断一致。

现在项目最大的资产不是：

```text
AI
```

而是：

```text
Index
Catalog
Identity
Context
Ranking
Action
Resolver
Effect
Workflow
Plugin
MCP
Generation
Lifecycle
Recovery
```

这些已经形成了一套相对完整的 **execution substrate**。

所以真正正确的演进应该是：

```text
1.0
 ↓
Search Intelligence
 ↓
Plugin Ecosystem
 ↓
Workflow Runtime
 ↓
AI
 ↓
Agent
```

而不是：

```text
1.0
 ↓
LLM
 ↓
Agent
 ↓
发现 Search 不够
 ↓
发现 Catalog 不够
 ↓
发现 Workflow 不够
 ↓
重新改底层
```

你上传的路线图本身也明确提出这个原则：先 Foundation，再 Capability，再 Productization，再 Ecosystem。

---

# 20. 我最终给这个项目的评审结论

| 领域                    | 当前判断                 |
| --------------------- | -------------------- |
| Launcher Core         | ✅ Closed             |
| Native UI             | ✅ Closed             |
| Application discovery | ✅ Closed             |
| Persistent Catalog    | ✅ Core Slice → 2.0继续 |
| File Index            | ✅ Closed             |
| Incremental Index     | ✅ Closed             |
| Search Core           | ✅ Closed             |
| Search Intelligence   | 📌 2.x               |
| Context               | ✅ Closed             |
| Action / Authority    | ✅ Frozen             |
| Plugin Runtime        | ✅ Closed             |
| Plugin Registry       | ✅ Closed             |
| Plugin SDK            | ✅ Core / ⚠️ tooling  |
| Workflow Core         | ✅                    |
| Workflow Product      | 📌 2.x               |
| AI Foundation         | ✅                    |
| AI Product            | 📌 2.x               |
| MCP                   | ✅ Foundation         |
| Recovery              | ✅ Core               |
| Performance           | ✅ 基线                 |
| Release QA            | ⚠️ 尚有尾项              |
| Documentation         | ⚠️ 明显需要统一            |
| Packaging             | ⚠️ RC，签名仍未完成         |

所以我会正式把当前版本定义为：

> **Launcher 1.0 RC1：核心产品完成、核心架构冻结、平台基础设施基本完成；剩余主要是 Release Closure，而不是核心架构缺陷。**

最新 RC1 commit 明确标记为 `Native Launcher 1.0.0 RC1`，并声称 623 tests、zero warnings、topology pass；但仓库内旧测试文档仍保留 62/62、582 等历史数字，因此首先应该做一次 **1.0 Documentation/Release Closure**，再进入 P2.4。

## 我建议下一步

现在最有价值的不是继续零散评审代码，而是直接把现有项目**冻结成一份 `Launcher 1.0 Final Audit`**：逐条把原始 `01-design-spec`、`03-test-plan`、P2.3/1.0 契约和最新代码映射成 `Closed / Core Slice / Experimental / Planned / Failed`，同时列出**必须在 1.0 GA 前修复的项目**和**可以正式推迟到 P2.4 的项目**。这样之后 Agentic Coding 就有一份真正的“1.0 baseline”。
