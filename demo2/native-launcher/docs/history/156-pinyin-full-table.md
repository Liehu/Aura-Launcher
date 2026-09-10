# 156 — Pinyin 完整拼音表（A04 收尾优化）

> 日期：2026-09-10。范围：146 号交接最后一项代码优化——"完整拼音表"。
> 输入基线：history/155（865 tests）。

## 交付

- `crates/launcher-domain/src/pinyin_table.rs`（新）：准确音节表——
  约 200 个音节组覆盖 ~800 个常用简体字（char → 无调全拼），以
  `(syllable, chars)` 紧凑格式编写，首次使用时展开为按码点排序的
  char→syllable 索引（二分查找，OnceLock 缓存）。只收录最常见读音
  确定无疑的字；一字一条目、确定性。
- `crates/launcher-domain/src/pinyin.rs` 升级：
  - **table-first**：`pinyin_initial` / `pinyin_initials` 优先查表取
    准确首字母，表外字符保留旧的边界近似兜底——首字母索引从
    "码点区间粗估"升级为"真实读音"（此前 `说→z`、`明→n` 一类错误
    消失）；
  - **`pinyin_full()`**（新）：全拼罗马化——表内字取全音节、表外
    CJK 退回首字母、非 CJK 跳过（与前缀匹配语义一致）。
- `crates/launcher-indexer`：FTS5 增加 `pinyin_full` 列：
  - 老库迁移：FTS 表不支持加列——打开时检测旧形态（sqlite_master +
    pragma_table_info），DROP 后按新 schema 重建（派生状态，元数据表
    不动，AC-C01-3 语义不变），由后续 rebuild/rescan 回填；
  - rebuild_fts / fts_upsert 双写两列；查询改为单条 `files_fts MATCH`
    ——整表 MATCH 本就覆盖全部列（name/pinyin_init/pinyin_full），
    顺带消除了多 MATCH 表达式在 JOIN 上下文的
    "unable to use function MATCH" 风险。
- 测试 7 条：常用字读音 spot-check、索引有序性、未知字 None、
  table-first 首字母、全拼罗马化（文件夹/报告/报表 + 混排 + 表外字
  兜底）、全拼查询命中中文文件名、老 schema 打开自动迁移后可用。

## Gate 结果

- `cargo test --workspace`：**872 passed / 0 failed**（865 → 872，+7）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 状态

146 号交接清单全部完成。P2.5–P2.9 各阶段代码项收口；剩余仅
MSIX 签名（等外部证书）。
