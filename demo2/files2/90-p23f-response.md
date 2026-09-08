# 90 — P2.3-F UI / Interaction QA 实施记录（核心自动化切片）

日期：2026-09-07。基线：597 tests / 零警告 / Release Gate 全 PASS。

---

## 交付

`crates/launcher-core/tests/ui_interaction.rs` — 6 项 Core 层交互状态机测试，覆盖 90 号文档可自动化部分：

### F4 Selection / Invocation
- `selection_bounds_across_query_changes`：全匹配→选中末位→无匹配查询→结果清空，selection 不越界。
- `query_supersession_prevents_stale_results`：SearchSession query-id 取代旧查询。

### F5 Favorite consistency
- `favorite_signal_consistent_across_queries`：收藏 → 三次不同查询信号持续 → 取消后信号消失。

### F4 Favorite boost
- `favorite_boost_changes_ranking`：Beta Browser 从 #2 升至 #1（usage boost 验证排名变化）。

### C10 Startup state machine
- `startup_state_transitions`：starting → healthy JSON round-trip。

### INV-SEARCH-003 端到端
- `empty_query_returns_recent_with_actions`：空查询 recents 非空且全带可执行 actions。

## 覆盖对照（90 号 §27 Exit Criteria）

| 类别 | 覆盖 |
|---|---|
| Selection | ✅ bounds + supersession |
| Query supersession | ✅ |
| Favorite consistency | ✅ 跨查询 |
| Empty query recents | ✅ |
| Context semantic change | ✅ p22f 测试（既有） |
| Cache invalidation | ✅ p22f 测试（既有）|
| Execution replay prevention | ✅ p23c exec_ids 测试（既有）|
| Keyboard navigation | ⏳ P2.3-F 手工（需要真实 GUI 环境）|
| DPI 125/150/200% | ⏳ P2.3-F 手工 |
| Popup soak 1000x | ⏳ P2.3-F 手工 |
| Workflow UI | ⏳ P2.3-F 手工 |

## 验证

```text
cargo test --workspace   597 passed / 0 failed（+6）
cargo build --workspace  zero warnings
release_gate.py --require-baseline  十 Gate 全 PASS（VR 15/15 stable）
```
