# 176 — P3-UX Batch UX-B：搜索高亮 + 结果行重设计 + 空查询网格

> 日期：2026-09-11。范围：P3-UX 第二批（UX-B 搜索与结果）。
> 输入基线：history/175。

## 交付

- **搜索高亮（P30X-11/12）**：
  - `fuzzy.rs` 新增 `FuzzyMatch` 结构体（score + positions）与
    `fuzzy_match()` 函数（一次遍历收集匹配位置）；
  - `to_result_items_with_query()`：在 title 中查找 query 的
    case-insensitive 首次出现，预拆分为 before/match/after 三段；
  - `result-row.slint` 渲染三段标题：匹配段 accent 色 + bold；
  - title-before/title-match/title-after 字段穿透 AppWindow 边界。
- **结果行重设计（P30X-13）**：28px 图标（原 20px）+ 副标题样式调整
  （type-secondary 字号）+ 快捷键徽章（primitives/shortcut-badge）预留。
- **空查询网格（P30X-14）**：空查询时显示最近使用的图标网格
  （≤8 个，28px 图标 + 12px 名称），数据源 = recent_commands。

## Gate 结果

- `cargo test --workspace`：**925 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：18 crates ok
