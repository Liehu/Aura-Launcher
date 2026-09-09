# 150 — P2.6 E02–E05：Slint 画布 VIEW 接线（Editor Surface 正式上线）

> 日期：2026-09-10。范围：P2.6 E 线收尾（`P2.6 开发设计规范` §25-§27，
> 146 号交接优先级 4；E02 投影层 127-132 号已备）。输入基线：history/149
> （835 tests）。

## 交付

- **E03 surface ↔ app 接线**（`crates/launcher-ui`）：
  - `graph-editor.slint`：`GraphEditorSurface` 由独立 Window 改为
    `inherits Rectangle` 的同窗口模式组件（与 WorkflowRuntime 同模式，
    review 18 §17：无双窗口/无 overlay 层级）；补 `close-requested` 回调
    与 Close 按钮；行内 remove 按钮正式接到 `remove-node(node_id)`；
    内容驱动 `content-height`（工具栏 + 行 + 状态行）。
  - `app.slint`：新增 `editor-visible` 模式（`in-editor` 状态参与模式
    互斥与 content-driven 高度计算）；Esc/Close 关闭并回落主模式；
    editor 属性/回调全量穿过 AppWindow 边界。
- **E04 undo/redo**：UI Undo/Redo 按钮经回调进入宿主 `EditorSurface`
  （GraphEditor 栈式 undo/redo + 投影刷新），can_undo/can_redo 驱动
  按钮可用性。
- **E05 import/export**：`editor_surface.rs`（launcher-app）新增宿主
  glue——`ensure_editor`（有 `<data>/editor-draft.json` 则 import_json
  恢复草稿，否则单节点新图）；`export_draft`（export_json = 先验证后
  落盘 §10，非法图拒绝保存并把 INVALID 原因写回状态行）。
- **入口**：`EditorCommandProvider`（provider_id `editor`，搜索
  "editor/workflow" 或空查询出现 "Open Workflow Editor"），host 侧
  namespace 路由进 `open_editor`——引擎永远看不到 editor 命令。
  launcher-app `main.rs` 全量回调接线（add/remove/undo/redo/export/
  closed）。
- 红线维持：列表式声明 surface（无自由画布/无脚本）；UI 只渲染投影行，
  全部逻辑在宿主；launcher-ui 不依赖 launcher-workflow。

## Gate 结果

- `cargo test --workspace`：**835 passed / 0 failed**（launcher-ui
  editor_surface 集成测试迁移到 AppWindow 模式断言）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 待续（更新后优先级）

1. P2.7 D 线（Approval UI/交互式 clarify §18/D04 计划编辑——B02
   PlanDocument 与本批 editor surface 均已备消费）
2. P2.7 E/F/G/H（Memory/Privacy、Product UX、QA、Release）
3. MSIX 签名（等外部证书）；Pinyin 完整拼音表（优化项）
