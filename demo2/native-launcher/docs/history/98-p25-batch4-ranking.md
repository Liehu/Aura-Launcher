# 98 — P2.5 Batch 4: Ranking v2（D02 权重集中 + D03 可解释排序 + D05 质量语料）

> 日期：2026-09-09。范围：P2.5 第四批（`demo2/files2/101-p2.5-0.1.md` §D）。
> 输入基线：history/97（679 tests）。

## Task P25-D02 — Centralized Ranking Weights（G-P25-D02 WEIGHTS-CENTRALIZED）

- `RankingWeights` 补齐此前散落的打分常数：`title_exact/title_prefix/
  title_contains`（原 100/60/40）与 `plugin_hint`（原 45.0，ADR-0011 上限）。
  **默认值 = 旧 magic numbers**，生产行为逐位不变（全部既有排序测试原样绿）。
- `RANKING_WEIGHTS_VERSION = 2` + `to_config_json()` / `from_config_json()`：
  **AC-D02-2** 非法配置按字段回退默认（负值/NaN 丢弃）、未知版本整体拒绝、
  cap 字段钳制 ≤1000——永不 panic、永不半应用。
- AC-D02-1：`score_with_parts()` 是唯一打分来源，title 档位全部走权重；
  AC-D02-3：`exact > prefix > contains` 与 "boost 不制造候选" 不变量由
  既有 + 新增测试持续固化。

## Task P25-D03 — Explainable Ranking（G-P25-D03 EXPLAINABLE-RANKING）

- `ScoreParts`（plugin_hint/title/keyword/fuzzy/multi_token/type_prior）+
  `MatchedField`（Title/TitleStem/TitleWord/Keyword/Fuzzy/None，spec 步骤 4
  matched fields）。
- **同源保证**：`score()` = `score_with_parts(...).total()` 的薄包装——
  explanation 与实际分数在构造上不可分叉（AC-D03-1/2）；测试断言
  total == score（epsilon）跨 4 类候选。
- AC-D03-3：结构断言——解释模型 serde shape 不含 capability/authority 字段。
- 过程中三个测试用例设计错误被逐一修正（TitleStem 需要 stem 严格胜出的
  用例：query == 完整 stem），实现语义未动。

## Task P25-D05 — Search Quality Corpus

- `tests/ranking_v2.rs` 语料回归：**"chrome" 必须 Chrome 应用压过
  chrome-*.url/.svg/.md 扩展噪音**（KNOWN-ISSUES #9 的规格化验收）+ 层级
  顺序确定性 + multi-token 全 token 命中要求（无 OR 噪音）。

## D04/D06 说明（推迟）

- D04 merge evidence：P2.2-F（history/82）已实现"合并保元数据"并有测试；
  补充 provenance 字段（Command 级 sources）需要动全部 Command 构造点，
  推迟并归入 Search Contract v2 的 Candidate 模型（P25-B06 深化）。
- D06 ranking diagnostics：由 D03 ScoreParts + G13 证据链承担主要面，
  per-query 诊断计数器推迟到 E 线基准一起做。

## Gate 结果

- `cargo test --workspace`：**687 passed / 0 failed**（679 → 687，+8）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.5 剩余

E 线（E01 混合基准 / E02 provider stress / E03 并发 stress / E04 supersede
race / E05 FTS 恢复 / E06 UI soak + coordinator 默认切换决策）、
F 线（F01 CI / F02 性能门禁 / F03 文档冻结 / F04 gate 收口）、
A04+C04（Pinyin，独立批次）。
