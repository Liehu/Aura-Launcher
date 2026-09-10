我把 `crates.zip` 里的代码按 **架构边界 → P1-B/C/D/E → P2 核心 → 安全/可靠性 → 测试覆盖** 交叉看了一遍。

总体判断：

> **这批代码的架构方向是对的，但目前不建议直接把它标成“P1/P2 全部完成”。**
>
> P1-B、P1-C 的基础语义基本成立；P1-D 有几处实际实现缺陷，且目前看**还没有真正接入完整的 Agent Runtime 闭环**；P2 已经做出了第一批纵向切片，但 Search/Indexer/App/History/Plugin/Packaging 距离我们之前定义的 Launcher 1.0 仍有明显缺口。

更重要的是，我发现了 **4 个应立即修正的问题**，其中 2 个我会列为高优先级安全/语义问题。

---

# 1. 总体评估

我按现在代码给一个实际状态：

| 模块                          | 当前判断                    |
| --------------------------- | ----------------------- |
| P1-B Persistent MCP Session | ✅ 基本完成                  |
| P1-C Workflow v0.2          | ✅ 基本完成                  |
| P1-D Agent Runtime          | 🔴 未达到完成标准              |
| P1-E UI Contract / 基础 UI    | 🟡 基础完成，QA 未完整          |
| P2 Search Core              | 🟡 有基础，但还不是 2.0         |
| P2 App Search               | 🟡 部分完成                 |
| P2 File/Folder Search       | 🟡 基础实现                 |
| P2 Ranking/History          | 🟡 基础完成，有 bug/设计缺口      |
| P2 Action                   | 🟡 基础完成                 |
| P2 Plugin                   | 🟡 基础 Plugin 已有，生态平台未完成 |
| P2 Windows Integration      | 🟡 初步                   |
| P2 Settings                 | 🟡 初步                   |
| P2 Performance              | 🟡 有测试，但缺产品级基准          |
| P2 Packaging                | ❌ 未完成                   |
| P2 App Index 2.0            | ❌ 未完成                   |

所以目前更像：

```text
P1 Architecture Foundation
        ✅

Launcher Core v0.x
        🟡
        ↓
还需要一轮“收口修复 + 产品化”
        ↓
Launcher 1.0
```

---

# 2. 第一处必须立即修：Plugin ExecutionId

这是我认为这份代码里最值得优先处理的问题。

## 当前链路

`Core::execute_effect()` 已经拿到了：

```rust
execution_id: &str
```

而且此前你已经冻结：

> **每个 execution attempt 由上层一次性铸造 ExecutionId，executor 不得重新铸造。**

但实际 Plugin 路径是：

```text
WorkflowRunner
   ↓
CoreWorkflowBackend
   ↓
core.next_execution_id()
   ↓
Core::execute_effect(..., execution_id)
   ↓
execute_plugin_action(...)
   ↓
PluginProvider::execute_action(...)
   ↓
PluginHandle::execute_action(...)
   ↓
next_execution_id()       ← 又重新生成
```

也就是说：

```text
Workflow ExecutionId = E1
Plugin RPC ExecutionId = E1' / E2 / ...
```

目前 `launcher-plugin-host/src/lib.rs` 里还有一个：

```rust
static EXEC_SEQ: AtomicU64
```

并由：

```rust
fn next_execution_id()
```

自己产生 ExecutionId。

### 这违反现有不变量

你此前已经明确冻结：

```text
INV-AUTH-005
每个 attempt exactly one execution_id
```

以及 `Core::execute_effect()` 自己的注释也明确写着：

> registry never re-mints。

但是 Plugin 路径实际上重新 mint 了。

### 正确改法

应该一路透传：

```rust
execute_plugin_action(
    plugin_id,
    action_id,
    input,
    execution_id,
    context_generation,
)
```

然后：

```rust
Provider::execute_action(
    action_id,
    input,
    execution_id,
    context_generation,
)
```

再到：

```rust
PluginHandle::execute_action(
    action_id,
    input,
    execution_id,
    context_generation,
)
```

最终 RPC：

```json
{
  "execution_id": "e-17"
}
```

整个链路只出现一个：

```text
E17
```

### 优先级

**P0 修复级别。**

在修掉之前，我不会建议把 P1-D / Plugin 执行链正式 freeze。

---

# 3. 第二处必须立即修：Agent Runtime 目前有真实逻辑 bug

`launcher-ai/src/agent.rs` 里：

```rust
budget.record_replan();
replans += 1;
status = AgentRunStatus::Replanning;
replans += 1;
```

`replans` **加了两次**。

所以：

```text
实际 replans = 1
diagnostics.replans = 2
```

而 `BudgetController` 又只增加一次：

```rust
budget.record_replan();
```

导致：

```text
AgentRunOutcome.replans
```

和：

```text
BudgetController.replans
```

不一致。

这不是风格问题，是明确的计数 bug。

### 修复

删除其中一次：

```rust
budget.record_replan();
replans += 1;
status = AgentRunStatus::Replanning;
```

更进一步：

> **不要同时维护两套 replan counter。**

`AgentRunOutcome` 应直接来自：

```rust
budget.replans()
```

这样不会再出现：

```text
local counter
+
budget counter
```

漂移。

---

# 4. 更重要：P1-D 其实还没有形成真正的 Agent 闭环

这比上面的计数 bug 更值得注意。

`run_bounded_agent()` 当前：

```text
Observe
    ↓
catalog()
    ↓
Plan
    ↓
Proposal
    ↓
Execute
    ↓
failure
    ↓
new catalog()
    ↓
Plan
```

问题是：

### Agent 的 planner 实际只收到

```rust
planner.plan(goal, &catalog)
```

没有：

```text
ExecutionResult
Observation
ReplanReason
previous turn
```

也就是说当前 Planner 看不到：

```text
“上一轮执行失败了什么？”
“为什么失败？”
“执行结果是什么？”
```

它得到的基本还是：

```text
goal + catalog
```

因此当前所谓：

```text
Replan
```

实际上更像：

```text
重新调用 planner
```

而不是我们之前定义的：

```text
Observe Result
→ Understand Changed State
→ Replan
```

---

# 5. `Observation` 数据结构已经写了，但没有真正进入核心循环

你已经有：

```rust
Observation
ExecutionSummary
AgentTurn
```

而且 `observation.rs` 里还有：

```rust
recent_executions
```

但 `run_bounded_agent()` 并没有真正：

```text
ExecutionRecord
     ↓
Observation
     ↓
Planner
```

这就是目前 P1-D 的最大结构缺口。

换句话说：

```text
数据结构已经有
```

但是：

```text
runtime semantics 尚未完全接上线
```

---

# 6. Agent 的 Confirmation 也没有形成完整闭环

现在：

```rust
AgentExecutionHost::confirmation_required()
```

默认：

```rust
false
```

而 `CoreAgentHost` **没有 override**。

于是 Agent Runtime 自己认为：

```text
no confirmation
```

然后执行。

后面实际上又进入：

```text
WorkflowRunner
```

由 Workflow 的 confirmation 机制把它挡住。

最终结果更接近：

```text
Agent
 → request
 → Workflow paused
 → Agent receives execution failure
```

而不是：

```text
Agent
 → WaitingForConfirmation
 → UI confirmation
 → Resume AgentRun
 → fresh resolve
 → execute
```

这意味着此前 P1-D 设计里的：

```text
WaitingForConfirmation
```

目前从架构上存在，但**没有真正完成端到端接线**。

---

# 7. 我建议 P1-D 必须增加一个真正 E2E

现在必须有：

```text
Agent Goal
   ↓
Observe
   ↓
Plan A
   ↓
Execute A
   ↓
A fails
   ↓
Observation(result=A failed)
   ↓
Replan
   ↓
Plan B
   ↓
Execute B
   ↓
Complete
```

测试必须断言：

```text
turns = 2
replans = 1
executions = 2
E1 != E2
```

还要明确：

```text
E1 never replayed
```

再做一个：

```text
Agent
 ↓
destructive proposal
 ↓
WaitingForConfirmation
 ↓
Confirm
 ↓
fresh resolve
 ↓
Enew
```

否则 P1-D 实际还是“Planner + bounded loop”，还不是完整 Agent Runtime。

---

# 8. 第三个比较严重的问题：Search 实际没有 Dedup

`launcher-search/src/lib.rs` 的注释写的是：

> Deduplicate by `(provider_id, id)`

但 `rank_with_boost()` 实际做的是：

```rust
for ...
commands.retain(...)
commands.sort(...)
commands.truncate(...)
```

**没有 dedup。**

而现有测试：

```rust
cmd("a", "test")
Command {
    provider_id: "files",
    ..cmd("a", "test")
}
```

故意使用了：

```text
(apps, a)
(files, a)
```

这本来就是两个不同 identity，因此这个测试其实不能证明 dedup。

### 这是一个真实产品问题

应用可能同时来自：

```text
Start Menu
Registry
Portable
Shortcut
Executable
```

最终 Search 可能出现：

```text
Chrome
Chrome
Chrome
Chrome
```

所以 P2 必须增加：

```text
SearchIdentity
```

至少：

```text
Application → canonical application identity
File        → normalized absolute path
Folder      → normalized absolute path
Command     → provider + command
Plugin      → provider + command
```

然后：

```text
candidate generation
 → canonical dedup
 → ranking
 → Top-K
```

---

# 9. Search 还有一个更重要的问题：Provider 截断发生得太早

当前 FileProvider：

```rust
const MAX_FILE_HITS: usize = 30;
```

然后：

```text
FileProvider
  → top 30
  → Core global ranking
  → top 12
```

如果某个真正应该排 Top-3 的文件在 FileProvider 本地排序里是第 31：

```text
它永远到不了 global ranker。
```

这对于以后引入：

```text
History
Context
Application prior
Path matching
```

会越来越明显。

更合理：

```text
Provider retrieval
       ↓
bounded but sufficiently large candidate set
       ↓
global normalization
       ↓
dedup
       ↓
ranking
       ↓
Top-K
```

---

# 10. Search 现在实际上没有 Path Search

Indexer 支持：

```sql
lower(name) LIKE ...
```

只搜索：

```text
name
```

不搜索：

```text
path
parent
extension
```

而用户的典型行为包括：

```text
D:\Projects\Foo
```

或者：

```text
projects
```

或者：

```text
2026\report
```

当前 FileProvider 找不到：

```text
path match
```

所以我认为：

> **P2 Search 2.0 还没有完成。**

至少应该增加：

```text
name score
path score
extension score
directory score
```

而不是单纯 filename substring。

---

# 11. Indexer 发现一个真正的资源限制 bug

`launcher-indexer/src/lib.rs`：

```rust
pub const MAX_INDEX_ENTRIES: i64 = 500_000;
```

但 `scan_into()` 的：

```rust
count
```

是**每次递归调用局部变量**。

例如：

```text
root
 ├── dir1 → 300k
 ├── dir2 → 300k
 ├── dir3 → 300k
```

每个递归：

```text
count < 500k
```

都能继续。

所以：

```text
MAX_INDEX_ENTRIES = 500k
```

实际上不是整个 rebuild 的全局上限。

更严重的是多 root：

```text
for root in roots {
    count += scan_into(...)
}
```

limit 也没有真正 global enforcement。

### 这与项目一贯的 bounded design 是冲突的

应该改为共享：

```rust
ScanBudget {
    entries
}
```

或者直接把：

```text
remaining
```

作为参数向递归传递。

最终保证：

```text
整个 rebuild
≤ MAX_INDEX_ENTRIES
```

---

# 12. Indexer 的 Reparse Point 防护现在不成立

这里：

```rust
fn is_junction_like(_p: &Path) -> bool {
    false
}
```

实际上相当于：

```text
reparse/junction detection = disabled
```

注释说：

> depth bound and entry limit protect against cycles

但是：

```text
depth bound
```

并不等于：

```text
不会反复扫描巨大的其他树
```

例如一个 junction 指向很大的系统目录：

```text
D:\A\junction → C:\Windows
```

你仍然可能扫描大量内容，直到 depth/entry budget。

结合上一条“全局 entry limit 实际失效”，这里的风险更明显。

### 建议

P2 File Indexer 必须正式处理：

```text
reparse point
junction
symlink
```

至少：

```text
default: do not follow
```

这是 Windows 文件索引器应该采用的更稳妥默认。

---

# 13. `record_use()` 有明确重复插入 bug

`launcher-indexer/src/lib.rs`：

```rust
pub fn record_use(...) {
    self.conn.execute(
        "INSERT INTO history(...)",
        ...
    )?;

    self.record_use_titled(...)?;
}
```

也就是说：

```text
record_use()
```

会：

```text
INSERT
+
record_use_titled()
  → INSERT
```

一次调用变成两条 history。

虽然你当前 Core 的 P2 路径使用的是：

```text
record_use_with_title()
```

所以新路径绕开了这个 bug。

但这是一个明确的 API defect。

应该直接：

```rust
record_use(...)
    → record_use_titled(..., "")
```

不要前面再 INSERT 一次。

---

# 14. History 当前还缺一个产品级策略

现在记录逻辑主要是：

```text
successful execution
→ history
```

方向正确。

但需要明确三类：

```text
Rankable
NonRankable
Sensitive
```

例如：

```text
Password generator
Token
Credential-related action
```

不应该因为用户用了 100 次，就在普通搜索里无限 boost。

另外至少测试：

```text
success → recorded
failure → not recorded
```

---

# 15. App Registry 目前只是“第一代 App Provider”

你目前实现了：

```text
Start Menu .lnk
+
Uninstall Registry
```

这是有用的，但还不是我们定义的 App Index 2.0。

现在：

```text
.lnk
```

只是被当作：

```text
path = xxx.lnk
```

而不是：

```text
.lnk
 ↓
target executable
 ↓
ApplicationIdentity
```

所以：

```text
Run as Administrator
```

对 Start Menu shortcut 基本无法出现，因为：

```rust
path.ends_with(".exe")
```

才添加 RunAsAdmin。

---

# 16. App Dedup 也还不是真正的 App Dedup

现在 dedup：

```rust
(name.to_lowercase(), path.to_lowercase())
```

因此：

```text
Chrome
C:\...\chrome.exe
```

和：

```text
Google Chrome
C:\...\chrome.exe
```

不会按 application identity 合并。

更典型：

```text
Chrome.lnk
C:\Program Files\Google\Chrome\Application\chrome.exe
```

会变成两个对象。

所以 P2 App Index 2.0 必须加入：

```text
ApplicationIdentity
```

---

# 17. RecentFilesProvider 目前名义上是 Recent，实际排序不是 Recent

这里：

```rust
out.sort_by_key(|e| e.name.to_lowercase());
```

直接按：

```text
name
```

排序。

这意味着：

```text
Recent
```

实际上并没有：

```text
recent time
```

排序。

另外 test 已经表明：

```text
lnk.link_target()
```

当前 fixture 下是：

```text
None
```

于是：

```text
target = .lnk
```

这意味着近期文件 Action 可能最终操作的是：

```text
foo.docx.lnk
```

而不是：

```text
foo.docx
```

所以：

> **RecentFilesProvider 目前更像“Recent shortcut catalog”，还不是完整 Recent Files。**

---

# 18. Plugin Platform：基础运行层有，但生态层还没有

当前 `PluginProvider + PluginHost + Plugin API` 已经能做：

```text
Manifest
Spawn
Initialize
Query
Action
Timeout
Crash isolation
Python runtime
```

这是基础 Plugin MVP。

但此前 P2-F 定义的：

```text
Install
Enable
Disable
Update
Repair
Compatibility
Plugin config
SDK CLI
```

在这个 zip 中基本看不到完整实现。

因此我会判定：

```text
P2 Plugin Runtime
✅

P2 Plugin Ecosystem
❌ / partial
```

---

# 19. PluginProvider 的身份设计目前有一点别扭

```rust
fn id(&self) -> &str {
    "plugin"
}
```

真正 Command：

```text
provider_id = "plugin:<manifest.id>"
```

而 Core resolver 又需要：

```text
p.id()
+
p.plugin_identity()
```

去做匹配。

目前能工作，但意味着：

```text
Provider identity
```

和：

```text
Command identity
```

不是同一个概念。

建议以后正式区分：

```text
ProviderKind = Plugin
ProviderInstanceId = plugin:<id>
```

否则插件数量增加后：

```text
plugin
plugin
plugin
```

在 diagnostics / registry / provider lifecycle 里会比较 awkward。

---

# 20. P2 Settings 目前基本还是配置 DTO，不是 Settings 系统

现在 `AppConfig` 有：

```text
hotkey
theme
autostart
index_dirs
result_limit
python_path
mcp
```

这是基础配置。

但是之前定义的 Settings 需要：

```text
providers
search paths
hotkey
theme
history
index
plugins
startup
behavior
```

并且：

```text
schema version
migration
reset
validation
```

目前还没有形成完整的：

```text
ConfigService
ConfigSchemaVersion
Migration
AtomicSave
```

---

# 21. Config Save 还有可靠性问题

当前：

```rust
std::fs::write(path, toml_text)?;
```

直接覆盖。

桌面软件在：

```text
写配置过程中 crash
断电
杀进程
```

可能出现：

```text
half-written config
```

建议改为：

```text
write temp
 → flush
 → rename atomically
```

并考虑：

```text
config.bak
```

尤其因为：

```text
load_or_create()
```

遇到解析错误会：

```text
warn
→ AppConfig::default()
```

虽然不会覆盖原文件，但用户实际上会看到：

```text
所有设置恢复默认
```

这会很危险。

更好的策略：

```text
invalid config
→ preserve file
→ rename to config.invalid-<timestamp>
→ generate defaults
```

至少要告诉用户发生过 migration/recovery。

---

# 22. P2 Windows Integration：目前是“基础 Windows 能力”，还不是 Windows Integration

目前确认已有：

```text
Multi-monitor popup placement
CreateMutex
RunAsAdmin
Copy path
```

这些都值得保留。

但还缺：

```text
Windows Settings
Control Panel
System tools
Shell verbs
Open With
Uninstall
AppUserModelID
MSIX/UWP
```

因此这里我仍然建议保留：

```text
P2-G
Windows Integration
```

作为未完成的大块。

---

# 23. RunAsAdmin 需要补测试

目前我没有看到足够的 Windows E2E 覆盖：

```text
RunAsAdmin
```

至少需要验证：

```text
.exe
space path
Unicode path
long path
quoted path
```

以及最关键的：

```text
ActionResolver
→ confirmation
→ ActionEngine
→ ShellExecute("runas")
```

不能出现：

```text
RunAsAdmin
```

成为一条绕开 Confirmation 的特殊执行路径。

---

# 24. Search Quality Corpus 是非常好的东西，但现在还不够

`launcher-core/tests/quality.rs` 是目前 P2 中我比较认可的一部分。

你已经有：

```text
Top-1
Top-3
Type Prior
Determinism
```

这是正确方向。

但还应加入：

```text
path match
history boost
dedup
plugin hint
application identity
unicode normalization
typo/fuzzy
extension
folder match
```

以及：

```text
MRR
Top-1
Top-3
Top-5
```

而不是只有几个手写 query。

---

# 25. 一个非常值得增加的 Ranking Regression

现在 Search Ranking 有：

```text
Exact
Prefix
Contains
Fuzzy
Type Prior
Plugin hint
History
```

建议专门测：

```text
Exact > Prefix > Contains > Fuzzy
```

在所有情况下仍然成立。

例如：

```text
exact + no history
```

vs：

```text
prefix + huge history
```

要定义：

```text
history 能不能跨 match class？
```

你当前设计说：

> 只在已匹配候选里 boost。

这个原则很好，但还应该正式写测试矩阵。

---

# 26. 一个架构层面的建议：开始引入 Canonical Identity

现在很多地方还在使用：

```text
String id
String path
provider_id
command_id
```

随着：

```text
History
Ranking
App
Plugin
Workflow
Agent
```

越来越复杂，会出现很多：

```text
"这个 String 到底是什么身份？"
```

我建议 P2 后半程正式增加：

```rust
ApplicationId
FileId
FolderId
ProviderId
CommandId
ActionId
PluginId
```

尤其：

```text
SearchIdentity
```

应该成为 Search / History / Favorites / Ranking 的统一锚点。

这会显著降低以后复杂度。

---

# 27. 目前测试体系最大的缺口

你的测试数量已经不少，但存在一个很明显的问题：

> **单元测试很多，真实产品路径的 E2E 不够。**

目前尤其缺：

```text
User
 ↓
Hotkey
 ↓
Search
 ↓
Select
 ↓
Action
 ↓
History
 ↓
Ranking next query
```

这个真实闭环。

以及：

```text
User
 ↓
file search
 ↓
Copy Path
 ↓
history
```

```text
User
 ↓
plugin search
 ↓
plugin action
 ↓
plugin process crash
 ↓
search again
```

---

# 28. 我建议新增一个“Launcher Product E2E”

这个非常重要。

例如定义：

```text
PRODUCT-E2E-001
```

完整验证：

```text
Create test catalog
    ↓
search
    ↓
rank
    ↓
select
    ↓
resolve action
    ↓
execute
    ↓
record history
    ↓
repeat search
    ↓
history boost
```

最终必须验证：

```text
第一次
Chrome rank #3

使用 Chrome

第二次
Chrome rank #1
```

这样才真正证明：

```text
Search + Action + History + Ranking
```

是一个闭环。

---

# 29. 性能目前还不能算 P2-I 完成

你现在有：

```text
100 providers
1000 commands
20 rounds
<50ms average
```

这个测试很好，但它只是：

```text
synthetic provider stress
```

真正 Windows Launcher 需要另外测试：

```text
5000 apps
100k files
500k files
20 plugins
MCP provider
history 10k
```

至少测：

```text
cold startup
warm startup
popup show
search p50
search p95
action panel
memory RSS
handle count
thread count
```

尤其你之前的定位是：

> **低内存 Launcher**

所以 P2-I 必须最终有：

```text
Memory Baseline
```

---

# 30. 还有一个重要缺口：Indexer 的“增量更新”

现在：

```rust
rebuild()
```

是：

```text
DELETE FROM files
    ↓
重新扫描
```

这对于：

```text
100k
500k
```

文件规模并不适合作为长期运行方案。

你之前的产品方向明显需要：

```text
initial scan
+
incremental updates
+
watch
```

因此我建议把：

```text
P2-M Advanced Indexing
```

提高优先级。

甚至可以拆：

```text
P2-C.1 Initial Index
P2-C.2 Incremental Index
P2-C.3 Watcher
P2-C.4 Repair
```

---

# 31. Windows File Watcher 最终应该成为必须项

推荐：

```text
ReadDirectoryChangesW
```

形成：

```text
Filesystem
   ↓
Change Event
   ↓
Indexer Queue
   ↓
SQLite Upsert/Delete
```

而不是：

```text
每次启动
→ full rebuild
```

这对于你这种大量文件场景非常重要。

---

# 32. 目前我认为最值得新增的几个能力

综合代码之后，我会补充下面这些，按优先级排列。

## 必须补

```text
1. Canonical Search Identity
2. Global index entry budget
3. Reparse/junction handling
4. Plugin ExecutionId propagation
5. Agent real Observe→Result→Replan
6. Agent Confirmation/Resume E2E
7. Search dedup
8. Search path matching
9. Incremental file indexing
10. App .lnk → target resolution
11. Config atomic save/migration
12. Product E2E
13. Real memory/performance baseline
14. Installer / upgrade
```

---

# 33. 强烈推荐

```text
15. Search ranking explainability
16. Search evaluation corpus v2
17. History privacy classification
18. Recent-files real timestamp sorting
19. Plugin Enable/Disable
20. Plugin compatibility validation
21. Windows Settings provider
22. Windows system tools provider
23. Portable App provider
24. Application identity normalization
25. Safe Mode / Repair / Reset
26. Local diagnostics dashboard
```

---

# 34. 暂时完全不用增加

目前我反而建议**不要再加新架构**：

```text
❌ another AI layer
❌ another Agent abstraction
❌ another workflow engine
❌ another executor
❌ another plugin protocol
❌ distributed runtime
❌ cloud sync
❌ marketplace
```

你现在的问题不是能力不够，而是：

> **已有能力需要真正收敛成一个稳定的 Launcher。**

---

# 35. P1 我建议这样收口

## P1-B

```text
✅ close
```

只补：

```text
BusinessError → Session stays Ready
```

测试即可。

## P1-C

```text
✅ close
```

再补：

```text
branch convergence
output/transition transaction
```

## P1-D

```text
❌ do not close yet
```

先修：

```text
ExecutionId
replan counter
Observation integration
Confirmation lifecycle
real E2E
```

## P1-E

```text
🟡 close functional/UI contract
```

但把：

```text
DPI
keyboard walkthrough
memory soak
real Windows QA
```

列成 release candidate gate。

---

# 36. P2 则建议重新排序

根据代码现状，我会重新排：

```text
P2.1 Search Core
      ↓
P2.2 File Index / Incremental
      ↓
P2.3 Application Identity
      ↓
P2.4 Action 2.0
      ↓
P2.5 History / Ranking
      ↓
P2.6 Plugin Platform
      ↓
P2.7 Windows Integration
      ↓
P2.8 Settings / Migration
      ↓
P2.9 Performance / Memory
      ↓
P2.10 Packaging
```

其中：

```text
P2.1 → P2.5
```

属于绝对核心。

---

# 37. 最终我给当前代码的评级

如果以“工程架构质量”评价：

```text
Architecture       A-
Security model      A-
Contract discipline A
Testing discipline  B+
Search implementation B
Indexer              B-
App integration      B-
Agent runtime        C+
Productization       C
```

**这个 C+ 并不是说 Agent 设计差，而是目前实现和之前冻结的 Agent Contract 之间仍有明显差距。**

反过来看：

```text
P0
+
P1-B
+
P1-C
+
P2 Search/Action/Plugin 基础
```

这部分底座其实已经相当完整。

---

# 38. 现在最重要的不是继续写功能，而是修这 8 个点

我会建议下一轮 coding agent 直接执行：

```text
P1-FIX-01
Plugin ExecutionId single-source propagation

P1-FIX-02
Agent replan counter single-source

P1-FIX-03
Agent Observation → Planner integration

P1-FIX-04
Agent Confirmation/Resume E2E

P2-FIX-01
Global IndexEntry budget

P2-FIX-02
Search canonical dedup

P2-FIX-03
History double-insert

P2-FIX-04
File search candidate/path strategy
```

这 8 项完成后，再继续 P2。

---

# 39. 我现在需要你补的文件

这次 zip 已经足够让我发现不少问题，但要完成**最终产品级验收**，还缺一个关键部分：

```text
apps/launcher-app/
```

以及 workspace 顶层：

```text
Cargo.toml
Cargo.lock
```

另外最好给：

```text
scripts/
    check_topology.py
    release_gate.py
```

原因很简单：目前这份 zip 只能让我看到：

```text
crate 内部实现
```

但看不到：

```text
真正启动时注册了哪些 Provider
Indexer 是否实际启动
AppRegistryProvider 是否真的注册
RecentFilesProvider 是否真的注册
PluginProvider 是否如何发现
History 是不是实际接入
Hotkey → UI → Core → Action 的真实链路
P1-D AgentRuntime 是否实际接线
P1-E UI 是否实际使用新的 ViewModel
```

而这些恰恰决定：

> **“代码里实现了”是否等于“产品里真的可用”。**

所以目前我不会要求你继续做更多功能；**把 `apps/launcher-app + workspace Cargo + release gate` 给出来后，我建议直接做一次“Launcher 1.0 Release Audit”，把上述问题逐项定成 `BLOCKER / MUST / SHOULD / DEFERRED`，然后就可以进入最后的基础功能打磨，而不是继续扩张架构。**
