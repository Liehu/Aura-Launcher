这批 **P2.1 Foundation Batch 1** 的取舍是合理的，而且比一次性把 Search/Indexer/Identity 全部重构掉更稳。

从你给出的完成记录看，我会把当前状态调整为：

```text
P2.1 Foundation Batch 1
        ✅ PASS

已完成：
├── Empty Query / Recent
├── Application Identity v1
├── Path Identity normalization
├── RankingWeights
├── Search regression corpus v1
└── Search/Indexer invariants

未完成但已正确挂账：
├── IndexCoordinator
├── Watcher
├── Coalescer
├── Incremental Index
├── Typed SearchIdentity
├── SearchCoordinator
└── SearchProvider v2
```

尤其是你这次**没有提前做 Typed SearchIdentity + Provider trait 全量换型**，我认为是正确的。因为目前应用归并和路径规范化已经先把最重要的 identity 语义问题探索出来，再进行统一类型化，可以减少一次返工。

---

# 一、Empty Query：应该直接视为 P2.1 正式能力

这是这批里面产品价值最高的改动之一。

原来：

```text
Alt+Space
↓
空输入
↓
No Results
```

现在：

```text
Alt+Space
↓
Recent / Frequent
↓
Enter
↓
Execute
```

对于 Launcher 来说，这个变化很重要，因为很多时候用户根本不需要搜索，只想：

```text
打开最近使用的程序
重复刚才的操作
快速打开常用工具
```

你采用：

```text
history usage
+
live catalog
```

重新 join，而不是直接把历史记录当作可执行对象，我认为这个设计正确。

---

# 二、这里建议再冻结一个 invariant

现在应该正式增加：

```text
INV-SEARCH-003

Empty-query results MUST resolve against the current live catalog.
Historical records alone MUST NOT constitute executable authority.
```

也就是说：

```text
History
   ↓
identity
   ↓
current catalog
   ↓
current Action
```

而不是：

```text
History
   ↓
old action descriptor
   ↓
execute
```

这对插件尤其重要。

例如：

```text
昨天：
plugin.foo.calculate
```

今天插件已经：

```text
disabled
version changed
action removed
```

空查询仍然可以显示历史名称，但最终执行必须经过当前 catalog / resolver。

---

# 三、Application Identity v1：方向正确

你这次做到：

```text
Chrome.lnk
+
chrome.exe
```

归并成：

```text
Chrome
```

这是非常好的第一阶段 identity。

而且：

> Start Menu 显示名优先。

这个产品策略很合理。

用户看到：

```text
Google Chrome
```

而不是：

```text
chrome.exe
```

这是 UI/identity 两层兼顾。

---

# 四、但不要把当前“exe identity”过早定义成最终 ApplicationId

你现在实际上是：

```text
Shortcut
 ↓
resolved executable
 ↓
normalized exe identity
```

这可以叫：

```text
Application Identity v1
```

但不要马上冻结成：

```text
“Executable path == Application identity”
```

因为以后：

```text
MSIX
UWP
AUMID
Portable
same executable, different launch configuration
```

都会出现。

所以建议现在保持抽象：

```text
ApplicationIdentity
```

内部暂时：

```text
ExecutableIdentity
```

未来可以：

```text
AUMID
PackageIdentity
ExecutableIdentity
PortableIdentity
```

都映射进：

```text
ApplicationIdentity
```

这正好符合你当前“先语义、后 typed model”的策略。

---

# 五、应用归并一定要保留 Source Information

你现在做：

```text
Chrome.lnk
chrome.exe
    ↓
Chrome
```

很好。

但 ApplicationRecord 不应该只留下：

```text
path = chrome.exe
```

应该保留：

```text
sources:
    StartMenu
    Registry
```

或者：

```text
discovery_sources
```

因为以后：

```text
用户卸载 Chrome
```

可能发生：

```text
Registry source disappears
StartMenu stale entry remains
```

如果没有 source information，会很难判断：

```text
“这个应用是不是还存在？”
```

---

# 六、应用 Identity 合并还需要一个“冲突”概念

未来可能遇到：

```text
Foo.exe
```

同时有：

```text
Foo.lnk
arguments = --profile=A

Foo.lnk
arguments = --profile=B
```

它们：

```text
same executable
```

但实际上：

```text
different launch targets
```

这时不要简单：

```text
merge everything
```

建议后面区分：

```text
ApplicationIdentity
+
LaunchVariant
```

例如：

```text
Chrome
  ├── Default
  ├── Work
  └── Incognito
```

这属于以后 Action/shortcut enhancement，现在不用做。

但 **data model 最好不要把这些语义抹掉**。

---

# 七、Path Identity normalization：这一项我非常认可

你明确使用：

```text
lowercase
separator normalization
..
collapse
```

并且：

> **不访问磁盘。**

这非常重要。

千万不要这里直接：

```text
canonicalize()
```

或者：

```text
realpath()
```

因为一旦访问 filesystem：

```text
identity resolution
```

就可能变成：

```text
junction traversal
symlink resolution
I/O
```

你现在保持：

```text
lexical only
```

是正确的。

---

# 八、但路径规范化建议定义 Windows-specific rules

目前至少应该明确：

```text
C:\Foo
c:/foo
```

相等。

同时：

```text
C:\Foo\
C:\Foo
```

相等。

另外：

```text
C:\Foo\..\Bar
C:\Bar
```

相等。

但是：

```text
\\server\share\foo
```

应该与：

```text
C:\foo
```

保持完全不同 identity。

以及：

```text
\\?\C:\foo
```

是否与：

```text
C:\foo
```

相等，需要正式定义。

这一块非常容易在以后 FileId / Watcher 中产生 bug。

---

# 九、RankingWeights：这次抽象值得保留

你把：

```text
magic numbers
```

收成：

```text
RankingWeights
```

很好。

更重要的是：

> **default 值不改变。**

这使得：

```text
P2.1 structural refactor
```

不会突然变成：

```text
ranking behavior change
```

这对于 regression 非常重要。

---

# 十、下一阶段不要马上做“更聪明的 ranking”

现在应该保持：

```text
lexical
+
type
+
history
+
context
```

等：

```text
SearchIdentity
Incremental Index
Evaluation Corpus
```

稳定以后，再调权重。

否则：

```text
ranking changes
+
candidate changes
+
identity changes
```

一起发生，会非常难定位问题。

---

# 十一、Search corpus v1 已经有价值，但下一步要升级为“Golden Dataset”

当前：

```text
mixed-provider
Top-1
Duplicate Rate
```

是不错的起点。

下一步应该增加：

```text
application
file
folder
plugin
command
mixed
path
empty-query
history
```

例如：

```json id="p2corpus"
{
  "query": "vsc",
  "expected_top": ["application:visual-studio-code"],
  "max_rank": 1
}
```

并逐步支持：

```text
expected_top
acceptable_set
forbidden_duplicates
```

这样不用要求每个 query 只有唯一正确答案。

---

# 十二、Duplicate Rate = 0 还不够

例如：

```text
Chrome
Chromium
Chrome Uninstall
```

三个都不同，没有重复。

但是：

```text
Chrome Uninstall
```

如果排第一：

```text
DuplicateRate = 0
```

仍然是坏 ranking。

因此 Corpus 后面需要：

```text
Top-1 accuracy
Top-3 recall
MRR
NDCG
```

不用一次全部实现。

第一步：

```text
Top-1
Top-3
DuplicateRate
```

即可。

---

# 十三、现在最重要的一步：Typed SearchIdentity

这一项仍然应该做，但你目前选择后置是对的。

原因是：

你现在已经通过：

```text
Application identity v1
Path identity
```

探索出了实际语义。

下一阶段可以正式冻结：

```rust
pub enum SearchIdentity {
    Application(ApplicationId),
    File(FileId),
    Folder(FolderId),
    Command(CommandId),
    PluginCommand(PluginCommandId),
    System(SystemObjectId),
}
```

但不要让这个 enum 直接决定 provider 实现。

---

# 十四、Identity 的推荐分层

最终：

```text
RawCandidate
    ↓
IdentityResolver
    ↓
SearchIdentity
    ↓
CandidateView
```

其中：

```text
RawCandidate
```

可以来自：

```text
Start Menu
Registry
SQLite
Plugin
MCP
```

而：

```text
SearchIdentity
```

是统一语义。

---

# 十五、Application Identity 不要放进 launcher-search

建议：

```text
launcher-domain
    ApplicationId
    FileId
    FolderId
    SearchIdentity

launcher-providers
    Application discovery
    Application identity resolution

launcher-search
    dedup/ranking
```

这样 Search 不需要知道：

```text
StartMenu
Registry
MSIX
```

具体是什么。

---

# 十六、P2.1 最大工作量已经非常明确：Incremental Index

你自己估计：

```text
1000+ lines
20+ tests
```

我认为这个估算是合理的。

而且不要一次把：

```text
Watcher
Queue
Coalescer
Indexer
SQLite
Recovery
Generation
Health
```

全部揉成一块。

建议拆成：

```text
P2.1-B1
FileChange model

P2.1-B2
Watcher

P2.1-B3
Bounded queue

P2.1-B4
Coalescer

P2.1-B5
Incremental writer

P2.1-B6
Overflow recovery

P2.1-B7
Index health
```

---

# 十七、我建议 Watcher 第一版直接采用“事实源”思想

Watcher event：

```text
CREATE
MODIFY
DELETE
RENAME
```

只是：

> **提示 indexer 去检查真实 filesystem state。**

不要把 event 当成：

```text
“数据库应该是什么”
```

而是：

```text
event
 ↓
filesystem re-stat
 ↓
database truth
```

这可以抵御：

```text
event reorder
missing event
race
```

---

# 十八、例如 CREATE

收到：

```text
Created(A)
```

不要直接：

```sql
INSERT A
```

而是：

```text
stat(A)
 ↓
still exists?
 ↓
yes
 ↓
extract metadata
 ↓
upsert
```

如果已经不存在：

```text
no-op
```

---

# 十九、DELETE 也是一样

收到：

```text
Deleted(A)
```

可以：

```sql
DELETE A
```

但如果：

```text
A recreated immediately
```

事件顺序混乱，就应该依靠：

```text
filesystem truth
```

重新检查。

---

# 二十、Rename 要特别谨慎

建议第一版：

```text
RENAMED(old, new)
```

处理成：

```text
DELETE old
UPSERT new
```

这是最简单可靠的。

不要现在为了保留 FileId 引入：

```text
Windows File ID
NTFS identity
USN Journal
```

这些属于以后优化。

---

# 二十一、Watcher overflow：这一项一定要设计好

Windows watcher 最危险的不是普通事件，而是：

```text
buffer overflow
```

发生之后：

```text
你不知道漏了哪些事件。
```

所以必须：

```text
Overflow
 ↓
mark root Dirty
 ↓
rescan root
```

这一步是 correctness requirement。

---

# 二十二、Dirty Root 不应该触发全盘 rebuild

例如：

```text
D:\Projects
```

overflow。

应该：

```text
rescan D:\Projects
```

而不是：

```text
rebuild entire C/D index
```

这会决定以后大规模文件库的性能。

---

# 二十三、Watcher Queue 必须 bounded

这个必须坚持你整个项目的统一原则：

```text
bounded
```

比如：

```text
10k
```

达到上限：

```text
coalesce
+
DirtyRoot
```

而不是：

```text
VecDeque 无限增长
```

---

# 二十四、Rebuild 与 Watcher 的并行问题

这是下一批最值得提前设计的部分。

生命周期：

```text
Rebuild
   +
Watcher
```

不能简单：

```text
rebuild begins
→ stop watcher
```

否则 rebuild 期间文件变化全部丢失。

也不能：

```text
rebuild writer
+
incremental writer
```

并行写 SQLite。

推荐：

```text
Watcher
   ↓
bounded event queue
       │
       ├── rebuild running
       │      ↓
       │   accumulate
       │
       └── rebuild complete
              ↓
          replay/coalesce
              ↓
           current
```

---

# 二十五、Index Generation 现在值得一并设计

例如：

```text
generation = 42
```

Rebuild：

```text
42 → 43
```

Watcher batch：

```text
43 → 44
```

Search cache：

```text
query + generation
```

这样以后：

```text
Index changed
```

无需手工到处 invalidate cache。

---

# 二十六、Search cache 现在可以暂时不做，但 Generation 要预留

这两件事不是一回事：

```text
Query Cache
    later

IndexGeneration
    now
```

我建议 generation 现在就进入 Index metadata。

---

# 二十七、Index Health 也应该在这一阶段形成

至少：

```rust
pub enum IndexStatus {
    Empty,
    Ready,
    Updating,
    Rebuilding,
    Degraded,
    Failed,
}
```

以及：

```text
indexed_entries
pending_events
last_success
last_error
generation
```

---

# 二十八、Empty Query + Index Status 的组合要特别测试

例如：

```text
Launcher starts
↓
Index empty
↓
background rebuild
```

此时空查询应该：

```text
Recent / Apps / Commands
```

仍然能够工作。

而文件：

```text
file results
```

可能提示：

```text
Indexing…
```

而不是整个 Launcher：

```text
No results
```

---

# 二十九、App Index 不应该依赖 File Index

一个常见错误是：

```text
Application
    ↓
File index
```

这样：

```text
index unavailable
```

应用搜索也没了。

应该：

```text
Application Index
    independent

File Index
    independent
```

两者最后汇总到 Search。

---

# 三十、P2.1 需要开始考虑 Provider Health

现在 Provider：

```text
Apps
Files
Plugins
MCP
Context
Recent
```

最好都有：

```text
Ready
Updating
Unavailable
Degraded
```

这样 SearchCoordinator 可以：

```text
provider failure
→ partial results
```

而不是：

```text
one provider failure
→ entire search failure
```

---

# 三十一、一个值得加入的 Search policy

```text
INV-SEARCH-004

Failure or unavailability of one provider MUST NOT discard successful
results from unrelated providers unless the query explicitly targets
that provider.
```

例如：

```text
MCP provider dead
```

不能导致：

```text
Chrome
VS Code
Downloads
```

全部没了。

---

# 三十二、P2.1 的错误处理要开始区分

建议：

```text
ProviderError
IndexError
IdentityError
RankingError
QueryError
```

而：

```text
SearchError
```

作为上层 aggregate。

这样 diagnostics 才能知道：

```text
File index failed
```

还是：

```text
Plugin provider unavailable
```

而不是：

```text
Search failed
```

---

# 三十三、你当前 P2.1 的最大潜在返工点

不是 Watcher。

是：

> **Application identity 与未来 typed SearchIdentity 接口之间的映射。**

现在最好不要在 application provider 中硬编码：

```text
ApplicationId = exe path
```

而是：

```text
ApplicationIdentityResolver
```

哪怕内部现在只有：

```text
ExecutableIdentity
```

也把接口先留出来。

这样 MSIX/UWP 将来不会改整个 Search API。

---

# 三十四、P2.1 不建议现在增加 MSIX/UWP

你挂账这个决定仍然正确。

因为：

```text
MSIX
AUMID
Package Family
```

会让 ApplicationIdentity 复杂度明显上升。

等：

```text
ApplicationIdentity v1
```

和：

```text
typed SearchIdentity
```

稳定之后再加。

---

# 三十五、Portable Apps 同理

Portable App 最大的问题不是扫描 `.exe`。

而是：

```text
哪个 exe 是用户认为的“应用”？
```

例如：

```text
D:\Tools\foo.exe
D:\Tools\bar.exe
```

没有：

```text
publisher
installer
AUMID
registry
```

所以后面需要一个：

```text
PortableApp heuristic
```

但现在不值得混入。

---

# 三十六、P2.1 我建议增加一个“Search Explain”第一版

不用现在做 UI。

只做：

```rust
pub struct RankingExplanation {
    pub features: Vec<(Feature, f32)>,
}
```

例如：

```text
Chrome
exact       +40
prefix      +20
history     +12
type         +5
----------------
77
```

后面你调 Ranking 时会非常有用。

---

# 三十七、当前 Batch 1 的测试量

你这次：

```text
520 tests
```

没有问题。

但下一批不要追求：

```text
+100 tests
```

而应该追求：

```text
每个新增 invariant
→ 至少一个 regression test
```

尤其：

```text
Identity
Watcher
Budget
Recovery
```

测试质量比数量重要。

---

# 三十八、我建议下一批直接定义为

# P2.1-B：Incremental Index Engine

只做：

```text
FileChange
Watcher
BoundedQueue
Coalescer
IndexCoordinator
IncrementalWriter
DirtyRoot
OverflowRecovery
IndexGeneration
IndexHealth
```

不做：

```text
MSIX
UWP
Portable
Icon cache
Context ranking
ML
```

---

# 三十九、P2.1-B 的状态机

建议：

```text
Stopped
   ↓
Starting
   ↓
Ready
   │
   ├── Updating
   │      ↓
   │     Ready
   │
   ├── Rebuilding
   │      ↓
   │     ReplayPending
   │      ↓
   │     Updating
   │      ↓
   │     Ready
   │
   └── Degraded
          ↓
       Repairing
          ↓
        Ready
```

---

# 四十、P2.1-B 最关键的 invariant

```text
INV-INDEX-005
Watcher queue is bounded.

INV-INDEX-006
Watcher overflow marks the affected root dirty and triggers bounded
recovery.

INV-INDEX-007
Incremental updates do not require a full index rebuild.

INV-INDEX-008
Rebuild and incremental writes are serialized through one writer boundary.

INV-INDEX-009
The index remains queryable while background maintenance is running.

INV-INDEX-010
Index updates are eventually consistent with filesystem state.
```

---

# 四十一、P2.1-B 最关键的 E2E

### Create

```text
A
↓
create B
↓
search B
```

### Delete

```text
A
↓
delete A
↓
search A
```

### Rename

```text
A
↓
rename A → B
↓
search A = none
search B = found
```

### Burst

```text
create/modify × 1000
↓
bounded queue
↓
coalesce
↓
correct final index
```

### Overflow

```text
overflow
↓
dirty root
↓
rescan
↓
correct
```

### Rebuild overlap

```text
rebuild
+
events
↓
replay
↓
correct final state
```

---

# 四十二、P2.1-B 性能测试

不要先测试：

```text
million files
```

先用：

```text
10k
100k
500k
```

测试：

```text
initial rebuild
incremental 1 file
incremental burst 1000
rename 1000
delete 1000
overflow repair
```

---

# 四十三、一个非常重要的性能指标：Watcher-to-Search Latency

定义：

```text
file created
    ↓
watch event
    ↓
index update
    ↓
search sees it
```

测：

```text
p50
p95
p99
```

这个才是真正的“实时搜索”。

---

# 四十四、另一个性能指标：Startup-to-Searchable

现在你已经解决：

```text
startup blocking rebuild
```

下一步测：

```text
process start
 ↓
window visible
 ↓
search usable
```

不要：

```text
window visible
```

但：

```text
search provider blocked
```

---

# 四十五、当前项目到这里的路线实际上已经很清晰

```text
P2.1 Batch 1
✅ Identity foundation
✅ Empty query
✅ Ranking abstraction
✅ Search regression foundation

P2.1 Batch 2
→ Incremental Index
→ Watcher
→ Coalescer
→ Recovery

P2.1 Batch 3
→ Typed SearchIdentity
→ SearchCoordinator
→ Provider v2
→ Global ranking/dedup

P2.1 Batch 4
→ Application Identity 2.0
→ MSIX/UWP
→ Portable
→ performance hardening
```

这比“一口气完成整个 P2.1”稳得多。

---

# 四十六、我建议目前不要做这几个东西

明确冻结：

```text
❌ Agent Confirmation
❌ Agent memory
❌ Workflow loops
❌ MCP 新协议
❌ Embedding
❌ ML ranking
❌ Full text search
❌ OCR
❌ Cloud
```

甚至：

```text
❌ Icon Cache
❌ Query Cache
❌ Favorites
```

都可以等 Search/Index 核心稳定。

---

# 四十七、Batch 1 的最终验收

按照你这批实际完成内容，我建议正式记录：

```text
P2.1 Foundation Batch 1

Status: ✅ ACCEPTED

Accepted:
  Empty-query recent/frequent
  Application identity v1
  Lexical path normalization
  RankingWeights
  Search corpus v1
  Search/index invariants

Deferred:
  Incremental watcher/index
  Typed SearchIdentity
  SearchCoordinator
  Provider contract v2
  MSIX/UWP
  Portable Apps
  Icon cache
  Query cache
  Context ranking
  Favorites
  Performance thresholds
```

这个状态是健康的。

---

# 四十八、我会特别建议你现在正式冻结两个概念

### `Application Identity v1`

```text
Discovery Record
       ↓
resolved executable
       ↓
normalized executable identity
       ↓
Application
```

### `File Identity v1`

```text
filesystem path
       ↓
lexical normalization
       ↓
FileId
```

以后：

```text
Application Identity v2
```

再扩展：

```text
AUMID
MSIX
Portable
LaunchVariant
```

而：

```text
File Identity v2
```

以后再考虑：

```text
volume serial
file ID
USN
```

这样不会过早设计复杂 identity。

---

# 四十九、现在最值得做的不是下一次“大批功能”

而是直接进入：

> **P2.1 Batch 2 — Incremental Index Engine**

其目标只定义成：

```text
现有 SQLite index
        +
Windows filesystem change stream
        ↓
bounded / coalesced / recoverable
        ↓
最终与 filesystem eventually consistent
```

完成后再做 Typed SearchIdentity。

这样顺序就严格符合你这次已经实践验证的原则：

```text
Identity semantics
    ↓
Index lifecycle
    ↓
Typed contract
    ↓
Global Search
    ↓
Ranking optimization
```

**这条顺序我建议继续保持，不要反过来。**

目前这批 520 tests / zero warnings / release gate pass，可以作为 **P2.1 Foundation Batch 1 checkpoint**；下一批只聚焦 Incremental Index Engine，会比继续扩充 Application Provider 或 Ranking 更有价值。
