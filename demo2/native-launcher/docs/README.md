# docs/README — 文档导航（P2.1/P2.2 阶段归档）

> 本目录收录 P2.1 / P2.2 阶段**已定稿归档**的设计规范与批次实施记录。
> 归档为**移动**语义：原件已从 `files2/` 移除，此处是唯一权威副本。
> `files2/` 仅保留仍活跃的评审/规划文档（60/61/62/64/68/70/72/74/80/82 号等，'
'含未完成的 P2.2-D/E 主体、P2.1-D.1 持久 Catalog、P2.3 规划）。
>
> 状态标签（review 82 §22-23 采纳）：**DESIGNED → IMPLEMENTED → INTEGRATED → ACCEPTED** 四态分离，不再用单一 ✅ 表达。

## specs/ — 已定稿设计规范（含实施状态头）

| 文件 | 状态 |
|---|---|
| `P2.1-B-Incremental-Index-Engine-v0.1.md` | **IMPLEMENTED (v0.1)**；后置 merge-iterator recovery、root 重生细化 |
| `P2.1-C-SearchCoordinator.md` | **PARTIALLY IMPLEMENTED**；语义去重/空查询/失败隔离已落地，Contract v2 挂账 |
| `P2.1-D-Application-Discovery-2.0.md` | **PARTIALLY IMPLEMENTED**；PackageManager+Portable+exe 归并已落地，持久 Catalog 挂账（P2.1-D.1） |
| `P2.1-E-Icon-Cache-and-Performance.md` | **PARTIALLY IMPLEMENTED**；后端管线 E1-E4 已落地，E5 UI 接线挂账（随 VR 重生成） |
| `P2.2-A-Favorites-Pinning-v0.1.md` | **IMPLEMENTED (core)**；展示区/reorder UI/stale 呈现挂账 |
| `P2.2-B-Query-Cache-v0.1.md` | **IMPLEMENTED (core)**；generation 全字段（含 application/ranking）；SingleFlight 明确不做（final-snapshot-only） |
| `P2.2-C-Context-aware-Ranking-v0.1.md` | **PARTIALLY IMPLEMENTED**；FolderProximity/foreground/语义代际已落地，TimeBucket 挂账 |
| `P2.2-D-Plugin-Ecosystem-Management-v0.1.md` | **PARTIALLY IMPLEMENTED**（Runtime Safety Slice）；Registry/Package/Capability 主体未实施 |
| `P2.2-E-Installer-Upgrade-Recovery-v0.1.md` | **PARTIALLY IMPLEMENTED**（Bootstrap/Recovery Foundations）；MSIX/UpgradeCoordinator 未实施 |

## history/ — 批次实施记录（定稿，按时间序）

| 文件 | 内容 | 批次结果 |
|---|---|---|
| `63-p2-launcher10-review.md` | P2 定位评审 + 搜索/Windows 集成第一批 | ✅ 477 tests |
| `65-p2-fix-implementation.md` | 64 号评审 8 项修复（ExecutionId 单源等） | ✅ 512 tests |
| `66-launcher10-acceptance-audit.md` | 宿主接线审计 + MUST/SHOULD/DEFERRED 冻结清单 | ✅ 验收材料 |
| `67-launcher10-closure.md` | MUST-1..4（Workflow 触发/Settings 闭环/安装包/基线） | ✅ 514 tests |
| `69-fix-bug-response.md` | 68 号评审核实（Mutex 生命周期/启动重建移出主链等） | ✅ 514 tests |
| `71-p21-foundation-batch1.md` | P2.1 第一批（空查询 recents/身份归并/RankingWeights） | ✅ 520 tests |
| `73-p21-fix-response.md` | 72 号收口小项（路径身份规则/INV-SEARCH-003/004） | ✅ 526 tests |
| `75-p21-q-response.md` | 74 号验收（Batch 1 CLOSED）+ 设备命名空间规则 | ✅ 527 tests |
| `77-p21b-incremental-index.md` | **P2.1-B Incremental Index Engine v0.1**（watcher E2E） | ✅ 535 tests |
| `78-p21c-response.md` | P2.1-C 语义身份去重切片 | ✅ 539 tests |
| `79-p21d-response.md` | P2.1-D 第一批（MSIX/PackageManager + Portable） | ✅ 547 tests |
| `80-p21e-response.md` | P2.1-E Icon 管线后端（E1-E4）+ 基线记录 | ✅ 552 tests |
| `81-p22-batches-response.md` | P2.2-A~E 核心子集（Favorites/Cache/Context/Quarantine/Health） | ✅ 565 tests |
| `82-p22f-response.md` | P2.2-F Cross-Batch Contract Closure（CacheKey 补齐/Context 代际/合并保元数据） | ✅ 566 tests |
| `89-rc1-closure.md` | **RC1 挂账收口**：Registry backup/restore + update_handoff 协议与启动接线 + MSIX 打包（无签名）+ 30min soak | ✅ 623 tests |
| `90-ga-closure.md` | **1.0 GA Closure**：GA-1/2/3/4/5/7 完成（文档统一/hotkey P50 307µs/10k UI soak/indexer crash 注入/CI/released manifest）；GA-6 签名暂缓 | ✅ gate 全 PASS |
| `91-p24-batch1-catalog.md` | **P2.4 Batch 1**：Application Catalog 2.0（A01 身份/A02 schema v2/A03 reconcile/A04 lifecycle/A05 权威读路径）+ C02 trust/C03 capability 决策 | ✅ 638 tests |
| `92-p24-batch2-index.md` | **P2.4 Batch 2**：File Index 2.0（B01/B04 root 状态机 + B02 watcher 重注册退避 + B05 健康模型 + B06 soak）+ A06 catalog 恢复 conformance | ✅ 642 tests |
| `93-p24-batch3-cli.md` | **P2.4 Batch 3**：C01 installation revision + C06 结构化诊断 + launcher-plugin CLI（D01-D05，新 app） | ✅ 654 tests |
| `94-p24-batch4-ef-closure.md` | **P2.4 收口**：E01-E05 诊断打通/快照落盘/Replay + F（G13 P2.4 conformance 入 gate） | ✅ 655 tests / G01-G13 PASS |
| `95-p25-batch1-query-intelligence.md` | **P2.5 Batch 1**：Search Contract v2（P25-001）+ Query Normalizer v2（A01）+ Intent Detector（A02）+ 显式 Filter（A03） | ✅ 671 tests |
| `96-p25-batch2-coordinator.md` | **P2.5 Batch 2**：SearchCoordinator（B01 骨架/B02 有界并发 fan-out/B03 panic 隔离/B04 取消守卫/B06 路由 v0.1） | ✅ 676 tests |
| `97-p25-batch3-fts.md` | **P2.5 Batch 3**：FTS5 检索层（C01 schema/增量同步/自愈 + C02 catalog FTS + C05 LIKE 合并回退 + C06 扩展点） | ✅ 679 tests |
| `98-p25-batch4-ranking.md` | **P2.5 Batch 4**：Ranking v2（D02 权重集中+版本化配置回退 + D03 ScoreParts 同源解释 + D05 质量语料回归） | ✅ 687 tests |
| `99-p25-batch5-ef-closure.md` | **P2.5 收口**：E03/E04 并发 stress + supersede race + E05 FTS 恢复 + F（G14 P2.5 conformance 入 gate，G01~G14 PASS）；Pinyin 推迟 | ✅ 690 tests |
| `100-p26-batch1-graph.md` | **P2.6 Batch 1**：Workflow 2.0 图模型 + DAG 验证器（A01/A02，DAG-only 政策） | ✅ 701 tests |
| `101-p27-batch1-ai-contract.md` | **P2.7 Batch 1**：评审（P2.6 依赖裁决）+ AI Contract v1（P27-001：Intent/PlanStep/AgentProposal/RiskLevel，fail-closed 校验） | ✅ 705 tests |
| `117-p27-batch2-intent.md` | **P2.7 Batch 2**：Intent/Entity 模型（A03，确定性规则解析，无 LLM/网络） | ✅ 755 tests |
| `118-p27-batch3-structured-output.md` | **P2.7 Batch 3**：结构化输出校验器（A05，LLM 输出唯一通道 fail-closed） | ✅ 763 tests |
| `119-p27-batch4-clarification.md` | **P2.7 Batch 4**：Clarification Engine（A06，确定性澄清决策） | ✅ 767 tests |
| `120-p27-batch5-prompt-builder.md` | **P2.7 Batch 5**：Prompt Builder + 注入清洗（A04/E06-lite/A02 上下文预算） | ✅ 770 tests |
| `121-p27-batch6-agent-session.md` | **P2.7 Batch 6**：Agent Session 状态机（C01/C02，白名单迁移+步预算） | ✅ 773 tests |
| `133-p27-batch7-risk-classifier.md` | **P2.7 Batch 7**：B06 Risk Classifier（L0-L4 确定性分类，未知 fail-closed） | ✅ 793 tests |
| `134-p27-batch8-pipeline.md` | **P2.7 Batch 8**：Agent Pipeline 串联（Intent→澄清→Prompt→LLM→校验→风险分类，注入式 LLM 闭包） | ✅ 797 tests |
| `102-p28-kickoff.md` | **P2.8 开工**：三份文档评审（69 任务/11 组）+ 依赖裁决（与 P2.6/P2.7 可并行）+ 6 批次计划 | 📋 计划冻结 |
| `102-p28-kickoff.md` | **P2.8 开工**：三份文档评审（69 任务/11 组）+ 依赖裁决（与 P2.6/P2.7 可并行）+ 6 批次计划 | 📋 计划冻结 |
| `103-p29-kickoff.md` | **P2.9 开工**：系统集成层评审（Capability Resolver/Adapter 架构/风险分类）+ 6 批次计划 | 📋 计划冻结 |
| `104-p29-batch1-contract.md` | **P2.9 Batch 1**：系统集成契约基座（Capability/Risk/Target/Command/Resolver 骨架/Mock，纯类型零 OS） | ✅ 708 tests |
| `105-p29-batch2-file-adapter.md` | **P2.9 Batch 2**：File Adapter（SystemCommand → 既有 host-owned Action 映射，fail-closed） | ✅ 711 tests |
| `122-p29-batch3-process-window.md` | **P2.9 Batch 3**：Process/Window 能力分类与命令构建（fail-closed taxonomy） | ✅ 776 tests |
| `123-p29-batch4-shell-power.md` | **P2.9 Batch 4**：Shell/URI/Notification/Power 分类与命令构建（fail-closed） | ✅ 779 tests |
| `124-p29-batch5-policy.md` | **P2.9 Batch 5**：System Policy（origin 白名单+风险上限）+ origin 传播 | ✅ 780 tests |
| `125-p29-batch6-closure.md` | **P2.9 收口**：Race/Security/Fault 全矩阵 + G16 入 gate（G01~G16 PASS）；Windows Adapter 后置 | ✅ 784 tests |
| `130-p29-b6-p26-e02.md` | **P2.9 Batch 6 落档 + P2.6 E02**：Editor 宿主投影层（EditorSurface/EditorRow，ViewModel 模式，E05 验证面前身） | ✅ 790 tests |
| `131-p26-batch10-editor-surface.md` | **P2.6 Batch 10**：E03 GraphEditorSurface（Slint 列表式编辑器 VIEW + 回调） | ✅ 790 tests |
| `132-p26-batch11-closure.md` | **P2.6 收口**：E04/E05 接线证明 + G17 workflow conformance 入 gate（G01~G17 PASS）＝ P2.6 完成 | ✅ 791 tests |
| `106-p26-batch2-engine.md` | **P2.6 Batch 2**：执行语义（A03 WaitAll Join/skip 穿透 + A04 条件引擎 + A05 VariableStore） | ✅ 715 tests |
| `107-p26-batch3-durable-store.md` | **P2.6 Batch 3**：B01 Durable Run Store（SQLite checkpoint 持久化/按状态恢复扫描/损坏重建）+ A06 契约 kit | ✅ 718 tests |
| `108-p26-batch4-scheduler.md` | **P2.6 Batch 4**：Durable Scheduler（B02 每节点 checkpoint + B03 执行循环 + B05 失败策略 + B06 暂停/恢复不重执行） | ✅ 722 tests |
| `109-p26-batch5-approval.md` | **P2.6 Batch 5**：Human Approval（C01 契约/C02 存储/C03 面向/C04 执行安全：approve 执行、reject 跳过、restart-safe） | ✅ 727 tests |
| `110-p26-batch6-expiry-triggers.md` | **P2.6 Batch 6**：C05 审批过期/取消（fail-closed）+ D01/D02 Trigger 契约与持久 FIFO 队列 | ✅ 729 tests |
| `126-p26-batch7-parallel.md` | **P2.6 Batch 7**：B04 Parallel 执行（ready 集合并发 + 确定性输出应用 + StepExecutor: Send） | ✅ 784 tests |
| `127-p26-batch8-triggers.md` | **P2.6 Batch 8**：触发源消费接线（trigger-service 后台线程 → start_workflow 管线） | ✅ 784 tests |
| `128-p26-batch9-editor-state.md` | **P2.6 Batch 9**：E01/E07 Graph Editor 状态 + 有界 Undo/Redo + E06 导入导出（§10 门在 export） | ✅ 787 tests |
| `129-p26-eline-handoff.md` | **P2.6 E 线交接**：E01/E07/E06 ✅；E02-E05 Slint VIEW 接线说明（逻辑就绪只差绑定） | 📋 交接文档 |
| `111-p28-batch1-foundation.md` | **P2.8 Batch 1**：Foundation（PluginIdentity + SHA-256 Integrity + 生命周期状态机 fail-closed） | ✅ 734 tests |
| `112-p28-batch2-resolver.md` | **P2.8 Batch 2**：Dependency Resolver + InstallPlan（确定性拓扑/缺失/环可解释/共享依赖单次） | ✅ 740 tests |
| `113-p28-batch3-transactional-install.md` | **P2.8 Batch 3**：事务化 Install（stage→activate + 逆序回滚，回滚失败可报告） | ✅ 743 tests |
| `114-p28-batch4-trust.md` | **P2.8 Batch 4**：Trust Model fail-closed 矩阵 + 生命周期管理器（§8/§9/§35） | ✅ 745 tests |
| `115-p28-batch5-repository.md` | **P2.8 Batch 5**：Repository Index（原子持久化/发布校验和）+ Marketplace 搜索（确定性/trust badge） | ✅ 748 tests |
| `105-p29-batch2-file-adapter.md` | **P2.9 Batch 2**：File Adapter（SystemCommand → 既有 host-owned Action 映射，fail-closed） | ✅ 711 tests |
| `106-p26-batch2-engine.md` | **P2.6 Batch 2**：执行语义（A03 WaitAll Join/skip 穿透 + A04 条件引擎 + A05 VariableStore） | ✅ 715 tests |
| `107-p26-batch3-durable-store.md` | **P2.6 Batch 3**：B01 Durable Run Store（SQLite checkpoint 持久化/按状态恢复扫描/损坏重建）+ A06 契约 kit | ✅ 718 tests |
| `108-p26-batch4-scheduler.md` | **P2.6 Batch 4**：Durable Scheduler（B02 每节点 checkpoint + B03 执行循环 + B05 失败策略 + B06 暂停/恢复不重执行） | ✅ 722 tests |
| `109-p26-batch5-approval.md` | **P2.6 Batch 5**：Human Approval（C01 契约/C02 存储/C03 面向/C04 执行安全：approve 执行、reject 跳过、restart-safe） | ✅ 727 tests |
| `110-p26-batch6-expiry-triggers.md` | **P2.6 Batch 6**：C05 审批过期/取消（fail-closed）+ D01/D02 Trigger 契约与持久 FIFO 队列 | ✅ 729 tests |

## 权威文档（不在本目录）

- 冻结契约与不变量：`docs/` 根目录（PLUGIN-CONTRACT / UI-CONTRACT / INVARIANTS 等）
- 项目手册（里程碑/红线/闸门）：`docs/PROJECT-HANDBOOK.md`
- State Generation Matrix：见 `history/82-p22f-response.md` §三
