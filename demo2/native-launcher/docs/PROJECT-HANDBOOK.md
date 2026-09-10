# PROJECT HANDBOOK — Native Launcher（新会话一次性导读）

> **本文件用途**：为新的 Agent 会话/开发者提供项目全景、冻结契约索引、执行边界、测试闸门与剩余工作。读完本文件即可安全开工；各主题细节见对应链接文档。
>
> **更新**：2026-09-10（**P2.10 收口**，见 `docs/history/159-p210-closure.md`）。
> 当前基线：**887 tests 全绿 / zero warnings / topology 17 crates + 9 apps**；
> 阶段状态唯一真相：`docs/phase/status.md`（P2.4–P2.10 全 ACCEPTED，外部
> 证书项 BLOCKED-EXTERNAL）；运行语义契约：`docs/contracts/`。

---

# 1. 项目定位

键盘优先的 Windows 原生 Launcher（Rust + Slint，软件渲染器，无 Electron/WebView/CEF）。

核心架构一句话（ADR-0014 冻结）：

> **所有"想做什么"的参与者都是 Action Producer（UI / Plugin / Workflow / AI / MCP）；所有"实际做什么"都必须经 ActionResolver 与 ActionEngine。ActionEngine 是唯一 Effect 执行网关。**

```text
Producer (UI/Plugin/Workflow/AI/MCP)
        │  Command / ActionDescriptor / ActionProposal
        ▼
  ActionResolver   ← identity / capability / context / confirmation
        ▼
  ResolvedAction（可信）
        ▼
  ActionEngine     ← 唯一 Effect 闸门
        ▼
     Effect（system.* 经 OS；plugin.* 经 PluginBroker；mcp.* 经 McpExecutor 预留）
```

# 2. Workspace 布局（17 crates + 9 apps）

| 成员 | 职责 |
|---|---|
| `crates/launcher-domain` | 纯模型：Command/Action/ActionDescriptor/Provider/Manifest/Workflow 类型 + `resolve_descriptor` |
| `crates/launcher-search` | 确定性评分/排序 |
| `crates/launcher-context` | ContextSnapshot 引擎（Explorer COM / 前台应用） |
| `crates/launcher-action` | Action Engine（Effect 唯一路径，validate 拒绝 disabled/unconfirmed） |
| `crates/launcher-ipc` | 插件 JSON-RPC 消息模型 + sanitize（`execute_action` 已加入） |
| `crates/launcher-indexer` | 目录扫描 + SQLite 索引 |
| `crates/launcher-workflow` | Workflow Runner 状态机（orchestration only） |
| `crates/launcher-mcp` | MCP Adapter：stdio transport + catalog + 投影（proposal-only） |
| `crates/launcher-plugin-host` | 插件进程生命周期 + Job Object 隔离 |
| `crates/launcher-plugin-api` | 插件作者 Rust SDK（`serve()` / `serve_with_actions()`） |
| `crates/launcher-core` | Provider 注册 + query 编排 + McpProvider + execute_plugin_action 路由 |
| `crates/launcher-config` | TOML 配置（hotkey/theme/index_dirs/python_path/mcp.servers） |
| `crates/launcher-hotkey` | 全局热键注册线程 |
| `crates/launcher-providers` | Uninstall 注册表 + recent files |
| `crates/launcher-ui` | Slint 呈现层（Main/Action/Workflow surface + primitives） |
| `apps/launcher-app` | 主程序（UI+Core 合体，ADR-0002） |
| `apps/launcher-indexer-service` | 独立索引进程 |
| `apps/example-echo-plugin` / `apps/example-testplugins` | 插件示例 + Plugin Contract Test Kit 载体 |
| `apps/calculator-plugin` | Protocol Canonical Reference（协议变更硬门禁） |
| `apps/calculator-plus` | Domain/UI Reference（COMMAND/ACTION-CONTRACT 实现 + CAT-WF kit） |
| `apps/example-mcp-server` | MCP stdio fixture server（mcp-calculator） |
| `apps/launcher-bench` | 性能基线/回归/soak/VR 截图数据生成参考 |

# 3. 冻结契约与规格（全部在 `docs/`）

| 契约 | 状态 | 一句话 |
|---|---|---|
| `PLUGIN-CONTRACT-v0.1.md`（+ §29/§30 Addendum） | FROZEN | 插件协议：NDJSON JSON-RPC、initialize/query_id/shutdown、execute_action、capability 点分名、8 类失败、Resource limits |
| `COMMAND-CONTRACT-v0.1.md` | FROZEN v0.1 | Command 身份三层 + primary=first Ready + requires_context（eligibility） |
| `ACTION-CONTRACT-v0.1.md` | FROZEN v0.1 | ActionDescriptor → ActionResolution → ResolvedAction → Engine；Capability 单调性；CapabilityDenied = Execution state |
| `WORKFLOW-CONTRACT-v0.1.md` | FROZEN v0.1 | 五对象（Definition/Run/StepRun/Invocation/FailurePolicy）+ 线性 Step + at-least-once |
| `UI-CONTRACT-v0.1.md` | FROZEN v0.1（canonical） | 3 Mode（Main/Action/Workflow）+ Confirmation 子状态 + 状态迁移矩阵 + 50 条 UI-ACC |
| `VISUAL-DESIGN-SPEC-v0.1`（外部稿，评审见 `VISUAL-DESIGN-SPEC-REVIEW.md`） | FROZEN v0.1 | Design Tokens（theme.slint 单一注入点）/ 640×420 基线 / Motion ≤200ms / 禁 Dashboard 化 |
| `UI-CONTRACT-COMPARISON.md` | 存档 | 两版本合并决议（D1：单击=选中并执行 primary） |
| `docs/contracts/EFFECT-AUTHORITY.md`（P210） | FROZEN | 权威链 Producer→Resolver→Policy→Approval→Engine→AuthorizedEffect→Adapter；adapter 无 authority 参数 |
| `docs/contracts/EXECUTION-SEMANTICS-v1.md`（P210） | FROZEN | CommandResult/EffectState/StepStatus/Retry 矩阵/Recovery Protocol 统一词汇 |
| `docs/contracts/STATE-GENERATION.md`（P210） | FROZEN | 六类 Lifecycle State + Generation 注册表（失败不递增；Generation ≠ Identity） |

**依赖方向铁律**：`UI-CONTRACT 定义行为` → `VISUAL-DESIGN-SPEC 定义外观` → `Slint 渲染`。视觉规范不得反向改变行为契约。

# 4. ADR 索引（`docs/adr/`）

| ADR | 主题 |
|---|---|
| 0001–0003 | IPC 协议 / UI+Core 合体 / 索引边界 |
| 0004 | Query cancellation（query supersession） |
| 0005 | 插件 Job Object 隔离（CREATE_SUSPENDED→assign→resume） |
| 0006 | Capability enforcement（Requested→Granted→Enforced） |
| 0007 | Plugin Contract v0.1 冻结（NDJSON-RPC + Python SDK 前置） |
| 0008 | 协议边界硬化（frame 256KB / stdout 红线 / 双 Resolver / Retry 语义） |
| 0009 | Contract 冻结 + Python SDK + `runtime.type=python` |
| 0010 | RuntimeResolver 单一扩展点 + SDK 边界 |
| 0011 | Command & Action Contract 冻结（ResolvedAction 信任边界） |
| 0012 | Native Action UI 边界（ActionPanel 只呈现/选择） |
| 0013 | Advanced Action Lifecycle（Shortcut/Confirmation/Context-gen/Paste） |
| 0014 | Unified Action Execution（MCP/AI/Workflow = Producer） |
| 0015 | Workflow Orchestration 契约冻结 |
| 0016 | AI Planner（ActionProposal 结构性信任边界） |
| 0017 | UI Behavior Contract 冻结（3 Mode + 迁移矩阵） |
| 0018 | MCP Adapter（launcher-mcp + stdio + catalog + McpProvider） |

# 5. Invariants（完整表见 `docs/INVARIANTS.md`）

关键分组（INV-001~069 + P210 节 INV-EFFECT-101~106 / INV-IDENTITY-101~102 /
INV-RETRY-101 / INV-RECOVERY-101 / INV-AGENT-101 / INV-STATE-101 全表见
INVARIANTS.md，此处为新会话最易踩的）：

- **执行边界**：INV-004（Effect 只经 ActionEngine）、INV-026/033（raw ActionDescriptor 永不触达 Engine；Engine 只收 ResolvedAction）、INV-037（ActionPanel/UI 不直接执行 Effect）。
- **身份**：INV-016/028/029/035（UI 与持久化一律 stable id：command_id/action_id/workflow_run_id；provider_id Host 权威；禁 index/文本匹配）。
- **Capability**：INV-027（`Action.requires ⊆ Manifest.capabilities`，禁自动补权限）、INV-068（MCP metadata 只是 Policy 输入）。
- **呈现**：INV-062/063/064/065（Workflow UI 只读状态、MCP 无独立 UI 路径、Context 经 Presentation、失败呈现消费分类结果）。
- **插件**：INV-013/019/020/021/022（路径 confinement、stdout=协议通道、256KB 帧上限、单 in-flight、legacy profile 隔离）。
- **MCP**：INV-066~069（proposal-only、config-owned identity、metadata 非授权、单点失败映射）。
- **Search/Index/Identity（P2.1，review 70 §71/106-108 采纳部分）**：
  - INV-SEARCH-001：交互 UI 路径上的 Search 永不同步执行无界文件系统扫描（FIX-05 已落实）。
  - INV-SEARCH-002：最终排名只作用于规范化去重后的候选集，history/context boost 不得制造原本不存在的候选（launcher-search 测试已固化）。
  - INV-INDEX-001：单次 rebuild 全局有界（MAX_INDEX_ENTRIES 共享预算）。
  - INV-INDEX-003：reparse point（junction/symlink）默认不跟随。
  - INV-INDEX-004（部分）：索引维护 eventual consistent、bounded、可重启、与交互搜索可用性解耦（watcher 增量维护后置 P2.1-F）。
  - INV-IDENTITY-001：等价发现记录在最终排名/呈现前归并为一个 canonical identity（应用 exe 身份合并已实现；完整 typed SearchIdentity 后置）。
  - INV-SEARCH-003：空查询结果必须对当前 live catalog 解析——历史记录本身不构成可执行权威（recent_commands 的 catalog join 即此不变量的实现）。
  - INV-SEARCH-004：单个 provider 失败不得丢弃无关 provider 的成功结果（Core::search 错误隔离，quality 测试固化）。
  - ApplicationIdentityResolver 接缝（executable_identity）已预留：v1=exe 身份，未来 AUMID/MSIX 解析器实现同签名接入，Search 不感知来源。
  - IndexGeneration：index_meta.generation **在 rebuild 成功提交时** +1（事务内递增，失败不涨）——语义是"已提交的索引版本"，为 cache 失效与 diagnostics 预留（review 72 §25/26、74 §7；P2.1-B 中按 batch commit 递增而非每事件）。
  - INV-IDENTITY-003：身份解析必须 PURE——只读元数据/解析/规范化，不得产生或执行 Effect（executable_identity 文档已固化，review 74 §11）。
  - 设备命名空间（`\.\...`）不是文件系统身份：`is_device_namespace` 判定，身份函数原样保留（大小写折叠），永不与真实路径身份冲突（review 74 §3）。
  - INV-SEARCH-004 措辞（review 74 §1）：单 provider 失败不得压制无关 provider 的成功结果；仅当查询显式定向该 provider 时才呈现 "targeted provider unavailable"（定向语义随 P2.1-B/Provider v2 落地）。

# 6. 测试体系与闸门

## 6.1 三套契约测试 + 行为回归

| Kit | 位置 | 覆盖 |
|---|---|---|
| Plugin Contract Test Kit | `apps/example-testplugins/tests/contract.rs`（16 项） | 协议层：握手/版本协商/query_id 回显/manifest profile/stdout 红线/frame 限制/优雅关闭/进程树回收 |
| Domain Contract Kit | `apps/calculator-plus/tests/`（CAT-001~012 + mvp3/mvp4 acceptance） | 命令身份/路由顺序/primary 解析/capability 单调性/stale 隔离/ResolvedAction 边界 |
| UI-ACC-001~050 | `docs/UI-CONTRACT-v0.1.md` §20 | UI 行为基线（39✅/3◐/8→已随 Workflow Surface 大幅收敛） |

**硬闸门**：任何改动后 `cargo test --workspace` 全绿 + `cargo build` 零警告 + `python scripts/check_topology.py` 通过。改 `launcher-ipc/plugin-api/plugin-host` → 必须过 calculator conformance；改 `launcher-domain/launcher-action` → 必须过 calculator-plus conformance（AGENTS.md）。

## 6.2 Visual Regression（⑧）

```bash
LAUNCHER_SNAPSHOT_DIR=<dir> ./target/debug/launcher-app
# → VR-001..VR-010 BMP（640×420 logical / Dark / 100%）
# 回归 = 基线目录与新目录逐字节 diff
```

10 个冻结状态：Main Empty/Results/Selected/Error；Action Normal/Disabled/Confirmation；Workflow Running/Paused/Failed。场景定义：`apps/launcher-app/src/visual_scenarios.rs`。

## 6.3 其他关键测试

- Workflow runner 全生命周期：`crates/launcher-workflow/tests/cat_wf.rs`（CAT-WF-001~012）
- MCP Provider/transport/fixture：`crates/launcher-mcp`（13 单测）+ `apps/example-mcp-server/tests/mcp_provider_e2e.rs`（6 项真实 stdio E2E）
- Plugin SDK 跨语言：`crates/launcher-plugin-host/tests/python_sdk_e2e.rs`（`LAUNCHER_PYTHON` 可指定解释器）

# 7. 里程碑进度（索引）

> P2.10-011 文档对齐：阶段状态的**唯一真相**是 `docs/phase/status.md`
> （四态词汇：ACCEPTED/IMPLEMENTED/DEFERRED/BLOCKED-EXTERNAL，含每个
> 后置项与理由）；逐批次事实证据在 `docs/history/`（63→159 号，不可
> 回写）。本节只保留里程碑 → 权威链接的索引，不再承载状态事实。

| 里程碑 | 一句话 | 权威状态 |
|---|---|---|
| 1.0 GA | 核心管线/UI/插件执行链，G01–G14 gate | `docs/phase/status.md` |
| P2.4 Foundation | Catalog 2.0 / Index 2.0 / Trust / CLI | `docs/phase/status.md` |
| P2.5 Search Intelligence | Contract v2 / Coordinator / FTS5 / Ranking / Pinyin 全表 | `docs/phase/status.md` |
| P2.6 Workflow 2.0 | Graph/Durable/Approval/Trigger/Editor Surface | `docs/phase/status.md` |
| P2.7 AI/Agent | A–G 全线 + H（语料/CI 载体）；签名 BLOCKED-EXTERNAL | `docs/phase/status.md` |
| P2.8 Plugin Lifecycle & Local Ecosystem | 本地生态闭环；Marketplace/签名 DEFERRED/BLOCKED-EXTERNAL | `docs/phase/status.md` |
| P2.9 System Integration | Capability/Policy/Windows Adapter（真 Win32） | `docs/phase/status.md` |
| P2.10 Hardening | Effect Authority/动态目标/执行语义/Replan/E2E/文档对齐 | `docs/phase/status.md` |

# 8. 剩余工作（全部有归属，无悬空）

| 项 | 归属 | 说明 |
|---|---|---|
| Streamable HTTP transport / OAuth remote auth | Phase 11+ backlog | 见 docs/MCP-COMPATIBILITY.md §3/§8 |
| 多语言 SDK interop 矩阵（Py/TS/Go/C#） | Phase 11+ backlog | 需外部 SDK/网络环境 |
| 应用内 Workflow 触发的 pause/resume 全链路 | ◐ | service 通道/callback/呈现已就绪；缺触发源 |
| 插件 empty-text discovery | DISCOVERY-TODO-001（MCP 半边已闭合） | 未实现 discovery 的插件如实降级 CommandNotFound |
| UI-ACC 残余 ◐/⏳ | Workflow surface 手工抽查、selected 非颜色标记细化、Level 3 独立面板 | 见 `docs/UI-CONTRACT-CONFORMANCE-2026-09-05.md` |
| Visual Polish / v0.2 | 窗口位置策略/间距微调/图标资产/主题切换 | 契约与基线已冻结，纯视觉层 |
| MSIX 证书签名 + 完整升级状态机 | GA 外部 / P2.3-G 尾款 | 打包与 handoff 消费已就绪（history/89）；缺签名证书与升级器宿主 |

# 9. 常用命令

```bash
cargo test --workspace          # 行为回归闸门（当前 887 全绿；含 P2.7 G 线/语料/P2.10 E2E）
cargo build --workspace         # 零警告检查
python scripts/check_topology.py  # 拓扑 + SDK 依赖守卫
python scripts/release_gate.py    # Release Gate（G01~G14：1.0 + P2.4/P2.5 conformance + evidence/manifest；AI 测试随 cargo test 全量运行）
LAUNCHER_SNAPSHOT_DIR=<dir> ./target/debug/launcher-app   # 10 张 VR 基线
LAUNCHER_PYTHON=<python> cargo test -p launcher-plugin-host  # Python SDK E2E
```

# 10. 新会话开工须知（红线速查）

1. 不改契约语义除非先过评审 + ADR（INV-009/010）；UI/VISUAL/WORKFLOW/ACTION/COMMAND/PLUGIN 六份 FROZEN 契约是现状，不是提案。
2. 一切执行经 Resolver → Engine；任何"为了方便"的直连 Effect 都是架构回归（INV-033/044/066）。
3. 所有跨边界引用用 stable id（command_id/action_id/run_id/execution_id）；provider_id 永远 Host 权威。
4. Capability 单调性：requires ⊆ manifest；metadata（MCP annotations 等）只是 Policy 输入。
5. UI 永远是 projection：不解析语义、不判 capability、不执行 Effect（INV-035~037/062~065）。
6. 系统 Effect 只能经 `execute_system_effect`（Resolver policy → engine 铸造 token → adapter）；adapter 边界没有 confirmed/authority 参数，token move-only 一次性（INV-EFFECT-101~106）。
7. 效果不确定性引用 EXECUTION-SEMANTICS-v1：Timeout ≠ Failed；Unknown/Executing 不盲目重试，走 Recovery 路由；Replan 绝不重执行 Succeeded 步骤。
8. 阶段状态查 `docs/phase/status.md`，不要在 HANDBOOK/README 里新增状态事实；history 不可回写。
9. 每轮收尾：`cargo test --workspace` 全绿 + `cargo build` 零警告 + `check_topology.py` 通过。
