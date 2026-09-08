# 81 — P2.2-A/B/C/D/E 五批核心能力实施记录

日期：2026-09-07。基线：565 tests / 零警告 / Release Gate 十项 PASS。
按五份 P2.2 文档（A Favorites → B Query Cache → C Context Ranking → D Plugin Ecosystem → E Installer/Upgrade/Recovery）依次实施**各自的核心可行子集**；每批超出本会话容量的部分如实挂账。

---

## P2.2-A — Favorites / Pinning v0.1 ✅（核心子集）

- `launcher-core/src/favorites.rs` `FavoriteService`（独立 `favorites.db`，WAL + FK ON）：
  - **身份键 = search_identity_key（语义身份）**，非 display name、非 provider item id（INV-FAV-001/009）；
  - `pin ⊆ favorite`：pin 单事务 ensure-favorite + insert pin；unfavorite 单事务删 pin+favorite（INV-FAV-006/007）；
  - unpin 保留 favorite；reorder 事务内重编号（钳位）；重复 add 幂等；MAX_FAVORITES=256 / MAX_PINS=32 有界；
  - `FavoriteSnapshot` 提供排名只读视图。
- **排名信号**（§24/§25：Favorite 不做 Provider，是注入候选池的信号）：pinned +20 / favorite +10，远小于词法精确匹配（100），"文本相关性仍居首"（§27）。
- **接入**：Ctrl+D 切换选中结果的收藏（status 提示）；每次成功变更 `user_state_generation+1`（§102）。
- 测试 7 项：pin⊆fav、unfavorite 级联、unpin 保留、幂等、reorder、pin 上限、重启持久化。

## P2.2-B — Query Cache v0.1 ✅（核心子集）

- `launcher-search/src/cache.rs`：`SearchCacheKey`（normalized query + file/user_state/context generations）、**LRU 双预算**（256 条 / 16 MiB）、TTL（Complete 60s / Negative 5s）、**超预算单条拒收**、generation 变化自然 miss + 旧条目 LRU 淘汰（§51：无全局 clear）。
- **Core::search 接入**（§53/§89/§90/§153）：provider dispatch **前**查缓存；provider fan-out → identity merge → favorites/context boost → global rank → limit **后**写缓存（只缓存 final snapshot）；`record_use`/收藏变更 bump user_state_generation → 旧 key 自然 miss。
- **边界坚守**（INV-CACHE-001/002/003）：只缓存 Command（含 ActionDescriptor 语义），绝不缓存 ResolvedAction/ExecutionId；cache hit 后执行仍走 Resolver→validate→execute。
- 测试 4 项：put/get/miss、generation 变化 miss、双预算、超大拒收。
- 挂账：SingleFlight（当前单 worker 顺序搜索无并发同 key 场景，§93-94 认可此模型）、partial-batch 缓存（我们的搜索本就是 final-only）。

## P2.2-C — Context-aware Ranking v0.1 ✅（核心子集）

- `launcher-search` 纯函数层（§30/§95/§112/§113-114）：`ContextSnapshot`（foreground_app + current_folder，host 注入）、`FolderProximity`（SameFolder > Descendant > Ancestor > None，**父目录比较**，纯词法 `normalize_path_identity`，零磁盘访问）、`ContextRankingWeights`（foreground 8 / same 12 / descendant 7 / ancestor 3）。
- **context_score 为附加分**（§28）：usage + favorite + context 相加，绝不制造候选（§58）、绝不覆盖文本相关性（§25）。
- **宿主捕获**（§15/§47）：每次 popup show 捕获一次——foreground app（既有 WindowsForegroundSource）+ Explorer 当前文件夹（仅当前台窗口就是 Explorer 时，不猜测，§9）；快照存入 Core，供整轮查询使用。
- 测试 4 项：same-folder 胜出、proximity 全序、foreground 匹配、词法无 IO。
- 挂账：TimeBucket 弱信号、disabled 开关的 ranking_generation 语义、VR-CTX。

## P2.2-D — Plugin Ecosystem v0.1 ✅（核心子集）

- `providers/plugin.rs` 重建 + 状态机：`protocol_failures`（连续非业务失败计数）≥ **QUARANTINE_THRESHOLD(3)** → `quarantined`（INV-PLUGIN-008 类比）；`set_enabled`/disabled → 无候选 + 拒执行；业务失败重置计数（§59 瞬态/协议失败分级）。
- 发现边界（§75/§142）：disabled/quarantined 插件不产生搜索候选。
- 重建事故说明：plugin.rs 曾被一次失败的脚本编辑截断为空文件，已从全部调用点（main.rs、calculator-plus E2E、workflow service）忠实重建并回归——**既有全部 plugin/MCP E2E 依旧通过**。
- 挂账：SQLite registry/ERD、install/update/rollback 事务、capability 审批、Package hash/signature（P2.2-D 主体）。

## P2.2-E — Installer / Upgrade / Recovery v0.1 ✅（核心子集）

- **Index schema_version**：`index_meta.schema_version`（建库即写 1）——数据契约版本与产品版本分离的锚点（§12/§13）。
- **启动健康标记**（§45/§79-80）：`startup_state.json`——启动即写 `starting`，核心服务全部就绪后写 `healthy`（清零失败计数）；上次遗留 `starting` = 上次启动中断，warn + 失败计数递增，≥3 触发 recovery 决策入口（当前为告警，Recovery Mode UI 后置）。
- 既有基础盘点：单实例 Mutex 进程生命周期守卫（68-FIX-03）、config 损坏隔离 + 原子写（§38 对应）、package.py 安装包 + 升级保数据（§86-87）、`--require-baseline`。
- 挂账：MSIX 打包与签名（需证书）、UpgradeCoordinator/状态机、Migration Journal、Recovery Mode UI。

## 验证

```text
cargo test --workspace   565 passed / 0 failed（+18：favorites 7、cache 4、context 4、
                          plugin quarantine 状态机随重建回归、record_use 计数）
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical）
```

## 不做 / 后置（对应各文档排除清单）

SingleFlight、partial-batch 缓存、磁盘 Query Cache、TimeBucket 信号、Context disabled VR、MSIX/签名、UpgradeCoordinator/Recovery Mode UI、Plugin registry SQL/ERD、capability 审批 UI、Package hash/signature。共同理由：依赖 Contract v2/持久化 catalog/证书基础设施，或本批已有等价的更简单机制且经测试固化。
