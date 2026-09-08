# SEARCH-QUALITY.md — 搜索质量体系

排名改动不再凭感觉："我想要的结果排第一了吗"用固定语料量化。

## 语料（Corpus）

语料 = **固定 fixture 目录**（`crates/launcher-core/tests/quality.rs` 内置，代表真实机器形态：应用带关键词、文件带扩展名）+ **固定查询集**：

```text
chrome  ch    vscode  calc   notepad   git
terminal sett  control  down   term     pdf    code
```

每条查询定义：
- **Expected Top 1**（必须精确命中，如 `chrome → Google Chrome`）
- **Expected Top 3 / acceptable set**（模糊/歧义查询，要求期望结果进入前三）

## 度量

- **Top1 Accuracy**：CORPUS_TOP1 全量断言（当前 10/10 = 100%）
- **Top3 acceptable**：CORPUS_TOP3 全量断言
- **确定性**：同一语料两次运行结果序列必须完全一致（MRR 等 rank-based 指标在此之上计算）

运行：`cargo test -p launcher-core --test quality`

## 已捕获的回归示例（本体系的实际价值）

引入语料测试时立即发现并修复了两个真实问题：
1. `chrome-shortcut.url`（文件）凭借全标题 prefix 分压过 `Google Chrome` 的 contains 分 → 引入**候选视图**（标题词/文件 stem 取最大分）修复；
2. 关键词命中按拼接串计算，`"chrome browser".starts_with("chrome")` 只得 25 分 → 改为**逐关键词**计分（exact 50 / prefix 25 / contains 15）。

## 演进规则

- 修改 `launcher-search::score` 的任何权重/结构，必须先跑 quality 测试；
- 新增/调整语料条目 = 更新本文件 + `quality.rs`，同一个 PR；
- 未来接入 frequency/recency 特征（设计 spec 7.3 预留）时，先在此登记新语料（如带使用次数的 fixture）再动实现。
