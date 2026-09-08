# 97 — P2.5 Batch 3: FTS5 检索层（C01/C02/C05/C06；C03/C04 部分随行）

> 日期：2026-09-08。范围：P2.5 第三批（`demo2/files2/101-p2.5-0.1.md` §C）。
> 输入基线：history/96（676 tests）。

## Task P25-C01 — FTS5 Schema（G-P25-C01 FTS5-SCHEMA）

- `launcher-indexer`：`files_fts`（name + path UNINDEXED）幂等迁移
  （`CREATE VIRTUAL TABLE IF NOT EXISTS`，FTS5 不可用时优雅降级并记日志，
  `fts_available` 标志下沉到 Indexer）。
- **增量同步**：`apply_batch` 的 upsert/delete 在**同一事务**内同步 FTS
  （与 metadata 原子，generation 语义零改动——AC-C01-4）；delete 先按实际
  path 清 FTS 再删 metadata（join 顺序正确）。
- **重建**：`rebuild_fts()` 全量从 metadata 重灌；search 遇 FTS 错误/空结果
  时自愈重试一次（AC-C01-3）。
- **损坏恢复**：FTS 侧损坏不影响 metadata 表读写（AC-C01-2）；测试
  `fts_rebuild_from_metadata_is_lossless` 验证 drop→rebuild 无损。
- 旧库迁移无损（建表即迁移，metadata 行零触碰——AC-C01-1）。

## Task P25-C05 — LIKE Fallback（合并语义）

- 检索策略升级为 **FTS 排名优先 + LIKE 合并去重**（按 id 去重，总量受
  limit 约束）：FTS 只索引文件名，PATH 片段匹配由 LIKE 补位——既获得 FTS
  相关性排序，又保证 P2-FIX-04 的 path-fragment 行为零回归
  （`path_fragment_matches` 继续绿）。FTS MATCH 输入经引号转义为 prefix
  phrase（用户文本永不变 MATCH 语法）。

## Task P25-C02 — Application FTS Retrieval（G-P25-C02 APP-FTS）

- `CatalogStore`：`app_fts`（display_name + identity_key UNINDEXED），
  reconcile 事务内全量重灌（catalog 行数小，原子一致）；catalog 仍是
  source of truth（AC-C02-1），FTS 命中**映射回 canonical record**
  （AC-C02-2）。
- LIKE 子串回退（`rome` 命中 Chrome）+ 空结果语义保持（AC-C02-3）。
- 测试 `fts_search_maps_to_canonical_and_falls_back`。
- 过程中修复两处实现缺陷：fts_available 二次加锁死锁（Mutex 不可重入）、
  LIKE ESCAPE 字符被 Rust 转义吃掉。

## Task P25-C06 — Content Index Extension Point

- `launcher-indexer::CONTENT_INDEX_ENABLED = false`（显式禁用，符合 C01
  forbidden 列表）；启用路径（content 列 + extractor hook + 同一
  metadata-authoritative 契约）已写入文档注释。

## 本批未做（P2.5 剩余）

- C03 文件/目录 FTS 检索适配：**已由 C01 的 files_fts 覆盖主链路**（FileProvider
  走 Indexer::search 即得 FTS 加速）；C04 Pinyin 检索依赖 A04（拼音罗马化表），
  与 A04 一并推迟到下一批；D 线（Ranking v2/Explainability）、E 线（混合基准
  + coordinator 默认切换）、F 线（收口）后续批次执行。

## Gate 结果

- `cargo test --workspace`：**679 passed / 0 failed**（676 → 679，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
