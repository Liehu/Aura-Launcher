# 128 — P2.6 Batch 9：E01/E07 Graph Editor 状态 + Undo/Redo（+ E06 导入导出）

> 日期：2026-09-09。范围：P2.6 第九批（`P2.6 开发设计规范` §25-§27）。
> 输入基线：history/127（784 tests）。

## 交付

- `crates/launcher-workflow/src/editor.rs`（新）：
  `GraphEditor`——纯模型编辑器（Slint 画布是其 VIEW）：
  - 编辑操作：add_node / remove_node（entry 不可删）/ add_edge /
    remove_edge / set_condition；
  - **§27 Undo/Redo**：有界栈（100 层），新编辑清空 redo 分支，
    undo/redo/replay 语义测试（110 编辑 → 100 层 → 全 undo 回到最早
    被跟踪状态 → 全 redo）；
  - **草稿态语义**：中间编辑允许暂不可达节点/暂态环（用户还在连线），
    **§10 验证门在 export**（`export_json` 拒绝无效图）；删除 entry 节点
    仍被结构性拒绝。
- **E06 导入/导出**：`export_json`/`import_json`——保存/加载前的契约门。

## Gate 结果

- `cargo test --workspace`：**787 passed / 0 failed**（784 → 787，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.6 剩余

E02-E05（Slint 画布/节点/边 UI/配置面板——VIEW 层，消费本模型）、
F/G QA+Release 收口。
