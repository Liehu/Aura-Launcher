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
| `102-p28-kickoff.md` | **P2.8 开工**：三份文档评审（69 任务/11 组）+ 依赖裁决（与 P2.6/P2.7 可并行）+ 6 批次计划 | 📋 计划冻结 |
| `103-p29-kickoff.md` | **P2.9 开工**：系统集成层评审（Capability Resolver/Adapter 架构/风险分类）+ 6 批次计划 | 📋 计划冻结 |
| `104-p29-batch1-contract.md` | **P2.9 Batch 1**：系统集成契约基座（Capability/Risk/Target/Command/Resolver 骨架/Mock，纯类型零 OS） | ✅ 708 tests |
| `105-p29-batch2-file-adapter.md` | **P2.9 Batch 2**：File Adapter（SystemCommand → 既有 host-owned Action 映射，fail-closed） | ✅ 711 tests |
| `106-p26-batch2-engine.md` | **P2.6 Batch 2**：执行语义（A03 WaitAll Join/skip 穿透 + A04 条件引擎 + A05 VariableStore） | ✅ 715 tests |

## 权威文档（不在本目录）

- 冻结契约与不变量：`docs/` 根目录（PLUGIN-CONTRACT / UI-CONTRACT / INVARIANTS 等）
- 项目手册（里程碑/红线/闸门）：`docs/PROJECT-HANDBOOK.md`
- State Generation Matrix：见 `history/82-p22f-response.md` §三
