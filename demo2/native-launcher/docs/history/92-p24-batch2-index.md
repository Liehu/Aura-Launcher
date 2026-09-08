# 92 — P2.4 Batch 2: File Index Maintenance 2.0（B01–B06）+ Catalog A06

> 日期：2026-09-08。范围：P2.4 第二批（`demo2/files2/01-P2.4-DESIGN-SPEC.md` §5）。
> 输入基线：history/91（638 tests）。

## Task P24-B05 — Health Model（先行，其余 B 任务的观测面）

- `launcher-domain::IndexStatus` 新增（spec §5.3 全字段）：
  `watcher_count` / `unavailable_roots` / `last_success_ms` / `last_failure_ms` /
  `recovery_count`，`impl Default`。
- `IndexHealth` 新增 `Unavailable` 变体（root 缺失专用，区别于 Degraded：
  Degraded=恢复中故障，Unavailable=等待 root 重现，索引保持可查询）。

## Task P24-B01/B04 — Root 状态机（Configured→Available→Watching→Unavailable→Reappeared）

- coordinator 不再在 spawn 时静默丢弃不存在的 root（旧代码 `retain(exists)`，
  被过滤的 root 永久失去监控）：全部 root 进入状态机，缺失的从 Unavailable 起
  步，由 maintenance sweep 以有界速率轮询重现。
- root 消失路径：watcher 报错（`WatcherExited`，新消息变体）或 sweep 的
  存在性巡检（≤1s 间隔的 stat，兜底 watcher 线程未死的情况）→ Unavailable，
  **索引保持可查询**（Delete 事件照常应用，读路径不受影响）。
- 重现路径：sweep 发现 exists → `rescan_root`（有界子树，不跟随 reparse
  point）→ `WindowsWatcher::spawn_root` 重注册 → recovery_count+1 → Ready。
- E2E（`tests/root_lifecycle.rs::b04_*`）：真实删除/重建目录，全链路验证
  （Unavailable → 查询可用 → 重现 → Ready → generation 推进 → 重注册的
  watcher 跟踪新事件）。

## Task P24-B02 — Watcher 重注册（有界退避）

- `WindowsWatcher::spawn_root()`（新公开 API）：单 root 重注册；watch 错误
  上报 `WatcherExited` 而非伪装成 Overflow（Overflow=缓冲区溢出且线程存活，
  两者恢复路径不同）。
- `Maintenance` 调度器：退避 1s 翻倍至 60s 封顶，`REWATCH_MAX_ATTEMPTS=6`
  次后转 60s 慢速轮询——**永不无限速重试、永不放弃**（自愈语义）。
- E2E 覆盖：b04 的重注册 watcher 后续事件闭环。

## Task P24-B03 — DirtyRoot（已有，本批确认）

P2.1-B 已交付 overflow→DirtyRoot→有界子树恢复（`incremental_e2e::overflow_*`
与 search-during-maintenance 用例继续全绿），本批未改动其语义。

## Task P24-B06 — Long-run Maintenance Soak

- `tests/root_lifecycle.rs::b06_maintenance_soak_bounded`：
  `LAUNCHER_INDEX_SOAK_SECONDS` 缩放时长（默认 4s CI 档；可扩长跑）。
  持续 churn（create/modify/delete），断言：soak 结束 quiescent（无无界
  backlog）、generation 推进、索引可搜索。

## 修复的潜伏缺陷（本批发现）

`Maintenance.unavailable` 的键曾出现大小写双拼（recovery_pass 的 dirty 字符串
与 watcher 消息字符串大小写来源不同），导致恢复后 `unavailable_roots` 永不清
零。修复：unavailable/rewatch 键统一走 `normalize_path_identity` 规范化。

## Task P24-A06 — Catalog 恢复 Conformance

- `catalog.rs::corrupt_rebuild_resets_lifecycle_to_ready`：损坏重建后
  generation 归零、lifecycle 全部回 ready——stale/broken 是可重建元数据，
  不得作为 authority 残留（"metadata != authority" 的恢复侧镜像）。

## Contract / Architecture impact

- 无冻结契约变更。`IndexStatus`/`IndexHealth`/`CoordinatorMsg` 为
  launcher-domain/indexer 内部扩展（加字段/加变体，无语义改写）。
- launcher-app 的 index-status 观测线程无需改动（字段只增不改）。

## Performance Impact

- 常态路径增加：sweep 每秒至多 N roots 次 stat（N=配置 roots 数，本地内存
  级成本）+ 每次循环 `unavailable.len()` 等字段拷贝——可忽略。
- 搜索/写入路径零改动；p95 7µs/23µs 基线不受影响。

## Gate 结果

- `cargo test --workspace`：**642 passed / 0 failed**（638 → 642，+4）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
- P2.1-B 既有 e2e（b10 eventual consistency、overflow recovery、
  search-during-maintenance）全部继续通过——coordinator 改造向后兼容。

## P2.4 剩余（下一批）

P2.4-C01/C06（lifecycle 模型整合进 Provider 侧、诊断结构化）、
P2.4-D（launcher-plugin CLI：init/validate/build/package/install/run/inspect）、
P2.4-E（Dev Mode/RPC Inspector/Diagnostic Snapshot/Replay）、P2.4-F（Gate
G-A~G-R 接入 release_gate + 文档收口）。
