# 77 — P2.1-B Incremental Index Engine v0.1 实施记录

日期：2026-09-07。基线：535 tests / 零警告 / Release Gate 十项 PASS。
本批按 76 号文档（P2.1-B 冻结契约）实施，范围严格限定 File Index。

---

## 一、交付组件（全部落位 `launcher-indexer` + `launcher-domain`，未新增 crate）

| 组件 | 位置 | 要点 |
|---|---|---|
| `FileChange/Kind`、`FileObservation`、`VerifiedChange`、`IndexHealth/Status` | `launcher-domain/src/file_change.rs` | 跨层纯模型。事件=提示（INV-INDEX-004）；writer 只收 VerifiedChange；rename 在验证后坍缩为 Delete(old)+Upsert(new)（§30），writer 不懂 Windows rename |
| `BoundedQueue` | `incremental.rs` | INV-INDEX-001：硬容量（默认 4096），满即拒收并累计 dropped，由 Coordinator 标记包含根 dirty |
| `Coalescer` | `incremental.rs` | 按 `normalize_path_identity` 身份合并（§13/§17）：burst→单一身份，rename 注册 old+new 两个身份；纯可单测 |
| `DirtyRootSet` | `incremental.rs` | 路径包含归并（§15/20）：`C:\A` 吞掉 `C:\A\B`，兄弟独立 |
| `WindowsWatcher` | `watcher.rs` | ReadDirectoryChangesW（overlapped），递归子树；**IO 完成事件与停止事件严格分离**（共享手动重置事件会让首次完成被误判为停止——E2E 抓出的真实 bug）；ERROR_NOTIFY_ENUM_DIR / 0 字节完成 → Overflow 消息 |
| Verification | `coordinator.rs` | re-stat 事实源（§17/§29）：Present→Upsert(observation)，Missing→Delete；设备命名空间路径永不进入身份 |
| `Indexer::apply_batch` | `lib.rs` | INV-INDEX-005/007/008：单事务批量提交；**generation 仅在成功 commit 时 +1**；upsert 按 normalized identity 去重（不同大小写拼写是同一文件）；无 tombstone（§32） |
| `Indexer::rescan_root` | `lib.rs` | 有界子树恢复（§16-19）：删该根前缀 + 真实重扫（reparse 不跟随）+ 单次 generation；**不是全盘 rebuild**（INV-INDEX-002） |
| `IndexCoordinator` | `coordinator.rs` | 单线程策略核：watcher 常开 → queue → coalesce → verify → 单 writer batch；启动重建与 watch 并行、事件保留回放（§24，无 rebuild gap）；overflow 优先但分批不饿死正常变更；状态机 Empty→Rebuilding→Ready/Updating/Degraded |
| Health/Status | `IndexStatus` | Search/UI 只看这个（§22/§27）：health/generation/pending/dirty/indexed/last_error |

## 二、宿主接线

`build_core`：`watch_enabled = true`（config 新增项）时，后台重建线程被 **IndexCoordinator** 取代——启动即初始 rescan（单一 writer），随后 watch + 增量维护；FileProvider / history 沿用各自读连接（WAL + busy_timeout，INV-INDEX-006：维护期间搜索始终可查）。`watch_enabled = false` 时退回原后台全量重建路径。

## 三、E2E 与验收对照（76 §63 十 Gate）

真实 ReadDirectoryChangesW + 临时目录真实文件操作（`tests/incremental_e2e.rs`）：

- **B10（总验收）`b10_eventual_consistency_with_real_watcher`**：create/modify/rename/delete/mkdir → quiescent（Ready 且持续静止 ≥1.5s）→ `normalize(filesystem) == index` ✓；改名后旧身份不可搜、新身份可搜 ✓
- **B5 `overflow_triggers_dirty_recovery_not_failure`**：queue_capacity=2 强制溢出 → DirtyRoot → bounded rescan → 最终一致 ✓（无 panic/OOM/full rebuild）
- **B7 `search_remains_queryable_during_maintenance`**：维护运行期间 50 次读连接查询 + 持续写 ✓
- **B3 coalescing 规则表**（§12）：burst/rename 链身份收敛 ✓；B2 queue 硬上限 + dropped 计数 ✓；B4 reparse 不跟随（scan 侧 REPARSE_POINT 检查延续）✓
- **B9 generation**：空批次不变、失败不变、成功 +1（既有 generation 测试 + apply_batch 语义）✓

开发中由 B10 E2E 抓出并修复的两个真实缺陷：① watcher 把 IO 完成事件与停止事件共用一个手动重置 event，首个文件变更即被误判为停止而静默死亡；② `FILE_NOTIFY_INFORMATION.FileName` 是 `[u16;1]` 声明，直接切片越界 panic——均改用 raw parts / 双事件解决。

## 四、与 76 文档的偏差（有意为之，已记录）

1. **Recovery 采用"删根前缀+真实重扫"而非 merge-iterator diff**（§19 的备选方案）：对单个 dirty root 有界且正确；merge-iterator 优化留给 P2.1-B 后续。
2. **rebuild 并发**：初始重建就是 Coordinator 自己的 rescan（事件天然保留在 queue 中，恢复后应用），比 §25 的 sequence cutover 更简单且满足"无 rebuild gap"；`IndexedEvent.sequence` 未引入。
3. 配置项只加 `watch_enabled`；queue/batch 等数值按 §59 建议作为实现常量（非 API contract）。
4. 根消失/重生的 WatcherStatus 细化（§21/§22/§50/§51）：当前降级为 Degraded + 日志，root 重生恢复依赖 overflow/dirty 机制，专项完善后置。

## 五、验证

```text
cargo test --workspace   535 passed / 0 failed（+8：coalescer 1、queue 1、dirty 1、
                          generation 1、B10/overflow/B7 E2E 3、coalescing 规则与 queue 映射并入既有）
cargo build --workspace  zero warnings
check_topology.py        PASS（indexer 不依赖 action/workflow/ai/mcp/ui —— §62 guard 天然满足）
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical）
```

## 六、下一步

P2.1-B v0.1 冻结。后续候选（按 72/74/76 顺序）：merge-iterator recovery、root 消失/重生的 WatcherStatus 细化、Batch 3（Typed SearchIdentity / SearchCoordinator / Provider v2）。
