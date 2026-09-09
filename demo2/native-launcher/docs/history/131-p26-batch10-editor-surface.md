# 131 — P2.6 Batch 10：E03 Editor Slint Surface（graph-editor.slint）

> 日期：2026-09-09。范围：P2.6 第十批（E03 Slint surface 消费层）。
> 输入基线：history/130（790 tests）。

## 交付

- `crates/launcher-ui/ui/graph-editor.slint`（新）：
  `GraphEditorSurface`（Window）——列表式编辑器 VIEW（v1 列表形态，
  满足"声明式有限组件集"红线）：
  - `EditorRowItem` 行（node_id / action_ref / condition 徽标 + remove
    按钮）；`rows` 模型属性由宿主投影（E02 的 `EditorSurface.rows`）；
  - 回调：`add-node`（LineEdit 输入 node-id）/ `remove-node` /
    `undo-requested` / `redo-requested` / `export-requested`；
  - `can-undo` / `can-redo` / `status-text` 属性。
- 全部颜色/字体走 Theme token（bg-canvas/text-primary/text-secondary/
  border-default），`Button`/`LineEdit` 来自 std-widgets。
- app.slint re-export 该 surface（Rust 侧可实例化）。
- E04 节点配置面板（LineEdit 已含 node-id 输入雏形）与 E05 验证错误
  显示（status-text 承载 validate() 错误）的宿主接线归下一批。

## Gate 结果

- `cargo test --workspace`：**790 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：ok
- Slint surface 编译通过（build.rs 实测）。
