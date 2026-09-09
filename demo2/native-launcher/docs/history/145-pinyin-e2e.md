# 145 — P2.5/P2.6 A04/C04 Pinyin 端到端集成

> 日期：2026-09-10。范围：Pinyin 首字母搜索端到端集成（跨阶段尾项）。
> 输入基线：history/144（824 tests）。

## 交付

- `launcher-indexer`：`files_fts` schema 升级为
  `fts5(name, pinyin_init, path UNINDEXED)`——`pinyin_init` 列存储
  `pinyin_initials(name)` 的首字母串（C00 范围→GB2312 排序边界映射）。
- **`fts_upsert`**：增量同步时计算并写入 pinyin_init。
- **`rebuild_fts`**：改为 Rust 侧逐行计算 pinyin initial 后批量插入
  （SQLite 无法原生做 CJK 边界映射）。
- **`search_fts_inner`**：MATCH 扩展为 `?1 OR pinyin_init MATCH ?1`——
  用户输入拼音首字母可命中中文文件（AC-A04-1）。
- 旧 FTS 表自动重建（DROP+CREATE 含新列，`rebuild_fts` 全量重灌）。
- 测试：`fts_schema.rs` 3 条（FTS5 可用 / 重建无损 / 损坏自愈）继续
  通过；既有 incremental_e2e 5 条全过。

## Gate 结果

- `cargo test --workspace`：**824 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 意义

跨阶段尾项 Pinyin（A04/C04）的搜索端已实现。完整拼音表（含韵母/声调）
与 UI 输入法候选为后续优化项，但首字母匹配已可满足基本使用场景。
