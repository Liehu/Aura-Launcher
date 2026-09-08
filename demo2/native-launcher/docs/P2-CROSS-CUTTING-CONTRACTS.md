# P2 Cross-Cutting Contracts — Launcher 1.0 横向契约冻结

> **状态**：ACCEPTED（review 82 §26、83-roadmap §六 两次要求后正式立档）。
> 本文件是全项目唯一的横向契约登记处。任何新批次引入新的 ID / Generation /
> State，必须先在此登记（谁产生 / 谁消费 / 何时递增 / 是否持久化 / 是否影响
> Search Cache / 是否跨 restart），否则不得实现。

---

## 1. ID 目录（Execution / Runtime / Lifecycle）

| ID | 产生者 | 生命周期 | 持久化 | 规则 |
|---|---|---|---|---|
| `SearchRequestId` | 宿主 SearchSession | 一次查询会话 | ❌ | 最新 request 独占 UI 发布（ADR-0004） |
| `ExecutionId` | Core（宿主执行边界） | 一次执行 attempt | ❌ | 每 attempt 恰好一个，registry never re-mints（INV-AUTH-005） |
| `RuntimeId` | RuntimeManager | 插件进程生命周期 | ❌ | 与 ExecutionId 严格分层（INV-PLUGIN-011） |
| `ProtocolSessionId` | Plugin/MCP session | 协议会话 | ❌ | 与 RuntimeId 独立 |
| `PluginId` | manifest | 安装生命周期 | ✅ | 不含 version、不随安装目录变（P2.2-D §3） |
| `PluginInstanceId` | Registry | 一次安装 revision | ✅ | 用于 staging/update/rollback |
| `UpgradeTransactionId` | Upgrade Coordinator | 一次升级事务 | ✅ | 独立于 Execution/Runtime/Session（P2.2-E §6） |

**铁律**：任何两类 ID 不得互相替代或复用数值空间。

## 2. Generation 目录（数据版本 / Cache 正确性）

| Generation | Owner | 递增时机 | 持久化 | 影响 Search Cache |
|---|---|---|---|---|
| `IndexGeneration`（file） | Indexer | rebuild/rescan 成功 commit 后 | ✅ index_meta | ✅ |
| `ApplicationGeneration` | 宿主（插件生命周期/catalog 物化） | 插件 enable/disable/quarantine、catalog reconcile | ❌（进程内） | ✅ |
| `UserStateGeneration` | Core（favorites/usage） | 每次成功收藏/使用变更 | ❌ | ✅ |
| `ContextGeneration` | Core（语义比较） | foreground/folder 语义变化 | ❌ | ✅ |
| `RankingGeneration` | Core（预留） | 权重可运行时配置后 | ❌ | ✅（字段已入 Key） |
| `CatalogGeneration`（application 持久层） | CatalogStore | reconcile commit | ✅ catalog_meta | （经 ApplicationGeneration 折算） |
| `PluginCatalogGeneration` | Plugin Registry | lifecycle 变更 | ✅ plugins.db | 经 ApplicationGeneration 折算 |

**铁律**（review 82 §7-8 / 74 §25）：Generation = 已提交的数据版本；失败不涨；
增量按 batch 递增；永不与时间戳/事件计数混用。Cache Key =
`(normalized query, file, application, user_state, context, ranking)`。

## 3. State Matrix（哪些变化影响什么）

| 变化 | Search Cache | User State | 持久 Catalog | Plugin Runtime |
|---|---|---|---|---|
| 文件增删改 | file_generation++ → miss | — | — | — |
| 应用发现/打包变化 | application_generation++ → miss | — | reconcile | — |
| 插件 enable/disable/quarantine | application_generation++ → miss | — | — | 生命周期独立 |
| 收藏/取消/排序 | user_state_generation++ → miss | ✅ | — | — |
| 执行成功 | user_state_generation++ → miss | usage 计数 | — | failure 计数清零 |
| 前台/文件夹语义变化 | context_generation++ → miss | — | — | — |
| 排名权重变化 | ranking_generation++ → miss | — | — | — |

## 4. Authority 边界（不可协商）

1. 唯一 ActionEngine：`launcher-action`（validate → execute）。
2. Cache/Favorites/Context/Icon 只产生信号或快照，永不产生 authority。
3. Plugin/MCP/AI metadata 是 DATA，不能变成 capability 或 Favorite 授权。
4. UI 只发 UiCommand；任何持久化变更走 Resolver → engine → service。
5. Context 是 ranking input（INV-CONTEXT-001/002）；Identity 解析纯函数
   （INV-IDENTITY-003）；Identity 折叠不丢弃来源（INV-IDENTITY-001）。

## 5. 生命周期统一原则

Plugin 与 Launcher 升级共享**设计原则**（stage → validate → activate →
health check → running；失败 → rollback/quarantine/recovery），但实现对象
分离：Plugin 用 Registry+Broker+RuntimeManager；Launcher 用 Windows package
model + UpgradeCoordinator。二者不得合并为同一 lifecycle class。
