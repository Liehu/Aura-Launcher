# 141 — P2.7 Batch 14：Context Builder（A02）

> 日期：2026-09-10。范围：P2.7 第十四批（`P2.7 开发设计规范` §10-§12）。
> 输入基线：history/139（811 tests）。

## 交付

- `crates/launcher-ai/src/context_builder.rs`（新）：
  `build_context(foreground_app, current_folder, favorites, budget) ->
  Vec<ContextItem>`——从搜索结果/favorites/前台 app 组装有界上下文；
  **预算**：字符总量不超过 `budget`；**清洗**：经 `sanitize_untrusted` +
  `bound_chars`；**确定性**：同输入恒同输出。
- `render_context_json()`：渲染为 JSON 字符串供 prompt 嵌入。
- 测试 3 条：foreground/folder/favorites 组装、预算限制、空输入。

## Gate 结果

- `cargo test --workspace`：**817 passed / 0 failed**（811 → 817，+6）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

A04（Prompt Builder 深化——prompt.rs 已有基础）、B01（Tool Catalog 投影
——catalog.rs 已有 ActionCatalogItem 基础）、B02/B03（Plan Schema/校验器
= agent_contract/structured_output 已备）、B05（Workflow Proposal =
workflow_proposal.rs 已备）、C03-C07 宿主接线（agent_loop.rs 已备编排
核心）、D/E/F/G/H 线。
