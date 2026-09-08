# PROJECT HANDBOOK — Native Launcher（新会话一次性导读）

> **本文件用途**：为新的 Agent 会话/开发者提供项目全景、冻结契约索引、执行边界、测试闸门与剩余工作。读完本文件即可安全开工；各主题细节见对应链接文档。
>
> **更新**：2026-09-08（**P2.4 Batch 2**，见 `docs/history/92-p24-batch2-index.md`；Batch 1 见 91 号；权威逐条基线 `demo2/files2/97-launcher-1.0-final-audit.md`）。基线：642 tests 全绿 / zero warnings / topology 17 crates + 8 apps / S0=S1=S2=0 / 1.0 manifest=released（签名待补）/ hotkey P50 307µs·P95 19.7ms / 10k UI soak PASS。

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

# 2. Workspace 布局（17 crates + 8 apps）

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

关键分组（INV-001~069 全表见 INVARIANTS.md，此处为新会话最易踩的）：

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

# 7. 里程碑进度

> 2026-09-07 修订：按 review 82 §22-23 采用 **DESIGNED → IMPLEMENTED → INTEGRATED → ACCEPTED** 四态。下表只列**最后状态**；详细分批记录见 `docs/history/`（20 份，至 89 号）。

```text
── 基础设施 ────────────────────────────────────────────
MVP1~4.4   P0~P1 全部六批                              ✅ ACCEPTED
           (Core/Search/Plugin/MCP/Workflow/AI/Runtime/
            ProtocolSession/Coordinator/Auth/LLM)

── P2.1 Search Foundation ─────────────────────────────
P2.1-A     Semantic Identity Dedup + path identity     ✅ ACCEPTED
           (rank_with_boost 二段去重 + 语义身份 + Path Identity)
P2.1-B     Incremental Index Engine v0.1               ✅ ACCEPTED
           (RDC watcher + bounded queue + coalescer +
            coordinator + B10 E2E；宿主接线 watch_enabled)
P2.1-C     SearchCoordinator                           ⏳ Batch 3
           (语义去重切片已落地；Contract v2 主体挂账)
P2.1-D     Application Discovery 2.0                   ⏳ Batch 4
           (PackageManager+Portable+exe归并已落地；
            持久Catalog/增量reconcile挂账=P2.1-D.1)
P2.1-E     Icon Cache + Performance Baseline           ⏳ Batch 5
           (Icon后端E1-E4+debug基线已落地；E5 UI接线挂账)

── P2.2 Product Capability ────────────────────────────
P2.2-A     Favorites / Pinning                         ✅ IMPLEMENTED
           (SQLite + 语义身份 + Ctrl+D + boost + 事务)
P2.2-B     Query Cache                                 ✅ IMPLEMENTED
           (generation key 全字段 + LRU + TTL + stale拒)
P2.2-C     Context-aware Ranking                       ✅ IMPLEMENTED (v0.1)
           (FolderProximity + foreground + 语义代际失效)
P2.2-D     Plugin Ecosystem Management                 ⏳ Safety Slice ✅
           (quarantine持久化+disabled无候选已落地；
            Registry SQL/Package/Capability UI挂账)
P2.2-E     Installer / Upgrade / Recovery              ⏳ Bootstrap ✅
           (schema_version/启动健康/崩溃环/handoff/
            package.py已落地；MSIX/UpgradeCoordinator挂账)

── P2.2-F Cross-Batch Contract Closure ────────────────
           CacheKey全字段 / Context代际闭环 / lookup顺序 /
           subtitle回填 / State Generation Matrix       ✅ ACCEPTED

── P1 收口 + P2.1-D.1 + P2.2-D/E 补充 ─────────────────
P1 收口    Candidate Merge + Icon UI + Release基线      ✅
           (actions并集 + IconService UI接线 + VR重生成 +
            Release基线 app p95 7µs / file p95 23µs / 1.8MB)
P2.1-D.1   Persistent Application Catalog              ✅
           (catalog.db reconcile + generation commit+1)
P2.2-D 补   Plugin Registry 持久化                     ✅
           (plugins.db enabled/quarantined/failures)
P2.2-E 补   崩溃环 + handoff + 卸载策略                 ✅
           (3次失败→DEGRADED BOOT + update_handoff消费)

── P2.3 Release Closure ───────────────────────────────  ⚠️ PARTIAL
P2.3-A     Architecture & Contract Baseline            ✅ ACCEPTED
           (LAYER_GUARDS + 30+不变量×锚点矩阵 + 权威链冻结)
P2.3-B     Product E2E                                 ⚠️ Core Slice
           (golden path搜索/排名/历史 + workflow目录 + 真实 Effect E2E ✅
            (real_effect_e2e: search→rank→execute→history→re-rank +
             真实FileProvider索引) ；GUI E2E / 键盘导航 ⏳)
P2.3-C     Reliability / Recovery                      ✅ ACCEPTED
           (C6/C7 持久层损坏恢复 + startup state machine ✅；
            C3 真实多进程 crash soak→quarantine持久化 ✅；
            C4 MCP recovery 矩阵(cash/breach 混合+循环 soak) ✅；
            C11 资源 soak(RSS/handle/thread 有界,env 可扩 30min/4h) ✅；
            C12 Recovery Golden Suite(20 场景×5 层) ✅；
            修复:catalog.db/plugins.db 截断损坏打开时自愈)
P2.3-D     Performance/Memory Closure（D1–D7）          ✅ ACCEPTED
           (D2 真实进程启动 ≈152ms≤500ms + D3 search p95 7/24µs + D4
            idle RSS 26.4MB≤50MB + D5 双 soak(popup 1000x/memory trend)
            + D6 thresholds.json 4 预算全字段 + D7 G10 全字段判定)
P2.3-E     Security/Trust Boundary Closure                    ✅
           (75 adversarial tests S0=S1=S2=0 + query cap 512 + LAYER_GUARDS)
P2.3-F     UI/Interaction QA                           ✅ ACCEPTED
           (selection/supersession/favorite 逻辑切片 ✅；
            键盘 walkthrough（真实 Slint 按键管线：query→↓/↑→Enter→Esc
            报告 pass）✅；DPI walkthrough（125%/150% ScaleFactorChanged
            注入，apply_ui_scale 公式断言 + 窗口存活）✅；
            popup 1000x soak ✅（P2.3-D5）；真实显示器 DPI 矩阵 = 手动清单)
P2.3-G     Installer/Upgrade/Migration QA              ⚠️ Foundation
           (package.py + uninstall.ps1 + 数据保留已有；MSIX/签名⏳)
P2.3-H     Release Engineering & CI                    ✅ ACCEPTED
           (G01~G12 统一 Pipeline；G08 = VR + GUI QA walkthroughs；
            G10 = 全字段性能预算；G12 = Evidence/Manifest/Artifact
            Closure（dist zip + perf baseline SHA-256 封存）)
P2.3-I     Release Candidate                           ◐ RC1
           ── 已达成 ──
           · Real Effect E2E                              ✅ closed
           · P2.3-C/D/F/H 全部收口                          ✅
           · Final Release Gate = G01~G12                 ✅ 12/12 PASS
           · RC artifact 冻结：dist/NativeLauncher-1.0.0-win64.zip +
             EVIDENCE.json（SHA-256）+ RELEASE-MANIFEST（status=
             release-candidate）
           ── RC1 挂账（2026-09-08 收口，见 docs/history/89-rc1-closure.md）──
           · Plugin Registry 安全选择恢复（backup/restore）      ✅
             （plugins.db.bak 随写随备 + 损坏 quarantine+恢复；
              C12 golden 场景按 backup-first 策略更新）
           · MSIX/签名/UpgradeCoordinator                        ◐
             （make_msix.py 无签名打包 ✅ + update_handoff
              写/读/consume-once 协议 + 启动消费接线 ✅；
              证书签名 + 完整升级状态机 = 外部步骤/后续）
           · 30min/4h 资源长剖面                                  ✅
             （1800s 档已跑并入档 benchmarks/c11-soak-30min-run.log；
              4h 档保留为 nightly 项）
           ── Non-blocking (2.x) ──
           · Provider Contract v2 / Catalog incremental
           · Agent product integration / Durable Workflow / Marketplace

── P2.4 Foundation Closure（spec: files2/01-P2.4-DESIGN-SPEC.md）────
P2.4-0     Baseline/Documentation Closure               ✅ (= GA Closure, 90号)
P2.4-A     Application Catalog 2.0                      ✅ Batch 1+2
           (A01 身份五级precedence + A02 schema v2迁移 +
            A03 observations reconcile + A04 lifecycle +
            A05 权威读路径/generation接线 + A06 恢复
            conformance —— 见 91/92 号)
P2.4-C     Plugin Control Plane 2.0                     ◐
           (C02 trust 阶梯 + C03 capability 决策持久化 ✅
            91号；C04 quarantine/ C05 backup-restore ✅ 89号；
            C01 lifecycle 整合 / C06 诊断 ⏳)
P2.4-B     File Index Maintenance 2.0                   ✅ Batch 2
           (B01/B04 root 状态机 Unavailable→重现恢复 +
            B02 watcher 重注册有界退避 + B05 健康模型
            全字段 + B06 soak —— 见 92 号；B03 DirtyRoot
            P2.1-B 已有)
P2.4-C     Plugin Control Plane 2.0                     ◐
           (C02 trust 阶梯 + C03 capability 决策持久化 ✅
            91号；C04 quarantine/ C05 backup-restore ✅ 89号；
            C01 lifecycle 整合 / C06 诊断 ⏳)
P2.4-D/E/F SDK CLI+诊断 / Release Closure                 ⏳ 未开始
```

# 7.5 P2.1/P2.2 文档归档（review 82 §26 采纳）

P2.1/P2.2 阶段的定稿设计规范与批次实施记录已归档：

- `docs/specs/` — 9 份已定稿设计规范（带 IMPLEMENTED / PARTIALLY IMPLEMENTED 状态头）
- `docs/history/` — 批次实施记录（63→87，按时间序，含每批测试基线）
- `docs/README.md` — 导航 + 状态矩阵摘要

仍活跃的评审/规划文档在 `files2/`（P2.2-D/E 主体、P2.1-D.1 持久 Catalog、P2.3 规划）。
四态状态模型（DESIGNED → IMPLEMENTED → INTEGRATED → ACCEPTED）自本手册 2026-09-07 修订起生效，不再用单一 ✅ 表达批次完成度。

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
cargo test --workspace          # 行为回归闸门（当前 617 全绿）
cargo build --workspace         # 零警告检查
python scripts/check_topology.py  # 拓扑 + SDK 依赖守卫
python scripts/release_gate.py    # Launcher 1.0 Release Gate（G01~G12 + evidence/manifest）
LAUNCHER_SNAPSHOT_DIR=<dir> ./target/debug/launcher-app   # 10 张 VR 基线
LAUNCHER_PYTHON=<python> cargo test -p launcher-plugin-host  # Python SDK E2E
```

# 10. 新会话开工须知（红线速查）

1. 不改契约语义除非先过评审 + ADR（INV-009/010）；UI/VISUAL/WORKFLOW/ACTION/COMMAND/PLUGIN 六份 FROZEN 契约是现状，不是提案。
2. 一切执行经 Resolver → Engine；任何"为了方便"的直连 Effect 都是架构回归（INV-033/044/066）。
3. 所有跨边界引用用 stable id（command_id/action_id/run_id/execution_id）；provider_id 永远 Host 权威。
4. Capability 单调性：requires ⊆ manifest；metadata（MCP annotations 等）只是 Policy 输入。
5. UI 永远是 projection：不解析语义、不判 capability、不执行 Effect（INV-035~037/062~065）。
6. 每轮收尾：`cargo test --workspace` 全绿 + `cargo build` 零警告 + `check_topology.py` 通过。
