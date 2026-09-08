# Launcher 1.0 Final Audit（1.0 冻结基线）

> **本文件用途**：把当前 `main`（commit `d7d20eb`，Native Launcher 1.0.0 RC1）逐条对照原始
> `01-design-spec-v0.1.md`、`03-test-plan-v0.1.md`、P2.3/1.0 冻结契约与最新代码，
> 冻结为一份可被 Agentic Coding 直接消费的 **1.0 baseline**。
>
> 状态模型（五态）：
> - **Closed** — 已实现、有测试/证据、契约冻结，1.0 GA 不再动。
> - **Core Slice** — 1.0 范围内只交付了核心切片，剩余部分已明确定义归属。
> - **Experimental** — 有真实代码与测试，但定位为探索/ foundation，不承诺产品级。
> - **Planned** — 原始规范要求，但 1.0 未实现，已正式推迟（多为 P2.4+）。
> - **Failed** — 尝试过且验收未通过（本审计中为空，无此类项）。
>
> 证据基线：623 tests 全绿 / zero warnings / topology 17 crates + 8 apps /
> S0=S1=S2=0 / G01~G12 PASS / 30min soak PASS（RSS 8.4→12.2MB）/
> app search p95 7µs、file search p95 23µs、idle RSS 26.4MB、真实进程启动 ≈152ms。
> 审计日期：2026-09-08。上游评审：`96-v1-reveiw.md`。

---

# 0. 总判定

> **Launcher 1.0 = Core Product Complete（RC1）**。
> 核心架构冻结、垂直闭环闭环、执行权边界（Producer → Resolver → Engine → Effect）冻结；
> 剩余工作是 **Release/QA/文档收口**，不是核心架构缺陷。
> 核心产品完成度 ≈92-95%；严格对照原始 v0.1 Test Plan ≈80-85%。

---

# A. 对照 01-design-spec-v0.1.md（逐节）

| # | 规范条目 | 状态 | 证据 / 差异说明 |
|---|---|---|---|
| §1 | Trigger→Query→Candidate→Command→Action→Effect 核心抽象 | **Closed** | `launcher-domain` Command/ActionDescriptor + ADR-0014 统一执行链；Producer(UI/Plugin/Workflow/AI/MCP) → Resolver → Engine 唯一 Effect 闸门 |
| §1.1 | 8 条产品原则（轻量常驻/搜索优先/Native UI/按需 runtime/插件隔离/可测试） | **Closed** | 软件渲染 idle RSS 26.4MB≤50MB；插件 Job Object 隔离；INV-001~069 |
| §2.1 | Rust stable + Slint + SQLite + tracing + cargo | **Closed** | 与规范一致 |
| §2.1 | **Search = SQLite FTS5** | **Failed→改约** | 实现为 bounded `LIKE` + 确定性 ranking（`launcher-indexer/src/lib.rs:165`），p95 23µs 实测达标。**文档漂移已裁决：1.0 = SQLite metadata + bounded LIKE；FTS5 推迟 P2.5**。禁止 agent 按 FTS5 重写 Indexer |
| §2.1 | CI GitHub Actions | **Planned** | 仓库无 `.github/workflows`；闸门以本地 `release_gate.py`（G01~G12）执行。GA 前需接入 CI（见 GA 清单） |
| §2.2 | 禁 Electron/CEF/WebView/嵌入 runtime/插件动态库进 Core | **Closed** | 无违例；拓扑与 LAYER_GUARDS 守卫 |
| §3 | UI/Core/Indexer/PluginHost/AI 进程边界 | **Core Slice** | UI+Core 合体（ADR-0002，规范明文允许）；plugin-host 独立进程 ✅；indexer-service app 存在但主链内嵌；AI 为 in-process module（proposal-only，无独立进程必要） |
| §4.1 | Idle Private ≤50MB / Hard ≤80MB / runtime 0MB 常驻 | **Closed** | 26.4MB 实测（D4）；Python/Node/AI 0 常驻 |
| §4.2 | 延迟初始化/有界缓存/按需 preview | **Closed** | IconService 8MB 硬预算（INV-ICON-004）；LRU query cache；bounded queue |
| §4.3 | 观测指标 9 项 | **Core Slice** | RSS/CPU/handle/thread/搜索延迟已采集（bench + soak）；**hotkey→UI P50/P95 未仪表化**（GA 清单）；IPC latency 随 UI+Core 合体弱化 |
| §5 | Command 模型 + Provider 四方法 + Action 七类 | **Closed** | COMMAND-CONTRACT v0.1 FROZEN；ACTION-CONTRACT v0.1 FROZEN；ActionPanel 只呈现（ADR-0012） |
| §6 | Context Engine 独立服务 + Snapshot | **Core Slice** | foreground app + Explorer 目录 + generation ✅；**selected items / 剪贴板 / 非 Explorer 前台未做**（KNOWN-ISSUES #7，推迟 P2.4/P2.8） |
| §7.1 | Search 流程（Normalize→…→Render） | **Core Slice** | Normalize/Score/Dedup/Rank/Limit ✅；**Detect intent / Provider 选择（策略路由）未做**，provider fan-out 为**顺序执行**（`Core::search` 注释明示 deterministic；P2.5-F 并发化） |
| §7.2 | MVP Intent 四类 | **Core Slice** | Category（Application/File/Command/Folder）存在并参与 type prior；无显式 intent 检测阶段 |
| §7.3 | 确定性 ranking（exact>prefix>token>fuzzy + frequency/recency/context/priority） | **Closed** | RankingWeights 显式化；INV-SEARCH-002（boost 不制造候选）；SEARCH-QUALITY fixture |
| §8 | Indexer Phase 1（扫描+SQLite+FTS+mtime+size） | **Closed** | 检索技术差异见 §2.1 行；字段齐全；Indexer 不做 UI/排序/Action |
| §8 | Indexer Phase 2（NTFS MFT/USN） | **Planned** | 未实现；**但已超额交付 Incremental Index v0.1（watcher+bounded queue+coalescer+generation）**，超出原 Phase 1 要求 |
| §8 | Indexer API 五方法 | **Closed** | search/status/rebuild + watcher 增量；get_metadata 由 files 表直接查询承担 |
| §9.1/9.2 | 插件分层；MVP 只做 Built-in + 一个 External Host | **Closed（超额）** | Native/Python 双 runtime + Rust SDK + Conformance Kit；WASM/Node 预留未做（符合规范"不要同时实现"） |
| §9.3 | 插件生命周期 + OnDemand + IdleTimeout | **Closed** | Plugin Contract §lifecycle；spawn cooldown / idle shutdown / 进程树回收 |
| §9.4 | Capability 声明 + Host 侧检查（allow/deny 即可） | **Closed** | requires⊆manifest 单调性（INV-027）+ registry capabilities 表 + Host 检查 |
| §10 | 声明式 Native UI Schema（无 HTML/无脚本） | **Closed** | 插件结果为数据结构；UI-CONTRACT v0.1 FROZEN；ActionPanel 无执行权 |
| §11 | Workflow 抽象（线性 steps，DAG 后置） | **Closed（按规范范围）** | WORKFLOW-CONTRACT v0.1 FROZEN：五对象+线性 Step+at-least-once；DAG/Editor/Durable = P2.6 |
| §12 | AI 独立边界，MVP 只定义接口 | **Closed（按规范范围，超额）** | launcher-ai + LlmProvider + LLMPlanner proposal-only（ADR-0016）；AI 产品化 = P2.7 |
| §13 | SQLite 核心表 + WAL/migration/bounded history | **Core Slice** | WAL ✅、bounded history ✅、schema_version 迁移 ✅；实际表集合与规范清单不同（files/plugins/favorites/catalog/plugins.db/catalog.db/index.db 分库），**无统一 migration framework**——分库各自 schema_version 已满足 1.0 |
| §14 | 错误分类 8 类 + 插件崩溃不杀 Core | **Closed** | Plugin Contract 8 类失败；crash soak→quarantine 持久化（C3）；C12 golden 20 场景×5 层 |
| §15 | tracing + 核心事件清单 | **Closed** | launcher.started 等事件落地；memory.sampled 由 soak 采集承担 |
| §16 | 安全基线 8 条 | **Closed** | 75 adversarial tests S0=S1=S2=0；路径 confinement（INV-013）；stdout 红线；metadata 非授权（INV-068） |
| §17 | 目录结构 | **Closed** | 实际 17 crates + 8 apps 为超集；testkit 由 example-testplugins + calculator-plus 承担 |
| §18 | 9 项 ADR 必须项 | **Closed** | ADR-0001~0018 全覆盖 |
| §19 | MVP 结束定义（垂直闭环 + 7 类测试） | **Closed** | 垂直闭环 ✅（含真实 GUI E2E + 键盘 walkthrough）；100-provider stress ✅（1000 mock providers 超额）；memory soak ✅ |

# B. 对照 03-test-plan-v0.1.md（逐节）

| # | 测试项 | 状态 | 证据 / 差异 |
|---|---|---|---|
| §3 | Domain/Search/Context/Plugin unit tests | **Closed** | 623 tests；Pinyin 显式 **Planned**（规范原文"预留"） |
| §4 | App/File/Context integration（含 Unicode/重名/大文件名） | **Closed** | real_effect_e2e（真实 FileProvider 索引）；fixture corpus（SEARCH-QUALITY） |
| §5 | IPC 9 项（malformed/timeout/disconnect/restart/dup id…） | **Closed** | launcher-ipc sanitize + Plugin Contract Kit 16 项（frame 限制/stdout 红线/优雅关闭） |
| §6 | Plugin contract：Normal/Slow/Crash/Malformed/Flood | **Closed** | Kit 全覆盖 + crash loop soak→quarantine（C3）+ MCP cash/breach 矩阵（C4） |
| §7 | UI smoke ×1000 循环 | **Core Slice** | popup 1000× soak（D5）✅；**专用 UI loop 自动化 + 10,000 show/hide soak 未做**（TESTING.md 自认，GA 清单） |
| §8.1 | Hotkey latency P50≤20ms / P95≤35ms | **Planned** | **未仪表化**（KNOWN-ISSUES #2）——GA 清单首位 |
| §8.2 | Search P50/P95 目标 | **Closed** | app p95 7µs、file p95 23µs（远优于 ≤10ms/≤50ms 目标） |
| §8.3 | IPC P50≤1ms | **Core Slice** | UI+Core 合体后同进程调用，规范目标以 plugin JSON-RPC 帧处理代替验证 |
| §9.1 | Idle 5min ≤50MB | **Closed** | 26.4MB + 双 soak（popup 1000×/memory trend） |
| §9.2 | 100,000 random queries soak | **Core Slice** | C11 soak 111,500 cycles（search/history/favorite churn，30min PASS）覆盖同等规模；随机 query 语料形态未单独固化 |
| §9.3 | 10,000 show/hide soak | **Planned** | 未做（GA 清单） |
| §9.4 | Plugin spawn/query/shutdown ×1000 | **Closed** | plugin spawn soak + MCP soak 100 cycles（G09） |
| §9.5 | 1000 mock providers stress | **Closed** | stress tests（G09） |
| §10 | Fault injection 10 项 | **Core Slice** | DB corruption/lock、plugin crash/hang/spam、Explorer 缺失 ✅（C6/C7/C12 golden 20×5）；**Indexer 崩溃重启自动化注入未做**（GA 清单） |
| §11 | Concurrency（100 并发 query 等） | **Core Slice** | query supersession（ADR-0004）+ SearchSession + stress ✅；100 并发专项未单列 |
| §12 | Fuzz/Property | **Core Slice** | adversarial 75 项（S0=S1=S2=0）覆盖 parser/path 性质；正式 fuzz 框架未引入 |
| §13 | benchmark baseline + 回归规则 | **Closed** | perf_baseline.py + thresholds.json 4 预算 + G10 全字段判定 + SHA-256 封存 |
| §14 | CI gates（fmt/clippy -D warnings/test） | **Core Slice** | 本地 release_gate G01（zero-warning build）等执行；fmt/clippy 未入 gate；GitHub Actions 缺（GA 清单，同 §2.1） |
| §15 | Manual UX 7 项 | **Core Slice** | 键盘 walkthrough（真实按键管线）+ DPI walkthrough ✅；真实显示器 DPI 矩阵 = 手动清单 |
| §16 | Release Acceptance 四组 | **Core Slice** | Functional/Performance ✅；Reliability：1000 UI loops ✅（等价 soak）、plugin crash ✅、**Indexer restart ✘**；Engineering：docs **未统一**（GA 清单）、benchmark ✅、known issues ✅、红线 ✅ |

# C. 对照 P2.3 / 1.0 契约与里程碑（PROJECT-HANDBOOK §7）

| 领域 | 状态 | 说明 |
|---|---|---|
| 六份 FROZEN 契约（Plugin/Command/Action/Workflow/UI/Visual） | **Closed** | 全部 FROZEN；ADR-0007~0018 权威链 |
| MVP1~4.4 基础设施六批（含 AI/Runtime/Auth/LLM） | **Closed** | ACCEPTED |
| P2.1-A~E Search Foundation | **Closed / Core Slice** | A/B ✅；C（SearchCoordinator Contract v2）、D（Catalog as Search SoT）、E（TimeBucket）尾项 → P2.4 |
| P2.2-A~F Product Capability | **Closed / Core Slice** | Favorites/Cache/Context/F 交叉收口 ✅；D 插件生态主体、E MSIX/UpgradeCoordinator 主体 → P2.4 |
| P2.3-A/E/F/G/H 架构/安全/UI QA/Release Eng | **Closed** | G12 证据链 + RC artifact 冻结 |
| P2.3-B Product E2E | **Core Slice** | golden path + real Effect E2E ✅；GUI E2E 常态化 ⏳ |
| P2.3-C Reliability | **Closed** | C3/C4/C6/C7/C11/C12 全收口 |
| P2.3-D Performance | **Closed** | D1~D7；启动 152ms / RSS 26.4MB |
| P2.3-I RC1 | **Core Slice** | 见 GA 清单 |
| Plugin Registry backup/restore | **Closed** | history/89：损坏 quarantine + .bak 恢复（C12 golden 已按 backup-first 更新） |
| Update Handoff / UpgradeCoordinator 消费端 | **Closed** | history/89：§25/§26/§50 协议 + 启动接线 + 启动健康裁决（§51） |
| MSIX 打包 | **Core Slice** | make_msix.py 无签名包 ✅；**签名 + 升级器宿主 = 外部步骤** |
| MCP（stdio/catalog/E2E/compat matrix） | **Closed** | 超出 1.0 必需范围 |
| AI Foundation（proposal-only） | **Experimental** | 真实代码+E2E，产品化（Agent/Query Understanding）= P2.7 |
| Streamable HTTP / OAuth | **Planned** | Phase 11+，MCP-COMPATIBILITY §3/§8 |
| 多语言 SDK 矩阵（TS/Go/C#） | **Planned** | 需外部环境 |
| Search Intelligence / Durable Workflow / Marketplace / Agent 产品 | **Planned** | P2.5~P2.7 / 3.x |

---

# D. 1.0 GA 前必须修复（GA Blockers）

> 判据：不修复则"1.0 GA"的声明不成立（文档失真、验收断链或原始 Test Plan 的
> Release Acceptance 硬项未过）。按优先级排序。

| # | 项 | 类型 | 动作 |
|---|---|---|---|
| GA-1 | **文档状态统一（Documentation Freeze）** | 文档漂移 | README 去掉 "v0.1 MVP" 改为 "1.0 RC1"；TESTING.md 的 62/62、各文档 355/582/623 测试数统一为单一权威出处（建议以 release_gate 产出为准并在文档中引用）；01-design-spec 的 FTS5 表述按本审计 A-§2.1 裁决改写（1.0=bounded LIKE，FTS5→P2.5）；ARCHITECTURE.md 同步 17 crates 现状 |
| GA-2 | **Hotkey latency 仪表化** | Test Plan §8.1 / Release Acceptance 硬项 | 实现 Ctrl+Space→visible popup 的 P50/P95 测量（目标 ≤20ms/≤35ms），入档 benchmarks；至少给出"实测值或明确已知限制" |
| GA-3 | **10,000 show/hide UI soak** | Test Plan §9.3 | 在 D5 popup soak 基础上扩到 10k 循环跑一次入档（基础设施已有） |
| GA-4 | **Indexer crash/restart 故障注入自动化** | Test Plan §10 / §16 | indexer-service 崩溃→重启→索引 eventual consistent 的自动用例（coordinator 已支持重启语义，补驱动） |
| GA-5 | **CI（GitHub Actions）** | Spec §2.1 / Test Plan §14 | `.github/workflows`：cargo build 零警告 + cargo test --workspace + check_topology；Release profile 手动触发 release_gate |
| GA-6 | **MSIX 证书签名路径** | P2.3-G 尾款 | 取得代码签名证书并 signtool 签名打包产物（打包与 handoff 消费端已就绪）；至少把"无签名=已知限制"写入 RELEASE-MANIFEST |
| GA-7 | **EVIDENCE/MANIFEST 重生成 + GA 宣告** | Release Eng | 上述完成后重跑 release_gate.py，将 EVIDENCE.json / RELEASE-MANIFEST 的 status 从 release-candidate 翻为 released，并在 PROJECT-HANDBOOK §7 记录 |

**明确不阻塞 GA**（即使 Test Plan 有名字）：100 并发专项、fuzz 框架、Pinyin、100k 随机 query 固化、fmt/clippy 入 gate（建议随手做，非硬项）。

# E. 正式推迟到 P2.4 及以后（Deferrals）

| 归属 | 项 | 理由 |
|---|---|---|
| **P2.4 Foundation** | Catalog as Search Source of Truth（Catalog 2.0：Discovery→Coordinator→Catalog→Search） | 1.0 已交付 persistence/generation/recovery；authoritative read path 是演进而非欠账 |
| **P2.4** | Search Contract v2（SearchCoordinator 主体：SearchRequestId、provider timeout 隔离、partial results） | 当前顺序 fan-out 在本地 provider 规模下达标（p95 23µs） |
| **P2.4** | Plugin Ecosystem 主体（Registry SQL 管理 UI、包管理 install/update、Capability 审批 UI） | 1.0 交付 Runtime Safety Slice + Registry 持久化 + backup/restore 已足够运行 |
| **P2.4** | Plugin Developer CLI / Dev Diagnostics（init/build/package/logs/inspect） | SDK/Kit/Reference ✅；工具链是生态项 |
| **P2.4** | Context 扩展（selected items、非 Explorer 前台、clipboard context） | KNOWN-ISSUES #7 |
| **P2.4** | Workflow 触发源框架（hotkey/plugin/AI/schedule 触发 pause/resume 全链路） | service 通道已就绪，缺触发源（review 32 决议） |
| **P2.5** | Search Intelligence（intent、策略路由、并发 fan-out、explainability、FTS5/内容索引、Pinyin） | 依赖 P2.4 Foundation |
| **P2.6** | Workflow 2.0（Visual Editor、DAG、Human Approval、Durable Execution） | 1.0 按规范只做线性 steps |
| **P2.7** | AI/Agent 产品化（Query Understanding、Agent runtime、Agent↔Workflow） | proposal-only foundation 已冻结；**2.x 不先做 AI**（先 Foundation） |
| **P2.8/2.9** | Windows Intelligence / File Intelligence（MFT/USN、NTFS 深度集成） | 超出 1.0 范围 |
| **Phase 11+** | MCP Streamable HTTP + OAuth、多语言 SDK 矩阵 | 外部依赖 |
| **3.x** | Marketplace / Cloud | — |

# F. 给 Agentic Coding 的使用规则

1. 本文件是 **1.0 唯一权威 baseline**：与 README/TESTING.md 等历史文档冲突时，以本文件 + `docs/PROJECT-HANDBOOK.md` 为准（GA-1 完成后以更新后的文档为准）。
2. **Closed 项不许重构**：六份 FROZEN 契约、Authority 链（Resolver→Engine）、C12 golden 语义只能按 ADR 流程演进。
3. **Failed 为空**：不存在"做过但没做成"的项；所有未完成项都是有序推迟，不要"补做" D/E 区的项来"修复 1.0"。
4. GA 工作顺序 = GA-1 → GA-7 顺序执行；每完成一项更新本文件勾选状态。
5. 一切新工作默认落在 P2.4 及以后，开工前先读本文件对应行确认不在 GA Blocker 清单里。
