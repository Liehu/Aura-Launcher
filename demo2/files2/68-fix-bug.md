我按这次你补充的完整接线材料重新审了一遍。这里有一个比较重要的结果：

> **你补的 `launcher-app` 解决了我上一轮最大的“是否真的接线”疑问，但代码审计反而发现了两处比之前更值得优先处理的架构问题。**
>
> 一处是 **Action 主链实际上没有统一经过 ActionEngine**；另一处是 **Windows 单实例 Mutex 实际没有被进程持有**。

另外，你这次上传的 zip 确实包含了我要求的：

```text
apps/launcher-app
scripts/check_topology.py
scripts/release_gate.py
Cargo.toml
Cargo.lock
```

所以这次已经可以做真正的宿主接线审计，而不是只看 crate 内部。

---

# 一、先给最终判断

目前我会把状态调整为：

```text
P1-B Persistent Session        ✅ 基本通过
P1-C Workflow v0.2             ✅ 基本通过
P1-D Agent Runtime             🟡 crate 内完成，宿主未接入
P1-E UI                        🟡 基本完成，QA 仍有挂账

P2 Search/Ranking             🟡 第一批可用
P2 App/File                   🟡 第一批可用
P2 Action                     🔴 主链存在架构绕过
P2 Plugin                     🟡 基础接线
P2 Windows                    🟡 有真实接线，但有单实例 bug
P2 Settings                   🟡 初级
P2 Performance               🟡 尚未形成真正 baseline
P2 Packaging                 🔴 未完成
```

因此目前**还不应该给 Launcher 1.0 release candidate**。

但并不是因为功能太少，而是因为发现了两个应该在继续扩功能之前先修掉的工程问题。

---

# 二、最高优先级问题：普通 Action 实际绕过了 ActionEngine

这是目前最重要的问题。

你在 `main.rs::execute_action_by_id()` 中的实际执行：

```rust
std::thread::spawn(move || {
    match launcher_action::execute(&action) {
        ...
        Ok(Effect::PluginInvoked { ... }) => {
            ...
            st.core.execute_effect(...)
        }
```

也就是说：

```text
system.open
system.copy
system.delete
...
```

走的是：

```text
UI
 ↓
launcher_action::execute()
 ↓
Effect
```

而不是我们已经冻结的：

```text
UI
 ↓
ActionProposal / resolved action
 ↓
ActionEngine
 ↓
Effect
```

只有：

```text
PluginInvoked
```

出现之后才重新进入：

```text
Core::execute_effect()
```

---

# 三、这不是“实现风格”，而是架构 invariant 违反

你之前已经明确冻结：

```text
Producer
  → Resolver
  → ActionEngine
  → Effect
```

以及：

> ActionEngine 是唯一 Effect gateway。

但是当前宿主路径实际上有两条：

```text id="path1"
UI
 ↓
launcher_action::execute()
 ↓
Effect
```

以及：

```text id="path2"
UI
 ↓
launcher_action::execute()
 ↓
PluginInvoked
 ↓
Core::execute_effect()
 ↓
Executor
```

所以：

> **当前真正的 ActionEngine 单一 Effect Gateway 在 launcher-app 这一层并没有成立。**

这比我上一轮发现的 Plugin ExecutionId 问题更值得重视。

---

# 四、这会带来什么实际问题？

### 1. ExecutionId

普通 system action：

```text
Open
Copy
Reveal
Paste
```

现在没有统一的：

```text
ExecutionId
```

因此你前面已经建立起来的：

```text
AgentRun
WorkflowRun
Execution
History
Diagnostics
```

无法对所有 Action 建立一致的 execution lineage。

---

### 2. Confirmation

虽然宿主自己的：

```text
pending_confirmation
```

确实挡住了第一次执行。

但这套 confirmation 现在属于：

```text
launcher-app
```

而不是：

```text
ActionResolver / ActionEngine
```

这样以后任何其他触发源：

```text
Workflow
Agent
Plugin
MCP
```

都可能形成自己的 confirmation 逻辑。

这正是你之前一直试图避免的。

---

### 3. Authorization

现在 system action 是：

```text
UI
 → launcher_action::execute
```

所以未来如果 `ActionResolver` 增加：

```text
CapabilityDenied
Context
Risk
Policy
```

就可能出现：

```text
UI path
```

与：

```text
Workflow path
```

行为不同。

---

# 五、正确修复方式

最终应该统一成：

```text
execute_action_by_id()
        ↓
Core / ActionEngine
        ↓
Effect
```

即：

```rust
let execution_id = st.core.next_execution_id();

st.core.execute_resolved_action(
    resolved_action,
    &execution_id,
    ...
)
```

具体 API 名字可以不同，但语义必须是：

```text
launcher-app
    不直接调用 launcher_action::execute()

launcher-app
    只调用 Core/ActionEngine
```

以后：

```text
UI
Workflow
Agent
```

都走同一条。

---

# 六、建议把它正式冻结成新 invariant

```text
INV-AUTH-007

launcher-app MUST NOT directly invoke the low-level action executor.
All executable Actions, including host-owned system actions, must pass
through ActionEngine before producing an Effect.
```

再加：

```text
INV-AUTH-008

Every executable Action attempt, regardless of provider type,
has exactly one ExecutionId created by the host execution boundary.
```

这样：

```text
system
plugin
mcp
workflow
agent
```

都统一了。

---

# 七、第二个明确 bug：CreateMutexW 并没有被持有

这里是：

```rust
fn enforce_single_instance() -> anyhow::Result<bool> {
    ...
    let _handle = unsafe { CreateMutexW(None, false, &name)? };
    Ok(unsafe { GetLastError() } != ERROR_ALREADY_EXISTS)
}
```

表面上看：

```text
_handle
```

持有 Mutex。

但这个变量的生命周期只到：

```text
enforce_single_instance()
```

函数返回。

函数返回之后：

```text
_handle
```

被 Drop。

于是：

```text
main()
 ↓
enforce_single_instance()
 ↓
CreateMutex
 ↓
return
 ↓
HANDLE closed
```

所以进程后续实际上**没有持续持有 mutex**。

也就是说：

```text
Instance 1
  ↓
main continues
  ↓
mutex handle closed

Instance 2
  ↓
CreateMutex
  ↓
可能再次成功
```

这意味着：

> **你目前的 single-instance 实现并没有真正实现“single instance for process lifetime”。**

---

# 八、这个 bug 应该立刻修

正确方式：

```rust
struct SingleInstanceGuard {
    handle: HANDLE,
}
```

或者：

```rust
static / AppState-owned HANDLE
```

让它生命周期覆盖整个：

```text
launcher process
```

例如：

```text id="guardflow"
main
 ↓
guard = enforce_single_instance()
 ↓
AppState / runtime
 ↓
run_event_loop()
 ↓
guard dropped
 ↓
process exits
```

不要：

```text
bool
```

作为唯一返回值。

应该：

```rust
Option<SingleInstanceGuard>
```

或者：

```rust
Result<Option<SingleInstanceGuard>>
```

---

# 九、甚至还有第二层 single-instance 问题

你当前：

```rust
if !enforce_single_instance()? {
    return Ok(());
}
```

第二实例：

```text
→ 发现已有实例
→ 直接退出
```

这满足：

```text
“不会启动第二份”
```

但不满足一个优秀 Launcher 应有的：

```text
Existing instance
    ↑
second activation
    ↓
focus/show existing window
```

例如从：

```text
Explorer
Start Menu
shortcut
shell
```

启动第二次。

理想：

```text
Instance 2
 ↓
signal Instance 1
 ↓
show launcher
 ↓
focus input
 ↓
Instance 2 exits
```

这项可以后置到 Windows Integration，但 **mutex handle lifetime 是现在必须修的 bug**。

---

# 十、还有一个接线层面的重要发现：Workflow Backend 也绕过了 ActionEngine

`workflow_service.rs`：

```rust
impl launcher_workflow::ActionExecutor for AppStateBackend {
    fn execute(...) {
        ...
        match launcher_action::execute(&action) {
            Ok(_) => ...
        }
    }
}
```

这意味着：

```text
Workflow
 ↓
AppStateBackend
 ↓
launcher_action::execute()
```

同样没有统一进入：

```text
ActionEngine
```

Plugin 分支又特殊走：

```text
Core::execute_plugin_action()
```

所以现在实际有：

```text
UI system action
      └── launcher_action::execute()

Workflow system action
      └── launcher_action::execute()

Plugin action
      └── Core

MCP action
      └── Core
```

这比单纯的 UI 问题更严重。

---

# 十一、现在应该正式做一次“Execution Gateway Convergence”

你现在最应该做的不是继续 P2 功能，而是把：

```text
UI
Workflow
Agent
Plugin
MCP
```

全部收敛：

```text
                ┌──────────────┐
                │ User / UI    │
                └──────┬───────┘
                       │
                ┌──────▼───────┐
                │ Workflow     │
                └──────┬───────┘
                       │
                ┌──────▼───────┐
                │ Agent        │
                └──────┬───────┘
                       │
                       ▼
                ActionProposal /
                ResolvedAction
                       │
                       ▼
                ┌──────────────┐
                │ ActionEngine │
                └──────┬───────┘
                       │
                     Effect
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
       System       Plugin         MCP
```

**所有执行源只能在 ActionEngine 之后分流。**

---

# 十二、这会顺便解决 ExecutionId 的统一问题

最终：

```text
UI
  → E100

Workflow
  → E101

Agent
  → E102

Plugin
  → E103

MCP
  → E104
```

每一个都经过同一个 execution boundary。

不应该：

```text
System action
  → no execution id

Plugin
  → execution id

MCP
  → execution id
```

---

# 十三、这也是我建议你暂时不要关闭 P1-D Confirmation 的原因

虽然 Agent Confirmation 可以后置，但你的基础：

```text
Confirmation
```

现在其实仍然有两套：

```text
launcher-app pending_confirmation
```

以及：

```text
Workflow Confirmation
```

最终应该只有一个真正的：

```text
ActionEngine confirmation lifecycle
```

然后 UI 只是：

```text
Confirm
Reject
```

---

# 十四、你补的 Agent 宿主接线结果是好的——但确实没接

这一点需要明确表扬/认可你的审计结论：

`launcher-app/Cargo.toml` 当前确实没有：

```toml
launcher-ai = ...
```

而我在 `main.rs` 中也没有发现：

```text
AgentRuntime
run_bounded_agent
AgentHost
launcher_ai
```

真正运行接线。

所以：

```text
Agent crate
    ✅

Agent E2E
    ✅

Agent app integration
    ❌
```

你自己的审计判断是正确的。

因此我建议：

> **不要为了“P1 完整”强行把 Agent 接到 Launcher。**

现在将：

```text
P1-D Agent Runtime
```

定义为：

```text
experimental / library-complete
```

完全合理。

---

# 十五、Workflow 目前也属于“半接线”，但可以接受

现在实际：

```text
Tray menu
 ↓
start_workflow()
 ↓
WorkflowRunner
 ↓
Workflow UI
```

这个链路确实是真实的。

所以：

```text
Workflow engine
✅

Host integration
✅

Product trigger
🟡 only tray demo
```

这与之前判断一致。

不过这里还有一个小问题：

```rust
def.validate().expect(...)
```

这意味着如果未来用户配置的 Workflow definition 有问题：

```text
invalid definition
→ panic
```

当前 demo 场景没问题，但到了产品化以后必须改成：

```text
validate()
→ WorkflowDefinitionError
→ UI diagnostics
```

这属于 P2/P3，不阻塞当前基础 Launcher。

---

# 十六、Build Core 的 Index Rebuild 是我现在非常建议提高优先级的一项

`build_core()`：

```rust
let n = indexer.rebuild(&roots)?;
```

发生在：

```text
Launcher startup
```

每次启动都会 rebuild。

所以现在流程实际上：

```text
Alt+Space / launcher process start
    ↓
build_core
    ↓
scan index dirs
    ↓
rebuild SQLite
    ↓
start UI
```

如果：

```text
100k files
500k files
```

这会直接破坏你最核心的：

> **Launcher “秒开”。**

这一项甚至比：

```text
advanced app index
```

更值得优先做。

---

# 十七、所以“增量索引 / Watcher”我现在会升级成 MUST

上一轮我把它列在后续。

现在看到宿主接线后，我会调整：

```text id="newmust"
P2 MUST

Incremental Index
Watcher
Startup without full rebuild
```

目标应该：

```text
Launcher startup
 ↓
open existing SQLite
 ↓
UI ready
 ↓
background index maintenance
```

而不是：

```text
scan first
then UI
```

---

# 十八、`default_index_dirs()` 也需要特别关注

当前：

```text
default_index_dirs()
+
cfg.index_dirs
```

再：

```text
indexer.rebuild()
```

如果默认目录里面含：

```text
Downloads
Documents
Desktop
Pictures
```

那启动时就可能是一个很重的操作。

建议最终：

```text
UI startup
    ≠
index rebuild
```

必须彻底解耦。

---

# 十九、App Provider 还有一个宿主接线 bug

你前一轮已经实现：

```text
.lnk target resolution
```

但是宿主的：

```rust
start_menu_entries_from_existing()
```

仍然：

```rust
resolved_target: None,
path: PathBuf::from(target),
```

也就是说：

```text
AppProvider
→ 找到 .lnk
→ target
→ 转换成 AppEntry
→ 把 resolved_target 丢掉
```

所以：

> **你报告里说“目录扫描时解析 `.lnk` target 并提供 RunAsAdmin”，这个能力在宿主合并 Start Menu provider 的这条路径上仍然可能被丢失。**

这尤其影响：

```text
Start Menu Chrome.lnk
```

因为它经过：

```text
start_menu_entries_from_existing()
```

而不是直接从你改好的 `AppRegistryProvider` parser 得到完整 target metadata。

### 建议

这个函数不要再：

```text
AppProvider
 → Command
 → AppEntry
```

这样二次降维。

最好统一：

```text
StartMenuScanner
 → AppEntry {
      path,
      resolved_target,
      ...
   }
```

直接送给：

```text
AppRegistryProvider
```

---

# 二十、P2 Search 的真实宿主链现在确认成立

这部分我现在可以正式认可。

启动时确实注册：

```text
Context
Applications
Recent
Files
MCP
Plugins
```

所以此前我最担心的：

```text
“Provider 实现存在但没有 register”
```

现在可以排除。

---

# 二十一、History 闭环也确实成立

现在宿主中：

```text
successful system effect
    ↓
record_use_with_title()

successful plugin/mcp effect
    ↓
record_use_with_title()
```

然后：

```text
next search
 ↓
ranking usage boost
```

这是真闭环。

所以 P2-D：

```text
History basic integration
```

可以判定：

```text
✅
```

只是：

```text
privacy classification
query-result affinity
decay model
```

仍然属于增强项。

---

# 二十二、还有一个产品层面的历史问题

当前 History 依赖：

```text
successful execution
```

这很好。

但 Action：

```text
Copy Path
```

成功一次是否应该和：

```text
Open
```

一样提升整个 command？

例如：

```text
test.pdf
Copy Path
```

可能让：

```text
test.pdf
```

整个结果被提高。

这不是 bug，但以后应区分：

```text
result usage
action usage
```

例如：

```text
Chrome opened
```

应该提升 Chrome；

而：

```text
Chrome → Copy path
```

未必应该提升 Chrome 的 open ranking。

这属于 P2 Ranking 2.0，可以后置。

---

# 二十三、`release_gate.py` 现在必须重写

这是我对这次材料最明确的“新增项”。

你现在的 `release_gate.py` 顶部还是：

```text
MVP4.3 Release Gate
```

artifact：

```text
artifacts/mvp4.3/
```

manifest：

```text
version: 4.3.0
```

而 Gate 内容仍然主要围绕：

```text
MCP compatibility
MCP E2E
old VR
```

甚至 `g_compat()` 里面可以看到：

```python
r = cargo_testsuite(a)
r = cargo_testsuite(a)
```

同一个测试套件连续执行了两次。

这不是大问题，但说明这个 gate 已经成为**历史脚本**，并没有跟当前产品状态同步演进。

---

# 二十四、更关键的是它没有检查我们现在真正关心的 P2

当前 Release Gate 没有：

```text
Search product E2E
App discovery
File discovery
History closed loop
Action gateway
Plugin product flow
Single instance
Config recovery
Indexer startup cost
Launcher benchmark
```

也没有检查：

```text
launcher-app
```

实际是否接入：

```text
providers
actions
history
```

所以：

> **现在这套 release_gate 通过，也不能证明 Launcher 1.0 ready。**

---

# 二十五、建议升级为 `LAUNCHER-1.0-RELEASE-GATE`

建议最终：

```text
G1  build
G2  workspace tests
G3  topology
G4  architecture
G5  security
G6  MCP regression
G7  product E2E
G8  search quality
G9  indexer safety
G10 Action gateway
G11 Plugin E2E
G12 Windows integration
G13 visual regression
G14 performance
G15 memory/soak
G16 config migration
G17 packaging
G18 docs
```

其中：

```text
G10 Action gateway
```

必须新增测试：

```text
system action
plugin action
MCP action
workflow action
```

全部证明：

```text
ActionEngine
```

是唯一 gateway。

---

# 二十六、Topology 目前本身没问题，但它检查不到 Action bypass

`check_topology.py` 做得很好：

```text
launcher-runtime
launcher-ai
launcher-ui
HTTP library
SDK dependency
```

这些 boundary 有 guard。

但：

```text
launcher-app
```

调用：

```text
launcher_action::execute()
```

属于**调用语义问题**，不是 Cargo dependency topology。

所以需要增加：

```text
source architecture guard
```

例如：

```text
launcher-app/src/**/*.rs
```

禁止：

```text
launcher_action::execute(
```

允许：

```text
core.execute_
ActionEngine
```

这样以后就不会有人把低层 executor 又接回宿主。

---

# 二十七、我建议新增一个非常强的 source guard

```text
APP-ARCH-001

apps/launcher-app MUST NOT invoke launcher_action::execute directly.
```

以及：

```text
WF-ARCH-002

launcher-workflow adapters MUST NOT invoke low-level action executor.
```

这样可以把这次发现的 bug 永久固定下来。

---

# 二十八、P1-E 的接线现在基本确认了

`main.rs` 确实：

```text
AppWindow
 ↓
UI events
 ↓
state
 ↓
Action / Workflow
```

而 `launcher-ui` 没有：

```text
launcher-core
launcher-mcp
launcher-runtime
```

依赖。

所以此前的：

```text
UI architecture boundary
```

可以通过。

---

# 二十九、但 Visual Regression 的 release gate 仍然不够严谨

当前：

```text
if baseline doesn't exist:
    create baseline
    PASS
```

这意味着一个全新 CI workspace：

```text
第一次执行
→ 无 baseline
→ 自动接受当前截图
→ PASS
```

这不是严格意义上的 regression gate。

对于开发环境没问题，但 Release CI 不应该这样。

应该：

```text
Developer:
  bootstrap baseline explicitly

CI:
  baseline MUST already exist
```

否则：

```text
视觉破坏
+
删除 baseline
+
CI
```

就可以自动 pass。

---

# 三十、另外，当前 VR 数量从 10 → 15 是合理的

这一点没问题。

而你还保留：

```text
15/15 byte-for-byte
```

这个做法我认可。

只是应该进一步区分：

```text
Visual Determinism
```

和：

```text
Real Windows UI Acceptance
```

也就是之前所说的：

```text
VR
+
manual/automated environment QA
```

---

# 三十一、我对本次材料的最终评级

如果只评价：

### 架构设计

```text
A
```

### P1-B/C 实现

```text
A-
```

### Agent Runtime crate

```text
B+
```

因为现在是 minimal bounded agent，不是完整产品接线。

### P2 Search

```text
B
```

基础链成立，但：

```text
canonical identity
incremental indexing
ranking 2.0
```

还没有。

### P2 Windows

```text
B-
```

有真实接线，但：

```text
Mutex lifetime
second-instance activation
```

需要修。

### P2 Productization

```text
C+
```

因为：

```text
Settings
Installer
Performance baseline
Incremental index
```

都还没形成完整闭环。

---

# 三十二、现在我建议只修 5 件事，然后再继续 P2

不要再扩大范围。

## P0-FIX

### `FIX-01 Action Gateway`

统一：

```text
UI
Workflow
Agent
Plugin
MCP
     ↓
ActionEngine
     ↓
Effect
```

禁止宿主/Workflow adapter 直接：

```text
launcher_action::execute()
```

---

### `FIX-02 ExecutionId`

确保：

```text
System
Plugin
MCP
Workflow
Agent
```

每个 execution attempt：

```text
exactly one ExecutionId
```

---

### `FIX-03 Single Instance Lifetime`

让 Mutex Handle：

```text
process lifetime
```

而不是：

```text
function lifetime
```

---

### `FIX-04 Start Menu .lnk Identity**

不要在：

```text
AppProvider → Command → AppEntry
```

转换过程中丢失：

```text
resolved_target
```

---

### `FIX-05 Launcher Startup Index**

把：

```text
indexer.rebuild()
```

从：

```text
blocking startup
```

移出。

至少先：

```text
open existing DB
→ show UI
→ background rebuild
```

最终再进：

```text
incremental watcher
```

---

# 三十三、然后进入真正的 Launcher 1.0 收尾

顺序建议变成：

```text
当前
 ↓
5 个 FIX
 ↓
Action Gateway Regression
 ↓
Product E2E
 ↓
Incremental Index
 ↓
Settings
 ↓
Performance / Memory
 ↓
Installer
 ↓
Launcher 1.0
```

而：

```text
Agent Confirmation
Advanced Workflow
MCP advanced
AI enhancements
```

继续后置。

---

# 三十四、还有一个值得你马上记录的架构事实

你现在整个项目实际上已经出现了两个“层级非常成熟”的部分：

```text
MCP / Runtime
```

和：

```text
Workflow / Agent
```

反而最需要收口的是：

```text
Launcher Core
```

也就是：

```text
Search
App
File
Action
History
Windows
```

所以接下来不要再追求：

```text
更多抽象
更多 protocol
更多 Agent feature
```

而是把：

```text
Search → Action → History
```

真正做成产品的**黄金路径**。

---

## 最终判定

你这次补充的完整接线材料非常有价值，它证明了我上一轮最大的疑问——**“是不是只有 crate 实现，没有宿主接线”**——在 Search、Provider、History、MCP、Workflow 等方面大多不是问题。

但宿主源码同时暴露出两个必须优先修的真实问题：

```text
🔴 launcher-app / workflow_service
   直接调用 launcher_action::execute()
   → 绕过统一 ActionEngine gateway

🔴 enforce_single_instance()
   Mutex HANDLE 在函数返回后被 Drop
   → single-instance 实际不成立
```

以及三个重要但可顺手收口的问题：

```text
🟠 ExecutionId 统一执行边界
🟠 Start Menu .lnk target 在宿主转换中丢失
🟠 启动时同步 rebuild index，破坏 Launcher 启动性能
```

这 5 项修完后，我认为项目就可以停止“架构纠错”，正式进入 **Launcher 1.0 产品闭环**。

下一轮最有价值的材料反而不再是更多代码，而是修复后的 **`release_gate.py` + Product E2E 测试**：那时就可以对当前项目做一次真正的 **Launcher 1.0 Release Audit**，把剩余工作压缩成最终的 `BLOCKER / MUST / SHOULD / DEFERRED` 四档。
