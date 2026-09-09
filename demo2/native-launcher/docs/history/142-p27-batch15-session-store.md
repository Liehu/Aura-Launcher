# 142 — P2.7 Batch 15：Agent Session Store（C07 持久化）

> 日期：2026-09-10。范围：P2.7 第十五批（`P2.7 开发设计规范` §26）。
> 输入基线：history/141（817 tests）。

## 交付

- `crates/launcher-ai/src/agent_session_store.rs`（新）：
  `AgentSessionStore`（SQLite/WAL，`agent_sessions` 表）——持久化
  `AgentSessionRecord`（session_id/goal/turn/status/updated_at_ms）。
  save（原子 upsert）、load（单 session）、list_active（Running/Paused
  恢复扫描）、finish（终态删除）。损坏重建（与 catalog/plugins 一致）。
- 修复了 Cargo.toml 中 `[dependencies.rusqlite]` 段导致的依赖吞并问题
  （thiserror/tracing/ureq 被意外归入 rusqlite scope）。
- 测试 3 条：跨 reopen 持久化、活跃 session 恢复扫描、损坏重建。

## Gate 结果

- `cargo test --workspace`：**820 passed / 0 failed**（817 → 820，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
