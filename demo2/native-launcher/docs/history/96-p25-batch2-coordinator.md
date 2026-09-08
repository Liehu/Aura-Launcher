# 96 — P2.5 Batch 2: SearchCoordinator（B01–B04 + B06 v0.1 路由）

> 日期：2026-09-08。范围：P2.5 第二批（`demo2/files2/101-p2.5-0.1.md` §B）。
> 输入基线：history/95（671 tests）。

## Task P25-B01 — SearchCoordinator Skeleton（G-P25-B01 COORDINATOR-SKELETON）

- `crates/launcher-core/src/search_coordinator.rs`（新）：统一编排入口，
  管线 = `search_request_v2`（Query Understanding）→ Strategy 路由 →
  fan-out → **provider 顺序确定性合并**（completion 顺序不影响结果顺序）。
- **兼容模式**：`Core::search` 的 sequential 路径原样保留（spec 步骤 6），
  默认仍是 legacy——切换默认等 E 线混合基准数据后决定。协调器是独立入口，
  零行为回归。
- AC-B01-3：协调器只调 `Provider::query` 并合并/排序候选，**永不执行
  Effect**。

## Task P25-B02 — Bounded Concurrent Fan-out（G-P25-B02 BOUNDED-FANOUT）

- Provider 包装为 `Arc<Mutex<Box<dyn Provider>>>`；fan-out 按
  `max_parallelism` 分组到 `std::thread::scope` worker（每组内顺序执行）。
- admission control：`in_flight` 原子计数 + `max_in_flight_seen` 峰值指标。
- 测试 `b02_bounded_fanout_deterministic`：**100 个 provider、并发上限 4**，
  in-flight ≤ 4（AC-B02-1）、100 条结果一个不少（AC-B02-3 无 orphan）、
  三次运行合并顺序逐条相同（AC-B02-2）。

## Task P25-B03 — Provider Timeout/Panic Isolation

- 每次查询经 `catch_unwind`：**panicking provider 被隔离**，错误入
  `errors`，其余 provider 照常贡献，状态降级 `Partial`（AC-B02-4 /
  INV-SEARCH-004）。`Err` 路径经 `take_last_error()` 钩子收集。
- 测试 `b03_panicking_provider_isolated`：中间 provider panic，其余 3 个
  结果完整。

## Task P25-B04 — Supersede/Cancellation Guard

- `search(raw, cancel: Option<&AtomicBool>)`：每 provider 之间轮询取消
  标志，置位即返回 `Cancelled` + 已完成的部分结果。
- 测试 `b04_cancellation_guard`：8 个慢 provider 中途取消 → Cancelled +
  部分结果。

## Task P25-B06 v0.1 — Search Routing Planner

- `routes_to()` 路由表：FilterKind → provider id 前缀（app → app*；
  file/folder → file*/recent*；cmd → workflow/settings/context*；
  plugin → plugin*；wf → workflow*）。显式 filter → `Routed([kind])`，
  只查询匹配 provider（非匹配者零调用）。
- 测试 `b06_filter_routes_to_matching_providers_only`：`app:chrome` 时
  workflow provider 计数为 0。
- （说明：初版测试把并发仪表 mock 当计数器用导致误报——mock 是"当前
  in-flight"语义加后即减，终值恒 0；测试设计修正，实现无 bug。）

## Task P25-B05 — Partial Result Contract

- 由 `CoordinatorResult.state`（Complete/Partial/TimedOut/Cancelled/Failed，
  Batch 1 冻结枚举）+ errors 列表承担：部分失败=Partial+errors 逐 provider
  记录。TimedOut/Cancelled 在 B03 预算参数接入时激活（timeout 隔离的
  Duration 参数留待与 E 线基准共同定标）。

## Contract / Architecture impact

- 无冻结契约变更；`Core::search` 未动（兼容模式）。
- 接线决策（默认切换到 coordinator）推迟到 P25-E 混合基准之后——先有
  数据再换默认，避免无基准的调度变更。

## Performance Impact

- 常态路径零变化（legacy 仍默认）；协调器路径经 B02 测试实测 100 provider
  × 5ms 延迟在并行度 4 下 ~125ms 内完成（顺序执行需 ~500ms）。

## Gate 结果

- `cargo test --workspace`：**676 passed / 0 failed**（671 → 676，+5）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.5 剩余

C 线（FTS5/Pinyin/LIKE fallback）、D 线（Ranking v2/Explainability）、
E 线（混合基准 + 默认切换决策 + stress）、F 线（CI/文档/gate 收口）。
