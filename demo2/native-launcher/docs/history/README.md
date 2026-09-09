# docs/README — 文档导航（批次记录归档）

> 本目录收录各阶段**已定稿归档**的设计规范与批次实施记录。
> `files2/` 仅保留仍活跃的评审/规划文档。
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
| `86-p23-launch-record.md` | **P2.3 Launch**：Release Gate G01~G12 首跑 | ✅ RC1 |
| `89-rc1-closure.md` | **RC1 挂账收口**：Registry backup/restore + update_handoff 协议与启动接线 + MSIX 打包（无签名）+ 30min soak | ✅ 623 tests |
| `90-ga-closure.md` | **1.0 GA Closure**：GA-1/2/3/4/5/7 完成；GA-6 签名暂缓 | ✅ gate 全 PASS |
| `91-p24-batch1-catalog.md` | **P2.4 Batch 1**：Application Catalog 2.0 + C02/C03 | ✅ 638 tests |
| `92-p24-batch2-index.md` | **P2.4 Batch 2**：File Index 2.0 + A06 catalog 恢复 | ✅ 642 tests |
| `93-p24-batch3-cli.md` | **P2.4 Batch 3**：C01 revision + C06 diagnostics + launcher-plugin CLI | ✅ 654 tests |
| `94-p24-batch4-ef-closure.md` | **P2.4 收口**：E01-E05 诊断打通/快照/Replay + G13 gate | ✅ 655 tests |
| `95-p25-batch1-query-intelligence.md` | **P2.5 Batch 1**：Search Contract v2 + Query Normalizer + Intent + Filter | ✅ 671 tests |
| `96-p25-batch2-coordinator.md` | **P2.5 Batch 2**：SearchCoordinator（B01-B06） | ✅ 676 tests |
| `97-p25-batch3-fts.md` | **P2.5 Batch 3**：FTS5 检索层（C01/C02/C05/C06） | ✅ 679 tests |
| `98-p25-batch4-ranking.md` | **P2.5 Batch 4**：Ranking v2（D02/D03/D05） | ✅ 687 tests |
| `99-p25-batch5-ef-closure.md` | **P2.5 收口**：E03/E04 stress/race + E05 FTS 恢复 + F（G14 入 gate）；Pinyin 推迟 | ✅ 690 tests |
| `100-p26-batch1-graph.md` | **P2.6 Batch 1**：图模型 + DAG 验证器 | ✅ 701 tests |
| `101-p27-batch1-ai-contract.md` | **P2.7 Batch 1**：AI Contract v1 | ✅ 705 tests |
| `102-p28-kickoff.md` | **P2.8 开工**：评审 + 6 批次计划 | 📋 计划冻结 |
| `103-p29-kickoff.md` | **P2.9 开工**：评审 + 6 批次计划 | 📋 计划冻结 |
| `104-p29-batch1-contract.md` | **P2.9 Batch 1**：系统集成契约基座 | ✅ 708 tests |
| `106-p26-batch2-engine.md` | **P2.5 Batch 2**：执行语义（Join/条件/变量） | ✅ 715 tests |
| `107-p26-batch3-durable-store.md` | **P2.6 Batch 3**：Durable Run Store + A06 | ✅ 718 tests |
| `108-p26-batch4-scheduler.md` | **P2.6 Batch 4**：Durable Scheduler（B02-B06） | ✅ 722 tests |
| `109-p26-batch5-approval.md` | **P2.6 Batch 5**：Human Approval | ✅ 727 tests |
| `110-p26-batch6-expiry-triggers.md` | **P2.6 Batch 6**：C05 审批过期 + Trigger 队列 | ✅ 729 tests |
| `111-p28-batch1-foundation.md` | **P2.8 Batch 1**：PluginIdentity + Integrity + 状态机 | ✅ 734 tests |
| `112-p28-batch2-resolver.md` | **P2.8 Batch 2**：Dependency Resolver + InstallPlan | ✅ 740 tests |
| `113-p28-batch3-transactional-install.md` | **P2.8 Batch 3**：事务化 Install（stage→activate→回滚） | ✅ 743 tests |
| `114-p28-batch4-trust.md` | **P2.8 Batch 4**：Trust Model fail-closed 矩阵 | ✅ 745 tests |
| `115-p28-batch5-repository.md` | **P2.8 Batch 5**：Repository Index + Marketplace 搜索 | ✅ 748 tests |
| `116-p28-batch6-closure.md` | **P2.8 收口**：Audit + 集成 E2E + G15 入 gate | ✅ 751 tests |
| `117-p27-batch2-intent.md` | **P2.7 Batch 2**：Intent/Entity 模型 | ✅ 755 tests |
| `118-p27-batch3-structured-output.md` | **P2.7 Batch 3**：结构化输出校验器 | ✅ 763 tests |
| `119-p27-batch4-clarification.md` | **P2.7 Batch 4**：Clarification Engine | ✅ 767 tests |
| `120-p27-batch5-prompt-builder.md` | **P2.7 Batch 5**：Prompt Builder + 注入清洗 | ✅ 770 tests |
| `121-p27-batch6-agent-session.md` | **P2.7 Batch 6**：Agent Session 状态机 | ✅ 773 tests |
| `122-p29-batch3-process-window.md` | **P2.9 Batch 3**：Process/Window 分类 | ✅ 776 tests |
| `123-p29-batch4-shell-power.md` | **P2.9 Batch 4**：Shell/Power 分类 | ✅ 779 tests |
| `124-p29-batch5-policy.md` | **P2.9 Batch 5**：System Policy + origin 传播 | ✅ 780 tests |
| `125-p29-batch6-closure.md` | **P2.9 收口**：Race/Security/Fault + G16 入 gate | ✅ 784 tests |
| `126-p26-batch7-parallel.md` | **P2.6 Batch 7**：B04 Parallel 执行 | ✅ 784 tests |
| `127-p26-batch8-triggers.md` | **P2.6 Batch 8**：触发源消费接线 | ✅ 784 tests |
| `128-p26-batch9-editor-state.md` | **P2.6 Batch 9**：Graph Editor 状态 + Undo/Redo + E06 导入导出 | ✅ 787 tests |
| `129-p26-eline-handoff.md` | **P2.6 E 线交接**：E01/E07/E06 ✅；E02-E05 说明 | 📋 交接 |
| `130-p29-b6-p26-e02.md` | **P2.9 Batch 6 落档 + P2.6 E02**：Editor 宿主投影层 | ✅ 790 tests |
| `131-p26-batch10-editor-surface.md` | **P2.6 Batch 10**：GraphEditorSurface（Slint VIEW + 回调） | ✅ 790 tests |
| `132-p26-batch11-closure.md` | **P2.6 收口**：E04/E05 接线证明 + G17 入 gate ＝ P2.6 完成 | ✅ 791 tests |
| `133-p27-batch7-risk-classifier.md` | **P2.7 Batch 7**：B06 Risk Classifier | ✅ 793 tests |
| `134-p27-batch8-pipeline.md` | **P2.7 Batch 8**：Agent Pipeline 串联 | ✅ 797 tests |
| `135-p27-batch9-p26f-stress.md` | **P2.7 Batch 9 + P2.6-F**：durable stress/recovery + pipeline 风险收敛 | ✅ 800 tests |
| `137-p26-final-closure.md` | **P2.6 最终收口**：F01 Diagnostics + F02-F06 QA ＝ **P2.6 完成** | ✅ 804 tests |
| `138-p27-batch11-provider-caps.md` | **P2.7 Batch 11**：A01 Provider Capabilities + Health Check | ✅ 806 tests |
| `90-ga-closure.md` | **1.0 GA Closure**（历史里程碑：GA-1/2/3/4/5/7 完成，GA-6 签名暂缓） | ✅ 790 tests @ GA |
| `91-p24-batch1-catalog.md` | **P2.4 Batch 1**：Catalog 2.0 + trust/capability | ✅ 638 tests |
| `92-p24-batch2-index.md` | **P2.4 Batch 2**：File Index 2.0 | ✅ 642 tests |
| `93-p24-batch3-cli.md` | **P2.4 Batch 3**：launcher-plugin CLI + C01/C06 | ✅ 654 tests |
| `94-p24-batch4-ef-closure.md` | **P2.4 收口**：E01-E05 + G13 gate | ✅ 655 tests |
| `95-p25-batch1-query-intelligence.md` | **P2.5 Batch 1**：Search Contract v2 + Query Intelligence | ✅ 671 tests |
| `96-p25-batch2-coordinator.md` | **P2.5 Batch 2**：SearchCoordinator（B01-B06） | ✅ 676 tests |
| `97-p25-batch3-fts.md` | **P2.5 Batch 3**：FTS5 检索层 | ✅ 679 tests |
| `98-p25-batch4-ranking.md` | **P2.5 Batch 4**：Ranking v2（D02/D03/D05） | ✅ 687 tests |
| `99-p25-batch5-ef-closure.md` | **P2.5 收口**：stress/race/FTS 恢复 + G14 入 gate；Pinyin 推迟 | ✅ 690 tests |
| `100-p26-batch1-graph.md` | **P2.6 Batch 1**：图模型 + DAG 验证器 | ✅ 701 tests |
| `101-p27-batch1-ai-contract.md` | **P2.7 Batch 1**：AI Contract v1 | ✅ 705 tests |
| `102-p28-kickoff.md` | **P2.8 开工**：评审 + 6 批次计划 | 📋 冻结 |
| `103-p29-kickoff.md` | **P2.9 开工**：评审 + 6 批次计划 | 📋 冻结 |
| `104-p29-batch1-contract.md` | **P2.9 Batch 1**：系统集成契约基座 | ✅ 708 tests |
| `106-p26-batch2-engine.md` | **P2.6 Batch 2**：执行语义 | ✅ 715 tests |
| `107-p26-batch3-durable-store.md` | **P2.6 Batch 3**：Durable Run Store + A06 | ✅ 718 tests |
| `108-p26-batch4-scheduler.md` | **P2.6 Batch 4**：Durable Scheduler | ✅ 722 tests |
| `109-p26-batch5-approval.md` | **P2.6 Batch 5**：Human Approval | ✅ 727 tests |
| `110-p26-batch6-expiry-triggers.md` | **P2.6 Batch 6**：C05 + Trigger 队列 | ✅ 729 tests |
| `111-p28-batch1-foundation.md` | **P2.8 Batch 1**：Foundation | ✅ 734 tests |
| `112-p28-batch2-resolver.md` | **P2.8 Batch 2**：Dependency Resolver | ✅ 740 tests |
| `113-p28-batch3-transactional-install.md` | **P2.8 Batch 3**：事务化 Install | ✅ 743 tests |
| `114-p28-batch4-trust.md` | **P2.8 Batch 4**：Trust Model | ✅ 745 tests |
| `115-p28-batch5-repository.md` | **P2.8 Batch 5**：Repository + 搜索 | ✅ 748 tests |
| `116-p28-batch6-closure.md` | **P2.8 收口**：Audit + G15 入 gate | ✅ 751 tests |
| `117-p27-batch2-intent.md` | **P2.7 Batch 2**：Intent/Entity 模型 | ✅ 755 tests |
| `118-p27-batch3-structured-output.md` | **P2.7 Batch 3**：结构化输出校验器 | ✅ 763 tests |
| `119-p27-batch4-clarification.md` | **P2.7 Batch 4**：Clarification Engine | ✅ 767 tests |
| `120-p27-batch5-prompt-builder.md` | **P2.7 Batch 5**：Prompt Builder + 注入清洗 | ✅ 770 tests |
| `121-p27-batch6-agent-session.md` | **P2.7 Batch 6**：Agent Session 状态机 | ✅ 773 tests |
| `122-p29-batch3-process-window.md` | **P2.9 Batch 3**：Process/Window 分类 | ✅ 776 tests |
| `123-p29-batch4-shell-power.md` | **P2.9 Batch 4**：Shell/Power 分类 | ✅ 779 tests |
| `124-p29-batch5-policy.md` | **P2.9 Batch 5**：System Policy + origin 传播 | ✅ 780 tests |
| `125-p29-batch6-closure.md` | **P2.9 收口**：Race/Security/Fault + G16 | ✅ 784 tests |
| `126-p26-batch7-parallel.md` | **P2.6 Batch 7**：B04 Parallel 执行 | ✅ 784 tests |
| `127-p26-batch8-triggers.md` | **P2.6 Batch 8**：触发源消费接线 | ✅ 784 tests |
| `128-p26-batch9-editor-state.md` | **P2.6 Batch 9**：Editor 状态 + Undo/Redo + E06 | ✅ 787 tests |
| `129-p26-eline-handoff.md` | **P2.6 E 线交接**：说明 | 📋 |
| `130-p29-b6-p26-e02.md` | **P2.9 Batch 6 + P2.6 E02**：Editor 投影层 | ✅ 790 tests |
| `131-p26-batch10-editor-surface.md` | **P2.6 Batch 10**：Slint surface | ✅ 790 tests |
| `132-p26-batch11-closure.md` | **P2.6 收口**：E04/E05 + G17 ＝ P2.6 完成 | ✅ 791 tests |
| `133-p27-batch7-risk-classifier.md` | **P2.7 Batch 7**：Risk Classifier | ✅ 793 tests |
| `134-p27-batch8-pipeline.md` | **P2.7 Batch 8**：Agent Pipeline | ✅ 797 tests |
| `135-p27-batch9-p26f-stress.md` | **P2.7 Batch 9 + P2.6-F**：stress/recovery + 风险收敛 | ✅ 800 tests |
| `137-p26-final-closure.md` | **P2.6 最终收口**：F01 + F/G ＝ P2.6 完成 | ✅ 804 tests |
| `138-p27-batch11-provider-caps.md` | **P2.7 Batch 11**：Provider Capabilities + Health Check | ✅ 806 tests |

## 权威文档（不在本目录）

- 冻结契约与不变量：`docs/` 根目录（PLUGIN-CONTRACT / UI-CONTRACT / INVARIANTS 等）
- 项目手册（里程碑/红线/闸门）：`docs/PROJECT-HANDBOOK.md`
- State Generation Matrix：见 `history/82-p22f-response.md` §三
