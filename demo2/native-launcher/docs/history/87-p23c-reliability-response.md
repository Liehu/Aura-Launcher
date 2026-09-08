# 87 — P2.3-C Reliability / Recovery 第一批实施记录

日期：2026-09-07。基线：576 tests / 零警告 / Release Gate 十 Gate PASS（含 G10）。

---

## 一、评审结论

87 号文档的 P2.3-C 十二项（C1-C12）是正确的可靠性收口清单。本批聚焦 **C6/C7 持久层损坏恢复**——所有持久层的"corrupt DB → 不启动失败"行为统一收口为确定模式：

| 持久层 | 损坏恢复策略 | 已有/新增 |
|---|---|---|
| index.db | 删除 + 重建（rebuildable derived state） | ✅ 本批（try_open → 失败 → 删文件 → 重试） |
| favorites.db | 隔离为 .db.corrupt + 重建（**用户数据不静默删除**） | ✅ 本批 |
| catalog.db | 删除 + 重建（rebuildable） | ✅ 本批（P2.1-D.1） |
| plugins.db | 删除 + 重建（所有插件默认 enabled = 安全默认） | ✅ 本批（P2.2-D） |
| config.toml | 隔离为 config.invalid-\<ts\> + 写默认 | ✅ 已有（67 号 MUST-2） |
| icon cache PNG | 删除条目 + 重抽取 | ✅ 已有（80 号 INV-ICON-005） |

统一模式：`try_open` → 失败 → 隔离/删除 → 重试 → 两个路径都能正常服务。

## 二、本批代码改动

1. `launcher-indexer::Indexer::open` 重构为 `try_open` + corrupt fallback（首次 init 失败 → 删文件含 WAL/SHM → fresh recreate）。
2. `favorites.rs` 同模式：corrupt → rename `.corrupt` + fresh recreate。
3. `plugin_registry.rs` 同模式：corrupt → fresh recreate（所有插件 default enabled = 安全默认）。
4. 新增测试：corrupt index.db recovered / corrupt favorites.db recreated。

## 三、P2.3-C 剩余挂账（逐会话推进）

- C1 Failure Taxonomy → 每个 crate 的错误枚举统一映射（本轮已有 WorkflowFailureClass 但未覆盖全）。
- C2 完整 startup state machine（recovery_required → UI 呈现）
- C3 Plugin crash soak（连续 spawn-crash 循环的真实进程测试）
- C4 MCP runtime/session 恢复矩阵（现有 security_transport 覆盖部分）
- C11 30min/4h soak（perf_baseline 已有但缺长跑 profile）
- C12 Recovery Golden Suite（汇总上述为统一 E2E）

## 四、验证

```text
cargo test --workspace   576 passed / 0 failed（+5：product_e2e 3 + favorites corruption 1 + indexer corruption 1）
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py          十 Gate 全 PASS（含 G10: release app p95 8µs 达标）
```
