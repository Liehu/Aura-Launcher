有，而且我看完这三个文件后，整体评价是：**这个 MVP 已经不是“验证 UI 能不能跑”的 Demo，而是已经把下一阶段真正需要的数据和边界跑出来了。**尤其是 `launcher-app` 的 UI+Core 合并、独立 indexer、外部进程 Plugin Host、12.6 MB idle Private，以及完整的异常插件契约测试，都已经足够支撑第一次架构复盘。

我现在反而**不建议马上继续加 AI / Workflow / MCP / Python / WASM**。先把 MVP 的测量结果、边界和几个架构缺口补齐，再决定 v0.2。

## 一、首先：目前架构方向我认可，而且 MVP 结果很好

目前最让我满意的是这几个决定：

```text
launcher-app
  UI + Core
      │
      ├── Providers
      ├── Search
      ├── Context
      ├── Action
      └── Plugin Broker
              │
              └── external process

launcher-indexer-service
              │
              └── SQLite

plugin
              │
              └── external process
```

你没有为了“架构漂亮”而一开始把所有东西拆成 5～10 个常驻进程，这是对低内存 Launcher 非常重要的。

而且实际结果：

> **Idle Private ≈ 12.6 MB**

已经明显超过了最初设定的 `<50 MB target / <80 MB budget`。README 也确认了 62/62 测试通过，并且完成了真实 GUI E2E。

所以现在最重要的事情不是“再加功能”，而是：

> **证明 12.6 MB 是真实的、可重复的、可持续保持的。**

---

# 二、第一件必须补：把“性能数字”从一个数字升级成完整性能基线

目前 README 里已经有：

```text
app p95 ≤ 10 ms
file p95 ≤ 50 ms
idle memory = 12.6 MB
```

并且有 `launcher-bench` 做 baseline/regression。

但是对于这个项目，我认为还不够。

建议新增：

# `docs/PERFORMANCE-METHODOLOGY.md`

明确规定：

```text
Memory:
    Private Bytes
    Working Set
    Commit
    Peak Private Bytes
    Plugin Host Private Bytes

CPU:
    Idle CPU %
    Search CPU
    Indexing CPU

Latency:
    Hotkey -> popup visible
    popup visible -> first results
    keystroke -> result update
    Enter -> process launch
    plugin spawn -> first result
    index query
```

至少形成：

| Benchmark         | P50 | P95 | P99 |  Budget |
| ----------------- | --: | --: | --: | ------: |
| Hotkey → Popup    |     |     |     | ≤ 20 ms |
| Query → Result    |     |     |     | ≤ 10 ms |
| File Search       |     |     |     | ≤ 50 ms |
| Enter → Action    |     |     |     | ≤ 50 ms |
| Idle Private      |     |     |     | ≤ 50 MB |
| Idle CPU          |     |     |     |  ≤ 0.1% |
| Plugin cold start |     |     |     |     TBD |

尤其要加入：

### 1. Cold start

```text
Windows boot
    ↓
Launcher not running
    ↓
Ctrl+Space
    ↓
first pixel
```

### 2. Warm start

```text
Launcher resident
    ↓
Ctrl+Space
    ↓
first pixel
```

这两个数字会非常有价值。

---

# 三、第二个必须补：进程生命周期图

你现在 `ARCHITECTURE.md` 已经很好地记录了 Plugin 生命周期：

```text
Discovered
→ Validated
→ Spawned
→ Running
→ Idle
→ Kill
```

并且明确 2 秒默认 timeout、idle 自动退出。

但是整个系统还缺：

# `docs/LIFECYCLE.md`

至少画出：

```text
BOOT
 │
 ▼
Core Init
 │
 ├── Hotkey
 ├── Config
 ├── Providers
 └── Tray
 │
 ▼
IDLE
 │
 ├───────────────┐
 ▼               ▼
Hotkey          Indexer event
 │
 ▼
POPUP
 │
 ├── Search
 ├── Context
 └── Plugin
 │
 ▼
ACTION
 │
 ▼
POPUP HIDDEN
 │
 ▼
IDLE
```

因为以后你会遇到很多：

```text
Core shutdown while plugin running?
Indexer shutdown while query running?
UI closed during Action?
Plugin timeout while result callback pending?
Second Ctrl+Space while popup transition?
```

这些不是单元测试问题，而是**状态机问题**。

---

# 四、第三个：你现在最值得补的是并发模型

目前 AGENTS 里已经规定：

> UI thread 不做 IO/CPU，并发变化必须有 concurrency test。

这很好。

但是目前文档还没有把：

```text
谁拥有谁
谁可以并发
谁必须 serial
谁可以 cancel
谁可以 supersede
```

定义下来。

这是 Launcher 非常核心的地方。

比如用户输入：

```text
ch
chr
chro
chrom
chrome
```

理论上可能：

```text
Query #1 ────────────────→ Result
Query #2 ─────────→ Result
Query #3 ───────────────→ Result
Query #4 ────────→ Result
Query #5 ─────────────────→ Result
```

最后出现：

```text
chrome
↓
显示 chrome
↓
旧 query 的结果回来
↓
UI 又变回 chr
```

因此应该明确：

# Query Cancellation / Supersession

```text
QueryID = 101
QueryID = 102
QueryID = 103
```

UI 永远只接受：

```text
result.query_id == current_query_id
```

旧查询：

```text
cancel / ignore
```

这个机制我建议现在就加。

---

# 五、第四个：Plugin 安全模型还需要明显加强

目前已经有：

```text
Manifest
capabilities
timeout
idle timeout
result limit 100
crash isolation
```

这已经很好。

但目前安全规范还比较像：

> “防插件把 Core 弄崩”

而不是：

> “插件是一个不可信代码执行环境”。

建议增加：

# `docs/SECURITY.md`

尤其定义：

```text
Plugin Trust Level
```

例如：

```text
Trusted Builtin
        ↓
Signed Plugin
        ↓
Local User Plugin
        ↓
Untrusted Plugin
```

还有几个目前非常值得提前定义的东西：

### executable 路径

必须明确：

```text
plugin.json
executable = "xxx.exe"
```

只能：

```text
resolve(plugin_dir / executable)
```

不能：

```text
..\..\something.exe
C:\xxx.exe
\\server\xxx.exe
```

还要考虑：

```text
junction
symlink
reparse point
```

---

### 子进程环境

要明确插件继承什么：

```text
Environment
CurrentDirectory
PATH
TEMP
User profile
stdin/stdout/stderr
```

尤其是：

> 不要因为插件方便而把 Core 的完整环境无条件传进去。

---

### Plugin Kill

当前文档写的是 timeout/crash 自动 kill。

下一步应该明确：

```text
kill plugin
```

是不是只 Kill：

```text
plugin.exe
```

还是整个：

```text
process tree
```

否则以后 Python Plugin：

```text
plugin.exe
   └── python.exe
        └── child.exe
```

可能留下孤儿进程。

Windows 下这里建议尽早设计：

> **Job Object**

---

# 六、第五个：Capability 现在有“声明”，还应该验证“执行”

你的 Manifest 已经有：

```json
"capabilities": []
```

并且 Host 做 allow/deny 检查。

但我建议把架构进一步明确成：

```text
Manifest
   │
   ▼
Declared Capabilities
   │
   ▼
Granted Capabilities
   │
   ▼
Runtime Enforcement
```

不能只是：

```text
plugin.json 写：
network=true
```

然后系统“记住”它申请了 network。

必须最终能够做到：

```text
plugin
  ↓
network.request()
  ↓
Broker
  ↓
Capability Check
  ↓
ALLOW / DENY
```

否则 capability 最终只是 metadata。

这个问题现在不用实现，但必须写入 ADR。

---

# 七、第六个：Indexer 现在是 MVP 中另一个非常值得深入的东西

你已经实测：

> **21 万条文件**

而架构文档又说明独立 indexer 已经可以运行，并通过 stdio JSON-RPC 提供 `status/rebuild/search/shutdown`。

这很好。

但是我建议不要继续扩大功能，而是先验证：

```text
cold index
incremental index
large index
corruption recovery
database migration
```

尤其要做：

```text
100K
500K
1M
5M
10M files
```

因为：

> 21 万文件能工作 ≠ 500 万文件还能保持优秀体验。

你的项目以后真正可能吃内存的地方，不一定是 UI，反而可能是：

```text
file metadata
ranking cache
SQLite cache
FTS
path strings
icons
```

所以这一块值得单独压测。

---

# 八、第七个：现在缺一个“搜索质量”测试体系

目前测试已经覆盖：

```text
ranking
provider
app
file
recent
plugin
```

这是**功能正确性**。

但 Launcher 最核心的体验其实是：

> “我输入这个东西，它是不是马上把我想要的结果放第一位？”

所以建议增加：

# `docs/SEARCH-QUALITY.md`

准备固定 Query Corpus：

```text
chrome
ch
vscode
calc
notepad
git
terminal
doc
pdf
down
sett
control
```

每个 Query 定义：

```text
Expected Top 1
Expected Top 3
Expected acceptable results
```

例如：

```text
Query: "chrome"

Top1:
Google Chrome

Top3:
Google Chrome
Chrome Profile Manager
Chrome Cleanup
```

最终可以建立：

```text
Top1 Accuracy
Top3 Accuracy
MRR
```

这样以后你改 ranking algorithm，不会凭感觉判断“好像更准”。

---

# 九、第八个：Context 现在其实是 MVP 最大的功能缺口

README 明确写了：

> ContextSnapshot 已实现，但 Explorer 当前目录还只是 trait boundary，没有接真实前台窗口。

这是我认为下一版最值得做的 P1。

因为：

```text
Wox
Flow
Raycast
```

都很强，但真正能让这个项目和“搜索框”拉开差距的是：

# Context Awareness

最终：

```text
Explorer
C:\Projects\EAKIS
        │
        ▼
Ctrl+Space
        │
        ▼
> terminal
        │
        ▼
Open Terminal Here
```

而不是简单：

```text
> terminal
Windows Terminal
```

所以我建议：

> **v0.1.x 的第一个真实产品功能就做 Explorer Context，而不是 AI。**

---

# 十、还有一个我强烈建议现在补：Action 必须成为 Effect 的唯一入口

你已经有：

```text
launcher-action
```

而且 ARCHITECTURE 已经明确：

> Action Engine 是 effects 的唯一路径。

这个方向非常正确。

建议把它正式写成：

```text
Query
    ↓
Result
    ↓
Action
    ↓
Effect
```

而禁止：

```text
Provider
   ↓
直接 ShellExecute
```

或者：

```text
Plugin
   ↓
自己执行任意 action
```

未来所有：

```text
open
copy
move
delete
launch
shell
url
clipboard
notification
```

全部进入：

```text
Action Engine
```

这样未来 Workflow、AI、MCP 全部可以复用：

```text
AI
 ↓
Action
 ↓
Effect
```

这会成为整个项目非常重要的统一抽象。

---

# 十一、Agentic Coding 方面还缺一个东西：ADR 驱动

你现在 AGENTS 已经规定：

> 架构变化需要 ADR；public contract、DB schema、UI 技术、进程边界变化都需要 ADR。

这个非常好。

但是我建议把 Agent 规则进一步升级：

```text
Task
 ↓
Does it change architecture?
 ├── No → implement
 └── Yes
       ↓
     ADR first
       ↓
     human/agent review
       ↓
     implement
```

并规定 Agent **不得自己删除或者修改已有 ADR 的历史结论**。

比如：

```text
docs/adr/
  ADR-0001-plugin-rpc.md
  ADR-0002-ui-core-same-process.md
  ADR-0003-indexer-process-boundary.md
  ADR-0004-query-cancellation.md
```

ADR 不只是“记录”，而是：

> **Agent 的架构记忆。**

这个对于长期 Agentic Coding 项目非常重要。

---

# 十二、我还发现了一个文档一致性问题

这个最好马上修掉。

README 写的是：

> `workspace 12 crates + 6 apps`



但你当前 `ARCHITECTURE.md` 里列出的 app 是：

```text
launcher-app
launcher-indexer-service
example-echo-plugin
example-testplugins
launcher-bench
```

也就是目前文档可见的是 **5 个 apps**，不是 6 个。

这个虽然是小问题，但对 Agentic Coding 来说非常重要：

> **架构文档必须是机器可验证的。**

建议以后 CI 加：

```text
workspace topology check
```

检查：

```text
Cargo.toml workspace.members
README
ARCHITECTURE
```

三者是否一致。

---

# 十三、我建议增加一个“Architecture Invariants”

这会非常适合 Agent。

新增：

# `docs/INVARIANTS.md`

例如：

```text
INV-001
UI thread MUST NOT perform blocking IO.

INV-002
Core MUST NOT embed Python/Node runtime.

INV-003
Plugin MUST NOT access UI internals.

INV-004
All external effects MUST pass Action Engine.

INV-005
Plugin results MUST be bounded.

INV-006
Provider query MUST NOT mutate global state.

INV-007
Untrusted plugin failure MUST NOT terminate Core.

INV-008
No unbounded global cache.

INV-009
Every public protocol change requires versioning.

INV-010
Every architecture boundary change requires ADR.
```

然后 Agent 每次改代码都可以：

```text
Check invariants
    ↓
Implement
    ↓
Test invariants
```

这比一大堆自然语言规范更适合 Agent。

---

# 十四、MVP 现在我建议暂时不要继续扩展

我会把现在的状态定义成：

```text
                  MVP v0.1
                     │
          ┌──────────┴──────────┐
          │                     │
       已验证                  未验证
          │                     │
   Native UI              Search Quality
   Low memory              Context
   Core                    Lifecycle
   File Index              Security
   Plugin IPC              Large Index
   Fault Isolation         Cancellation
   Benchmark               Long-term Soak
```

其中最值得做的不是：

```text
AI
MCP
Workflow
Python
Node
WASM
Marketplace
```

而是：

```text
       v0.1.x
          │
  ┌───────┼────────┐
  ▼       ▼        ▼
Context Search   Stability
  │       │        │
Explorer Ranking  Soak
```

---

# 十五、因此我建议下一阶段文档结构调整成这样

```text
docs/
│
├── 01-design-spec-v0.1.md
├── 02-agentic-coding-development-spec-v0.1.md
├── 03-test-plan-v0.1.md
├── 04-mvp-scope-v0.1.md
│
├── ARCHITECTURE.md
├── INVARIANTS.md              ← 建议新增
├── LIFECYCLE.md               ← 建议新增
├── PERFORMANCE-METHODOLOGY.md ← 建议新增
├── SEARCH-QUALITY.md           ← 建议新增
├── SECURITY.md                 ← 建议新增
│
└── adr/
    ├── ADR-0001-plugin-rpc.md
    ├── ADR-0002-ui-core-process.md
    ├── ADR-0003-indexer-boundary.md
    └── ADR-0004-query-cancellation.md
```

再增加：

```text
benchmarks/
tests/
fixtures/
```

形成：

```text
┌─────────────────────────────────────┐
│          Agentic Coding             │
├─────────────────────────────────────┤
│ AGENTS.md                           │
│ Architecture                        │
│ Invariants                          │
│ ADR                                 │
│ Task Contract                       │
├─────────────────────────────────────┤
│              Code                   │
├─────────────────────────────────────┤
│ Unit / Integration / Contract       │
│ Performance / Soak / Fault Injection│
├─────────────────────────────────────┤
│ Benchmark Baselines                 │
└─────────────────────────────────────┘
```

---

## 最后我的判断

**现在这版 MVP 已经足够优秀，可以进入“架构验证阶段”，不用重构。**

尤其是这三个数字很值得继续往下压测：

```text
62/62 tests
21 万文件
12.6 MB idle Private
```



我目前唯一会强烈要求在继续堆功能前补上的，是：

**① Query cancellation / concurrency model
② Plugin security + process-tree isolation
③ 性能测量方法标准化
④ Search quality corpus
⑤ Explorer Context 实现
⑥ Architecture Invariants + ADR 体系**

这六个补完后，**我认为就可以正式开始 v0.2，而 v0.2 再进入“Quick Switch + Preview + Workflow/AI 基础抽象”的阶段。**

另外，你这次给的三个文件已经能完成架构审查；但你文档里提到的 `docs/TESTING.md`、`docs/PERFORMANCE.md`、`docs/KNOWN-ISSUES.md` 等实际测试与性能明细没有随本次上传提供，所以我这里对 **62/62、12.6 MB、21 万文件**是按照 README/ARCHITECTURE 中的记录来判断，没有把尚未看到的测试细节当成已验证事实。
