# 95 — P2.5 Batch 1: Search Contract v2 + Query Intelligence（P25-001/A01/A02/A03）

> 日期：2026-09-08。范围：P2.5 第一批（`demo2/files2/101-p2.5-0.1.md`）。
> 输入基线：P2.4 完成（history/94，655 tests）。

## Task P25-001 — Search Contract v2（G-P25-001 SEARCH-CONTRACT-FROZEN）

- `crates/launcher-search/src/intelligence.rs`（新）：冻结 v2 DTO——
  `SearchRequestV2`（raw/normalized/tokens/filters/intent/strategy/budget_ms）、
  `SearchIntent`（8 类）、`SearchFilter`/`FilterKind`、`SearchStrategy`
  （All/Routed）、`ProviderCapability`、`SearchResultState`
  （**Complete/Partial/TimedOut/Cancelled/Failed 冻结**，serde 名钉死）。
- 兼容桥：`search_request_v2(raw, budget)` 从 legacy 字符串派生 v2 请求，
  旧搜索路径零改动（AC-001-4）。
- AC 验收：roundtrip ✅（AC-001-1）、状态冻结 ✅（AC-001-2）、
  **authority-free 结构断言**（请求 serde shape 不含 capability/authority 字段，
  AC-001-3）、legacy 兼容 ✅（AC-001-4）。

## Task P25-A01 — Query Normalizer v2（G-P25-A01 QUERY-NORMALIZATION）

- 纯函数 `normalize_query_v2`：lowercase + 空白折叠（Unicode 感知，含全角
  空格）+ tokens 切分 + wildcard/path-separator 检测 + **512 字节硬界**。
- 无外部依赖、无 I/O（AC-A01-3）；1000x 确定性重放测试（AC-A01-1）；
  中英文/Unicode/path/wildcard 全覆盖（AC-A01-2）。
- 修复实现过程中的空白折叠 bug（连续空格未合并，由测试抓出）。

## Task P25-A02 — Intent Detector（G-P25-A02 INTENT-DETERMINISTIC）

- `detect_intent` 确定性规则链：**显式 filter 语法绝对优先** → 结构模式
  （路径分隔符/尾分隔符 → File/Folder）→ 扩展名模式（exe/lnk → Application，
  文档扩展 → File）→ 命令语法（动词开头的多 token）→ 保守默认（裸词 =
  Application）；冲突信号降级 Mixed，无匹配 Unknown（AC-A02-1/2/3）。
- 8 类 Intent 全覆盖测试；100x 确定性重放（AC-A02-4：无 LLM/网络/I/O）。

## Task P25-A03 — Explicit Query Syntax / Filters（G-P25-A03 FILTER-CONTRACT）

- filter 语法：首 token `app:` / `file:` / `folder:` / `cmd:` / `plugin:` /
  `wf:`，值为剩余规范化查询（含 Unicode；kind-only 空值合法）。
- **未知 prefix 安全回退**为字面文本（`foo:bar`、`chrome:profile` 都不是
  filter）；filter 仅是 retrieval constraint（AC-A03-3/4），strategy 派生为
  `Routed([kind])`。

## 位置与接线

- 全部在 `launcher-search::intelligence`（加 serde/serde_json 依赖）。
- **尚未接入 Core::search 路由**——那是 P25-B06（Routing Planner）与
  B 线（SearchCoordinator）的任务；本批是纯契约+智能层，零行为回归风险
  （P2.1 搜索契约 e2e 全部原样通过）。

## Gate 结果

- `cargo test --workspace`：**671 passed / 0 failed**（655 → 671，+16）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.5 剩余（后续批次）

P25-002（cross-cutting 注册）、**B 线**（B01–B06 SearchCoordinator：
骨架/有界并发 fan-out/timeout 隔离/supersede 取消/partial results/routing
planner）、**C 线**（C01–C06 FTS5 schema/检索/Pinyin/LIKE fallback/content
扩展点）、**D 线**（D01–D06 Ranking v2/权重集中/Explainability/merge
evidence/quality corpus/diagnostics）、**E 线**（E01–E06 基准与 stress）、
**F 线**（F01–F04 CI/性能门禁/文档/gate 收口）。
